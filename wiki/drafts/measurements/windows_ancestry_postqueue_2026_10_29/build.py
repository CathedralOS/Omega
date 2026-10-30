import hashlib,json,os,shutil,subprocess,sys,time
from pathlib import Path
root=Path(__file__).resolve().parents[4]; flavor=sys.argv[1]; assert flavor in ('baseline','candidate')
data=root/'build/windows-ancestry-postqueue'/flavor; data.mkdir(parents=True)
assert not any(os.environ.get(k) for k in ('RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_PROFILE_RELEASE_CODEGEN_UNITS'))
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def inputs():
    names=subprocess.check_output(['git','ls-files','omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo'],cwd=root,text=True).splitlines()
    return {p:digest(root/p) for p in names}
manifest=inputs(); revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
target=root.parent/'parallelism-sweep/build/parallelism-target'
command=['mbx','build','--release','--locked','-p','omega','--bin','omega','--target-dir',str(target),'--message-format=json-render-diagnostics']
started=time.perf_counter()
with (data/'build.stdout').open('w',encoding='utf-8') as out,(data/'build.stderr').open('w',encoding='utf-8') as err:
    result=subprocess.run(command,cwd=root,env=dict(os.environ,CARGO_UNSTABLE_CHECKSUM_FRESHNESS='true',CARGO_BUILD_FINGERPRINT='content'),stdout=out,stderr=err)
report={'command':command,'exit':result.returncode,'source_revision':revision,'source_manifest':manifest,'wall_seconds':time.perf_counter()-started}
(data/'build.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
assert result.returncode==0
assert inputs()==manifest
sys.path.insert(0,str(root/'tools'))
from compiler_profile import record_cargo_build,require_optimized
binary=target/'release/omega.exe'; receipt=record_cargo_build(binary,(data/'build.stdout').read_text(encoding='utf-8'))
frozen=data/'producer'; frozen.mkdir()
for name in ('omega.exe','omega.exe.compiler-profile.json','omega.pdb'):
    if (binary.parent/name).exists(): shutil.copy2(binary.parent/name,frozen/name)
producer=require_optimized(frozen/'omega.exe'); producer['source_revision']=revision
(data/'producer.json').write_text(json.dumps({'producer':producer,'build_receipt':receipt},indent=2)+'\n',encoding='utf-8')
print(flavor,producer['sha256'],flush=True)
