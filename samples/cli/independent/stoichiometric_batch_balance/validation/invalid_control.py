#!/usr/bin/env python3
"""Require the exact authored bounds rejection, independently of runtime cases."""
import argparse,json,subprocess
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument("--omega",required=True)
options=parser.parse_args()
root=Path(__file__).resolve().parents[1]
command=[options.omega,"--check","--target","linux_x86_64",str(root/"controls/intended_invalid/main.omg")]
result=subprocess.run(command,capture_output=True,timeout=180)
expected=(root/"controls/intended_invalid/expected.txt").read_text().strip()
text=(result.stdout+result.stderr).decode(errors="replace")
errors=[line for line in text.splitlines() if "error:" in line]
passed=result.returncode==1 and expected in text and bool(errors) and all(expected in line for line in errors)
output=root/"build/invalid"
output.mkdir(parents=True,exist_ok=True)
(output/"stdout.txt").write_bytes(result.stdout)
(output/"stderr.txt").write_bytes(result.stderr)
(output/"result.json").write_text(json.dumps({"command":command,"exit":result.returncode,"expected_fragment":expected,"pass":passed},indent=2)+"\n")
print("Intended bounds rejection", "PASS" if passed else "FAIL")
print(text)
raise SystemExit(0 if passed else 1)
