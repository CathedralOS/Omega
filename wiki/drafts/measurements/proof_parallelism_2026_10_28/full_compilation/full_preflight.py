from pathlib import Path
import hashlib,importlib.util,json,os,re
r=Path.cwd();w=r/'build/proof-profile/full-compilation';w.mkdir(exist_ok=True)
spec=importlib.util.spec_from_file_location('cache',r/'wiki/drafts/measurements/proof_parallelism_2026_10_28/cache_diagnostics/cache_diagnostics_measure.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
for k in list(os.environ):
 if k.startswith('OMEGA_') and any(x in k for x in ['PROBE','PROFILE','COUNTS','MEASUREMENT','EDGE_CACHE']):os.environ.pop(k,None)
runner=m.parent.runner
for label,on in [('full-original-a',False),('full-detail-a',True),('full-original-b',False)]:
 runner.verify_inputs();runner.measure('baseline',label,'off',0,phase_profile=on,cold_cache=True);runner.verify_inputs()
 d=runner.OUT/label;x=json.loads((d/'result.json').read_text());assert x['image_sha256']=='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
 print('COMPLETED',label,x['wall_seconds'],x['cpu_seconds'],flush=True)
