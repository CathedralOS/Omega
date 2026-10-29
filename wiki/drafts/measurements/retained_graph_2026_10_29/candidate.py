"""Build one bounded-read candidate, then reuse the qualified comparison runner."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
DATA = ROOT / 'build/retained-graph-experiment'
OUTPUT = DATA / 'comparison'
OUTPUT.mkdir()
target = ROOT / 'build/parallelism-target'
env = dict(os.environ, CARGO_UNSTABLE_CHECKSUM_FRESHNESS='true', CARGO_BUILD_FINGERPRINT='content')
print('build bounded-read candidate', flush=True)
with (OUTPUT / 'build.stdout').open('w', encoding='utf-8') as output, (OUTPUT / 'build.stderr').open('w', encoding='utf-8') as errors:
    result = subprocess.run(['mbx', 'build', '-p', 'omega', '--release', '--locked', '--message-format=json-render-diagnostics', '--target-dir', str(target)], cwd=ROOT, env=env, stdout=output, stderr=errors)
if result.returncode:
    print((OUTPUT / 'build.stderr').read_text(encoding='utf-8', errors='replace')[-6000:])
    raise SystemExit(result.returncode)
sys.path.insert(0, str(ROOT / 'tools'))
from compiler_profile import record_cargo_build, require_optimized
binary = target / 'release/omega.exe'
receipt = record_cargo_build(binary, (OUTPUT / 'build.stdout').read_text(encoding='utf-8'))
candidate = OUTPUT / 'candidate'
candidate.mkdir()
for name in ('omega.exe', 'omega.exe.compiler-profile.json', 'omega.pdb'):
    shutil.copy2(binary.parent / name, candidate / name)
profile = json.loads((DATA / 'producer.json').read_text(encoding='utf-8'))['producers']['profile']
producers = {'baseline': require_optimized(profile['binary']), 'candidate': require_optimized(candidate / 'omega.exe')}
assert producers['baseline']['sha256'] == profile['sha256']
producers['baseline']['source_revision'] = profile['source_revision']
producers['candidate']['source_revision'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
(OUTPUT / 'producer.json').write_text(json.dumps({'producers': producers, 'build_receipt': receipt}, indent=2) + '\n', encoding='utf-8')
subprocess.run([sys.executable, 'wiki/drafts/measurements/unplaced_batch_2026_10_28/qualify.py', str(OUTPUT.relative_to(ROOT))], cwd=ROOT, check=True)
subprocess.run([sys.executable, 'wiki/drafts/measurements/retained_graph_2026_10_29/controls.py'], cwd=ROOT, check=True)
