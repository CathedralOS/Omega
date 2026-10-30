"""Freeze an optimized producer from either fixed integration workspace."""
import hashlib,json,os,shutil,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]; workspace=Path(sys.argv[1]); flavor=sys.argv[2]
assert flavor in ('baseline','candidate')
DATA=ROOT/'build/windows-custody-landing'/flavor; DATA.mkdir(parents=True)
assert not any(os.environ.get(k) for k in ('RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_PROFILE_RELEASE_CODEGEN_UNITS'))
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def inputs():
    names=subprocess.check_output(['git','ls-files','omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo'],cwd=workspace,text=True).splitlines()
    return {p:digest(workspace/p) for p in names}
manifest=inputs(); revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=workspace,text=True).strip()
target=ROOT.parent/'parallelism-sweep/build/parallelism-target'
command=['mbx','build','--release','--locked','-p','omega','--bin','omega','--target-dir',str(target),'--message-format=json-render-diagnostics']
started=time.perf_counter()
with (DATA/'build.stdout').open('w',encoding='utf-8') as out,(DATA/'build.stderr').open('w',encoding='utf-8') as err:
    result=subprocess.run(command,cwd=workspace,env=dict(os.environ,CARGO_UNSTABLE_CHECKSUM_FRESHNESS='true',CARGO_BUILD_FINGERPRINT='content'),stdout=out,stderr=err)
(DATA/'build.json').write_text(json.dumps({'command':command,'workspace':str(workspace),'source_revision':revision,'source_manifest':manifest,'exit':result.returncode,'wall_seconds':time.perf_counter()-started},indent=2)+'\n')
assert result.returncode==0; assert inputs()==manifest
sys.path.insert(0,str(ROOT/'tools'))
from compiler_profile import record_cargo_build,require_optimized
binary=target/'release/omega.exe'; receipt=record_cargo_build(binary,(DATA/'build.stdout').read_text(encoding='utf-8'))
frozen=DATA/'producer'; frozen.mkdir()
for name in ('omega.exe','omega.exe.compiler-profile.json','omega.pdb'):
    if (binary.parent/name).exists(): shutil.copy2(binary.parent/name,frozen/name)
producer=require_optimized(frozen/'omega.exe'); producer['source_revision']=revision
(DATA/'producer.json').write_text(json.dumps({'producer':producer,'build_receipt':receipt},indent=2)+'\n')
print(flavor,revision,producer['sha256'],flush=True)
