"""Sequential complete-application validation; never accepts package policy.
Requires a recorded compiler binary, project prepared through ordinary review.
"""
import argparse, hashlib, json, os, re, subprocess, time
from pathlib import Path
ROOT=Path(__file__).resolve().parent

def run(command, directory, stdin=b''):
    start=time.monotonic()
    completed=subprocess.run(command,input=stdin,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=240,cwd=directory)
    return completed, {'command':list(map(str,command)), 'exit':completed.returncode,
                       'seconds':round(time.monotonic()-start,3),
                       'stdout_sha256':hashlib.sha256(completed.stdout).hexdigest(),
                       'stderr_sha256':hashlib.sha256(completed.stderr).hexdigest()}

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',type=Path,required=True)
    parser.add_argument('--mode',choices=['check','native','both','invalid'],required=True)
    options=parser.parse_args()
    binary=options.omega.resolve(); mode=options.mode
    evidence=ROOT/'build'/'validation';evidence.mkdir(parents=True,exist_ok=True)
    records=[]; native_artifact=None
    if mode in ('check','invalid'):
        source=ROOT/'main.omg' if mode=='check' else ROOT/'controls/invalid_contract/main.omg'
        completed,row=run([str(binary),'--check',str(source)],ROOT)
        (evidence/f'{mode}.stdout').write_bytes(completed.stdout)
        (evidence/f'{mode}.stderr').write_bytes(completed.stderr)
        row['accepted']=(completed.returncode==0 if mode=='check' else completed.returncode==1 and
                         all(fragment in completed.stderr.decode() for fragment in
                             ['cannot prove ensures contract','result.value == 2']))
        records.append(row)
    else:
        manifest=json.loads((ROOT/'data/oracle.json').read_text())
        cases=dict(manifest['cases']); cases['runtime_oracle']={'exit':0}
        for name,expected in cases.items():
            command=([str(native_artifact)] if native_artifact else
                     [str(binary),'run','--keep']+(['--both'] if mode=='both' else [])+[str(ROOT/'main.omg')])
            completed,row=run(command,ROOT,(ROOT/'data'/f'{name}.stdin').read_bytes())
            (evidence/f'{mode}-{name}.stdout').write_bytes(completed.stdout)
            (evidence/f'{mode}-{name}.stderr').write_bytes(completed.stderr)
            row['case']=name
            row['accepted']=completed.returncode==expected['exit'] and completed.stdout==(ROOT/'data'/f'{name}.stdout').read_bytes()
            if mode=='both':
                row['terminal_agreement']=f"interp exit: {expected['exit']}" in completed.stderr.decode() and 'DECLINED' not in completed.stderr.decode()
                row['accepted']=row['accepted'] and row['terminal_agreement']
            records.append(row)
            print(f"{mode} {name}: exit={completed.returncode}, accepted={row['accepted']}",flush=True)
            if not row['accepted']:
                print(completed.stderr.decode()[-6000:],flush=True)
                break
            if mode=='native' and native_artifact is None:
                match=re.search(r'^build dir kept: (.+)$',completed.stderr.decode(),re.M)
                assert match,'successful CLI run must retain its published product'
                directory=Path(match[1])
                candidates=[p for p in directory.iterdir() if p.is_file() and os.access(p,os.X_OK)
                            and p.open('rb').read(4)==b'\x7fELF']
                assert len(candidates)==1,'expected one published Linux executable'
                native_artifact=candidates[0]
    output={'binary':str(binary),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
            'library_root':os.environ.get('OMEGA_LIBRARY_ROOT'),'target':'linux_x86_64',
            'oracle_sha256':hashlib.sha256((ROOT/'data/oracle.json').read_bytes()).hexdigest(),
            'mode':mode,'records':records,
            'native_artifact':str(native_artifact) if native_artifact else None,
            'native_artifact_sha256':hashlib.sha256(native_artifact.read_bytes()).hexdigest() if native_artifact else None}
    (evidence/f'{mode}.json').write_text(json.dumps(output,indent=2)+'\n')
    assert all(r['accepted'] for r in records),f'{mode} failed; see {evidence}'
    print(f'{mode}: {len(records)} accepted records')
