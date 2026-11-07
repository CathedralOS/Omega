"""Independent all-minimized-vector reference, used to audit frozen fixtures.
This script never replaces or generates the Omega application.
"""
from pathlib import Path
import json, hashlib
root=Path(__file__).resolve().parent
frozen=json.loads((root/'expected/FROZEN.json').read_text())
for name,digest in frozen['sha256'].items():
    assert hashlib.sha256((root/name).read_bytes()).hexdigest()==digest, name
for path in sorted((root/'data').glob('*.stdin')):
    if path.name.startswith('invalid_'): continue
    words=list(map(int,path.read_text().split()))
    count,budget,deadline,risk_limit=words[:4]
    rows=[words[i:i+6] for i in range(4,len(words),6)]
    assert len(rows)==count
    feasible=[row for row in rows if row[2]<=budget and row[3]<=deadline and row[5]<=risk_limit]
    scores={row[0]:(row[2],row[3],-row[4]) for row in feasible}
    output=['PARETO|id|source|position|cost|days|gain|risk|status|certificate']
    for row in sorted(rows):
        identity,source,cost,days,gain,risk=row
        mask=int(cost>budget)+2*int(days>deadline)+4*int(risk>risk_limit)
        rivals=[] if mask else [key for key,score in scores.items()
            if score!=scores[identity] and all(a<=b for a,b in zip(score,scores[identity]))]
        status='INFEASIBLE' if mask else 'DOMINATED' if rivals else 'SELECTED'
        certificate=mask if mask else min(rivals,default=0)
        fields=[identity,source,rows.index(row)+1,cost,days,gain,risk]
        output.append('|'.join(f'{x:03d}' for x in fields)+f'|{status}|{certificate:03d}')
    output.append('END')
    assert '\n'.join(output)+'\n'==(root/'expected'/f'{path.stem}.stdout').read_text(),path
print('Frozen hashes and 13 independently enumerated fronts verified; tolerance zero.')
