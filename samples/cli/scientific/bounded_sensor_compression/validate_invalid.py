#!/usr/bin/env python3
"""Separate complete intended-invalid source: require the authored rejection."""
import argparse
import json
from pathlib import Path
import subprocess
ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('--omega',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
control=ROOT/'controls/invalid_bound'
command=[str(args.omega.resolve()),'--check',str(control/'main.omg')]
observed=subprocess.run(command,capture_output=True,timeout=180)
expected=(control/'expected_fragment.txt').read_text().strip()
text=observed.stdout.decode(errors='replace')+observed.stderr.decode(errors='replace')
passed=observed.returncode==1 and expected in text
record=dict(command=command,exit=observed.returncode,expected_fragment=expected,passed=passed,stdout=observed.stdout.decode(errors='replace'),stderr=observed.stderr.decode(errors='replace'))
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record,indent=2))
raise SystemExit(0 if passed else 1)
