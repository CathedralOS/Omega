import json,sys
from pathlib import Path
sys.path.insert(0,str(Path('wiki/drafts/measurements/pass_ablation_2026_10_28').resolve()))
import run as experiment
omega=Path('build/pass-producer/debug/omega.exe').resolve()
output=Path('build/pass-ablation/performance-quality').resolve()
output.mkdir()
rows=[]
for name in experiment.SUBJECTS:
 for variant in ('A' if name=='control' else 'AB'):
  original=Path('build/pass-ablation/trial3/inputs')/name
  directory=output/(name+'-'+variant)
  directory.mkdir()
  source=directory/'main.omg'
  source.write_bytes((original/'main.omg').read_bytes())
  build=(original/'build.omg').read_text(encoding='utf-8')
  if variant=='B': build=build.replace(experiment.ENABLE,'')
  (directory/'build.omg').write_text(build,encoding='utf-8',newline='\n')
  arguments=[str(omega),'run','--both',str(source)]
  code,wall,transcript=experiment.capture(arguments,directory)
  row=dict(subject=name,variant=variant,command=arguments,exit=code,output=transcript,main_sha256=experiment.benchmark.sha256_file(source),build_sha256=experiment.benchmark.sha256_file(directory/'build.omg'))
  rows.append(row)
  (output/'results.json').write_text(json.dumps(rows,indent=2),encoding='utf-8')
  print(name,variant,code,transcript.strip(),flush=True)
  assert code==0 and 'native exit 0' in transcript and 'interp exit: 0' in transcript
