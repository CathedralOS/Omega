from pathlib import Path
import subprocess,shutil,json
f=Path('build/import-index')
subprocess.run(['python3',str(f/'build_measure.py'),'candidate'],check=True)
shutil.copy2(f/'harness/target/debug/all_threads',f/'harness-all-candidate')
for binary,name,extra in [('harness-candidate','candidate-unique',[]),('harness-candidate','candidate-shared',['--shared']),('harness-all-candidate','candidate-all',[])]:
 with (f/(name+'.tsv')).open('w') as out,(f/(name+'.stderr')).open('w') as err:subprocess.run([str(f/binary),str(f/'inputs.txt'),*extra],stdout=out,stderr=err,check=True)
 print(name,flush=True)
summary={}
for mode in ['unique','shared','all']:
 rows={leg:[line.split('\t') for line in (f/(leg+'-'+mode+'.tsv')).read_text().splitlines()] for leg in ['baseline','candidate']}
 assert len(rows['baseline'])==len(rows['candidate'])==289
 assert [r[3:] for r in rows['baseline']]==[r[3:] for r in rows['candidate']],mode
 summary[mode]={leg:dict(calls=sum(int(r[0]) for r in rr),bytes=sum(int(r[1]) for r in rr),length=[dict(calls=int(r[0]),bytes=int(r[1])) for r in rr if '/length/local/' in r[-1]][0]) for leg,rr in rows.items()}
(f/'allocation-comparison.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2),flush=True)
subprocess.run(['python3',str(f/'measure_cli.py')],check=True)
subprocess.run(['python3',str(f/'checks.py')],check=True)
