import json,os,subprocess,sys
from pathlib import Path
sys.path.insert(0,str(Path('tools').resolve()))
import build_slot
build_slot.POLL_SECONDS=.05
out=Path('build/deferred-completion-storage')
os.environ['CARGO_TARGET_DIR']=str(Path('build/gate-cli').resolve());os.environ['CARGO_BUILD_JOBS']='2'
results=[]
commands=[('tests',['mbx','nextest','run','-p','symbol-resolved-trees','-p','syntax-trees-to-symbol-resolved-trees']),('clippy',['mbx','clippy','-p','symbol-resolved-trees','-p','syntax-trees-to-symbol-resolved-trees','--all-targets'])]
with build_slot.hold():
 for name,command in commands:
  with (out/(name+'.log')).open('w') as log:
   result=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT)
  results.append(dict(name=name,command=command,exit=result.returncode))
  (out/'source-validation.json').write_text(json.dumps(results,indent=2)+'\n')
  print(name,result.returncode,flush=True)
  assert result.returncode==0,result.returncode
