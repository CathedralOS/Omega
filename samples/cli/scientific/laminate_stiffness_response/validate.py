#!/usr/bin/env python3
"""Sequential public-CLI validation; never accepts or retries package decisions."""
import argparse, hashlib, json, os, re, shutil, subprocess, tempfile, time
from pathlib import Path
import sys
sys.dont_write_bytecode = True
from reference import verify, assert_frozen_oracle
ROOT=Path(__file__).resolve().parent
CASES=[(str(i),(str(i)+'\n').encode()) for i in range(9)]+[
    ('9',b'9\n'),('x',b'x\n'),('empty',b''),('00',b'00\n'),('overlong',b'00000\n'),
    ('0',b'0'),('crlf',b'0\r\n')]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--omega',required=True)
    parser.add_argument('--route',choices=['check','native','both','terminal','invalid'],required=True)
    parser.add_argument('--executable',type=Path,help='one normally compiled native artifact for all cases')
    parser.add_argument('--terminal-sponsor',type=Path,help='public SDK sponsor linked to this compiler build')
    parser.add_argument('--timeout',type=int,default=600)
    arguments=parser.parse_args()
    oracle_hash=assert_frozen_oracle()
    binary=Path(arguments.omega).resolve()
    route=arguments.route
    output=ROOT/'build'/'validation'/route; output.mkdir(parents=True,exist_ok=True)
    record={'route':route,'binary':str(binary),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
      'library_root':os.environ.get('OMEGA_LIBRARY_ROOT'), 'target':'linux_x86_64',
      'oracle_sha256':oracle_hash,'runs':[]}
    if route=='terminal':
        if not arguments.terminal_sponsor: parser.error('--route terminal needs --terminal-sponsor')
        sponsor=arguments.terminal_sponsor.resolve()
        record['sponsor_sha256']=hashlib.sha256(sponsor.read_bytes()).hexdigest()
        observed=Path(tempfile.mkdtemp(prefix='capture-',dir=output))
        inputs=observed/'inputs'; inputs.mkdir()
        for index,(_,stdin) in enumerate(CASES): (inputs/f'{index:02}.stdin').write_bytes(stdin)
        command=[str(sponsor),str(ROOT),str(inputs),str(observed)]
        result=subprocess.run(command,input=b'',capture_output=True,timeout=arguments.timeout)
        (observed/'sponsor.stdout').write_bytes(result.stdout)
        (observed/'sponsor.stderr').write_bytes(result.stderr)
        assert result.returncode==0,result.stderr.decode(errors='replace')
        captures=(observed/'records.tsv').read_text().splitlines()
        assert len(captures)==len(CASES)==16,'must capture every original frozen case'
        for index,((mode,stdin),capture) in enumerate(zip(CASES,captures)):
            name,exit_text=capture.split('\t'); assert name==f'{index:02}',capture
            assert (inputs/f'{name}.stdin').read_bytes()==stdin
            stdout=observed/f'{name}.stdout'; stderr=observed/f'{name}.stderr'
            code=int(exit_text); verify(mode,stdout,code)
            assert stderr.read_bytes()==b'',stderr.read_bytes()
            record['runs'].append({'mode':mode,'stdin_hex':stdin.hex(),'exit':code,
                'stdout':str(stdout.relative_to(ROOT)),'stderr':str(stderr.relative_to(ROOT)),
                'accepted':True,'stdout_origin':'captured Terminal SDK'})
        (output/'results.json').write_text(json.dumps(record,indent=2)+'\n')
        return
    if arguments.executable:
        if route!='native': parser.error('--executable is only for --route native')
        record['executable_sha256']=hashlib.sha256(arguments.executable.read_bytes()).hexdigest()
    rows=CASES if route in ('native','both') else [('control',b'')]
    success=True
    for index,(mode,stdin) in enumerate(rows):
        source=ROOT/'controls/bounds/main.omg' if route=='invalid' else ROOT/'main.omg'
        command=([str(arguments.executable.resolve())] if arguments.executable else
            [str(binary),'--check',str(source)] if route in ('check','invalid') else
            [str(binary),'run']+(['--both'] if route=='both' else [])+['--keep',str(source)])
        started=time.monotonic(); print(f'running {route} {mode}: {command}',flush=True)
        try: result=subprocess.run(command,input=stdin,capture_output=True,timeout=arguments.timeout)
        except subprocess.TimeoutExpired as error:
            record['runs'].append({'mode':mode,'command':command,'timeout':arguments.timeout})
            success=False; break
        stdout=output/f'{index:02d}-{mode}.stdout'; stderr=output/f'{index:02d}-{mode}.stderr'
        stdout.write_bytes(result.stdout); stderr.write_bytes(result.stderr)
        row={'mode':mode,'command':command,'stdin_hex':stdin.hex(),'exit':result.returncode,'seconds':round(time.monotonic()-started,3),'stdout':str(stdout.relative_to(ROOT)), 'stderr':str(stderr.relative_to(ROOT))}
        try:
            if route=='check': assert result.returncode==0,result.stderr.decode(errors='replace')
            elif route=='invalid':
                text=result.stderr.decode(errors='replace')
                assert result.returncode==1,text
                # This is finalized from the documented fixed-array bounds diagnostic.
                assert (ROOT/'controls/bounds/expected.txt').read_text().strip() in text,text
                assert not any(word in text for word in ['package acceptance is missing','entry slot','unknown type']),text
            else:
                verify(mode,stdout,result.returncode)
                if arguments.executable: assert result.stderr==b'',result.stderr
                if route=='both': assert f'interp exit: {result.returncode}' in result.stderr.decode(errors='replace'),result.stderr
            row['accepted']=True
        except AssertionError as error:
            row['accepted']=False; row['failure']=str(error); success=False
        match=re.search(rb'build dir kept: ([^\n]+)',result.stderr)
        if match:
            retained=Path(os.fsdecode(match[1]))
            if retained.is_dir():
                destination=output/f'{index:02d}-{mode}-artifacts'
                shutil.copytree(retained,destination,dirs_exist_ok=True)
                row['artifacts']=str(destination.relative_to(ROOT))
        record['runs'].append(row)
        (output/'results.json').write_text(json.dumps(record,indent=2)+'\n')
        if not success: break
    (output/'results.json').write_text(json.dumps(record,indent=2)+'\n')
    raise SystemExit(0 if success else 1)

if __name__=='__main__': main()
