from pathlib import Path
import hashlib
import os
import json
import shutil
import subprocess
import sys
import time

record = Path(__file__).parent
label = sys.argv[1]
test_target = os.environ.get('VALIDATION_TEST_TARGET', 'corpus_runner')
workspace = Path.cwd()
sys.path.insert(0, str(workspace / 'tools'))
import compiler_profile

head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
delta = subprocess.check_output(['git', 'diff', 'HEAD', '--', 'omega-rust', 'source/library', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'])
(record / (label + '-source.patch')).write_bytes(delta)
command = ['cargo', 'rustc', '--locked', '-p', 'compiler', '--test', test_target, '--message-format=json', '--', '-C', 'strip=none']
started = time.monotonic()
result = subprocess.run(command, capture_output=True, stdin=subprocess.DEVNULL)
for stream in ('stdout', 'stderr'):
    (record / ((label + '-cargo.') + stream)).write_bytes(getattr(result, stream))
receipt = dict(source=head, patch_sha256=hashlib.sha256(delta).hexdigest(), command=command, exit=result.returncode, seconds=time.monotonic() - started)
(record / (label + '-build.json')).write_text(json.dumps(receipt, indent=2) + '\n')
assert subprocess.check_output(['git', 'diff', 'HEAD', '--', 'omega-rust', 'source/library', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']) == delta
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip() == head
assert result.returncode == 0, result.stderr[-4000:]
messages = [json.loads(line) for line in result.stdout.splitlines() if line.startswith(b'{')]
artifact = next(message for message in reversed(messages) if message.get('reason') == 'compiler-artifact' and message.get('executable') and message.get('target', {}).get('name') == test_target)
original = Path(artifact['executable'])
compiler_profile.record_cargo_build(original, result.stdout.decode())
binary = record / (label + '-omega')
subprocess.run(['/bin/cp', '-c', '-p', str(original), str(binary)], check=True)
shutil.copy2(str(original) + '.compiler-profile.json', str(binary) + '.compiler-profile.json')
receipt['executable'] = compiler_profile.require_optimized(binary)
(record / (label + '-producer.json')).write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt), flush=True)
