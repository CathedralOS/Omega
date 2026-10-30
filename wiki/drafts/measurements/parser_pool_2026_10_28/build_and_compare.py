"""Build/check one candidate under a single build-slot hold, then compare frozen binaries."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path.cwd()
evidence = root / 'build/parser-pool-experiment'
environment = dict(os.environ, CARGO_UNSTABLE_CHECKSUM_FRESHNESS='true', CARGO_BUILD_FINGERPRINT='content')


def step(name, command):
    print(name, flush=True)
    with (evidence / (name + '.stdout')).open('w', encoding='utf-8') as output, (evidence / (name + '.stderr')).open('w', encoding='utf-8') as errors:
        result = subprocess.run(command, cwd=root, env=environment, stdout=output, stderr=errors)
    if result.returncode:
        print((evidence / (name + '.stderr')).read_text(encoding='utf-8', errors='replace')[-6000:])
        raise SystemExit(result.returncode)


target = str(root / 'build/parallelism-target')
command = ['mbx','build','-p','omega','--release','--message-format=json-render-diagnostics','--target-dir',target]
step('build', command)
sys.path.insert(0, str(root / 'tools'))
from compiler_profile import record_cargo_build, require_optimized
binary = root / 'build/parallelism-target/release/omega.exe'
receipt = record_cargo_build(binary, (evidence / 'build.stdout').read_text(encoding='utf-8'))
candidate = evidence / 'candidate'
candidate.mkdir()
for name in ('omega.exe','omega.exe.compiler-profile.json','omega.pdb'):
    shutil.copy2(binary.parent / name, candidate / name)
producers = {flavor: require_optimized(evidence / flavor / 'omega.exe') for flavor in ('baseline','candidate')}
producers['baseline']['source_revision'] = 'bf39a3c901bc8ea3ee60b8b57fdc408a591dbacb'
producers['candidate']['source_revision'] = subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
(evidence / 'candidate-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
(evidence / 'producers.json').write_text(json.dumps(producers,indent=2)+'\n',encoding='utf-8')
step('parser-tests',['mbx','nextest','run','--release','-p','tokens-to-syntax-trees','-p','psi-compiler','-E','test(source_parsing)','--target-dir',target])
step('parser-clippy',['mbx','clippy','--release','-p','tokens-to-syntax-trees','-p','psi-compiler','--lib','--target-dir',target])
subprocess.run([sys.executable,'wiki/drafts/measurements/parser_pool_2026_10_28/compare.py'],check=True)
