"""Fork-only probe: launch the real product test binary with an inherited token."""
import json
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
raise SystemExit(subprocess.run([
    executables[0], "--ignored", "--exact",
    "update_download::handoff::tests::issue349_real_token_update_boundary", "--nocapture",
], check=False).returncode)
