import json,subprocess,platform
from pathlib import Path
p=Path('build/resolution-integration-final').resolve()
rows=json.loads((p/'setup.json').read_text())
compiler_inputs=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
consumer_inputs=sorted(set(str(Path(row['subject']).parent) for row in rows))
inputs=compiler_inputs+consumer_inputs+['tools/benchmark','tools/compiler_profile.py','wiki/drafts/measurements/resolution_boundary_2026_10_22/rebase/candidate-observations.json']
artifacts=[p/name for name in ['omega-candidate','candidate-producer.json','candidate-build.jsonl','candidate-build.log','tests.log','validate.py','setup.py','setup.json']]
artifacts += [Path(row['native']) for row in rows if row['positive']]
for directory in consumer_inputs+['source/library']:
    artifacts.extend(Path(directory).rglob('omega.lock'))
config=dict(context=dict(host=platform.platform(),target='macos_arm64',toolchain=subprocess.check_output(['rustc','--version'],text=True).strip(),scope='integration of representation ownership optimization; fixed ten consumers; no wall-clock claim'),work_classification=dict(kind='non_semantic',reason='Move owned expression storage and preflight existing rebase checks; preserve complete carriers, errors, source semantics and native images. This integrates already validated ownership changes without adding or altering a language rule.'),checks=[dict(id='resolved-storage-integration',kind='native',command=['python3',str(p/'validate.py')],inputs=inputs,artifacts=sorted(set(str(path.resolve()) for path in artifacts)),closure_reason='Conservative complete compiler/library/config closure, ten authored consumers, prior diagnostic/image oracles, reviewed validator and frozen producer/native outputs; tests log belongs to the integrated source build.',producer=dict(revision=(p/'base.txt').read_text().strip(),inputs=compiler_inputs,executables=[str(p/'omega-candidate')]),final=False,expected_exit=0,timeout_seconds=900)])
(p/'qualification-config.json').write_text(json.dumps(config,indent=2)+'\n')
