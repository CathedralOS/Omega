from pathlib import Path
import gzip,hashlib,json,shutil
root=Path.cwd();original=root/'.github/diagnostics/filesystem-range-replay';out=root/'build/fs-range-replay';out.mkdir(parents=True,exist_ok=True);src=out/'src';src.mkdir(exist_ok=True)
check=root/'omega-rust/psi/compilation/pipeline/01_source-compilation/src/operations/checking/src'
for name in ['known','prove']:
 text=(check/(name+'.rs')).read_text()
 if name=='prove':text+='\nimpl Prover<\'_> { pub(crate) fn replay_state(&self) { eprintln!("REPLAY_STATE remaining={} facts={} values={}",self.remaining_case_branches,self.facts.len(),self.values.count()); } }\n'
 (src/(name+'.rs')).write_text(text);(src/name).symlink_to(check/name,target_is_directory=True)
text=(original/'main.rs').read_text().replace('KNOWN_PATH',str(src/'known.rs')).replace('PROVE_PATH',str(src/'prove.rs')).replace('        drop(prover);','        prover.replay_state();\n        drop(prover);')
(src/'main.rs').write_text(text)
for name in ['inputs.json','expected.txt']:(out/name).write_bytes(gzip.decompress((original/(name+'.gz')).read_bytes()))
paths={'arena':'foundation/arena','semantic-vocabulary':'compilation/representations/semantic-vocabulary','proof-admission':'compilation/proof-admission','terminal-check-tables':'compilation/representations/terminal-psi/check-tables','terminal-psi':'compilation/representations/terminal-psi','terminal-codec':'compilation/representations/terminal-psi/codec','execution-admission':'foundation/execution-admission'}
manifest='''[package]
name="filesystem-range-replay"
version="0.0.0"
edition="2024"
[workspace]
[features]
test-support=[]
[profile.dev]
debug=0
opt-level=1
[profile.test]
debug=0
opt-level=1
[profile.dev.package.proof-admission]
opt-level=3
[profile.dev.package.terminal-semantics]
opt-level=3
[profile.dev.package.sha2]
opt-level=3
[dependencies]
mimalloc={version="0.1",default-features=false}
serde_json="=1.0.151"
'''
for name,path in paths.items():manifest+=name+'={path='+json.dumps(str(root/'omega-rust/psi'/path))+'}\n'
(out/'Cargo.toml').write_text(manifest);shutil.copy2(root/'Cargo.lock',out/'Cargo.lock')
files=[check/'known.rs',check/'prove.rs',check/'prove/tests.rs',original/'main.rs',original/'inputs.json.gz',original/'expected.txt.gz']
(out/'source-digests.json').write_text(json.dumps({str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in files},indent=2)+'\n')
