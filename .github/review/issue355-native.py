"""Fork-only Windows desktop probe using real input and isolated live shells."""
import ctypes
from ctypes import wintypes as wt
import json
from pathlib import Path
import re
import subprocess
import sys
import time

binary, source, head, destination = sys.argv[1:]
source = Path(source).resolve()
output = Path(destination).resolve()
output.mkdir(parents=True, exist_ok=True)
sys.path.insert(0, str(source / "scripts"))
from conformance.harness import ConformanceContext, ResolvedApp, require

checks, actions = [], []
report = {"head": head, "platform": "windows-2022", "checks": checks, "actions": actions,
          "limitations": "Hosted Windows desktop at one DPI; screenshots need visual review."}
ctx = ConformanceContext(ResolvedApp(binary), "windows-x86_64", output / "config",
                         output / "work", output / "logs")
user = ctypes.WinDLL("user32", use_last_error=True)
prototypes = {
    "EnumWindows": (wt.BOOL, [ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM), wt.LPARAM]),
    "GetWindowThreadProcessId": (wt.DWORD, [wt.HWND, ctypes.POINTER(wt.DWORD)]),
    "IsWindowVisible": (wt.BOOL, [wt.HWND]),
    "GetClientRect": (wt.BOOL, [wt.HWND, ctypes.POINTER(wt.RECT)]),
    "ClientToScreen": (wt.BOOL, [wt.HWND, ctypes.POINTER(wt.POINT)]),
    "SetForegroundWindow": (wt.BOOL, [wt.HWND]),
    "GetForegroundWindow": (wt.HWND, []),
    "SetWindowPos": (wt.BOOL, [wt.HWND, wt.HWND, ctypes.c_int, ctypes.c_int,
                               ctypes.c_int, ctypes.c_int, wt.UINT]),
    "SetCursorPos": (wt.BOOL, [ctypes.c_int, ctypes.c_int]),
    "mouse_event": (None, [wt.DWORD, wt.DWORD, wt.DWORD, wt.DWORD, ctypes.c_size_t]),
    "keybd_event": (None, [wt.BYTE, wt.BYTE, wt.DWORD, ctypes.c_size_t]),
    "GetDpiForWindow": (wt.UINT, [wt.HWND]),
    "SetProcessDpiAwarenessContext": (wt.BOOL, [wt.HANDLE]),
}
for name, (result, args) in prototypes.items():
    fn = getattr(user, name)
    fn.restype, fn.argtypes = result, args
user.SetProcessDpiAwarenessContext(wt.HANDLE(-4))


def layout():
    return ctx.tab_for_pane(ctx.snapshot(), ids[0])["layout"]


def leaves(node):
    return [node["pane_id"]] if node["type"] == "pane" else leaves(node["first"]) + leaves(node["second"])


def shot(name):
    result = subprocess.run(["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                             str(source / "scripts/ui_probe.ps1"), "-ProcId", str(pid),
                             "-Shot", str(output / (name + ".png"))], capture_output=True, text=True)
    require(result.returncode == 0, "screenshot failed: " + result.stdout + result.stderr)
    require((output / (name + ".png")).is_file(), "screenshot missing")


def move(point):
    require(user.GetForegroundWindow() == hwnd, "fixture window lost foreground")
    require(user.SetCursorPos(*point), "could not move pointer")
    actions.append({"mouse_move": point})
    time.sleep(0.2)


def down(point):
    move(point)
    user.mouse_event(2, 0, 0, 0, 0)
    actions.append({"mouse_down": point})
    time.sleep(0.15)


def up():
    user.mouse_event(4, 0, 0, 0, 0)
    actions.append({"mouse_up": True})
    time.sleep(0.4)


def key(vk, modifiers=()):
    require(user.GetForegroundWindow() == hwnd, "fixture window lost foreground")
    for modifier in modifiers:
        user.keybd_event(modifier, 0, 0, 0)
    try:
        user.keybd_event(vk, 0, 0, 0)
        user.keybd_event(vk, 0, 2, 0)
    finally:
        for modifier in reversed(modifiers):
            user.keybd_event(modifier, 0, 2, 0)
    actions.append({"virtual_key": vk, "modifiers": modifiers})
    time.sleep(0.4)


try:
    ctx.prepare()
    settings = ctx.config_dir / "pebrel_settings.txt"
    with settings.open("a", encoding="utf-8") as stream:
        stream.write("language=en-US\ntheme=Nord\nfollow_system_theme=0\ntabs_position=top\n"
                     "shell=powershell\nopacity=1\nblur=off\n")
    ctx.start()
    ctx.refresh_targets()
    ctx.detect_shell()
    ids = [ctx.pane_id]
    for anchor in [ids[0], None]:
        anchor = ids[-1] if anchor is None else anchor
        result = ctx.api("pane.split", {"window_id": ctx.window_id, "pane_id": anchor,
                                        "direction": "left_right"})
        ids.append(int(result["action"]["pane_id"]))
    original = layout()
    require(leaves(original) == ids, "three-pane layout was not established")
    roots = {}
    markers = {}
    for pane in ids:
        ctx.poll(lambda: ctx.read(pane), lambda value: bool(value["text"].strip()),
                 "shell did not become ready")
        roots[pane] = ctx.api("pane.procs", {"window_id": ctx.window_id, "pane_id": pane})["root_pid"]
        markers[pane] = f"ISSUE355_PANE_{pane}_READY"
        ctx.prompt(ctx.marker_command(f"ISSUE355_PANE_{pane}_", "READY"), pane)
        ctx.wait_for_line(re.compile(markers[pane]), pane)
    ctx.api("window.focus", {"window_id": ctx.window_id, "pane_id": ids[0]})
    pid = ctx.snapshot()["process_id"]
    windows = []
    def collect(handle, _):
        owner = wt.DWORD()
        user.GetWindowThreadProcessId(handle, ctypes.byref(owner))
        rect = wt.RECT()
        if owner.value == pid and user.IsWindowVisible(handle) and user.GetClientRect(handle, ctypes.byref(rect)):
            windows.append(((rect.right - rect.left) * (rect.bottom - rect.top), handle))
        return True
    callback = prototypes["EnumWindows"][1][0](collect)
    require(user.EnumWindows(callback, 0), "window enumeration failed")
    require(windows, "fixture has no visible native window")
    hwnd = max(windows)[1]
    require(user.SetWindowPos(hwnd, None, 60, 60, 1280, 800, 0x0040), "could not size fixture window")
    user.SetForegroundWindow(hwnd)
    ctx.poll(user.GetForegroundWindow, lambda handle: handle == hwnd, "could not foreground fixture")
    rect, origin = wt.RECT(), wt.POINT()
    require(user.GetClientRect(hwnd, ctypes.byref(rect)), "client bounds unavailable")
    require(user.ClientToScreen(hwnd, ctypes.byref(origin)), "client origin unavailable")
    scale = user.GetDpiForWindow(hwnd) / 96
    # This head's custom title bar is 48 logical px; pane headers are 24 px.
    # Runtime layout assertions below fail if native input misses the real grip.
    start = (origin.x + int(rect.right * 0.20), origin.y + round(60 * scale))
    target = (origin.x + int(rect.right * 0.82), start[1])
    report.update(client_bounds=[rect.right, rect.bottom], dpi=round(scale * 96),
                  pane_ids=ids, shell_pids_before=roots)
    time.sleep(0.6)
    shot("before")
    down(start)
    move(target)
    shot("hover-target")
    key(0x1B)
    up()
    require(layout() == original, "Escape changed the split layout")
    checks.append("native Escape cancelled the header drag")
    result = ctx.api("tab.new", {"window_id": ctx.window_id, "shell": "powershell",
                                 "cwd": str(ctx.work_dir)})
    other = int(result["action"]["pane_id"])
    ctx.api("window.focus", {"window_id": ctx.window_id, "pane_id": ids[0]})
    time.sleep(0.5)
    down(start)
    move(target)
    key(0x09, (0x11,))
    require(ctx.tab_for_pane(ctx.snapshot(), other)["active"], "native Ctrl+Tab did not switch tabs")
    key(0x09, (0x11, 0x10))
    require(ctx.tab_for_pane(ctx.snapshot(), ids[0])["active"], "native Ctrl+Shift+Tab did not return")
    up()
    require(layout() == original, "tab switching revived the old drag")
    checks.append("native keyboard switch away and back cancelled the gesture")
    down(start)
    move(target)
    up()
    ctx.poll(layout, lambda node: leaves(node) == [ids[2], ids[1], ids[0]], "native grip drag did not exchange positions")
    swapped = layout()
    expected = json.loads(json.dumps(original))
    def exchange(node):
        if node["type"] == "pane":
            node["pane_id"] = {ids[0]: ids[2], ids[2]: ids[0]}.get(node["pane_id"], node["pane_id"])
        else:
            exchange(node["first"])
            exchange(node["second"])
    exchange(expected)
    require(swapped == expected, "exchange changed directions or ratios")
    require(ctx.tab_for_pane(ctx.snapshot(), ids[0])["focused_pane_id"] == ids[0], "source focus changed")
    for pane in ids:
        after = ctx.api("pane.procs", {"window_id": ctx.window_id, "pane_id": pane})["root_pid"]
        require(after == roots[pane], "live shell PID changed")
        require(markers[pane] in ctx.read(pane)["text"], "terminal history disappeared")
        ctx.prompt(ctx.marker_command(f"ISSUE355_PANE_{pane}_", "AFTER"), pane)
        ctx.wait_for_line(re.compile(f"ISSUE355_PANE_{pane}_AFTER"), pane)
    checks.append("native exchange preserved ratios, pane IDs, shell PIDs, history and live IO")
    shot("exchanged")
    body = (target[0], origin.y + int(rect.bottom * 0.60))
    down(body)
    move((body[0] - 50, body[1]))
    up()
    require(layout() == swapped, "terminal-body selection reordered panes")
    checks.append("native terminal-body dragging did not start pane reordering")
    down(target)
    move(start)
    ctx.api("pane.close", {"window_id": ctx.window_id, "pane_id": ids[0]})
    up()
    snapshot = ctx.snapshot()
    window = next(item for item in snapshot["windows"] if item["id"] == ctx.window_id)
    require(len(window["tabs"]) == 2, "closing the dragged source created an extra tab")
    tab = ctx.tab_for_pane(snapshot, ids[1])
    require(sorted(leaves(tab["layout"])) == sorted(ids[1:]), "closing the dragged source revived its pane")
    checks.append("closing a native dragged source cancelled the gesture")
    shot("source-closed")
    report.update(status="passed", layout_before=original, layout_after=swapped)
except Exception as error:
    report.update(status="failed", error=str(error))
    raise
finally:
    user.mouse_event(4, 0, 0, 0, 0)
    for modifier in [0x11, 0x10]:
        user.keybd_event(modifier, 0, 2, 0)
    try:
        if ctx.process is not None:
            ctx.stop(force=True)
        ctx.app.close()
    except Exception as error:
        report.update(status="failed", cleanup_error=str(error))
        raise
    finally:
        (output / "acceptance.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
