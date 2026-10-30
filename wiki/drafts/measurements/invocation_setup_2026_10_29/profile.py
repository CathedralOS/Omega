"""Attribute warm Windows-only setup using the unchanged qualified post-queue producer."""
import hashlib,importlib.util,json,os,re,subprocess,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]; BASE=ROOT/'build/invocation-setup'; BASE.mkdir(exist_ok=True)
DATA=BASE/'diagnostic-profile'; DATA.mkdir()
spec=importlib.util.spec_from_file_location('first',ROOT/'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first=importlib.util.module_from_spec(spec); spec.loader.exec_module(first); first.EVIDENCE=DATA/'logs'; first.EVIDENCE.mkdir(); first.self_test()
native=ROOT/'build/native-queue-landing'; diagnostic=ROOT/'build/invocation-setup/diagnostic'
producer=json.loads((diagnostic/'producer.json').read_text(encoding='utf-8'))['producer']
assert producer['source_revision']==subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
inputs=json.loads((diagnostic/'build.json').read_text(encoding='utf-8'))['source_manifest']
assert all(first.digest(ROOT/p)==h for p,h in inputs.items())
accepted=json.loads((native/'capture-2/comparison/results.json').read_text(encoding='utf-8')); assert accepted['complete']
expected={s:next(r for r in accepted['single'] if r['subject']==s and r['flavor']=='candidate')['artifacts']['windows_x86_64']['sha256'] for s in first.SUBJECTS}
cache=Path(os.environ['LOCALAPPDATA'])/('OmegaPerf-setup-diagnostic-'+uuid.uuid4().hex[:12]); cache.mkdir(); assert not list(cache.iterdir())
records={'producer':producer,'controller_revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'source_manifest':inputs,'harness_sha256':first.digest(Path(__file__)),'cache_path':str(cache),'canonical_cache_path':str(cache.resolve()),'expected_images':expected,'seed':[],'profiles':[],'protocol':'Diagnostic only: ordinary-depth private user cache, single Windows target, temporary timer-only producer, fixed post-queue source with exact archived diagnostic patch, first visits separate from warm profiles. Exact images and actual native exits; phase attribution is not an A/B speedup.'}
def retain(): (DATA/'results.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
for profile in (False,True):
    for subject,source in first.SUBJECTS.items():
        label=('profile-' if profile else 'seed-')+subject; output=DATA/label; output.mkdir()
        env=dict(os.environ,LOCALAPPDATA=str(cache),OMEGA_LIBRARY_ROOT=str(ROOT/'source/library'),RAYON_NUM_THREADS='32'); env.pop('OMEGA_PHASE_PROFILE',None)
        assert not any(k.startswith(('MIMALLOC_','MI_')) for k in env)
        if profile: env['OMEGA_PHASE_PROFILE']='1'
        row,text=first.capture([producer['binary'],'--timings','--target','windows_x86_64','--build-dir',str(output),str(ROOT/source)],label,env)
        artifact=first.compiled_artifact(row,text,output); assert row['artifact_sha256']==expected[subject]
        runtime,_=first.capture([str(artifact)],label+'-native',dict(os.environ)); assert runtime['exit']==0
        row.update(subject=subject,native=runtime)
        if profile:
            assert str(cache.resolve()).lower() in text.splitlines()[0].lower()
            assert re.search(r'[1-9]\d* n\s+source.graph.loaded',text) and re.search(r'[1-9]\d* n\s+source.graph.prepared_hits',text)
            spans={m[3]:(float(m[1]),int(m[2])) for line in text.splitlines() if (m:=re.match(r'\s*([0-9.]+) ms\s+(\d+) x\s+(.+)',line))}
            assert all(spans[s][1]==1 for s in ('build.create_session','build.resolve_sources'))
            row['spans']=spans; row['setup_ms']=sum(spans[s][0] for s in ('build.create_session','build.resolve_sources')); row['setup_fraction']=row['setup_ms']/row['wall_ms']
            print(subject,round(row['wall_ms'],1),'ms; setup',round(row['setup_ms'],1),'ms;',round(row['setup_fraction']*100,1),'%',flush=True)
        records['profiles' if profile else 'seed'].append(row); retain()
assert all(first.digest(ROOT/p)==h for p,h in inputs.items())
assert first.digest(Path(__file__))==records['harness_sha256']
records['complete']=True; retain()
