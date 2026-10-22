from pathlib import Path
import subprocess,shutil,json
f=Path('build/import-custody');p=Path('omega-rust/omega/packages/compilation-inputs/src/source_imports/import_queue.rs');original=p.read_bytes()
try:
 text=original.decode().replace('            self.pending.push_back(entry.key().clone());','            eprintln!("QUEUE_CAPACITY\\t{}\\t{}", entry.key().0.as_os_str().len(), entry.key().0.capacity());\n            self.pending.push_back(entry.key().clone());')
 p.write_text(text)
 with (f/'build-capacity.log').open('w') as out:subprocess.run(['mbx','build','--manifest-path',str(f/'harness/Cargo.toml')],stdout=out,stderr=subprocess.STDOUT,check=True)
 shutil.copy2(f/'harness/target/debug/assembly-measurement',f/'harness-capacity')
 with (f/'capacity.stdout').open('w') as out,(f/'capacity.stderr').open('w') as err:subprocess.run([str(f/'harness-capacity'),str(f/'inputs.txt')],stdout=out,stderr=err,check=True)
finally:p.write_bytes(original)
rows=[];capacities=[]
for line in (f/'capacity.stderr').read_text().splitlines():
 parts=line.split('\t')
 if parts[0]=='QUEUE_CAPACITY':capacities.append((int(parts[1]),int(parts[2])))
 if parts[0]=='ROOTS':
  rows.append(dict(path=parts[-1],admissions=len(capacities),spare_bytes=sum(cap-length for length,cap in capacities),max_spare_bytes=max([cap-length for length,cap in capacities],default=0)));capacities=[]
assert len(rows)==289
summary=dict(sum_admissions=sum(r['admissions'] for r in rows),sum_spare_bytes=sum(r['spare_bytes'] for r in rows),max_spare_bytes_per_input=max(r['spare_bytes'] for r in rows),max_spare_bytes_per_key=max(r['max_spare_bytes'] for r in rows),length=[r for r in rows if '/length/local/' in r['path']],rows=rows)
(f/'capacity.json').write_text(json.dumps(summary,indent=2)+'\n');print({k:v for k,v in summary.items() if k!='rows'})
