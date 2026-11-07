#!/usr/bin/env python3
"""Sequential native + Terminal comparison, frozen byte/exit oracles."""
import argparse, hashlib, json, os, subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent
FROZEN='4e637f25b6173b516e75fa860e89ef31d502f08cd545f04fbc95da390ab7bec3'

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',required=True)
    parser.add_argument('--native-only',action='store_true')
    args=parser.parse_args()
    assert hashlib.sha256((ROOT/'data/oracle.json').read_bytes()).hexdigest()==FROZEN, 'frozen oracle modified'
    manifest=json.loads((ROOT/'data/oracle.json').read_text())
    for path,identity in manifest['oracle_inputs_sha256'].items():
        assert hashlib.sha256((ROOT/path).read_bytes()).hexdigest()==identity, f'oracle input changed: {path}'
    out=ROOT/'build/validation'; out.mkdir(parents=True,exist_ok=True)
    records=[]
    for case in manifest['cases']:
        name=case['name']
        command=[str(Path(args.omega).resolve()),'run']
        if not args.native_only: command.append('--both')
        command.append(str(ROOT/'main.omg'))
        result=subprocess.run(command,input=(ROOT/f'data/{name}.stdin').read_bytes(),capture_output=True,timeout=180)
        (out/f'{name}.stdout').write_bytes(result.stdout)
        (out/f'{name}.stderr').write_bytes(result.stderr)
        matched=result.returncode==case['exit'] and result.stdout==(ROOT/f'data/{name}.stdout').read_bytes()
        terminal='not-requested' if args.native_only else 'agreed' if f'interp exit: {case["exit"]}'.encode() in result.stderr else 'declined' if b'DECLINED' in result.stderr else 'unconfirmed'
        record=dict(name=name,command=command,exit=result.returncode,expected_exit=case['exit'],exact_stdout=matched,terminal=terminal)
        records.append(record)
        print(json.dumps(record),flush=True)
        (out/'results.json').write_text(json.dumps(records,indent=2)+'\n')
        if not matched:
            raise SystemExit(f'{name}: byte/exit mismatch; inspect {out}')
        if not args.native_only and terminal not in ('agreed','declined'):
            raise SystemExit(f'{name}: Terminal route {terminal}; native result retained')
    declined=sum(record['terminal']=='declined' for record in records)
    agreed=sum(record['terminal']=='agreed' for record in records)
    print(f'PASS: {len(records)} frozen native byte/exit oracles; Terminal agreed={agreed}, declined={declined}')
    if declined: raise SystemExit(2)
if __name__=='__main__': main()
