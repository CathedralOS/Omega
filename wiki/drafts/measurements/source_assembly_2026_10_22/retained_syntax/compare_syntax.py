import json,hashlib
from pathlib import Path
p=Path('build/retained-syntax-ownership')
a=json.loads((p/'syntax-baseline.json').read_text());b=json.loads((p/'syntax-candidate.json').read_text())
assert len(a['inputs'])==len(b['inputs'])==463
assert [r['path'] for r in a['inputs']]==[r['path'] for r in b['inputs']]
bad=[(x['path'],[k for k in x['observation'] if x['observation'].get(k)!=y['observation'].get(k)]) for x,y in zip(a['inputs'],b['inputs']) if x['observation']!=y['observation']]
assert not bad,bad[:12]
summary=dict(inputs=463,complete_logical_observations_match=True,debug_normalization='Resolve source-backed name handles with their actual forest owners and remove only the new owner metadata field; retain every pre-existing node and arena field.')
for label,v in [('baseline',a),('candidate',b)]:summary[label]=dict(identifier_size=v['identifier_size'],retained_source_references=sum(r['retained_syntax_source_references'] for r in v['inputs']),**{k:sum(r['metrics'][k] for r in v['inputs']) for k in v['inputs'][0]['metrics']})
(p/'syntax-comparison.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
