"""Freeze actual fixed-main images before comparing the integrated candidate."""
import importlib.util,json,os,re,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]; BASE=ROOT/'build/windows-custody-landing'; DATA=BASE/'baseline-seed'; DATA.mkdir()
spec=importlib.util.spec_from_file_location('first',ROOT/'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first=importlib.util.module_from_spec(spec); spec.loader.exec_module(first); first.EVIDENCE=DATA/'logs';first.EVIDENCE.mkdir();first.self_test()
producer=json.loads((BASE/'baseline/producer.json').read_text())['producer'];assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
sources={str(p.relative_to(ROOT)):first.digest(p) for name in first.SUBJECTS.values() for p in (ROOT/name).parent.rglob('*.omg')}
cache=Path(os.environ['LOCALAPPDATA'])/('OmegaPerf-custody-landing-seed-'+uuid.uuid4().hex[:12]);cache.mkdir()
result={'producer':producer,'targets':['windows_x86_64'],'source_manifest':sources,'observations':[],'cache_path':str(cache),'protocol':'Actual fixed ec727c baseline images and Windows exit0; compiler/library inputs frozen separately; candidate not executed before this catalog.'}
for subject,source in first.SUBJECTS.items():
    output=DATA/subject;output.mkdir();env=dict(os.environ,LOCALAPPDATA=str(cache),OMEGA_LIBRARY_ROOT=str(ROOT/'source/library'),RAYON_NUM_THREADS='32');env.pop('OMEGA_PHASE_PROFILE',None)
    row,text=first.capture([producer['binary'],'--timings','--target','windows_x86_64','--build-dir',str(output),str(ROOT/source)],subject,env)
    artifact=first.compiled_artifact(row,text,output);native,_=first.capture([str(artifact)],subject+'-native',dict(os.environ));assert native['exit']==0
    row.update(subject=subject,native=native,artifacts={'windows_x86_64':{'path':str(artifact),'sha256':row['artifact_sha256'],'bytes':artifact.stat().st_size}});result['observations'].append(row)
    print(subject,round(row['wall_ms'],1),'ms native0',flush=True)
assert all(first.digest(ROOT/p)==h for p,h in sources.items());assert first.digest(producer['binary'])==producer['sha256']
result['complete']=True;(BASE/'baseline.json').write_text(json.dumps(result,indent=2)+'\n');(BASE/'baseline-producer.json').write_text((BASE/'baseline/producer.json').read_text())
