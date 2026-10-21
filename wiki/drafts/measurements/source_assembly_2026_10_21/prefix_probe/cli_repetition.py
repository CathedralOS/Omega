import hashlib,json,os,statistics,subprocess,time
from pathlib import Path
folder=Path('build/assembly-perf')
executables={name:(folder/f'omega-{name}').resolve() for name in ('baseline','candidate')}
profiles={name:json.loads(subprocess.check_output([str(path),'--build-profile'],text=True)) for name,path in executables.items()}
assert all(profile['opt_level']=='1' for profile in profiles.values())
records=[]; best={name:float('inf') for name in executables}; last_improvement={name:time.monotonic() for name in executables}; started=time.monotonic();round_number=0
while round_number<10 or any(time.monotonic()-value<5 for value in last_improvement.values()):
 if time.monotonic()-started>90:break
 for name in (['baseline','candidate'] if round_number%2==0 else ['candidate','baseline']):
  result=subprocess.run([str(executables[name]),'--check','--timings',str(Path('samples/cli/facts/length/local/main.omg').resolve())],capture_output=True,text=True)
  assert result.returncode==0,(name,result.stdout,result.stderr)
  timings={line.split('ms  ',1)[1]:float(line.split()[0]) for line in result.stderr.splitlines() if 'ms  ' in line}
  assembly=timings['Step: assemble: PreparedSources -> AssembledSyntax']
  if assembly<best[name]:best[name]=assembly;last_improvement[name]=time.monotonic()
  records.append(dict(round=round_number,producer=name,exit=result.returncode,stdout=result.stdout,stderr=result.stderr,timings_ms=timings,load_average=os.getloadavg()))
 round_number+=1
 if round_number%5==0:print('paired rounds',round_number,'assembly minima',best,flush=True)
summary={}
for name in executables:
 summary[name]={}
 rows=[record for record in records if record['producer']==name]
 for label in ['Step: assemble: PreparedSources -> AssembledSyntax','Stage 02: TokenStreams -> SyntaxTrees','total elapsed']:
  values=[row['timings_ms'][label] for row in rows]
  summary[name][label]=dict(min=min(values),median=statistics.median(values),average=statistics.mean(values),max=max(values),trials=len(values))
 print(name,summary[name],flush=True)
result=dict(base='00ef417922c04aea560b7faa5ce05999eb1f0512',binaries={name:dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest(),profile=profiles[name]) for name,path in executables.items()},quiet_seconds=5,quiet_reached=all(time.monotonic()-value>=5 for value in last_improvement.values()),summary=summary,records=records)
(folder/'cli-observations.json').write_text(json.dumps(result,indent=2)+'\n')
