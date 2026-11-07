"""Run serial compiler and executable checks against the frozen oracle.
Check and intended-invalid routes remain separate. Never prepares/accepts packages.
"""
import argparse, hashlib, json, re, subprocess, tempfile, time
from verify_frozen import cases, main as verify_frozen
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', type=Path)
    parser.add_argument('--route', choices=['check','native','host-comparison','terminal','invalid'], required=True)
    parser.add_argument('--executable', type=Path)
    parser.add_argument('--terminal-sponsor', type=Path,
                        help='public SDK sponsor built from terminal_sdk.rs; direct Terminal streams')
    arguments = parser.parse_args()
    if arguments.route == 'terminal':
        if not arguments.terminal_sponsor: parser.error('--route terminal needs --terminal-sponsor')
    elif not arguments.omega:
        parser.error('--omega is required for this route')
    output = ROOT/'build'/'validation'
    output.mkdir(parents=True,exist_ok=True)
    verify_frozen()
    records = []
    def run(name, command, raw=b'', code=0, stdout=None, fragment=None):
        started=time.monotonic()
        limit=1200 if arguments.route in ('check','host-comparison','terminal') else 120
        try:
            completed=subprocess.run([str(x) for x in command],input=raw,capture_output=True,timeout=limit)
        except subprocess.TimeoutExpired as error:
            (output/(name+'.stdout')).write_bytes(error.stdout or b'')
            (output/(name+'.stderr')).write_bytes(error.stderr or b'')
            records.append(dict(name=name,command=[str(x) for x in command],exit=None,passed=False,
                                timed_out=True,seconds=round(time.monotonic()-started,3),limit_seconds=limit))
            print(name,'TIMEOUT',flush=True)
            return None
        (output/(name+'.stdout')).write_bytes(completed.stdout)
        (output/(name+'.stderr')).write_bytes(completed.stderr)
        passed = completed.returncode == code
        if stdout is not None: passed &= completed.stdout == stdout
        if fragment is not None:
            passed &= fragment.encode() in completed.stderr + completed.stdout
        records.append(dict(name=name,command=[str(x) for x in command],exit=completed.returncode,
                            passed=bool(passed),seconds=round(time.monotonic()-started,3)))
        print(name, 'PASS' if passed else 'FAIL', 'exit', completed.returncode,flush=True)
        return completed
    omega=arguments.omega.resolve() if arguments.omega else None
    binary=arguments.terminal_sponsor.resolve() if arguments.route == 'terminal' else omega
    root=ROOT/'main.omg'
    if arguments.route == 'check':
        run('check',[omega,'--check','--target','linux_x86_64',root])
    elif arguments.route == 'invalid':
        invalid=ROOT/'controls'/'intended_invalid'
        run('intended_invalid',[omega,'--check',invalid/'main.omg'],code=1,
            fragment=(invalid/'expected.txt').read_text().strip())
    elif arguments.route == 'terminal':
        # One accepted compilation, then fresh input/runtime per frozen case.
        # A unique directory prevents stale successful captures masking failure.
        observed=Path(tempfile.mkdtemp(prefix='terminal-sdk-',dir=output))
        roster=list(cases())
        names=observed/'cases.txt'
        names.write_text(''.join(case['name']+'\n' for case in roster))
        completed=run('terminal_sdk',[binary,ROOT,names,observed])
        if completed is not None and completed.returncode == 0:
            try:
                rows=(observed/'records.tsv').read_text().splitlines()[1:]
                assert len(rows)==len(roster)==44
                for case,row in zip(roster,rows):
                    name=case['name']; fields=row.split('\t'); folder=ROOT/'data'/name
                    assert (folder/'input.txt').read_bytes()==bytes.fromhex(case['input_hex'])
                    code=int((folder/'expected_exit.txt').read_text())
                    stdout=(observed/(name+'.stdout')).read_bytes()
                    stderr=(observed/(name+'.stderr')).read_bytes()
                    passed=(fields[:3]==[name,'PASS',f'Some({code})']
                            and stdout==(folder/'expected_stdout.txt').read_bytes() and stderr==b'')
                    records.append(dict(name='terminal_'+name,exit=code if passed else None,
                                        passed=passed,stdout_path=str(observed/(name+'.stdout')),
                                        stderr_path=str(observed/(name+'.stderr'))))
                    print('terminal_'+name,'PASS' if passed else 'FAIL',flush=True)
            except (OSError,AssertionError,IndexError) as error:
                records.append(dict(name='terminal_captures',passed=False,error=str(error)))
    else:
        for case in cases():
            folder=ROOT/'data'/case['name']
            raw=(folder/'input.txt').read_bytes()
            assert raw == bytes.fromhex(case['input_hex']), 'fixture bytes changed: '+case['name']
            code=int((folder/'expected_exit.txt').read_text())
            expected=(folder/'expected_stdout.txt').read_bytes()
            if arguments.route == 'native':
                if not arguments.executable: parser.error('--route native needs --executable')
                command=[arguments.executable.resolve()]
            else:
                command=[omega,'run','--both','--keep',root]
            completed=run(arguments.route+'_'+case['name'],command,raw,code,expected)
            if completed is None: break
            if arguments.route == 'host-comparison':
                # CLI stdout is native. Its Terminal comparison checks termination
                # only; require positive agreement rather than absence of decline.
                diagnostic=completed.stderr.decode(errors='replace')
                records[-1]['terminal_declined']='DECLINED' in diagnostic
                records[-1]['terminal_exit_agreement']=bool(re.search(
                    rf'^interp exit: {code}$',diagnostic,re.MULTILINE))
                records[-1]['passed'] &= (records[-1]['terminal_exit_agreement']
                    and not any(marker in diagnostic for marker in
                                ['DECLINED','DIVERGENCE','Terminal interpretation FAILED']))
            if not records[-1]['passed']: break
    report=dict(route=arguments.route,binary=str(binary),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                stdout_origin='direct Terminal SDK' if arguments.route=='terminal' else 'native' if arguments.route in ('native','host-comparison') else 'compiler diagnostics',
                target='linux_x86_64',records=records,
                source_sha256={str(f.relative_to(ROOT)):hashlib.sha256(f.read_bytes()).hexdigest()
                               for f in ROOT.rglob('*.omg') if 'build' not in f.relative_to(ROOT).parts})
    (output/(arguments.route+'.json')).write_text(json.dumps(report,indent=2)+'\n')
    return 0 if all(r['passed'] for r in records) else 1

if __name__ == '__main__': raise SystemExit(main())
