"""Check archived identities, complete streams and CPU ownership without reruns."""
from pathlib import Path
import gzip,hashlib,importlib.util,json,subprocess
HERE=Path(__file__).resolve().parent
ROOT=next(p for p in HERE.parents if (p/'AGENTS.md').is_file())
def digest(data):return hashlib.sha256(data).hexdigest()
def read(path):return gzip.decompress(path.read_bytes())
def main():
    manifest=json.loads((HERE/'manifest.json').read_text())
    for name,expected in manifest.items():assert digest((HERE/name).read_bytes())==expected,name
    spec=importlib.util.spec_from_file_location('recorded_cpu',HERE/'run.py')
    runner=importlib.util.module_from_spec(spec);spec.loader.exec_module(runner)
    runner.self_check()
    producer=json.loads((HERE/'producer.json').read_text());assert int(producer['build_profile']['opt_level'])>0
    source=json.loads((HERE/'source.json').read_text())
    assert digest((HERE/'diagnostic.patch').read_bytes())==source['patch_sha256']
    assert json.loads((HERE/'patch-checks.json').read_text())['files']==source['files']
    for name,expected in json.loads((HERE/'input-manifest.json').read_text())['inputs'].items():assert digest((ROOT/name).read_bytes())==expected,name
    records=[];outputs=[];images=[]
    for folder in sorted((HERE/'raw').iterdir()):
        record=json.loads(read(folder/'result.json.gz'));records.append(record)
        stdout=read(folder/'stdout.txt.gz');stderr=read(folder/'stderr.txt.gz')
        assert record['exit_code']==0 and record['compiler_sha256']==producer['executable_sha256'] and record['accepted_lock_sha256']==producer['lock_sha256']
        if 'cold' in record['label'] or record['label']=='checkcpu-both':assert record['cache_before']['files']==0
        else:assert record['cache_before']['files']>0
        if record['probe_enabled']:
            check=runner.partition(runner.parse(stderr.decode('utf-8')))
            assert check==record['checking']
            assert check['roots']==3
        else:assert runner.parse(stderr.decode('utf-8'))==[]
        if (folder/'runtime').is_dir():
            result=json.loads(read(folder/'runtime/result.json.gz'));output=read(folder/'runtime/stdout.txt.gz')
            assert result['exit_code']==0 and len(output)==record['stdout_bytes'] and digest(output)==record['stdout_sha256']
            outputs.append(output);images.append(record['image_sha256'])
        else:
            assert record['label']=='checkcpu-both' and b'native exit 0' in stderr and b'interp exit: 0' in stderr and b'DECLINED' not in stdout+stderr
            both=stdout
    assert len(records)==9 and len(images)==8 and len(set(images))==1
    assert all(output==outputs[0] for output in outputs) and both==outputs[0]
    assert len(outputs[0])==463 and runner.ORACLE in outputs[0]
    assert sum(r['probe_enabled'] and 'cold' in r['label'] for r in records)==4
    smoke=json.loads((HERE/'post-merge-smoke.json').read_text())
    assert smoke['status']=='completed' and smoke['conclusion']=='success' and smoke['headSha']==source['base']
    subprocess.run(['git','diff','--exit-code',source['base'],'--','omega-rust','source','samples'],cwd=ROOT,check=True)
    print(f'{len(manifest)} evidence hashes, all nine complete observations, exact CPU forests, eight identical native images and stdout, BOTH equality, source inputs and successful exact-base smoke verified.')
if __name__=='__main__':main()
