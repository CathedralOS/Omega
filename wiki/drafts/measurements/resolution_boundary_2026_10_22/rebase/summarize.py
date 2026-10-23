"""Join exact rebase comparisons to the preceding normalized-copy baseline."""
import json,sys,re,hashlib
from pathlib import Path
sys.path.insert(0,str(Path('tools/benchmark').resolve()))
from benchmark import TIMING_LINE
out=Path('build/resolved-rebase')
previous=json.loads(Path('wiki/drafts/measurements/resolution_boundary_2026_10_22/normalization/summary.json').read_text())
a=json.loads((out/'candidate-observations.json').read_text())
b=json.loads((out/'instrumented-observations.json').read_text())
assert a['metadata']['complete'] and b['metadata']['complete']
def clean(text):return '\n'.join(line for line in text.splitlines() if not TIMING_LINE.match(line) and not line.startswith('REBASE\t'))
for actual in b['records']:
    expected=next(r for r in a['records'] if r.get('path')==actual['path'] and r['producer']=='candidate')
    if actual['positive']:assert actual['native_sha256']==expected['native_sha256']
    else:assert clean(actual['output'])==clean(expected['output'])
rows=[]
for old in previous['work_reduction']:
    actual=next(r for r in b['records'] if r['path']==old['subject'])
    rebases=actual['rebases'];assert all(r['outcome']=='accepted' for r in rebases)
    rows.append(dict(subject=old['subject'],baseline_copies=old['candidate_clones'],candidate_copies=old['candidate_clones']-len(rebases),baseline_rows=old['candidate_copied_rows'],candidate_rows=old['candidate_copied_rows']-sum(r['removed_clone_rows'] for r in rebases),removed_rows=sum(r['removed_clone_rows'] for r in rebases)))
checks=[r for row in b['records'] for r in row.get('rebases',[])]
unit_log=(out/'oracle-tests.log').read_text()
unit_oracles=re.findall(r'REBASE\s*(\d+)\s*(accepted|rejected)\s*equal',unit_log)
assert unit_oracles and any(outcome=='rejected' for _,outcome in unit_oracles)
result=dict(decision='retained',work_reduction=rows,full_result_comparisons=len(checks),unit_result_comparisons=len(unit_oracles),unit_rejection_comparisons=sum(outcome=='rejected' for _,outcome in unit_oracles),all_equal=all(r['equal'] for r in checks),compile_observations=sum(not r.get('both') for r in a['records']+b['records']),standalone_native_executions=sum(r.get('native_exit')==0 for r in a['records']+b['records']),dual_engine_programs=sum(r.get('both',False) for r in a['records']),
            legacy_sources=json.loads((out/'legacy-sources.json').read_text()),timing_conclusion='No speedup established: single alternating A/B observation per workload under shared-host contention.')
(out/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
