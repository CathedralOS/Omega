"""Replay the immutable native product; preserve the real producer's older build provenance."""
from pathlib import Path
import gzip,hashlib,json,subprocess,sys
receipt=json.loads(Path(sys.argv[1]).read_text());root=Path.cwd()
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
for entry in receipt['artifacts']:assert digest(entry['path'])==entry['sha256'],entry['path']
assert hashlib.sha256((root/receipt['source_file']).read_bytes()).hexdigest()==receipt['source_sha256']
changed=subprocess.check_output(['git','diff','--name-only',receipt['semantic_base'],'HEAD','--',*receipt['producer_inputs']],text=True).splitlines()
assert changed==[receipt['source_file']],changed
assert int(json.loads(subprocess.check_output([receipt['compiler'],'--build-profile']))['opt_level'])>0
prepared=json.loads(Path(receipt['preparation_result']).read_text());assert prepared['compiler_sha256']==digest(receipt['compiler']) and prepared['terminal_stdout_agrees'] and prepared['exit_code']==0
expected=Path(receipt['expected_stdout']).read_bytes();assert len(expected)==463
assert Path(receipt['terminal_stdout']).read_bytes()==expected
r=subprocess.run([receipt['native']],stdin=subprocess.DEVNULL,capture_output=True,timeout=30)
assert r.returncode==0 and r.stdout==expected,(r.returncode,r.stdout)
print(json.dumps(dict(native_exit=r.returncode,stdout_bytes=len(r.stdout),stdout_sha256=hashlib.sha256(r.stdout).hexdigest(),native_sha256=digest(receipt['native']),
    terminal_agreement='retained fresh preparation observation; not reexecuted by this replay',producer_sha256=digest(receipt['compiler']),source_sha256=receipt['source_sha256'],original_build_base=receipt['original_build_base'])))
