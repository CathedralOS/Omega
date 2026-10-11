#!/usr/bin/env python3
"""Sequential, retained end-to-end validation using the public Omega CLI."""
import argparse, hashlib, json, os, subprocess, time, sys
sys.dont_write_bytecode=True
from pathlib import Path
from reference import verify

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',required=True,type=Path)
    arguments=parser.parse_args()
    project=Path(__file__).resolve().parent
    output=project/'build/validation';output.mkdir(parents=True,exist_ok=True)
    frozen=json.loads((project/'ORACLES.json').read_text())
    for name,digest in frozen['sha256'].items():
        assert hashlib.sha256((project/name).read_bytes()).hexdigest()==digest, 'frozen expectation changed'
    observations=[]
    def command(label,options,input_bytes=b''):
        invocation=[str(arguments.omega.resolve()),*options]
        start=time.monotonic()
        result=subprocess.run(invocation,input=input_bytes,capture_output=True,timeout=180)
        (output/f'{label}.stdout').write_bytes(result.stdout)
        (output/f'{label}.stderr').write_bytes(result.stderr)
        observations.append(dict(label=label,command=invocation,exit=result.returncode,elapsed_seconds=round(time.monotonic()-start,3)))
        (output/'commands.json').write_text(json.dumps(observations,indent=2)+'\n')
        print(label,result.returncode,flush=True)
        return result
    checked=command('check',['--check','--target','linux_x86_64',str(project/'main.omg')])
    assert checked.returncode==0,checked.stderr.decode()
    comparison_count=0
    for fixture in sorted((project/'data').glob('*.txt')):
        result=command(fixture.stem,['run','--both','--keep',str(project/'main.omg')],fixture.read_bytes())
        invalid=fixture.name.startswith('invalid_')
        expected_status=64 if invalid else 0
        assert result.returncode==expected_status,result.stderr.decode()
        if invalid:
            assert b'ERROR: malformed ADC settings or readings\n' in result.stdout
        else:
            verify(fixture,result.stdout.decode())
        if b'interp: DECLINED' in result.stderr:
            raise AssertionError('Terminal comparison unsupported: '+result.stderr.decode())
        assert f'interp exit: {expected_status}'.encode() in result.stderr,result.stderr.decode()
        comparison_count+=1
    rejected=command('intended_invalid',['--check','--target','linux_x86_64',str(project/'intended_invalid/main.omg')])
    assert rejected.returncode==1,rejected.stderr.decode()
    assert b'division' in rejected.stderr.lower() or b'divisor' in rejected.stderr.lower(),rejected.stderr.decode()
    summary=dict(valid_studies=5,runtime_invalid_controls=17,terminal_exit_comparisons=comparison_count,source_invalid_rejected=True,numerical_tolerance=0,
                 terminal_scope='public run --both compares completion status; native CSV checked byte-for-byte',
                 compiler_sha256=hashlib.sha256(arguments.omega.read_bytes()).hexdigest())
    (output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))
if __name__=='__main__': main()
