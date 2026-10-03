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
    move(x, y)
    apple(f'click at {{{round(x)}, {round(y)}}}')
    actions.append({"click": [x, y]})
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
    if scenario == "theme":
        settings()
        hit("Nord", lambda word: word["x"] > bounds[0] + bounds[2] * .5)
        anchor = wait(lambda: find("pebrel --version"), "native theme preview displayed")
        before = shot("01-theme-before-scroll")
        scroll(anchor["x"] - 250, anchor["y"] + 60, -350)
        choice = hit("Catppuccin Mocha", lambda word: word["x"] < bounds[0] + bounds[2] * .65)
        wait(lambda: find("Catppuccin Mocha", lambda word: word["x"] > bounds[0] + bounds[2] * .65),
             "selected lower theme updates the native preview")
        after = find("pebrel --version")
        assert after and abs(after["x"] - anchor["x"]) < 5 and abs(after["y"] - anchor["y"]) < 5
        checks.append("native preview position stays fixed while selecting a scrolled lower theme")
        report["preview_before"] = anchor
        report["preview_after"] = after
        report["lower_choice"] = choice
        shot("02-theme-after-scroll-and-selection")
    elif scenario == "line-height":
        terminal_lines(12)
        old_gap = spacing()
        shot("01-line-height-1.00")
        settings()
        row = reveal("Line height")
        hit("1.00", lambda word: word["text"].strip() == "1.00" and word["x"] > row["x"] and abs(word["y"] - row["y"]) < 28)
        key("a")
        key("1.46", "")
        apple("key code 36")
        actions.append({"key_code": 36})
        wait(lambda: saved("terminal_line_height") == "1.46", "native edit Enter persists 1.46")
        shot("02-native-line-height-setting")
        key(",")
        terminal_lines(12)
        new_gap = spacing()
        assert new_gap > old_gap * 1.2, (old_gap, new_gap)
        report["native_line_spacing_points"] = {"1.00": old_gap, "1.46": new_gap}
        checks.append("real terminal line spacing increases after the native settings commit")
        shot("03-line-height-1.46")
    elif scenario == "scrollbar":
        terminal_lines(120)
        move(bounds[0] + 25, bounds[1] + 15)
        before = shot("01-scrollbar-always")
        settings()
        row = reveal("Scrollbar visibility")
        control = wait(lambda: find("Always", lambda word: word["text"].strip() == "Always" and
                                    word["x"] > row["x"] and abs(word["y"] - row["y"]) < 28),
                       "native scrollbar dropdown caption located")
        click(bounds[0] + bounds[2] - 50, control["y"])
        hit("On hover")
        wait(lambda: saved("scrollbar_visibility") == "hover", "native dropdown persists hover")
        shot("02-scrollbar-native-setting")
        key(",")
        move(bounds[0] + 25, bounds[1] + 15)
        away = shot("03-scrollbar-hover-away")
        area = [bounds[0] + bounds[2] - 48, bounds[1] + 70, 44, bounds[3] - 100]
        hidden = diff(before, away, area)
        assert hidden["changed_pixels"] >= 80, hidden
        x, y, w, h = hidden["bounds"]
        assert 0 < w <= 12 and h >= 20, hidden
        move(x + w / 2, y + h / 2)
        hovered = shot("04-scrollbar-hover-inside")
        shown = diff(away, hovered, area)
        assert shown["changed_pixels"] >= 80, shown
        move(bounds[0] + 25, bounds[1] + 15)
        left = shot("05-scrollbar-hover-left")
        settled = diff(away, left, area)
        assert settled["changed_pixels"] < 25, settled
        report["scrollbar_right_edge_region_points"] = area
        report["scrollbar_changes"] = {"always_to_away": hidden, "away_to_hover": shown, "away_to_left": settled}
        checks.append("only the native terminal right edge changes on hover and clears on leave")
    else:
        raise ValueError(scenario)
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
