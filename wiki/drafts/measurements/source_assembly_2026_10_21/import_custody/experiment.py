from pathlib import Path
import subprocess,shutil,json,hashlib
f=Path('build/import-custody')
def run(command,log):
 with (f/log).open('w') as out: subprocess.run(command,stdout=out,stderr=subprocess.STDOUT,check=True)
def measure(name):
 (f/(name+'.patch')).write_bytes(subprocess.check_output(['git','diff','HEAD','--','omega-rust/']))
 for mode in ['unique','shared']:
  with (f/(name+'-'+mode+'.tsv')).open('w') as out,(f/(name+'-'+mode+'.stderr')).open('w') as err:
   subprocess.run([str(f/('harness-'+name)),str(f/'inputs.txt')]+(['--shared'] if mode=='shared' else []),stdout=out,stderr=err,check=True)
 print('measured',name,flush=True)
measure('baseline')
run(['python3',str(f/'change.py')],'change.log')
run(['mbx','build','--manifest-path',str(f/'harness/Cargo.toml')],'build-custody.log')
shutil.copy2(f/'harness/target/debug/assembly-measurement',f/'harness-custody')
measure('custody')
run(['python3',str(f/'change_queue.py')],'change-queue.log')
run(['rustfmt','--edition','2024','omega-rust/omega/packages/compilation-inputs/src/source_imports/frontier.rs','omega-rust/omega/packages/compilation-inputs/src/source_imports/import_queue.rs','omega-rust/omega/packages/compilation-inputs/src/source_assembly.rs'],'fmt.log')
run(['python3',str(f/'build_measure.py'),'candidate'],'build-candidate.log')
measure('candidate')
summary={}
for mode in ['unique','shared']:
 def rows(name):return [line.split('\t') for line in (f/(name+'-'+mode+'.tsv')).read_text().splitlines()]
 baseline=rows('baseline');assert len(baseline)==289
 summary[mode]={}
 for name in ['custody','candidate']:
  current=rows(name);assert len(current)==len(baseline)
  assert all(a[3:]==b[3:] for a,b in zip(baseline,current)),name+' output mismatch'
  changes=[(int(a[0])-int(b[0]),int(a[1])-int(b[1]),a[-1]) for a,b in zip(baseline,current)]
  summary[mode][name]=dict(calls_removed=sum(c[0] for c in changes),requested_bytes_removed=sum(c[1] for c in changes),affected=sum(c[0]!=0 or c[1]!=0 for c in changes),increases=[c for c in changes if c[0]<0 or c[1]<0],length=[c for c in changes if '/facts/length/local/' in c[2]],assembled=sum(a[3]=='assembled' for a in current),rejected=sum(a[3]=='rejected' for a in current))
(f/'allocation-summary.json').write_text(json.dumps(summary,indent=2)+'\n'); print(json.dumps(summary,indent=2),flush=True)
run(['python3',str(f/'measure_cli.py')],'timings.log')
