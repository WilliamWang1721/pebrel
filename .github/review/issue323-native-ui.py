"""Fork-only native desktop checks; isolated fixtures, exact candidate heads.

Launch/capture/input reuse qa-macos.py at abfc1a39b681225c0ec6a6faf73098b19f4e42c8.
This records hosted macOS evidence, not Windows tray/UAC or human approval.
"""
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import time

scenario, binary, source, head, output = sys.argv[1:]
output = Path(output).resolve()
output.mkdir(parents=True, exist_ok=True)
helper = output / "native-ui-vision"
checks, actions = [], []
proc = None
report = {"scenario": scenario, "head": head, "platform": "macos-26", "checks": checks,
          "actions": actions, "limitations": "Hosted macOS only; no Windows tray/UAC or manual DPI approval."}

def run(args, **kwargs):
    result = subprocess.run(args, capture_output=True, text=True, timeout=45, **kwargs)
    if result.returncode:
        raise AssertionError(result.stderr.strip() or str(args))
    return result.stdout

def apple(body):
    return run(["osascript", "-e", 'tell application "System Events"\n' + body + '\nend tell'])

def wait(check, label, seconds=35):
    deadline, last = time.monotonic() + seconds, None
    while time.monotonic() < deadline:
        try:
            value = check()
            if value:
                checks.append(label)
                return value
        except (AssertionError, ValueError, KeyError, subprocess.SubprocessError) as error:
            last = str(error)
        time.sleep(.5)
    raise AssertionError(f"{label}: {last}")

def key(text, modifiers="command down"):
    apple(f'set frontmost of first application process whose unix id is {proc.pid} to true\n'
          f'keystroke {json.dumps(text)} using {{{modifiers}}}')
    actions.append({"key": text, "modifiers": modifiers})
    time.sleep(.6)

def click(x, y):
    run([str(helper), "--click", str(x), str(y)])
    actions.append({"click": [x, y], "source": "CoreGraphics move/down/up across frames"})
    time.sleep(.5)

def move(x, y):
    run([str(helper), "--move", str(x), str(y)])
    actions.append({"move": [x, y]})
    time.sleep(.6)

def scroll(x, y, delta=-300):
    run([str(helper), "--scroll", str(x), str(y), str(delta)])
    actions.append({"scroll": [x, y], "pixels": delta})
    time.sleep(.6)

def shot(name):
    path = output / f"{name}.png"
    run(["screencapture", "-x", str(path)])
    return path

def words():
    data = json.loads(run([str(helper), str(shot("ocr-current"))]))
    report["capture"] = {key: value for key, value in data.items() if key != "words"}
    return data["words"]

def text_matches(word, text):
    return text.casefold() in word["text"].casefold()

def find(text, predicate=lambda word: True):
    return next((word for word in words() if text_matches(word, text) and predicate(word)), None)

def hit(text, predicate=lambda word: True):
    word = wait(lambda: find(text, predicate), f"native rendered text: {text}")
    click(word["x"], word["y"])
    return word

def settings():
    key(",")
    hit("Appearance", lambda word: word["x"] < bounds[0] + bounds[2] * .4)

def reveal(text):
    for _ in range(7):
        word = find(text, lambda word: word["x"] > bounds[0] + bounds[2] * .25)
        if word:
            return word
        scroll(bounds[0] + bounds[2] * .75, bounds[1] + bounds[3] * .7)
    raise AssertionError(f"setting not reachable: {text}")

def saved(key_name):
    raw = (config / "pebrel_settings.txt").read_text()
    return next((line.split("=", 1)[1] for line in raw.splitlines() if line.startswith(key_name + "=")), None)

def ctl(*args):
    response = json.loads(run([str(executable), "ctl", *args], env=env))
    assert response.get("ok"), response
    return response["result"]

def terminal_lines(count):
    command = ("PS1='native> '; printf '\\033[2J\\033[H'; for i in {1.." + str(count) +
               "}; do printf 'NATIVE_LINE_%03d\\n' \"$i\"; done")
    ctl("prompt", "--pane", str(pane_id), "--text", command)
    actions.append({"pane_prompt": command, "pane_id": pane_id})
    wait(lambda: find("NATIVE_LINE_"), "native shell fixture rendered")
    time.sleep(.6)

def spacing():
    positions = sorted(word["y"] for word in words() if re.fullmatch(r"NATIVE_LINE_\d+", word["text"]))
    assert len(positions) >= 5, positions
    gaps = sorted(b - a for a, b in zip(positions, positions[1:]) if b - a > 5)
    return gaps[len(gaps) // 2]

def diff(before, after, area):
    return json.loads(run([str(helper), "--diff", str(before), str(after), *map(str, area)]))

try:
    run(["swiftc", str(Path(__file__).with_name("native-ui-vision.swift")), "-o", str(helper)])
    app = output / "Pebrel.app"
    executable = app / "Contents/MacOS/pebrel"
    executable.parent.mkdir(parents=True)
    shutil.copy2(binary, executable)
    plist = plistlib.loads((Path(source) / "packaging/macos/Info.plist").read_bytes())
    plist.update(CFBundleName="Pebrel", CFBundleIdentifier="io.github.kuddev.pebrel")
    (app / "Contents/Info.plist").write_bytes(plistlib.dumps(plist))
    run(["codesign", "--force", "--deep", "--sign", "-", str(app)])
    config = output / "config"
    config.mkdir()
    (config / "pebrel_settings.txt").write_text(
        "language=en-US\ntheme=Nord\nfollow_system_theme=0\nfont_size=15\nui_font_size=14\n"
        "font_family=Menlo\nopacity=1\nblur=off\nrestore_session=0\nresume_ai=0\n"
        "auto_check_updates=off\nkeep_session=0\nfetch=0\nterminal_line_height=1.00\nscrollbar_visibility=always\n")
    env = {key: value for key, value in os.environ.items() if not key.startswith(("PEBREL_", "NEBULA_"))}
    env["PEBREL_CONFIG_DIR"] = str(config)
    log = (output / "native.log").open("w")
    proc = subprocess.Popen([str(executable)], env=env, stdout=log, stderr=subprocess.STDOUT)
    snap = wait(lambda: ctl("snapshot")["windows"], "native workspace started")
    pane_id = snap[0]["tabs"][0]["panes"][0]["id"]
    width, height = (900, 590) if scenario == "theme" else (1280, 900)
    words()
    width = min(width, report["capture"]["display_width"] - 40)
    height = min(height, report["capture"]["display_height"] - 80)
    if scenario == "theme":
        assert width > 720, "a wide native theme layout requires a wider display"
    apple(f'set p to first application process whose unix id is {proc.pid}\n'
          f'set frontmost of p to true\nset position of window 1 of p to {{20, 40}}\n'
          f'set size of window 1 of p to {{{width}, {height}}}')
    time.sleep(1)
    bounds = [float(value) for value in apple(
        f'set p to first application process whose unix id is {proc.pid}\n'
        'return (position of window 1 of p) & (size of window 1 of p)').strip().split(",")]
    report["window_bounds_points"] = bounds
    if scenario != "resume-args":
        raise ValueError(scenario)
    settings()
    hit("Agents", lambda word: word["x"] < bounds[0] + bounds[2] * .25)
    reveal("Automatic resume arguments")
    scroll(bounds[0] + bounds[2] * .75, bounds[1] + bounds[3] * .7, -350)
    title = wait(lambda: find("Automatic resume arguments"), "resume form reachable through native scroll")
    codex = wait(lambda: find("Codex", lambda word: word["y"] > title["y"] and word["x"] > bounds[0] + bounds[2] * .25), "Codex resume field rendered")
    save = wait(lambda: find("Save", lambda word: codex["y"] < word["y"] < codex["y"] + 55 and word["x"] > bounds[0] + bounds[2] * .6), "Codex Save control rendered")
    input_x, input_y = save["x"] - 180, save["y"]
    shot("01-resume-default")
    value = ["--yolo", "--config", "model=example", ""]
    encoded = json.dumps(value, separators=(",", ":"))
    click(input_x, input_y)
    key("a")
    apple('keystroke ' + json.dumps(encoded))
    time.sleep(.6)
    assert saved("agent_resume_args_codex") is None, "editing must remain a local draft"
    shot("02-resume-draft")
    move(save["x"], save["y"])
    shot("03-resume-save-hover")
    # Real Tab/Enter path from the focused input reaches Save.
    apple('key code 48')
    time.sleep(.4)
    shot("04-resume-save-keyboard-focus")
    apple('key code 36')
    wait(lambda: saved("agent_resume_args_codex") == encoded, "native keyboard Save preserves the exact parameter array including an empty string")
    wait(lambda: find("Saved", lambda word: save["y"] < word["y"] < save["y"] + 65), "native Save completion feedback")
    shot("05-resume-saved")
    click(input_x, input_y)
    key("a")
    apple('keystroke "--yolo; echo invalid"')
    click(save["x"], save["y"])
    wait(lambda: find("JSON", lambda word: save["y"] < word["y"] < save["y"] + 65), "invalid suffix reports a JSON validation error")
    assert saved("agent_resume_args_codex") == encoded, "invalid input must preserve the stored array"
    shot("06-resume-invalid")
    click(input_x, input_y)
    key("a")
    apple('keystroke "[]"')
    click(save["x"], save["y"])
    wait(lambda: saved("agent_resume_args_codex") == "", "native Save of [] restores default resume arguments")
    shot("07-resume-cleared")
    report["passed"] = True
except Exception as error:
    report["passed"] = False
    report["error"] = str(error)
    try:
        shot("failure")
    except Exception:
        pass
    raise
finally:
    if proc and proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
    (output / "acceptance.json").write_text(json.dumps(report, indent=2))
