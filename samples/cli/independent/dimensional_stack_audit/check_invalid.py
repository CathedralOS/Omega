#!/usr/bin/env python3
"""Validate only the complete intended-invalid source; wrong reason is failure."""
import argparse,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent
parser=argparse.ArgumentParser()
parser.add_argument('--omega',required=True)
args=parser.parse_args()
command=[str(Path(args.omega).resolve()),'--check','--target','linux_x86_64',str(ROOT/'controls/invalid_contract/main.omg')]
result=subprocess.run(command,capture_output=True,timeout=180)
out=ROOT/'build/validation';out.mkdir(parents=True,exist_ok=True)
(out/'invalid_contract.stdout').write_bytes(result.stdout)
(out/'invalid_contract.stderr').write_bytes(result.stderr)
combined=result.stdout+result.stderr
expected=(ROOT/'controls/invalid_contract/expected.txt').read_bytes().strip()
wrong_reasons=[token.decode() for token in [b'package acceptance is missing',b'cache root',b'entry slot',b'Incomplete',b'unknown identifier',b'expected token'] if token in combined]
passed=result.returncode==1 and expected in combined and not wrong_reasons
record=dict(command=command,exit=result.returncode,expected_fragment=expected.decode(),fragment_matched=expected in combined,wrong_reasons=wrong_reasons,passed=passed)
(out/'invalid_contract.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record,indent=2))
raise SystemExit(0 if passed else 1)
