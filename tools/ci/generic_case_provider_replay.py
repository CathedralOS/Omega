"""Temporary frozen-input baseline; no Cargo build and no current-Main claim."""
from pathlib import Path
import hashlib, importlib.util, json, os, shutil, subprocess, sys, time
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
from compiler_profile import require_optimized
OUT = ROOT / 'build/compiler-pilot'
OUT.mkdir(parents=True, exist_ok=True)
D = OUT / 'generic-case-provider-replay'
D.mkdir()
INPUT = ROOT / 'build/generic-retained-input'
def h(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def git(*a): return subprocess.check_output(['git', *a], cwd=ROOT, text=True).strip()
def save(name, value): (D / name).write_text(json.dumps(value, indent=2) + '\n')
candidate = git('rev-parse', 'HEAD')
assert not git('status', '--porcelain')
previous = json.loads((INPUT / 'summary.json').read_text())['actual_producer']
assert previous['revision'] == '1f3c4da0217eac2b7401cc0d3b3e0186d954e7e2'
inputs = json.loads((INPUT / 'generic-case-migration1/source-inputs.json').read_text())['files']
frozen = ROOT / 'build/verified-generic-producer-source1f3'
subprocess.run(['git', 'worktree', 'add', '--detach', str(frozen), previous['revision']], cwd=ROOT, check=True)
assert all(h(frozen / p) == value for p, value in inputs.items())
spec = importlib.util.spec_from_file_location('generic_frozen_owner', frozen / 'tools/ci/compiler_pilot.py')
owner = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = owner
spec.loader.exec_module(owner)
retained = OUT / 'producer-generic-case-provider-replay'
retained.mkdir()
for name in ['omega', 'corpus_runner', 'corpus_runner.compiler-profile.json']:
    shutil.copy2(INPUT / Path(previous['cli']).parent.name / name, retained / name)
cli, runner = retained / 'omega', retained / 'corpus_runner'
for binary in [cli, runner]: binary.chmod(binary.stat().st_mode | 0o111)
cli_profile, runner_profile = require_optimized(cli), require_optimized(runner)
assert cli_profile['sha256'] == previous['cli_profile']['sha256']
assert runner_profile['sha256'] == previous['runner_profile']['sha256']
producer = owner.CompilerProducer(previous['revision'], previous['tree'], previous['target'], previous['toolchain'], cli.resolve(), runner.resolve(), cli_profile, runner_profile)
producer.verify()
(retained / 'producer.json').write_text(json.dumps(producer.record(), indent=2) + '\n')
for binary, profile in [(cli, cli_profile), (runner, runner_profile)]:
    Path(str(binary) + '.verified-profile.json').write_text(json.dumps(dict(source_revision=producer.revision, source_tree=producer.tree, toolchain=producer.toolchain, observation=profile), indent=2) + '\n')
os.environ['OMEGA_LIBRARY_ROOT'] = str(frozen / 'source/library')

source = INPUT / 'generic-case-migration1/controls/competing-candidates'
project = D / 'frozen-competing-candidates'
project.mkdir()
files = {}
for name in ['main.omg', 'build.omg']:
    shutil.copy2(source / name, project / name)
    assert (source / name).read_bytes() == (project / name).read_bytes()
    files[name] = dict(sha256=h(project/name), text=(project/name).read_text())
fragments = ['slot `CaseObservation` has 4 covering provider plans for the selected target', 'choose one in build.omg', 'Build::select_provider<CaseObservation, ProviderType>']
import sample_native_gate
started = time.monotonic()
failure, raw = sample_native_gate.accept_review(str(cli), str(project), producer.target, 180)
(D/'competing-candidates.review.txt').write_text(raw)
passed = failure is not None and all(fragment in raw for fragment in fragments)
producer.verify()
assert git('rev-parse','HEAD') == candidate and not git('status','--porcelain')
assert all(h(project/name) == row['sha256'] for name,row in files.items())
summary = dict(candidate=candidate, actual_producer=producer.record(), Rust_build=False, current_Main_execution_claim=False, replay_of=dict(run=41463557701, candidate='9ec6893f3cf313dc27165ee561b2a7f4b8036654'), files=files, expected_fragments=fragments, phase='normal package review', seconds=time.monotonic()-started, failure=failure, runtime_executed=False, passed=passed)
save('verdict.json', summary)
save('source-inputs.json', dict(actual_compiler_revision=producer.revision, files=inputs))
(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary),flush=True)
sys.exit(0 if passed else 1)
