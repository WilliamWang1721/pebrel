"""Fork-only probe: launch the real product test binary with an inherited token."""
import json
import os
from pathlib import Path
import subprocess
import sys

records = [json.loads(line) for line in Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()]
executables = [record["executable"] for record in records
               if record.get("reason") == "compiler-artifact"
               and record.get("executable") and record["profile"]["test"]
               and record["target"]["name"] == "pebrel"]
if len(executables) != 1:
    raise SystemExit(f"expected one product test executable, got {executables}")
result = subprocess.run([
    executables[0], "--ignored", "--exact",
    "update_download::handoff::tests::issue349_real_token_update_boundary", "--nocapture",
], check=False, capture_output=True)
report = Path("tmp") / ("issue349-token-" + os.environ["PEBREL_349_EXPECT_ELEVATED"])
report.with_suffix(".stdout.txt").write_bytes(result.stdout)
report.with_suffix(".stderr.txt").write_bytes(result.stderr)
report.with_suffix(".json").write_text(json.dumps({
    "expected_elevated": os.environ["PEBREL_349_EXPECT_ELEVATED"],
    "executable": executables[0],
    "exit_code": result.returncode,
}), encoding="utf-8")
sys.stdout.buffer.write(result.stdout)
sys.stderr.buffer.write(result.stderr)
raise SystemExit(result.returncode)
