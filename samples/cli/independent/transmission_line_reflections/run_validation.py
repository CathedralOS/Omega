#!/usr/bin/env python3
"""Sequential application acceptance using an explicitly supplied compiler.
Package review must already be completed; this script never accepts a package.
"""
from pathlib import Path
import argparse,hashlib,json,os,platform,subprocess,sys,time
sys.dont_write_bytecode=True
from validate import validate
ROOT=Path(__file__).resolve().parent
REPOSITORY=ROOT.parents[3]
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',required=True)
    parser.add_argument('--route',choices=['check','native','both','invalid'],required=True)
    parser.add_argument('--executable')
    parser.add_argument('--executable-sha256')
    options=parser.parse_args()
    if options.executable:
        assert options.route=='native' and options.executable_sha256
        assert hashlib.sha256(Path(options.executable).read_bytes()).hexdigest()==options.executable_sha256
    compiler=Path(options.omega).resolve()
    logs=ROOT/'build/validation';logs.mkdir(parents=True,exist_ok=True)
    results=[]
    def run(name,arguments,stdin=b'',expected=0):
        command=([str(Path(options.executable).resolve())] if options.executable else [str(compiler),*arguments])
        started=time.time()
        output=subprocess.run(command,input=stdin,capture_output=True,cwd=REPOSITORY,timeout=300)
        (logs/f'{name}.stdout').write_bytes(output.stdout)
        (logs/f'{name}.stderr').write_bytes(output.stderr)
        record={'name':name,'command':command,'stdin_hex':stdin.hex(),'exit':output.returncode,
                'seconds':time.time()-started,'expected_exit':expected}
        results.append(record)
        (logs/f'{options.route}.json').write_text(json.dumps(results,indent=2)+'\n')
        assert output.returncode==expected,(name,output.returncode,output.stderr.decode(errors='replace'))
        return output
    root=str(ROOT/'main.omg')
    if options.route=='check': run('check',['--check','--target','linux_x86_64',root])
    elif options.route=='invalid':
        output=run('invalid_bounds',['--check',str(ROOT/'controls/invalid_bounds/main.omg')],expected=1)
        assert b'cannot prove index' in output.stderr+output.stdout,'wrong-reason rejection'
    else:
        prefix=['run']+(['--both'] if options.route=='both' else [])
        for identifier in range(6):
            for horizon in [1,7,24]:
                name=f'{options.route}_case_{identifier}_horizon_{horizon}'
                output=run(name,prefix+[root],f'{identifier} {horizon}\n'.encode())
                validate(identifier,horizon,logs/f'{name}.stdout')
                if options.route=='both':
                    assert b'interp exit: 0' in output.stderr,'Terminal execution declined/failed'
        fixture=json.loads((ROOT/'data/invalid_inputs.json').read_text())
        invalid=[bytes.fromhex(value) for value in fixture['inputs_hex']]
        for index,text in enumerate(invalid):
            output=run(f'{options.route}_invalid_{index}',prefix+[root],text,64)
            assert output.stdout==fixture['expected_stdout'].encode()
            if options.route=='both': assert b'interp exit: 64' in output.stderr
    print(f'completed {options.route}: {len(results)} sequential checks')
if __name__=='__main__': main()
