from pathlib import Path
import subprocess,os,json
out=Path('build/import-custody').resolve();root=Path.cwd()
rows=[]
for leg in ['both','check']:
 env=dict(os.environ,OMEGA_CORPUS_ROOT=str(root),OMEGA_LIBRARY_ROOT=str(root/'source/library'),OMEGA_CORPUS_FIXTURE_FILTER=(out/'corpus-filter.txt').read_text().strip(),OMEGA_CORPUS_RECORDS=str(out/f'candidate-{leg}-records.txt'))
 if leg=='both':env.update(OMEGA_CORPUS_NATIVE='1',OMEGA_CORPUS_INTERPRET='1')
 command=[str(out/'corpus-runner-candidate')]
 result=subprocess.run(command,env=env,capture_output=True,text=True)
 (out/f'candidate-{leg}-stdout.txt').write_text(result.stdout);(out/f'candidate-{leg}-stderr.txt').write_text(result.stderr)
 rows.append(dict(leg=leg,command=command,exit=result.returncode,environment={key:value for key,value in env.items() if key.startswith('OMEGA_')}));print(leg,result.returncode,flush=True)
(out/'corpus-capture.json').write_text(json.dumps(rows,indent=2)+'\n')
