"""Temporary exact incoming-proof and moved-runtime consumer check; CLI only."""
from pathlib import Path
import hashlib,json,os,re,shutil,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'tools'))
from ci.compiler_pilot import prepare_producer
import sample_native_gate
OUT=ROOT/'build/compiler-pilot';OUT.mkdir(parents=True,exist_ok=True)
D=OUT/'compound-index-join';D.mkdir(exist_ok=False)
def h(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
def save(name,value):(D/name).write_text(json.dumps(value,indent=2)+'\n')
def run(command,label):
 t=time.monotonic();r=subprocess.run(command,cwd=ROOT,text=True,capture_output=True,stdin=subprocess.DEVNULL,timeout=240)
 (D/(label+'.stdout')).write_text(r.stdout);(D/(label+'.stderr')).write_text(r.stderr)
 return r,time.monotonic()-t
def prepare(project,label):
 failure,raw=sample_native_gate.accept_review(str(producer.cli),str(project),producer.target,180)
 (D/(label+'.review.txt')).write_text(raw)
 return failure,raw
revision=git('rev-parse','HEAD');assert not git('status','--porcelain')
inputs={p:h(ROOT/p) for p in git('ls-files','--','Cargo.toml','Cargo.lock','rust-toolchain.toml','omega-rust','source/library').splitlines()}
save('source-inputs.json',dict(revision=revision,tree=git('rev-parse','HEAD^{tree}'),files=inputs))
sdk={k:v for k,v in inputs.items() if k.startswith('source/library/')};save('sdk-inputs.json',sdk)
for p in sdk:
 to=D/'sdk-snapshot'/p;to.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/p,to)
start=time.monotonic();producer=prepare_producer(OUT,need_runner=False);build_seconds=time.monotonic()-start
os.environ['OMEGA_LIBRARY_ROOT']=str(ROOT/'source/library')
summary=dict(revision=revision,producer=producer.record(),build_seconds=build_seconds,passed=False,CLI_only=True,source_inputs=len(inputs),sdk_files=len(sdk))
(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
SOURCE_ROSTER=[('pass/borrows/runtime_reference_call_scalar_snapshot_exit', True), ('pass/facts/compound_index_guard_through_call_result_exit', True), ('pass/facts/compound_index_guard_through_const_name_exit', True), ('pass/facts/compound_index_guard_through_field_exit', True), ('pass/facts/compound_index_guard_through_local_exit', True), ('pass/facts/compound_index_guard_through_match_join_exit', True), ('pass/facts/compound_index_guard_through_other_machine_exit', True), ('pass/facts/compound_index_guard_through_other_state_exit', True), ('pass/facts/compound_index_guard_through_parameter_exit', True), ('pass/facts/compound_index_guard_through_target_body_exit', True), ('fail/arithmetic/call_argument_snapshot_not_current_field', False), ('fail/arithmetic/repeated_call_argument_snapshots_are_distinct', False), ('fail/arithmetic/wrapping_guard_does_not_bound_original', False), ('fail/facts/compound_index_guard_through_call_result_rejected', False), ('fail/facts/compound_index_guard_through_const_name_rejected', False), ('fail/facts/compound_index_guard_through_field_rejected', False), ('fail/facts/compound_index_guard_through_local_rejected', False), ('fail/facts/compound_index_guard_through_match_join_rejected', False), ('fail/facts/compound_index_guard_through_other_machine_rejected', False), ('fail/facts/compound_index_guard_through_other_state_rejected', False), ('fail/facts/compound_index_guard_through_parameter_rejected', False), ('fail/facts/compound_index_guard_through_target_body_rejected', False), ('fail/facts/compound_index_guard_wrong_call_actual', False)]
source=[]
for index,(fixture,positive) in enumerate(SOURCE_ROSTER):
 project=ROOT/'tests/omega'/fixture;label='source-'+str(index);failure,review=prepare(project,label)
 report=D/(label+'.report.txt');command=[str(producer.cli),'--check','--target',producer.target,'--report-file',str(report),str(project/'main.omg')]
 if failure:
  raw=review;code=None;seconds=0;phase='review';checked=False
 else:
  result,seconds=run(command,label);raw=result.stdout+result.stderr;code=result.returncode;phase='check';checked=code==0 and report.is_file() and 'product: check\n' in report.read_text()
 expected=[] if positive else [s for s in (project/'expected.txt').read_text().splitlines() if s.strip()]
 passed=checked if positive else bool((failure or code!=0) and expected and all(s in raw for s in expected))
 row=dict(fixture=fixture,positive=positive,phase=phase,command=command,returncode=code,seconds=seconds,checked_product=checked,expected_fragments=expected,passed=passed,input_sha256={p.name:h(p) for p in project.iterdir() if p.name in ('main.omg','build.omg','expected.txt','expected_native_exit.txt')})
 source.append(row);save('source-observations.json',source);assert passed,(row,raw)
runtime=[]
for index,fixture in enumerate(['pass/facts/compound_index_guard_through_call_result_exit','pass/facts/compound_index_guard_through_match_join_exit','pass/facts/compound_index_guard_through_target_body_exit','pass/borrows/runtime_reference_call_scalar_snapshot_exit']):
 project=ROOT/'tests/omega'/fixture;label='both-'+str(index);failure,raw=prepare(project,label);assert failure is None,(fixture,failure,raw)
 command=[str(producer.cli),'run','--both',str(project/'main.omg')];result,seconds=run(command,label);lines=result.stderr.splitlines();passed=result.returncode==0 and 'native exit 0' in lines and 'interp exit: 0' in lines
 runtime.append(dict(fixture=fixture,command=command,returncode=result.returncode,seconds=seconds,native_zero='native exit 0' in lines,Terminal_zero='interp exit: 0' in lines,passed=passed,input_sha256={p.name:h(p) for p in project.iterdir() if p.name in ('main.omg','build.omg','expected_native_exit.txt')}));save('both-observations.json',runtime);assert passed,(fixture,result.stdout,result.stderr)
cf=[]
for name,place,old,new in [('selected-store','target_body','main.grid[main.b * 20 + main.a] = 7;','main.grid[main.b * 20 + main.a] = 8;'),('first-match-route','match_join','true -> mark_coordinate(main.a, &mut route, 17),','true -> mark_coordinate(main.a, &mut route, 19),')]:
 original=ROOT/'samples/cli/facts/compound_index_guard'/place;project=D/'mutants'/name;project.mkdir(parents=True)
 for n in ['main.omg','build.omg','sample.json','expected.txt']:
  if (original/n).is_file():shutil.copy2(original/n,project/n)
 p=project/'main.omg';s=p.read_text();assert s.count(old)>=1;p.write_text(s.replace(old,new,1));mutation_inputs={p.name:h(p) for p in project.iterdir() if p.is_file()}
 failure,raw=prepare(project,name);assert failure is None,(name,failure,raw)
 command=[str(producer.cli),'run','--both',str(p)];result,seconds=run(command,name);lines=result.stderr.splitlines();signals=[int(m.group(1)) for line in lines if (m:=re.fullmatch(r'native signal (\d+)',line))];trap='interp crash: Trap (host presentation agrees)' in lines;passed=result.returncode==255 and len(signals)==1 and trap
 cf.append(dict(name=name,command=command,returncode=result.returncode,seconds=seconds,native_signal=signals,terminal_trap=trap,mutation=dict(old=old,new=new,replacements=1),input_sha256=mutation_inputs,passed=passed));save('counterfactuals.json',cf);assert passed,(name,result.stdout,result.stderr)
producer.verify();assert all(h(ROOT/p)==v for p,v in inputs.items());assert git('rev-parse','HEAD')==revision and not git('status','--porcelain')
summary.update(passed=True,Source=len(source),Source_checked=sum(r['positive'] for r in source),Source_proper_rejects=sum(not r['positive'] for r in source),BOTH=len(runtime),counterfactuals=len(cf))
(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');save('verdict.json',summary)
print(json.dumps(summary),flush=True)
