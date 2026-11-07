#!/usr/bin/env python3
"""Sequential native/Terminal validation; never accepts package reviews."""
import argparse, hashlib, json, os, subprocess, sys
from pathlib import Path
from reference import reference

parser = argparse.ArgumentParser()
parser.add_argument("--omega", required=True)
parser.add_argument("--mode", choices=("native", "both"), required=True)
parser.add_argument("--executable", help="Reuse one compiled native application; native mode only")
parser.add_argument("--case", action="append")
options = parser.parse_args()
if options.executable and options.mode != "native":
    parser.error("--executable requires native mode")
root = Path(__file__).resolve().parents[1]
manifest = json.loads((root/"validation/frozen_oracle.json").read_text())
output = root/"build"/options.mode
output.mkdir(parents=True, exist_ok=True)
records = []
for case in manifest["cases"]:
    if options.case and case["name"] not in options.case:
        continue
    name = case["name"]
    raw = (root/"data"/(name+".stdin")).read_bytes()
    expected = (root/"data"/(name+".stdout")).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == case["input_sha256"]
    assert hashlib.sha256(expected).hexdigest() == case["stdout_sha256"]
    assert reference(raw) == (case["exit"],expected.decode())
    if options.executable:
        command = [str(Path(options.executable).resolve())]
    else:
        command = [options.omega,"run"]
        if options.mode == "both":
            command.append("--both")
        command += ["--keep", str(root/"main.omg")]
    result = subprocess.run(command,input=raw,capture_output=True,timeout=180)
    (output/(name+".stdout")).write_bytes(result.stdout)
    (output/(name+".stderr")).write_bytes(result.stderr)
    equal = result.returncode == case["exit"] and result.stdout == expected
    comparison = "not_requested"
    if options.mode == "both":
        comparison = "declined" if b"DECLINED" in result.stderr else "reported"
        equal = equal and comparison != "declined" and f"interp exit: {case['exit']}".encode() in result.stderr
    records.append({"name":name,"command":command,"exit":result.returncode,"expected_exit":case["exit"],"stdout_matches":result.stdout==expected,"comparison":comparison,"pass":equal})
    print(name, "PASS" if equal else "FAIL", "exit",result.returncode,flush=True)
    (output/"results.json").write_text(json.dumps(records,indent=2)+"\n")
    if not equal:
        print(result.stderr.decode(errors="replace"),file=sys.stderr)
        sys.exit(1)
print("All",len(records),"frozen cases passed",options.mode)
