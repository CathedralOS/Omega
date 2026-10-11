#!/usr/bin/env python3
"""Serial public-CLI and retained-native validation with a frozen external oracle.
Never edits review decisions or retries package acceptance.
"""
import argparse, hashlib, json, os, subprocess, sys, time
from pathlib import Path
ROOT=Path(__file__).resolve().parent

def invoke(command, label, output, data=None, timeout=600):
    started=time.monotonic()
    result=subprocess.run(command,input=data,capture_output=True,timeout=timeout)
    (output/f'{label}.stdout').write_bytes(result.stdout)
    (output/f'{label}.stderr').write_bytes(result.stderr)
    (output/f'{label}.exit').write_text(str(result.returncode)+'\n')
    record={'command':list(map(str,command)),'exit':result.returncode,'seconds':time.monotonic()-started}
    (output/f'{label}.command.json').write_text(json.dumps(record,indent=2)+'\n')
    print(label,result.returncode,flush=True)
    return result

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',type=Path,required=True)
    parser.add_argument('--route',choices=['check','native','both','invalid','terminal'],required=True)
    parser.add_argument('--cases',help='Comma-separated fixture subset; default is every case')
    options=parser.parse_args()
    binary=options.omega.resolve()
    output=ROOT/'build'/'validation'/options.route
    output.mkdir(parents=True,exist_ok=True)
    expected=json.loads((ROOT/'expected.json').read_text())
    assert hashlib.sha256((ROOT/'expected.json').read_bytes()).hexdigest()=='0b7b74dc28aaed61c8a6e6628abc7fd30744839f134e720560d7eb956df42fbe'
    for filename,digest in expected['fixture_sha256'].items():
        assert hashlib.sha256((ROOT/'data'/filename).read_bytes()).hexdigest()==digest
    prefix=[str(binary)]
    source=str(ROOT/'main.omg')
    if options.route=='check':
        result=invoke(prefix+['--check','--target','linux_x86_64',source],'check',output)
        return result.returncode
    if options.route=='terminal':
        result=invoke(prefix+['inspect-terminal','--machine','Application::main','--target','linux_x86_64',source],'terminal',output)
        return result.returncode
    if options.route=='invalid':
        result=invoke(prefix+['--check','--target','linux_x86_64',str(ROOT/'controls/invalid_bounds/control.omg')],'invalid',output)
        fragment=(ROOT/'controls/invalid_bounds/expected.txt').read_text().strip()
        assert result.returncode==1 and fragment in result.stderr.decode() and 'with 1 diagnostic(s)' in result.stderr.decode(), result.stderr.decode()
        # A wrong-reason refusal cannot satisfy the control.
        assert not any(word in result.stderr.decode().lower() for word in ('cannot resolve','unknown','package acceptance','parse error'))
        print('PASS intended bounds rejection')
        return 0
    if options.route=='native':
        result=invoke(prefix+['--target','linux_x86_64','--build-dir',str(ROOT/'build'/'native'),source],'compile',output)
        if result.returncode: return result.returncode
        executables=[path for path in (ROOT/'build'/'native').rglob('*') if path.is_file() and os.access(path,os.X_OK) and path.read_bytes()[:4]==b'\x7fELF']
        assert len(executables)==1, executables
        command=[str(executables[0])]
        (output/'executable.sha256').write_text(hashlib.sha256(executables[0].read_bytes()).hexdigest()+'\n')
    else:
        command=prefix+['run','--both','--keep','--target','linux_x86_64',source]
    names = options.cases.split(',') if options.cases else list(expected['cases'])
    assert len(names)==len(set(names)) and all(name in expected['cases'] for name in names)
    for name in names:
        result=invoke(command,name,output,(ROOT/'data'/f'{name}.txt').read_bytes())
        if options.route=='both':
            diagnostics=result.stderr.decode()
            assert 'DECLINED' not in diagnostics, (name,diagnostics)
            assert f"interp exit: {expected['cases'][name]['exit']}" in diagnostics and 'DIVERGENCE' not in diagnostics, (name,diagnostics)
    result=subprocess.run([sys.executable,str(ROOT/'reference.py'),'verify',str(output),','.join(names)])
    return result.returncode

if __name__=='__main__': raise SystemExit(main())
