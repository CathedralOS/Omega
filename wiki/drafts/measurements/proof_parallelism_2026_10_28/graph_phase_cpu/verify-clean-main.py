from pathlib import Path
import importlib.util,json
ROOT=Path.cwd();LEDGER=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28'
spec=importlib.util.spec_from_file_location('pure',LEDGER/'lazy_reference_graph/lazy_reference_run.py');pure=importlib.util.module_from_spec(spec);spec.loader.exec_module(pure)
r=pure.run('candidate','lazy-clean-final-both',comparison=True)
case=ROOT/'build/proof-profile/lazy-clean-final-both'
assert (case/'stdout.txt').read_bytes()==(LEDGER/'lazy_reference_graph/raw/lazy-original-cold-0/runtime/stdout.txt').read_bytes()
metadata=ROOT/'build/proof-profile/graph-phase-cpu/retained-implementation.json';record=json.loads(metadata.read_text())
assert r['compiler_sha256']==record['producer']['compiler_sha256'] and r['accepted_lock_sha256']==record['producer']['accepted_lock_sha256']
record['fresh_dungeon_both_observation']='clean-main'
metadata.write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n')
