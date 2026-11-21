import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import shutil
import subprocess
import sys
import tempfile

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

root = Path(os.environ['GITHUB_WORKSPACE'])
inputs = Path(__file__).resolve().parent
manifest = json.loads((inputs / 'manifest.json').read_text())
compiler = root / 'compiler'
assert platform.machine() == 'arm64'
head = subprocess.check_output(['git', '-C', str(compiler), 'rev-parse', 'HEAD'], text=True).strip()
assert head == manifest['compiler_source']
binary = compiler / 'target/debug/omega'
print('compiler source', head, 'binary SHA256', digest(binary), flush=True)
for rel, row in manifest['overlays'].items():
    dest = compiler / rel
    incoming = inputs / 'overlay' / rel
    assert digest(dest) == row['preimage_sha256'] and digest(incoming) == row['candidate_sha256'], rel
    shutil.copyfile(incoming, dest)
    print('std file', rel, digest(dest))
project = Path(tempfile.mkdtemp(dir=os.environ['RUNNER_TEMP']))
for name, sha in manifest['project_inputs'].items():
    assert digest(inputs / name) == sha
    shutil.copyfile(inputs / name, project / name)
build = project / 'build.omg'
old = manifest['build_relocation']['original_std_path']
contents = build.read_text()
assert contents.count(old) == 1
library = compiler / 'source/library'
build.write_text(contents.replace(old, str(library / 'std')))
print('original Build', manifest['project_inputs']['build.omg'], 'hosted Build', digest(build))
print('unmodified main', digest(project/'main.omg'), flush=True)
args = [str(binary), 'run', '--both', '--build-input', 'build.omg', '--build-input', 'main.omg', str(build)]
child = subprocess.Popen(args, cwd=project, stdin=subprocess.DEVNULL, env={**os.environ, 'OMEGA_LIBRARY_ROOT':str(library)})
pid, status, usage = os.wait4(child.pid, 0)
child.returncode = os.waitstatus_to_exitcode(status)
print('raw wait status', status, 'return code', child.returncode, 'maximum RSS (bytes)', usage.ru_maxrss, 'user seconds', usage.ru_utime, 'system seconds', usage.ru_stime, flush=True)
sys.exit(0 if child.returncode == 0 else 1)
