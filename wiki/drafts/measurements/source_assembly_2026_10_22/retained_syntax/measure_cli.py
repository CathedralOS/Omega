import hashlib,json,os,statistics,subprocess,time,tempfile,types
from pathlib import Path
folder=Path('build/retained-syntax-ownership')
os.environ['OMEGA_LIBRARY_ROOT']=str(Path('source/library').resolve())
executables={name:(folder/f'omega-{name}').resolve() for name in ('baseline','candidate')}
profiles={name:json.loads(subprocess.check_output([str(path),'--build-profile'],text=True)) for name,path in executables.items()}
assert all(profile['opt_level']=='1' for profile in profiles.values())
def run_observed(command):
 start=time.monotonic_ns()
 with tempfile.TemporaryFile() as output,tempfile.TemporaryFile() as errors:
  process=subprocess.Popen(command,stdout=output,stderr=errors)
  _,status,usage=os.wait4(process.pid,0)
  process.returncode=os.waitstatus_to_exitcode(status)
  output.seek(0);errors.seek(0)
  return types.SimpleNamespace(returncode=process.returncode,stdout=output.read().decode(),stderr=errors.read().decode(),resources=dict(wall_ns=time.monotonic_ns()-start,user_seconds=usage.ru_utime,system_seconds=usage.ru_stime,peak_rss_bytes=usage.ru_maxrss,minor_faults=usage.ru_minflt,major_faults=usage.ru_majflt))
records=[]; best={name:float('inf') for name in executables}; last_improvement={name:time.monotonic() for name in executables}; started=time.monotonic();round_number=0
while round_number<10 or any(time.monotonic()-value<5 for value in last_improvement.values()):
 if time.monotonic()-started>120:break
 for name in (['baseline','candidate'] if round_number%2==0 else ['candidate','baseline']):
  result=run_observed([str(executables[name]),'--check','--timings',str(Path('samples/cli/facts/length/local/main.omg').resolve())])
  assert result.returncode==0,(name,result.stdout,result.stderr)
  timings={line.split('ms  ',1)[1]:float(line.split()[0]) for line in result.stderr.splitlines() if 'ms  ' in line}
  assembly=timings['Step: assemble: PreparedSources -> AssembledSyntax']
  if assembly<best[name]:best[name]=assembly;last_improvement[name]=time.monotonic()
  records.append(dict(round=round_number,producer=name,exit=result.returncode,stdout=result.stdout,stderr=result.stderr,timings_ms=timings,resources=result.resources,load_average=os.getloadavg()))
 round_number+=1
 if round_number%5==0:print('paired rounds',round_number,'assembly minima',best,flush=True)
summary={}
for name in executables:
 summary[name]={}
 rows=[record for record in records if record['producer']==name]
 for label in ['Step: assemble: PreparedSources -> AssembledSyntax','Stage 02: TokenStreams -> SyntaxTrees','total elapsed']:
  values=[row['timings_ms'][label] for row in rows]
  summary[name][label]=dict(min=min(values),median=statistics.median(values),average=statistics.mean(values),max=max(values),trials=len(values))
 for label in rows[0]['resources']:
  values=[row['resources'][label] for row in rows]
  summary[name][label]=dict(min=min(values),median=statistics.median(values),average=statistics.mean(values),max=max(values))
 print(name,summary[name],flush=True)
result=dict(library_root=os.environ['OMEGA_LIBRARY_ROOT'],base=(folder/'base.txt').read_text().strip(),binaries={name:dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest(),profile=profiles[name]) for name,path in executables.items()},quiet_seconds=5,quiet_reached=all(time.monotonic()-value>=5 for value in last_improvement.values()),summary=summary,records=records)
(folder/'cli-observations.json').write_text(json.dumps(result,indent=2)+'\n')
