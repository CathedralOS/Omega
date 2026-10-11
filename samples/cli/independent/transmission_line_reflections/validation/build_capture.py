#!/usr/bin/env python3
"""Link the capture consumer against one already verified mbx compiler build.
Run after the compiler build stops; no parallel builds or dependency compilation.
"""
from pathlib import Path
import argparse,hashlib,json,subprocess,sys
parser=argparse.ArgumentParser()
parser.add_argument('--receipt',type=Path,help='JSONL receipt from mbx build -p omega --release --message-format=json')
parser.add_argument('--omega-sha256',help='verified compiler hash required with a receipt')
options=parser.parse_args()
root=Path(__file__).resolve().parents[5]
project=Path(__file__).resolve().parents[1]
target=(root/'target/release').resolve()
outputs=sorted((target/'build').glob('*/*/out'))
externals={}
artifacts={}
if options.receipt:
    if not options.omega_sha256: raise SystemExit('--receipt requires --omega-sha256')
    compilers=[]
    for line in options.receipt.read_text().splitlines():
        try: row=json.loads(line)
        except ValueError: continue
        if row.get('reason')!='compiler-artifact': continue
        name=row.get('target',{}).get('name')
        if name=='omega' and row.get('executable'): compilers.append(Path(row['executable']))
        if name in ['compiler','package_manager','terminal_interpreter','target']:
            candidates=[Path(item) for item in row.get('filenames',[]) if item.endswith('.rmeta')]
            if not candidates:
                candidates=[Path(item).with_suffix('.rmeta') for item in row.get('filenames',[]) if item.endswith('.rlib')]
            if len(candidates)==1: artifacts.setdefault(name,set()).add(candidates[0])
    if len(set(compilers))!=1: raise SystemExit('receipt must identify one production omega executable')
    if hashlib.sha256(compilers[0].read_bytes()).hexdigest()!=options.omega_sha256:
        raise SystemExit('receipt compiler hash differs from the verified binary')
for package in ['compiler','package_manager','terminal_interpreter','target']:
    candidates=(sorted(artifacts.get(package,set())) if options.receipt else
                [item for directory in outputs for item in directory.glob(f'lib{package}-*.rmeta')])
    if len(candidates)!=1: raise SystemExit(f'need one verified {package} build, found {candidates}')
    externals[package]=candidates[0]
command=['rustc','--edition=2024','-C','opt-level=1',str(project/'validation/terminal_capture.rs'),'-o',str(project/'build/terminal_capture')]
for directory in outputs:command+=['-L',f'dependency={directory}']
for package,path in externals.items():
    command+=['--extern',f'{package}={path}','--extern',f'{package}={path.with_suffix(".rlib")}']
(project/'build').mkdir(exist_ok=True)
(project/'build/terminal_capture_build.json').write_text(json.dumps(command,indent=2)+'\n')
(project/'build/terminal_capture_dependencies.json').write_text(json.dumps({
    'receipt_sha256':hashlib.sha256(options.receipt.read_bytes()).hexdigest() if options.receipt else None,
    'compiler_sha256':options.omega_sha256,
    'inputs':{str(path):hashlib.sha256(path.read_bytes()).hexdigest()
              for metadata in externals.values() for path in [metadata,metadata.with_suffix('.rlib')]},
    'source_sha256':hashlib.sha256((project/'validation/terminal_capture.rs').read_bytes()).hexdigest()
},indent=2)+'\n')
raise SystemExit(subprocess.run(command,cwd=root).returncode)
