from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys
ROOT=Path.cwd();OUT=ROOT/'build/proof-profile';WORK=OUT/'same-binary-graph';PRODUCER='same-binary-graph'
identity=json.loads((WORK/'source.json').read_text());assert hashlib.sha256((ROOT/identity['file']).read_bytes()).hexdigest()==identity['source_sha256']
subprocess.run([sys.executable,'build/proof-profile/freeze_core.py',PRODUCER],check=True)
dest=OUT/'producers'/PRODUCER;messages=(WORK/'corpus-build.jsonl').read_text(encoding='utf-8-sig');artifacts=[]
for line in messages.splitlines():
    try:row=json.loads(line)
    except ValueError:continue
    if row.get('reason')=='compiler-artifact' and row.get('target',{}).get('name')=='corpus_runner' and row.get('executable'):artifacts.append(Path(row['executable']))
assert len(artifacts)==1
sys.path.insert(0,str(ROOT/'tools'));from compiler_profile import record_cargo_build,require_optimized
record_cargo_build(artifacts[0],messages);require_optimized(artifacts[0]);shutil.copy2(artifacts[0],dest/'corpus_runner.exe')
profiles={name:json.loads(subprocess.check_output([str(dest/name),'--build-profile'])) for name in ['omega.exe','corpus_runner.exe']}
assert all(int(p['opt_level'])>0 for p in profiles.values())
frozen=dict(producer=PRODUCER,compiler_sha256=hashlib.sha256((dest/'omega.exe').read_bytes()).hexdigest(),accepted_lock_sha256=hashlib.sha256((dest/'omega.lock').read_bytes()).hexdigest(),
    build_profile=profiles['omega.exe'],corpus_sha256=hashlib.sha256((dest/'corpus_runner.exe').read_bytes()).hexdigest(),corpus_profile=profiles['corpus_runner.exe'])
(WORK/'producer.json').write_text(json.dumps(frozen,indent=2)+'\n',encoding='utf-8',newline='\n')
legs=[]
def observed(report):return [{k:o[k] for k in ['fixture','tier','status','expected_satisfied','facts','diagnostics']} for o in report['observations']]
for mode in ['eager','lazy']:
    env=os.environ.copy();env['OMEGA_CORPUS_JOBS']='1'
    for k in list(env):
        if k.startswith('OMEGA_') and any(x in k for x in ['PROBE','PROFILE','COUNTS','MEASUREMENT','EXPERIMENT']):env.pop(k,None)
    env['OMEGA_REFERENCE_GRAPH_EXPERIMENT_MODE']=mode
    for label,flags,count in [('positive',['--both'],4),('negative',[],3)]:
        baseline=json.loads((ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/lazy_reference_graph'/('baseline-quality-'+label+'.json')).read_text())
        target=WORK/(mode+'-'+label+'.json')
        command=[sys.executable,'tools/corpus_gate.py','--runner',str(dest/'corpus_runner.exe'),*flags,'--filter',baseline['filter'],'--json',str(target)]
        r=subprocess.run(command,env=env,capture_output=True);(WORK/(mode+'-'+label+'.log')).write_bytes(r.stdout+r.stderr)
        report=json.loads(target.read_text());assert report['exit_code']==r.returncode==1
        assert not report['oracle_failures'] and not report['infrastructure_error']
        assert len(report['observations'])==count and observed(report)==observed(baseline),(mode,label,observed(report))
        legs.append(dict(mode=mode,leg=label,gate_exit=r.returncode,baseline_equal=True,observations=count))
        print('QUALITY',mode,label,'baseline-equivalent; existing golden diff exit',r.returncode,flush=True)
(WORK/'quality-summary.json').write_text(json.dumps(dict(legs=legs,golden_recorded=False),indent=2)+'\n',encoding='utf-8',newline='\n')
