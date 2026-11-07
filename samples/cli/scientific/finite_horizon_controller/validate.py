"""Sequential native and CLI gate with frozen oracles and trajectory checks."""
import argparse, hashlib, json, os, pathlib, platform, re, subprocess
ROOT = pathlib.Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True)
parser.add_argument('--timeout', type=int, default=600)
options = parser.parse_args()
binary = str(pathlib.Path(options.omega).resolve())
artifacts = ROOT/'build/validation'; artifacts.mkdir(parents=True, exist_ok=True)
manifest = json.loads((ROOT/'data/oracle.json').read_text())
assert hashlib.sha256((ROOT/'data/cases.json').read_bytes()).hexdigest() == manifest['fixture_sha256']
problems = json.loads((ROOT/'data/cases.json').read_text())
records = []
metadata = {'revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
            'binary':binary,'binary_sha256':hashlib.sha256(pathlib.Path(binary).read_bytes()).hexdigest(),
            'target':'linux_x86_64','host':platform.platform(),'cpus':os.cpu_count(),
            'uid':os.getuid() if hasattr(os,'getuid') else None,
            'root_owner':os.stat('/').st_uid,
            'library_root':os.environ.get('OMEGA_LIBRARY_ROOT'),
            'source_sha256':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest()
                for p in sorted(ROOT.rglob('*')) if p.is_file() and 'build' not in p.relative_to(ROOT).parts
                and '__pycache__' not in p.parts and p.name not in ('omega.lock','omega.admissions')}}
(artifacts/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
def run(name, arguments, input_bytes=b'', program=binary):
    command = [program, *arguments]
    result = subprocess.run(command, input=input_bytes, capture_output=True, timeout=options.timeout)
    (artifacts/f'{name}.stdout').write_bytes(result.stdout)
    (artifacts/f'{name}.stderr').write_bytes(result.stderr)
    records.append({'name':name,'command':command,'exit':result.returncode,
                    'stdout_sha256':hashlib.sha256(result.stdout).hexdigest()})
    (artifacts/'commands.json').write_text(json.dumps(records,indent=2)+'\n')
    print(f'{name}: exit {result.returncode}', flush=True)
    return result
checked = run('check',['--check',str(ROOT/'main.omg')])
assert checked.returncode == 0, checked.stderr.decode(errors='replace')
built = run('build',['--build-dir',str(ROOT/'build/native'),'--report-file',str(artifacts/'compile-report.txt'),str(ROOT/'main.omg')])
assert built.returncode == 0, built.stderr.decode(errors='replace')
native_artifact = ROOT/'build/native'/('omega-program.exe' if os.name == 'nt' else 'omega-program')
assert native_artifact.is_file(), native_artifact
metadata['native_artifact'] = str(native_artifact)
metadata['native_artifact_sha256'] = hashlib.sha256(native_artifact.read_bytes()).hexdigest()
(artifacts/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
for case in manifest['cases']:
    key = str(case['selector'])
    input_bytes = (ROOT/f'data/{key}.stdin').read_bytes()
    expected = (ROOT/f'data/expected/{key}.stdout').read_bytes()
    # Execute the retained native build directly for every input. --both remains
    # a separate native/Terminal invocation and compares termination only.
    for route in ['native','both']:
        if route == 'native':
            result = run(f'{key}-{route}', [], input_bytes, str(native_artifact))
            assert result.stderr == b'', (key,result.stderr)
        else:
            result = run(f'{key}-{route}', ['run','--both',str(ROOT/'main.omg')], input_bytes)
        assert result.returncode == case['exit'], (key,route,result.stderr)
        assert result.stdout == expected, (key,route,result.stdout,expected)
        if route == 'both':
            assert f"interp exit: {case['exit']}".encode() in result.stderr, (key,result.stderr)
            assert b'DIVERGENCE' not in result.stderr, (key,result.stderr)
    if case['exit'] == 0:
        p = problems[int(key)]; text = result.stdout.decode(); x = p['initial']; total = 0
        rows = re.findall(r'step=(\d+) action=(-?\d+) state=(-?\d+)',text)
        assert len(rows) == p['horizon']
        assert p['lower'] <= x <= p['upper']
        for t,(step,action,state) in enumerate(rows):
            assert int(step) == t
            u = int(action); next_state = int(state)
            assert p['action_lower'] <= u <= p['action_upper']
            assert next_state == x + u + p['drift']
            assert p['lower'] <= next_state <= p['upper']
            x = next_state
            total += p['state_weight']*(x-p['target'])**2+p['action_weight']*u*u
        assert not p['exact_terminal'] or x == p['target']
        total += p['terminal_weight']*(x-p['target'])**2
        assert total == int(re.search(r'^cost=(\d+)$',text,re.M)[1])
invalid = run('intended-invalid',['--check',str(ROOT/'invalid/main.omg')])
assert invalid.returncode != 0
assert b'horizon' in invalid.stderr and (b'precondition' in invalid.stderr or b'required fact' in invalid.stderr or b'requires' in invalid.stderr or b'prove' in invalid.stderr), invalid.stderr
positive = run('local-byte-loan-control',['run','--both',str(ROOT/'reproduction/main.omg')],b'0\n')
assert positive.returncode == 70 and positive.stdout == b'', positive.stderr
assert b'interp exit: 70' in positive.stderr, positive.stderr
print(f"PASS: local byte loan control, check, {len(manifest['cases'])} native cases, {len(manifest['cases'])} Terminal comparisons, trajectory constraints and intended rejection")
