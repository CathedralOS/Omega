from pathlib import Path
import hashlib,json,platform,re,shutil,subprocess,sys
ROOT=Path.cwd();OUT=ROOT/'build/proof-profile';WORK=OUT/'graph-phase-cpu';PREFIX=Path('wiki/drafts/measurements/proof_parallelism_2026_10_28/graph_phase_cpu')
assert not subprocess.check_output(['git','status','--porcelain']).strip()
head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip();retained=json.loads((WORK/'retained-implementation.json').read_text());legacy=json.loads((ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/lazy_reference_graph/candidate-source.json').read_text())
producer=OUT/'producers/lazy-reference';prepared=OUT/'lazy-clean-final-both';kept=Path(re.search(r'build dir kept: (.+)',(prepared/'stderr.txt').read_text())[1].strip())
native=WORK/'qualified-native.exe';assert not native.exists();shutil.copyfile(kept/'omega-program.exe',native)
assert hashlib.sha256(native.read_bytes()).hexdigest()=='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
producer_inputs=[p for p in ['omega-rust','source','tools','Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo'] if subprocess.run(['git','cat-file','-e','HEAD:'+p],capture_output=True).returncode==0]
changed=subprocess.check_output(['git','diff','--name-only',legacy['semantic_base'],'HEAD','--',*producer_inputs],text=True).splitlines();assert changed==[legacy['file']]
expected=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/lazy_reference_graph/raw/lazy-original-cold-0/runtime/stdout.txt'
original=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/lazy_reference_graph'
files=[producer/'omega.exe',producer/'omega.lock',native,prepared/'result.json',prepared/'terminal.stdout',expected,original/'candidate-source.json',original/'candidate.patch',original/'build.log.gz']
receipt=dict(source_equivalent_checkpoint=head,original_build_base=legacy['base'],semantic_base=legacy['semantic_base'],source_file=legacy['file'],source_sha256=legacy['source_sha256'],
    original_patch_sha256=legacy['patch_sha256'],producer_inputs=producer_inputs,compiler=str(producer/'omega.exe'),native=str(native),preparation_result=str(prepared/'result.json'),
    terminal_stdout=str(prepared/'terminal.stdout'),expected_stdout=str(expected),
    provenance='The preserved compiler was built on the original base with the exact captured patch. Complete producer Git inputs equal that source snapshot; this clean checkpoint names those same source bytes, not a new compiler build.',
    artifacts=[dict(path=str(p),sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in files])
receipt_path=WORK/'producer-source-equivalence.json';receipt_path.write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8',newline='\n')
tracked=[expected,original/'candidate-source.json',original/'candidate.patch',original/'build.log.gz']
artifacts=[str(p) for p in files if p not in tracked]+[str(receipt_path)]
inputs=producer_inputs+['samples/cli/games/dungeon_crawler_cli','wiki/spec',(PREFIX/'qualify-native.py').as_posix()]+[p.relative_to(ROOT).as_posix() for p in tracked]
config=dict(context=dict(host=platform.platform(),target='windows_x86_64',purpose='native replay on the clean source-equivalent checkpoint; Terminal agreement is the preserved actual preparation observation',original_build_base=legacy['base'],source_equivalent_producer_revision=head),
    checks=[dict(id='dungeon-native-replay',kind='native',command=[sys.executable,str(PREFIX/'qualify-native.py'),str(receipt_path)],inputs=inputs,artifacts=artifacts,
        producer=dict(revision=head,inputs=producer_inputs,executables=[str(producer/'omega.exe')]),closure_reason='Complete compiler, library, toolchain, validator, sample and oracle closure, plus original build/patch evidence and immutable compiler/native/preparation artifacts. Exact source equality supports producer reuse; no semantic-scope narrowing.',final=False,expected_exit=0,timeout_seconds=120)])
(WORK/'qualification-config.json').write_text(json.dumps(config,indent=2)+'\n',encoding='utf-8',newline='\n')
print('Prepared pinned native replay with original build provenance and complete exact-source-equivalence closure:',head)
