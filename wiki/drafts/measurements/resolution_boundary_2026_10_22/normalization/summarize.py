"""Join observed carrier equality and native behavior with baseline clone counts."""
import hashlib
import json
from pathlib import Path
output=Path('build/resolved-normalization')
candidate=json.loads((output/'candidate-observations.json').read_text())
oracle=json.loads((output/'instrumented-observations.json').read_text())
assert candidate['metadata']['complete'] and oracle['metadata']['complete']
baseline=json.loads((output/'baseline-attribution.json').read_text())
rows=[]
for old in baseline:
    actual=next(r for r in oracle['records'] if r.get('path')==old['subject'])
    observed=actual['normalizations']
    removed=sum(r['removed_clone_rows'] for r in observed)
    assert removed==old['normalization_rows']
    assert len(observed)==old['normalization_clones']
    rows.append(dict(**old,candidate_clones=old['baseline_clones']-len(observed),candidate_copied_rows=old['baseline_rows']-removed,normalized_carriers_equal=all(r['equal'] for r in observed)))
# Instrumentation must preserve the same products and refusal diagnostics too.
import sys
sys.path.insert(0,str(Path('tools/benchmark').resolve()))
from benchmark import TIMING_LINE
def clean(text):
    return '\n'.join(line for line in text.splitlines() if not TIMING_LINE.match(line) and not line.startswith('NORMALIZATION\t'))
for actual in oracle['records']:
    expected=next(r for r in candidate['records'] if r.get('path')==actual['path'] and r['producer']=='candidate')
    if actual['positive']: assert actual['native_sha256']==expected['native_sha256']
    else: assert clean(actual['output'])==clean(expected['output'])
normalizations=[n for r in oracle['records'] for n in r.get('normalizations',[])]
result=dict(decision='retained',legacy_normalizer_sha256=hashlib.sha256((output/'legacy-normalizer.rs').read_bytes()).hexdigest(),work_reduction=rows,full_carrier_comparisons=len(normalizations),all_carriers_equal=all(n['equal'] for n in normalizations),
    compile_observations=len([r for r in candidate['records']+oracle['records'] if not r.get('both')]),
    standalone_native_executions=len([r for r in candidate['records']+oracle['records'] if r.get('native_exit')==0]),
    candidate_dual_engine_observations=len([r for r in candidate['records'] if r.get('both')]),
    invalid_controls=[r['subject'] for r in candidate['records'] if r.get('producer')=='candidate' and not r.get('both') and not r['positive']],
    timing_conclusion='No speedup established: single alternating A/B observation per workload on a contended shared host.',
    producer_receipts={name:json.loads((output/(name+'-producer.json')).read_text()) for name in ('candidate','instrumented')})
(output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='producer_receipts'},indent=2))
