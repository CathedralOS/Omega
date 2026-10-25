"""Observe real Terminal stdout through the recorded producer's public APIs.
Reuses a built producer's libraries instead of recompiling the compiler. Supports
normal Cargo deps and MBX managed outputs, including split rlib/rmeta metadata.
"""
import argparse,hashlib,json,os,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
REPOSITORY=ROOT.parents[3]
sys.dont_write_bytecode=True
sys.path.insert(0,str(ROOT/'validation'))
from validate import check_report,ORACLE_SHA256,load_oracle

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def build_observer(release,output,receipt):
    selected={}; search=set()
    for name in ('compiler', 'terminal_runtime','omega_build', 'package_manager','terminal_interpreter','target'):
        entries=receipt['libraries'][name]
        paths=[Path(entry['path']) for entry in entries]
        for path,entry in zip(paths,entries):
            assert path.is_file() and digest(path)==entry['sha256'],('producer artifact mismatch',path)
            search.add(path.parent)
        selected[name]=paths
    if (release/'deps').is_dir():search.add(release/'deps')
    search.update((release/'build').glob('*/*/out'))
    executable=output/'terminal-output'
    command=['rustc','--edition=2024','-C','opt-level=1',str(ROOT/'validation'/'terminal_output.rs'),'-o',str(executable)]
    for name,paths in selected.items():
        for path in paths:command+=['--extern',name+'='+str(path)]
    for directory in sorted(search):command+=['-L','dependency='+str(directory)]
    result=subprocess.run(command,capture_output=True,timeout=180)
    (output/'observer-build.stdout').write_bytes(result.stdout);(output/'observer-build.stderr').write_bytes(result.stderr)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    return executable,{'command':command,'exit':result.returncode,'executable_sha256':digest(executable),
                       'libraries':receipt['libraries']}

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--producer-release',required=True,type=Path)
    parser.add_argument('--producer-commit',required=True)
    parser.add_argument('--producer-receipt',required=True,type=Path)
    parser.add_argument('--fuel',type=int,default=100_000_000)
    parser.add_argument('--timeout',type=int,default=900)
    parser.add_argument('--output',type=Path,default=ROOT/'build'/'terminal-validation')
    parser.add_argument('--case',action='append',dest='cases')
    options=parser.parse_args();output=options.output.resolve();output.mkdir(parents=True,exist_ok=True)
    oracle=load_oracle();refusals=json.loads((ROOT/'validation'/'refusals.json').read_text())
    names=options.cases or [*oracle['cases'],*refusals]
    assert all(name in oracle['cases'] or name in refusals for name in names)
    assert options.fuel>0 and options.timeout>0
    release=options.producer_release.resolve()
    version=subprocess.check_output([str(release/'omega'),'--version'],text=True).strip()
    assert options.producer_commit in version,('producer revision mismatch',version,options.producer_commit)
    receipt=json.loads(options.producer_receipt.read_text())
    assert receipt['commit']==options.producer_commit
    assert receipt['binary_sha256']==digest(release/'omega')
    executable,build=build_observer(release,output,receipt)
    command=[str(executable),str(ROOT),str(output),str(options.fuel),str(options.timeout),*names]
    # The helper compiles once and executes the exact retained artifact for each
    # finite input. Every API call has an explicit sponsor allowance; the outer
    # process additionally has a wall-clock bound. A decline cannot become pass.
    log=output/'observer.log';started=time.monotonic()
    with log.open('wb') as stream:
        result=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT,timeout=options.timeout*len(names)+180)
    assert result.returncode==0,log.read_text(errors='replace')
    results=[]
    for name in names:
        code=int((output/(name+'.terminal.exit')).read_text())
        stdout=(output/(name+'.terminal.stdout')).read_bytes();stderr=(output/(name+'.terminal.stderr')).read_bytes()
        assert stderr==b'',(name,stderr)
        if name in oracle['cases']:
            assert code==0,(name,code);check_report(stdout,name,oracle)
        else:
            expected=refusals[name];assert code==expected['exit'] and expected['fragment'].encode() in stdout,(name,code,stdout)
        results.append({'case':name,'input_sha256':digest(ROOT/'data'/(name+'.txt')),'exit':code,'stdout_sha256':hashlib.sha256(stdout).hexdigest(),'numerical_or_refusal_oracle':'matched'})
    record={'accepted':True,'engine':'genuine Terminal runtime','source_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPOSITORY,text=True).strip(),
            'producer_commit':options.producer_commit,'producer_version':version,'producer_binary_sha256':digest(release/'omega'),'source_hashes':{str(path.relative_to(ROOT)):digest(path) for path in ROOT.rglob('*.omg') if 'build' not in path.relative_to(ROOT).parts},
            'producer_receipt_sha256':digest(options.producer_receipt),'refusals_sha256':digest(ROOT/'validation'/'refusals.json'),'oracle_sha256':ORACLE_SHA256,'target':'linux_x86_64','fuel_per_case':options.fuel,'elapsed_seconds':time.monotonic()-started,
            'observer_build':build,'command':command,'exit':result.returncode,'cases':results}
    (output/'results.json').write_text(json.dumps(record,indent=2)+'\n')
    print('Terminal: '+str(len(results))+' actual numerical/refusal outputs matched frozen expectations')
if __name__=='__main__':main()
