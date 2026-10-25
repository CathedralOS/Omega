"""Capture actual retained Terminal execution through the public project APIs."""
import argparse,hashlib,json,os,subprocess,time
from pathlib import Path
from validate import ROOT,load,check

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--producer',type=Path,required=True);parser.add_argument('--receipt',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    options=parser.parse_args();output=options.output.resolve();output.mkdir(parents=True,exist_ok=True);receipt=json.loads(options.receipt.read_text());oracle=load();selected={};search=set()
    assert receipt['binary_sha256']==digest(options.producer/'omega') and receipt['build_finished_success']
    for name in ('compiler', 'terminal_runtime','omega_build', 'package_manager','terminal_interpreter','target'):
        selected[name]=[]
        for artifact in receipt['libraries'][name]:
            path=Path(artifact['path']);assert digest(path)==artifact['sha256'];selected[name].append(path);search.add(path.parent)
    search.add(options.producer.resolve()/'deps');search.update((options.producer.resolve()/'build').glob('*/*/out'))
    executable=output/'terminal-output';command=['rustc','--edition=2024','-C','opt-level=1',str(ROOT/'validation/terminal_output.rs'),'-o',str(executable)]
    for name,paths in selected.items():
        for path in paths:command+=['--extern',name+'='+str(path)]
    for directory in sorted(search):command+=['-L','dependency='+str(directory)]
    observed=subprocess.run(command,capture_output=True,timeout=180)
    (output/'observer-build.stdout').write_bytes(observed.stdout);(output/'observer-build.stderr').write_bytes(observed.stderr);assert observed.returncode==0,observed.stderr.decode(errors='replace')
    started=time.monotonic();command=[str(executable),str(ROOT),str(output),'100000000','60',*oracle['cases']]
    with (output/'observer.log').open('wb') as stream:observed=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT,timeout=60*len(oracle['cases'])+180)
    assert observed.returncode==0,(output/'observer.log').read_text();results=[]
    for name in oracle['cases']:
        results.append(check((output/f'{name}.terminal.stdout').read_bytes(),(output/f'{name}.terminal.stderr').read_bytes(),int((output/f'{name}.terminal.exit').read_text()),name,oracle))
    record=dict(engine='retained canonical Terminal artifact; exact runtime provisions',producer=receipt,observer_sha256=digest(executable),source_hashes={str(path.relative_to(ROOT)):digest(path) for path in ROOT.rglob('*.omg') if 'build' not in path.relative_to(ROOT).parts},fuel_per_case=100000000,elapsed_seconds=time.monotonic()-started,cases=results)
    (output/'results.json').write_text(json.dumps(record,indent=2)+'\n');print('Terminal:',len(results),'actual frozen numerical/refusal cases matched')
if __name__=='__main__':main()
