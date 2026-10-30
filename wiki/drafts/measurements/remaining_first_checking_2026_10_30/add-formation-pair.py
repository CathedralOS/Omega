import re
from pathlib import Path
out=Path('build/remaining-first-checking')
path=out/'quality.py'
quality=path.read_text()
pair=['pass/calls/specification_result_without_guarantees_requires_formation','fail/calls/specification_result_without_guarantees_requires_formation']
baseline=re.sub(r'^fixtures=.*$',f'fixtures={pair!r}',quality,flags=re.M)
baseline=re.sub(r'^fixtures \+=.*\n','',baseline,flags=re.M)
baseline=baseline.replace("producer=base/'candidate/omega.exe'", "producer=root/'build/first-checking/candidate/omega.exe'").replace("'candidate-'+source.name", "'baseline-pair-'+source.name").replace("'quality.json'", "'formation-baseline.json'")
(out/'formation-baseline.py').write_text(baseline,encoding='utf-8',newline='\n')
quality=quality.replace('for fixture in fixtures:', f'fixtures += {pair!r}\nfor fixture in fixtures:')
quality+="\nsubprocess.run([sys.executable,str(base/'formation-baseline.py')],check=True)\n"
path.write_text(quality,encoding='utf-8',newline='\n')
