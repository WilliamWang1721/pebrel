"""Fork-only, bounded diagnosis of PR 455's new test; never an acceptance gate."""

import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time

SOURCE = "653b6f890a558d1b146a20034e149021869cc52a"
FIXTURE = Path("nebula_app/src/gpui_shell/workspace/tab_duplication/layout_tests.rs")
NAME = "duplicate_rebuilds_nested_mixed_layout_with_fresh_sessions"
TEST = "gpui_shell::workspace::tab_duplication::layout_tests::" + NAME


def instrument(evidence):
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    if head != SOURCE or os.environ["SOURCE_SHA"] != SOURCE:
        raise RuntimeError("source checkout must be exactly " + SOURCE)
    if subprocess.check_output(["git", "status", "--porcelain"], text=True).strip():
        raise RuntimeError("source checkout must start clean")
    original = FIXTURE.read_text()
    modified = original
    markers = [
        ("    cx.update(|cx| {", "body enter; before app initialization"),
        ("    let (_, window) = cx.add_window_view", "before window construction"),
        ("        let workspace = cx.new(|cx| {", "window builder entered; before workspace construction"),
        ("            assert!(workspace.restore_tab", "before source restoration"),
        ("            if let WorkspaceTab::Terminal { panes, zoomed, broadcast", "source restored"),
        ("            workspace.add_terminal_with(", "before inactive tab construction"),
        ("            workspace.duplicate_tab(0, window, cx);", "before duplication"),
        ("            let target = workspace.active;", "duplication returned"),
        ("            workspace.close_tab(target, window, cx);", "assertions passed; before duplicate close"),
        ("            assert_eq!(workspace.tabs.len(), 2);", "duplicate closed"),
        ("            workspace.close_tab(0, window, cx);", "before source close"),
        ("            workspace\n", "all tabs closed; workspace builder returning"),
        ("        Root::new(workspace, window, cx)", "before root construction"),
        ("    window.update(|window, cx| window.draw(cx).clear(cx));", "window constructor returned; before draw"),
    ]
    for anchor, message in markers:
        expected = 2 if anchor == "            workspace.close_tab(0, window, cx);" else 1
        if modified.count(anchor) != expected:
            raise RuntimeError("unexpected fixture anchor: " + anchor)
        indent = anchor[:len(anchor) - len(anchor.lstrip())]
        marker = f'crate::gpui_shell::try_write_stderr(format_args!("[pr455] {message}"));'
        modified = modified.replace(anchor, indent + marker + "\n" + anchor)
    entry = f"#[gpui::test]\nfn {NAME}(cx: &mut TestAppContext) {{"
    if modified.count(entry) != 1:
        raise RuntimeError("unexpected GPUI test entry")
    wrapper = f'''#[test]
fn {NAME}() {{
    crate::gpui_shell::try_write_stderr(format_args!("[pr455] wrapper enter"));
    diagnostic_gpui_duplicate();
    crate::gpui_shell::try_write_stderr(format_args!("[pr455] GPUI macro and teardown returned"));
}}

#[gpui::test]
fn diagnostic_gpui_duplicate(cx: &mut TestAppContext) {{'''
    modified = modified.replace(entry, wrapper)
    modified = modified.replace(
        "    window.update(|window, cx| window.draw(cx).clear(cx));\n}",
        '    window.update(|window, cx| window.draw(cx).clear(cx));\n'
        '    crate::gpui_shell::try_write_stderr(format_args!("[pr455] body finished; GPUI macro teardown follows"));\n}',
    )
    assertion_prefixes = ("assert!(", "assert_eq!(", "assert_ne!(")
    assertions = [line.strip() for line in original.splitlines()
                  if line.lstrip().startswith(assertion_prefixes)]
    if assertions != [line.strip() for line in modified.splitlines()
                      if line.lstrip().startswith(assertion_prefixes)]:
        raise RuntimeError("fixture assertions changed")
    FIXTURE.write_text(modified)
    patch = subprocess.check_output(["git", "diff", "--", str(FIXTURE)], text=True)
    (evidence / "instrumentation.patch").write_text(patch)
    return {
        "source": head,
        "test": TEST,
        "fixture_sha256": hashlib.sha256(original.encode()).hexdigest(),
        "instrumented_sha256": hashlib.sha256(modified.encode()).hexdigest(),
        "assertion_lines_preserved": len(assertions),
        "helper_sha": os.environ["GITHUB_SHA"],
        "instrumented_result_is_not_unmodified_acceptance": True,
    }


def collect_stack(pid, evidence):
    command = [
        "sudo", "-n", "timeout", "--kill-after=5s", "20s",
        "gdb", "--batch", "-q", "-p", str(pid),
        "-ex", "set pagination off", "-ex", "info threads",
        "-ex", "thread apply all bt 32", "-ex", "detach",
    ]
    with (evidence / "timeout-stack.log").open("w") as output:
        output.write(f"Diagnostic child PID: {pid}\nCommand: {command!r}\n")
        output.flush()
        try:
            result = subprocess.run(command, stdout=output, stderr=subprocess.STDOUT, timeout=30)
            status = "timeout" if result.returncode == 124 else "completed"
            return {"status": status, "exit_code": result.returncode}
        except subprocess.TimeoutExpired:
            output.write("Stack collector exceeded its outer 30 second deadline.\n")
            return {"status": "timeout"}


def run(command, deadline, log_path, evidence, stacks=False):
    print("[pr455 supervisor] starting:", command, flush=True)
    started = time.monotonic()
    process = subprocess.Popen(
        command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, errors="replace", start_new_session=True,
    )

    def relay():
        with log_path.open("w") as output:
            for line in process.stdout:
                output.write(line)
                output.flush()
                try:
                    item = json.loads(line)
                except json.JSONDecodeError:
                    print(line, end="", flush=True)
                else:
                    if isinstance(item, dict) and item.get("reason") == "compiler-message":
                        print(item["message"].get("rendered", ""), end="", flush=True)

    reader = threading.Thread(target=relay, daemon=True)
    reader.start()
    result = {"command": command, "deadline_seconds": deadline, "pid": process.pid}
    try:
        result["exit_code"] = process.wait(timeout=deadline)
        result["status"] = "completed"
    except subprocess.TimeoutExpired:
        result.update(status="timeout", exit_code=124)
        print("[pr455 supervisor] deadline reached; collecting this child's evidence", flush=True)
        if stacks:
            result["stack"] = collect_stack(process.pid, evidence)
        # Only this diagnostic process group is ended; existing Actions jobs are untouched.
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait(timeout=10)
    reader.join(timeout=5)
    result["log_stream_closed"] = not reader.is_alive()
    result["elapsed_seconds"] = round(time.monotonic() - started, 2)
    return result


def main():
    evidence = Path(sys.argv[1])
    evidence.mkdir(parents=True, exist_ok=True)
    timeout = int(os.environ["TEST_TIMEOUT_SECONDS"])
    if not 30 <= timeout <= 600:
        raise ValueError("test timeout must be between 30 and 600 seconds")
    report = instrument(evidence)
    report_path = evidence / "report.json"
    report_path.write_text(json.dumps(report, indent=2))
    build = [
        "cargo", "test", "--locked", "--config", ".github/ci-profile.toml", "--profile", "ci",
        "--workspace", "--features", "nebula/gpui-test-support", "--no-run",
        "--message-format=json-render-diagnostics",
    ]
    report["build"] = run(build, 1200, evidence / "build.log", evidence)
    report_path.write_text(json.dumps(report, indent=2))
    if report["build"]["exit_code"] != 0:
        return report["build"]["exit_code"]
    executables = []
    for line in (evidence / "build.log").read_text().splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (isinstance(item, dict) and item.get("reason") == "compiler-artifact" and item.get("executable")
                and item["target"]["name"] == "pebrel" and item["profile"]["test"]):
            executables.append(item["executable"])
    if len(executables) != 1:
        raise RuntimeError(f"expected one pebrel test binary, found {executables}")
    report["test_listing"] = run(
        [executables[0], "--list", "--exact", TEST], 30, evidence / "test-list.log", evidence,
    )
    report_path.write_text(json.dumps(report, indent=2))
    if (report["test_listing"]["exit_code"] != 0
            or (evidence / "test-list.log").read_text().splitlines().count(TEST + ": test") != 1):
        raise RuntimeError("exactly one selected diagnostic test must exist")
    command = [executables[0], "--exact", TEST, "--nocapture", "--test-threads=1"]
    report["test_run"] = run(command, timeout, evidence / "test.log", evidence, stacks=True)
    report_path.write_text(json.dumps(report, indent=2))
    return report["test_run"]["exit_code"]


if __name__ == "__main__":
    sys.exit(main())
