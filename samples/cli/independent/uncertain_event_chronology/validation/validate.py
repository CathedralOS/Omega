"""Run serial compiler and executable checks against the frozen oracle.
Check and intended-invalid routes remain separate. Never prepares/accepts packages.
"""
import argparse, hashlib, json, os, subprocess, time
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', type=Path, required=True)
    parser.add_argument('--route', choices=['check','native','terminal','invalid'], required=True)
    parser.add_argument('--executable', type=Path)
    arguments = parser.parse_args()
    output = ROOT/'build'/'validation'
    output.mkdir(parents=True,exist_ok=True)
    records = []
    def run(name, command, raw=b'', code=0, stdout=None, fragment=None):
        started=time.monotonic()
        completed=subprocess.run([str(x) for x in command],input=raw,capture_output=True,timeout=1200 if arguments.route in ('check','terminal') else 120)
        (output/(name+'.stdout')).write_bytes(completed.stdout)
        (output/(name+'.stderr')).write_bytes(completed.stderr)
        passed = completed.returncode == code
        if stdout is not None: passed &= completed.stdout.replace(b'\r\n',b'\n') == stdout
        if fragment is not None:
            passed &= fragment.encode() in completed.stderr + completed.stdout
        records.append(dict(name=name,command=[str(x) for x in command],exit=completed.returncode,
                            passed=bool(passed),seconds=round(time.monotonic()-started,3)))
        print(name, 'PASS' if passed else 'FAIL', 'exit', completed.returncode,flush=True)
        return completed
    omega=arguments.omega.resolve()
    root=ROOT/'main.omg'
    if arguments.route == 'check':
        run('check',[omega,'--check','--target','linux_x86_64',root])
    elif arguments.route == 'invalid':
        invalid=ROOT/'controls'/'intended_invalid'
        run('intended_invalid',[omega,'--check',invalid/'main.omg'],code=1,
            fragment=(invalid/'expected.txt').read_text().strip())
    else:
        for case in json.loads((ROOT/'data/cases.json').read_text()):
            folder=ROOT/'data'/case['name']
            raw=(folder/'input.txt').read_bytes()
            assert raw == bytes.fromhex(case['input_hex']), 'fixture bytes changed: '+case['name']
            code=int((folder/'expected_exit.txt').read_text())
            expected=(folder/'expected_stdout.txt').read_bytes()
            if arguments.route == 'native':
                if not arguments.executable: parser.error('--native needs --executable')
                command=[arguments.executable.resolve()]
            else:
                command=[omega,'run','--both','--keep','--target','linux_x86_64',root]
            completed=run(arguments.route+'_'+case['name'],command,raw,code,expected)
            if arguments.route == 'terminal':
                # DECLINED is recorded as unsupported, never interpreted agreement.
                diagnostic=completed.stderr.decode(errors='replace')
                records[-1]['terminal_declined']='DECLINED' in diagnostic
                records[-1]['passed'] &= not records[-1]['terminal_declined']
            if not records[-1]['passed']: break
    report=dict(route=arguments.route,binary=str(omega),binary_sha256=hashlib.sha256(omega.read_bytes()).hexdigest(),
                target='linux_x86_64',records=records)
    (output/(arguments.route+'.json')).write_text(json.dumps(report,indent=2)+'\n')
    return 0 if all(r['passed'] for r in records) else 1

if __name__ == '__main__': raise SystemExit(main())
