import hashlib,json,os,platform,re,statistics,subprocess
from pathlib import Path
root=Path.cwd()
records=[]
binaries={'baseline':root/'build/source-assembly/omega-baseline','profile':root/'target/debug/omega'}
profiles={name:json.loads(subprocess.check_output([str(path),'--build-profile'],text=True)) for name,path in binaries.items()}
for profile in profiles.values():
    assert profile['opt_level']!='0'
for round_number in range(20):
    for name in (['baseline','profile'] if round_number%2==0 else ['profile','baseline']):
        command=[str(binaries[name]),'--check','--timings',str(root/'samples/cli/facts/length/local/main.omg')]
        result=subprocess.run(command,capture_output=True,text=True)
        assert result.returncode==0,(command,result.stdout,result.stderr)
        records.append(dict(round=round_number,producer=name,exit=result.returncode,stdout=result.stdout,stderr=result.stderr,load_average=os.getloadavg()))
result=dict(base=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),host=platform.platform(),machine=platform.machine(),cpu_count=os.cpu_count(),profiles=profiles,binaries={name:dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for name,path in binaries.items()},command='omega --check --timings samples/cli/facts/length/local/main.omg',records=records)
Path('build/source-assembly/observations.json').write_text(json.dumps(result,indent=2)+'\n')
for name in binaries:
    rows=[row for row in records if row['producer']==name]
    for label in ['prepare: project','Step: assemble','Stage 05:','total elapsed']:
        values=[float(line.split()[0]) for row in rows for line in row['stderr'].splitlines() if label in line]
        print(name,label,'min/median/max',min(values),statistics.median(values),max(values),flush=True)
for label in ['validation','storage clone','entry imports','prelude','bindings']:
    values=[float(m.group(1))/1000 for row in records if row['producer']=='profile' for m in re.finditer(r'ASSEMBLY '+label+r': (\d+) us',row['stderr'])]
    print(label,'min/median/max',min(values),statistics.median(values),max(values))
