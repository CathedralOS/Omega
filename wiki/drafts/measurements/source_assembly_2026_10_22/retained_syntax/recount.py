from pathlib import Path
import subprocess,shutil,json,os
f=Path('build/retained-syntax-ownership');summary={}
os.environ['OMEGA_LIBRARY_ROOT']=str(Path('source/library').resolve())
for mode,extra in [('unique',[]),('shared',['--shared']),('all',[])]:
 rows={}
 for leg,alias in [('baseline','a'),('candidate','b')]:
  source=f/('harness-'+('all-' if mode=='all' else '')+leg);dest=f/('compare-'+alias)
  shutil.copy2(source,dest)
  with (f/(leg+'-'+mode+'.tsv')).open('w') as out,(f/(leg+'-'+mode+'.stderr')).open('w') as err:subprocess.run([str(dest),str(f/'inputs.txt'),*extra],stdout=out,stderr=err,check=True)
  rows[leg]=[line.split('\t') for line in (f/(leg+'-'+mode+'.tsv')).read_text().splitlines()]
 assert len(rows['baseline'])==len(rows['candidate'])==289
 assert [r[3:] for r in rows['baseline']]==[r[3:] for r in rows['candidate']]
 summary[mode]={leg:dict(calls=sum(int(r[0]) for r in rr),bytes=sum(int(r[1]) for r in rr),length=[dict(calls=int(r[0]),bytes=int(r[1])) for r in rr if '/length/local/' in r[-1]][0]) for leg,rr in rows.items()}
(f/'allocation-comparison.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
