import json,platform,subprocess
from pathlib import Path
out=Path('build/placed-result-selection-production').resolve()
base=Path('build/placed-result-selection').resolve();routed=Path('build/placed-result-selection-integrated').resolve()
compiler=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
checks=json.loads((routed/'observations.json').read_text())['observations']
consumers=sorted(set(str(Path(r['subject']).parent) for r in checks if r['mode']=='full' and not r['subject'].startswith('build/')))
artifacts=[out/name for name in ['omega-candidate','candidate-producer.json','candidate-build.jsonl','candidate-build.log','tests.log','tests-current.log','tests-current.json','clippy.log','validate.py','actual_validate.py','configure.py','runtime.json']]
artifacts += [routed/'full/runtime.json',routed/'observations.json']
artifacts += list((base/'witnesses').rglob('*.omg'))
artifacts += [out/('native-'+r['name']) for r in json.loads((out/'runtime.json').read_text())['observations'] if 'native' in r]
for directory in consumers+['source/library',str(base/'witnesses')]:artifacts.extend(Path(directory).rglob('omega.lock'))
config=dict(context=dict(host=platform.platform(),target='macos_arm64',toolchain=subprocess.check_output(['rustc','--version'],text=True).strip(),scope='Call-result discovery projection; exact demands, selected declarations, native images and refusals; no timing claim'),work_classification=dict(kind='non_semantic',reason='Retain the same call-result placement demand using resolver-owned lexical inputs and existing final routing. No language rule, admission authority or emitted program behavior changes.'),checks=[dict(id='placed-call-result-projection',kind='native',command=['python3',str(out/'actual_validate.py')],inputs=compiler+consumers+['tools/compiler_profile.py'],artifacts=sorted(set(str(p.resolve()) for p in artifacts)),closure_reason='Complete compiler/library/config closure, all tracked subjects plus exact frozen exploratory source artifacts, reviewed replay script, baseline outcomes, frozen production executable and native products. Unit-test and Clippy logs retain their actual observed source context.',producer=dict(revision=(out/'base.txt').read_text().strip(),inputs=compiler,executables=[str(out/'omega-candidate')]),final=False,expected_exit=0,timeout_seconds=900)])
(out/'qualification-config.json').write_text(json.dumps(config,indent=2)+'\n')
