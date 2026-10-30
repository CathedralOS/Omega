import os,subprocess,sys,shutil
from pathlib import Path
root=Path.cwd();out=root/'build/source-resolution-remaining'
os.environ['MBX_CACHE_DIR']=str(root/'build/conjunction-flow/recovery-cache');os.environ['MBX_LEARNED_INCREMENTAL']='0'
patch=subprocess.check_output(['git','diff','HEAD','--','omega-rust']);(out/'candidate.patch').write_bytes(patch)
def run(name,args):
 print(name,flush=True)
 with (out/(name+'.log')).open('w',encoding='utf-8') as log:subprocess.run(args,stdout=log,stderr=subprocess.STDOUT,check=True)
run('custody-tests',['mbx','nextest','run','-p','package-source','-E','test(custody::tests)','--target-dir','build/pass-producer'])
run('candidate-build',['mbx','build','-p','omega','--locked','--target-dir','build/pass-producer'])
(out/'candidate').mkdir(exist_ok=True);shutil.copyfile(root/'build/pass-producer/debug/omega.exe',out/'candidate/omega.exe')
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==patch
quality=(root/'build/source-resolution/quality.py').read_text()
quality=quality.replace("out=root/'build/source-resolution/quality'","out=root/'build/source-resolution-remaining/quality'")
old="producers={'A':root/'build/flow-evaluation/candidate/omega.exe','B':root/'build/source-resolution/candidate/omega.exe'}"
assert old in quality
quality=quality.replace(old,"producers={'A':root/'build/source-resolution/candidate/omega.exe','B':root/'build/source-resolution-remaining/candidate/omega.exe'}")
# Both variants already have root selection; preserve the same quality assertions.
(out/'quality.py').write_text(quality)
run('quality',[sys.executable,str(out/'quality.py')])
comparison=(root/'wiki/drafts/measurements/checking_simplification_2026_10_28/compare.py').read_text()
comparison=comparison.replace('import json','import json\nimport tempfile\nimport uuid',1)
comparison=comparison.replace('REPOSITORY = Path(__file__).resolve().parents[4]','REPOSITORY = Path.cwd()')
comparison=comparison.replace('cache = directory / "cache"','cache = Path(tempfile.gettempdir()) / ("omega-ancestry-" + uuid.uuid4().hex)')
comparison=comparison.replace('exit=code, wall_ms=wall, peak_job_memory_bytes=memory, stages=stages)','exit=code, wall_ms=wall, peak_job_memory_bytes=memory, stages=stages,\n                       source_cache_path=str(cache), source_cache_depth=len(cache.parts))')
assert 'source_cache_depth' in comparison and 'omega-ancestry-' in comparison
(out/'compare.py').write_text(comparison)
run('comparison',[sys.executable,str(out/'compare.py'),'build/source-resolution/candidate/omega.exe',str(out/'candidate/omega.exe'),str(out/'comparison')])
print('Candidate checks and comparison complete',flush=True)
