"""Fork-only native close/relaunch evidence for issue 370; production is unchanged."""
from __future__ import annotations

import ctypes
from ctypes import wintypes as wt
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from conformance.harness import ConformanceContext, ResolvedApp, require
from conformance.windows_standard_user import WindowsTokens


def dump(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def native_close(pid: int) -> None:
    user = ctypes.WinDLL("user32", use_last_error=True)
    callback = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
    user.EnumWindows.argtypes = [callback, wt.LPARAM]
    user.EnumWindows.restype = wt.BOOL
    user.GetWindowThreadProcessId.argtypes = [wt.HWND, ctypes.POINTER(wt.DWORD)]
    user.IsWindowVisible.argtypes = [wt.HWND]
    user.PostMessageW.argtypes = [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM]
    user.PostMessageW.restype = wt.BOOL
    windows = []

    @callback
    def collect(hwnd, _):
        owner = wt.DWORD()
        user.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and user.IsWindowVisible(hwnd):
            windows.append(hwnd)
        return True

    require(user.EnumWindows(collect, 0), "EnumWindows failed")
    require(len(windows) == 1, f"expected one visible owned window: {windows}")
    require(user.PostMessageW(windows[0], 0x10, 0, 0), "native WM_CLOSE failed")


def screenshot(path: Path) -> None:
    destination = str(path).replace("'", "''")
    command = (
        "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; "
        "$r=[System.Windows.Forms.SystemInformation]::VirtualScreen; "
        "$b=New-Object System.Drawing.Bitmap $r.Width,$r.Height; "
        "$g=[System.Drawing.Graphics]::FromImage($b); "
        "$g.CopyFromScreen($r.Left,$r.Top,0,0,$b.Size); "
        f"$b.Save('{destination}',[System.Drawing.Imaging.ImageFormat]::Png); "
        "$g.Dispose(); $b.Dispose()"
    )
    subprocess.run(["powershell.exe", "-NoProfile", "-Command", command], check=True, timeout=20)


def main() -> None:
    api = WindowsTokens()
    token = api.current_token()
    try:
        require(not api.elevated(token), "native restore must run with a verified ordinary token")
    finally:
        api.kernel.CloseHandle(token)
    output = Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    app = ResolvedApp(sys.argv[1])
    report = {"source": os.environ.get("GITHUB_SHA"), "ordinary_token": True}
    with tempfile.TemporaryDirectory(prefix="issue370-") as directory:
        root = Path(directory)
        ctx = ConformanceContext(app, "windows", root / "config", root / "work", output)
        try:
            ctx.prepare()
            with (ctx.config_dir / "pebrel_settings.txt").open("a", encoding="utf-8") as settings:
                settings.write("shell=pwsh\n")
            ctx.start(explicit_working_directory=False)
            ctx.refresh_targets()
            ctx.api("tab.rename", {"window_id": ctx.window_id, "tab_index": 0, "name": "issue370-a"})
            ctx.api("pane.split", {"window_id": ctx.window_id, "pane_id": ctx.pane_id, "direction": "left_right"})
            ctx.api("tab.new", {"window_id": ctx.window_id})
            ctx.api("tab.rename", {"window_id": ctx.window_id, "tab_index": 1, "name": "issue370-b"})
            saved = ctx.wait_for_session(
                lambda value: len(value.get("tabs", [])) == 2
                and [tab.get("custom_name") for tab in value["tabs"]] == ["issue370-a", "issue370-b"]
                and value["tabs"][0].get("layout", {}).get("kind") == "split"
            )
            for tab in saved["tabs"]:
                require(tab["launch"]["program"].lower().endswith("pwsh.exe"), f"pwsh launch identity missing: {tab}")
            before = ctx.snapshot()
            dump(output / "before-close.json", before)
            screenshot(output / "before-close.png")
            native_close(int(before["process_id"]))
            require(ctx._wait_process(timeout=20) == 0, "native close did not exit cleanly")
            ctx.stop(force=True)
            closed = json.loads(ctx.session_file.read_text(encoding="utf-8"))
            dump(output / "after-close-session.json", closed)
            require(closed.get("clean_exit"), "native close snapshot was not clean")
            require(closed["tabs"] == saved["tabs"], "native close/quit overwrote tabs, splits or shell identities")
            ctx.start(explicit_working_directory=False)
            ctx.refresh_targets()
            restored = ctx.snapshot()
            tabs = restored["windows"][0]["tabs"]
            require([tab["label"] for tab in tabs] == ["issue370-a", "issue370-b"], f"cold restore lost labels: {tabs}")
            require([len(tab["panes"]) for tab in tabs] == [2, 1], "cold restore lost split panes")
            for tab in tabs:
                for pane in tab["panes"]:
                    processes = ctx.api("pane.procs", {"window_id": ctx.window_id, "pane_id": pane["id"]})
                    shells = [proc["executable"] for proc in processes["processes"] if proc.get("depth") == 0]
                    require(any(name.lower().endswith("pwsh.exe") for name in shells), f"restored shell changed: {shells}")
            dump(output / "after-relaunch.json", restored)
            screenshot(output / "after-relaunch.png")
            report.update(status="passed", native_wm_close=True, restored_tabs=2, restored_panes=3, restored_pwsh=True)
        except Exception as error:
            report.update(status="failed", error=f"{type(error).__name__}: {error}")
            raise
        finally:
            dump(output / "report.json", report)
            ctx.stop(force=True)
            app.close()


if __name__ == "__main__":
    main()
