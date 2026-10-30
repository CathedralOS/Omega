from pathlib import Path
from collections import Counter
import json,sys
folder=Path(sys.argv[1]) if len(sys.argv)>1 else Path('build/proof-profile/payload-core-detail')
if not (folder/'result.json').exists():raise SystemExit('still running')
sums=Counter();counts=Counter();active=set()
for line in (folder/'stderr.txt').read_text().splitlines():
 fields=line.split('\t')
 if fields[0]=='SOURCE_QUERY_BEGIN':
  assert fields[1] not in active;active.add(fields[1]);counts['source_windows']+=1
 elif fields[0]=='SOURCE_QUERY_END':active.remove(fields[1])
 elif fields[0]=='REPLAY_DETAIL' and fields[1] in active:
  counts['query_results']+=1
  for metric in fields[2:]:
   key,value=metric.split('=')
   if key in ['steps','edges']:counts[key]+=int(value)
   else:sums[key]+=int(value)
 elif fields[0] in ['ENTRY_INIT','ENTRY_PREVIOUS_DROP'] and fields[1] in active:
  sums[fields[0]]+=int(fields[2]);counts[fields[0]]+=1
assert not active
r=json.loads((folder/'result.json').read_text());rows=[x for x in r['reconciliation'] if x['operation']=='memberships'];total=sum(x['metrics'].get('source_contract_queries_ns',0) for x in rows)
assert counts['source_windows']>0 and counts['query_results']>0
summary=dict(counts=counts,source_contract_ns=total,parts_ns=sums,outside_disjoint_intervals_ns=total-sum(sums[key] for key in ['ENTRY_PREVIOUS_DROP','ENTRY_INIT','setup','clone','observe','retire','transfer','canonical','edges_inclusive']))
# edges_inclusive overlaps edge closure, publication and joins; never sum it with them.
assert 0 <= summary['outside_disjoint_intervals_ns'] <= total
(folder/'detail.json').write_text(json.dumps(summary,indent=2))
print(json.dumps(summary,indent=2))
