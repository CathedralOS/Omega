from pathlib import Path
import hashlib,json,os,subprocess
ROOT=Path.cwd();WORK=ROOT/'build/proof-profile/graph-phase-cpu';LEDGER=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28'
source=json.loads((LEDGER/'lazy_reference_graph/candidate-source.json').read_text());assert hashlib.sha256((ROOT/source['file']).read_bytes()).hexdigest()==source['source_sha256']
assert subprocess.check_output(['git','diff','--name-only','456c5e984452a45d977295e254b3ca9ae3cc7750','--','omega-rust'],text=True).splitlines()==[source['file']]
producer=json.loads((LEDGER/'lazy_reference_graph/identities.json').read_text())[1];folder=ROOT/'build/proof-profile/producers/lazy-reference'
for name,key in [('omega.exe','compiler_sha256'),('omega.lock','accepted_lock_sha256')]:assert hashlib.sha256((folder/name).read_bytes()).hexdigest()==producer[key]
corpus=json.loads((LEDGER/'lazy_reference_graph/corpus-identity.json').read_text());runner=folder/'corpus_runner.exe';assert hashlib.sha256(runner.read_bytes()).hexdigest()==corpus['sha256']
for exe in [folder/'omega.exe',runner]:assert int(json.loads(subprocess.check_output([str(exe),'--build-profile']))['opt_level'])>0
env=os.environ.copy();env['OMEGA_CORPUS_JOBS']='1'
for key in list(env):
    if key.startswith('OMEGA_') and any(x in key for x in ['PROBE','PROFILE','COUNTS','MEASUREMENT','EXPERIMENT','PHASE_CPU']):env.pop(key,None)
legs=[]
def observed(report):return [{k:r[k] for k in ['fixture','tier','status','expected_satisfied','facts','diagnostics']} for r in report['observations']]
for name,flags,count in [('positive',['--both'],4),('negative',[],3)]:
    baseline=json.loads((LEDGER/'lazy_reference_graph'/('baseline-quality-'+name+'.json')).read_text());target=WORK/('clean-'+name+'.json')
    command=['python','tools/corpus_gate.py','--runner',str(runner),*flags,'--filter',baseline['filter'],'--json',str(target)]
    r=subprocess.run(command,env=env,capture_output=True);(WORK/('clean-'+name+'.log')).write_bytes(r.stdout+r.stderr)
    report=json.loads(target.read_text());assert report['exit_code']==r.returncode==1 and not report['oracle_failures'] and not report['infrastructure_error']
    assert len(report['observations'])==count and observed(report)==observed(baseline)
    legs.append(dict(leg=name,observations=count,gate_exit=r.returncode,baseline_equal=True))
    print('CLEAN',name,'fresh baseline-equivalent observations with verified exact-source retained runner; golden diff exit',r.returncode,flush=True)
(WORK/'retained-implementation.json').write_text(json.dumps(dict(file=source['file'],source_sha256=source['source_sha256'],semantic_base=source['semantic_base'],
    producer=producer,corpus_identity=corpus,legs=legs,golden_recorded=False,diagnostic_flags_retained=False,
    applicability='Compiler sources match the exact preserved pure candidate; all other Rust inputs and fixture sources match the fixed semantic base. Reused executables are digest/profile verified, not rebuilt solely for documentation commits.'),indent=2)+'\n',encoding='utf-8',newline='\n')
