import argparse, hashlib, importlib.util, json, os, pathlib, re, shutil, subprocess, sys, time
root = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'tools'))
from compiler_profile import record_cargo_build, require_optimized
parser = argparse.ArgumentParser()
parser.add_argument('mode', choices=['preserve', 'prepare'])
parser.add_argument('label')
parser.add_argument('--profile', action='store_true')
parser.add_argument('--sample', action='store_true')
parser.add_argument('--both', action='store_true')
parser.add_argument('--cold', action='store_true')
args = parser.parse_args()
home = root / 'build/prime-preparation-2'
if args.mode == 'preserve':
    binary = root / 'target/release/omega.exe'
    cargo_bytes = (home / f'{args.label}-cargo.jsonl').read_bytes()
    cargo = cargo_bytes.decode('utf-16' if cargo_bytes.startswith(b'\xff\xfe') else 'utf-8-sig')
    record_cargo_build(binary, cargo)
    destination = home / args.label
    destination.mkdir()
    for filename in ['omega.exe', 'omega.pdb', 'omega.exe.compiler-profile.json']:
        shutil.copyfile(binary.parent / filename, destination / filename)
    record = dict(source_revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(), producer=require_optimized(destination/'omega.exe'))
    patch = subprocess.check_output(['git','diff','--binary'],cwd=root)
    (destination/'source.patch').write_bytes(patch)
    record['patch_sha256'] = hashlib.sha256(patch).hexdigest()
    record['changed_inputs'] = {name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in subprocess.check_output(['git','diff','--name-only'],cwd=root,text=True).splitlines() if (root/name).is_file()}
    (destination/'producer.json').write_text(json.dumps(record,indent=2),encoding='utf-8')
    print(json.dumps(record),flush=True)
    sys.exit(0)
binary = home / args.label / 'omega.exe'
producer = require_optimized(binary)
trial = home / f'{args.label}-{time.time_ns()}'
trial.mkdir()
project = root / 'samples/cli/arithmetic/prime_counter'
for name in ['omega.lock', 'build']:
    current = project / name
    if current.exists():
        assert current.resolve().is_relative_to(project.resolve())
        current.rename(trial / ('previous-' + name))
spec = importlib.util.spec_from_file_location('benchmark',root/'tools/benchmark/benchmark.py')
benchmark = importlib.util.module_from_spec(spec); spec.loader.exec_module(benchmark)
environment = os.environ.copy()
if args.cold:
    environment['LOCALAPPDATA'] = str(trial/'cache')
environment['OMEGA_LIBRARY_ROOT'] = str(root/'source/library')
if args.profile:
    environment.update(OMEGA_REVIEW_TIMINGS='1', OMEGA_PHASE_PROFILE='1')
observations = []
def invoke(command, cwd=None):
    output = trial / f'command-{len(observations)}.txt'
    started = time.monotonic()
    with output.open('wb') as transcript:
        process = subprocess.Popen(list(map(str,command)),cwd=cwd or root,env=environment,stdin=subprocess.DEVNULL,stdout=transcript,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            if args.sample and len(observations) == 0:
                for delay in [10, 15, 15]:
                    try: process.wait(timeout=delay); break
                    except subprocess.TimeoutExpired:
                        with (trial/f'stack-{time.time_ns()}.txt').open('wb') as stack:
                            subprocess.run(['C:/Program Files/WindowsApps/Microsoft.WinDbg_1.2606.22001.0_x64__8wekyb3d8bbwe/amd64/cdb.exe','-pv','-p',str(process.pid),'-y',str(binary.parent),'-c','~* k 35; q'],stdout=stack,stderr=subprocess.STDOUT,timeout=25)
            process.wait(timeout=360)
        finally:
            if process.poll() is None: process.terminate(); process.wait(timeout=10)
    text = output.read_text(encoding='utf-8',errors='replace')
    observation = dict(command=list(map(str,command)), exit_code=process.returncode, seconds=time.monotonic()-started, transcript=output.name)
    observations.append(observation)
    print(json.dumps(observation),flush=True)
    return process.returncode, text
benchmark.run_plain = invoke
started = time.monotonic()
try:
    lock = benchmark.settle_package_review(binary,project,'windows_x86_64')
    result = dict(seconds=time.monotonic()-started, producer=producer, observations=observations, source_revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(), source_hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in project.glob('*.omg')}, lock_sha256=hashlib.sha256(lock.read_bytes()).hexdigest(), profile=args.profile, sampled=args.sample, cold=args.cold, cache_root=environment.get('LOCALAPPDATA'), library_root=environment['OMEGA_LIBRARY_ROOT'])
    comparison_lock=re.sub(rb'(?m)^compiler [0-9a-f]{64}$',b'compiler '+b'0'*64,lock.read_bytes())
    result['lock_without_compiler_identity_sha256']=hashlib.sha256(comparison_lock).hexdigest()
    if args.both:
        code, text = invoke([binary,'run','--both','--keep',project/'main.omg'])
        assert code == 8 and 'interp exit: 8' in text, text
        kept = re.search(r'build dir kept: (.+)',text)
        assert kept,text
        products=pathlib.Path(kept.group(1).strip())
        result['products']={str(path.relative_to(products)):hashlib.sha256(path.read_bytes()).hexdigest() for path in products.rglob('*') if path.is_file() and path.suffix in ['.exe','.terminal','.psi','.psiterm','.oterm']}
        result['engines_agree'] = True
    (trial/'result.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print('PREPARATION_RESULT',str(trial),result['seconds'],flush=True)
finally:
    for name in ['omega.lock','build']:
        current = project / name
        if current.exists(): current.rename(trial/('produced-'+name))
        previous = trial/('previous-'+name)
        if previous.exists(): previous.rename(current)
