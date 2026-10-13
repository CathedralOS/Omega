"""Public CLI qualification; serial execution with bounded finite stdin."""
import argparse
import hashlib
import json
import subprocess
import time
from pathlib import Path
from check_output import check, oracle, ROOT

def run(command, input=b'', timeout=300):
    started=time.monotonic()
    result=subprocess.run(command,input=input,capture_output=True,timeout=timeout)
    return result,dict(command=command,exit=result.returncode,seconds=time.monotonic()-started)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--omega',required=True);parser.add_argument('--native-executable');args=parser.parse_args()
    destination=ROOT/'build/validation/native';destination.mkdir(parents=True,exist_ok=True)
    records=[]
    result,record=run([args.omega,'--check',str(ROOT/'controls/invalid_interface/main.omg')])
    record['stdout']=result.stdout.decode();record['stderr']=result.stderr.decode();records.append(record)
    assert result.returncode==1 and 'cannot prove index `3` is within length 3' in record['stdout']+record['stderr'],record
    for name,expected in oracle()['cases'].items():
        command=[args.native_executable] if args.native_executable else [args.omega,'run','--keep',str(ROOT/'main.omg')]
        result,record=run(command,(ROOT/'data'/f'{name}.stdin').read_bytes())
        record.update(case=name,stdout_sha256=hashlib.sha256(result.stdout).hexdigest())
        (destination/f'{name}.stdout').write_bytes(result.stdout);(destination/f'{name}.stderr').write_bytes(result.stderr)
        records.append(record);(destination/'records.json').write_text(json.dumps(records,indent=2)+'\n')
        check(name,result.stdout.decode(),result.returncode)
        if args.native_executable: assert not result.stderr,name
        print('verified native '+name,flush=True)
    print(f'validated {len(oracle()["cases"])} native cases and interface refusal')

if __name__=='__main__':main()
