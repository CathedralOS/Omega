import json,pathlib,statistics,hashlib,subprocess
ROOT=pathlib.Path(__file__).resolve().parent
MASK=(1<<64)-1
SEED=0x6f6d65676162656e

def summarize(pairs):
 before=[float(p['baseline_ns']) for p in pairs];after=[float(p['candidate_ns']) for p in pairs]
 changes=[(b/a-1)*100 for a,b in zip(before,after)];differences=[b-a for a,b in zip(before,after)];random=SEED;boots=[];percentage_boots=[]
 for _ in range(4000):
  sample=[];percent=[]
  for _ in pairs:
   random^=(random<<13)&MASK;random^=random>>7;random^=(random<<17)&MASK;position=random%len(pairs);sample.append(differences[position]);percent.append(changes[position])
  boots.append(statistics.median(sample));percentage_boots.append(statistics.median(percent))
 boots.sort();percentage_boots.sort()
 return {'pairs':len(pairs),'baseline_median_ms':statistics.median(before)/1e6,'candidate_median_ms':statistics.median(after)/1e6,'paired_difference_median_ms':statistics.median(differences)/1e6,'paired_percent_median':statistics.median(changes),'paired_percent_change_bootstrap_95':[percentage_boots[100],percentage_boots[3899]],'median_difference_bootstrap_95_ms':[boots[100]/1e6,boots[3899]/1e6]}

def read_run(directory):
 events=[json.loads(line) for line in (directory/'journal.jsonl').read_text().splitlines()];report=json.loads((directory/'report.json').read_text());plan=dict(events[0]['plan']);plan.pop('name');plan.pop('pairs');plan['_host']=events[0]['host'];plan['_inherited_settings']=events[0]['inherited_settings']
 identities=next(e['sha256'] for e in events if e['event']=='identities');plan_key=next(key for key,value in identities.items() if value==events[0]['plan_sha256']);identities={key:value for key,value in identities.items() if key!=plan_key}
 rounds={}
 for event in events:
  if event['event']=='sample' and not event['sample']['warmup']:
   s=event['sample'];rounds.setdefault(s['round'],{})[s['role']]=s['primary_value']
 pairs=[{'batch':directory.parent.name,'round':number,'baseline_ns':s['baseline'],'candidate_ns':s['candidate']} for number,s in sorted(rounds.items())]
 observed=summarize(pairs);expected=report['comparison']['paired_percent_change_bootstrap_95'];assert observed['paired_percent_change_bootstrap_95']==expected,(observed,expected)
 return plan,identities,pairs,observed

results={}
for index,name in enumerate(['length','square']):
 runs=[read_run(ROOT/f'{kind}/case-{index:02}') for kind in ['comparison-suite','confirmation']]
 assert runs[0][0]==runs[1][0], 'workload/settings changed'
 assert runs[0][1]==runs[1][1], 'input/executable bytes changed'
 pairs=runs[0][2]+runs[1][2];results[name]={'initial':runs[0][3],'confirmation':runs[1][3],'all_observations':summarize(pairs),'raw_pairs':pairs}
 print(name,json.dumps(results[name]['all_observations']))
(ROOT/'pooled-result.json').write_text(json.dumps({'kind':'exploratory pooled paired analysis; all complete runs retained','assumptions':'Within-pair comparisons; percentile bootstrap does not model temporal correlation or contention. Initial and confirmation runs remain separately reported.','results':results},indent=2),encoding='utf-8')
