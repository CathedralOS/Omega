"""Temporary fixed-producer case-where baseline and selected-body prototype."""
from pathlib import Path
import hashlib,json,os,re,shutil,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[2]
INPUT=Path(os.environ['OMEGA_CASE_RETAINED_INPUT']).resolve()
sys.path.insert(0,str(ROOT/'tools'))
from compiler_profile import require_optimized
from ci.compiler_pilot import CompilerProducer
import sample_native_gate
OUT=ROOT/'build/compiler-pilot';OUT.mkdir(parents=True,exist_ok=True)
D=OUT/'case-where-row';D.mkdir(exist_ok=False)
def h(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=ROOT,text=True).strip()
def save(name,value):(D/name).write_text(json.dumps(value,indent=2)+'\n')
def run(command,label):
 started=time.monotonic();r=subprocess.run(command,cwd=ROOT,capture_output=True,text=True,stdin=subprocess.DEVNULL,timeout=240)
 (D/(label+'.stdout')).write_text(r.stdout);(D/(label+'.stderr')).write_text(r.stderr)
 return r,time.monotonic()-started
def locks(project,label):
 to=D/'consumed-acceptance'/label;to.mkdir(parents=True,exist_ok=True);record={}
 for name in ['omega.lock','omega.admissions']:
  p=project/name
  if p.is_file():
   dest=to/(name+'.txt');shutil.copy2(p,dest);record[name]=dict(sha256=h(p),bytes=p.stat().st_size,retained=str(dest.relative_to(D)))
 save(str(Path('consumed-acceptance')/label/'manifest.json'),dict(project=str(project),files=record))
def prepare(project,label):
 failure,text=sample_native_gate.accept_review(str(producer.cli),str(project),producer.target,180)
 (D/(label+'.review.txt')).write_text(text)
 return failure,text
def check(project,label,positive):
 failure,raw=prepare(project,label);command=[str(producer.cli),'--check','--target',producer.target,'--report-file',str(D/(label+'.report.txt')),str(project/'main.omg')]
 code=None;seconds=0;checked=False;phase='review'
 if failure is None:
  result,seconds=run(command,label);raw=result.stdout+result.stderr;code=result.returncode;phase='check';report=D/(label+'.report.txt');checked=code==0 and report.is_file() and 'product: check\n' in report.read_text()
 expected=[] if positive else [line for line in (project/'expected.txt').read_text().splitlines() if line.strip()]
 passed=checked if positive else bool((failure or code!=0) and expected and all(line in raw for line in expected))
 locks(project,label);return dict(project=str(project),command=command,positive=positive,phase=phase,returncode=code,seconds=seconds,checked_product=checked,expected_fragments=expected,passed=passed,inputs={p.name:h(p) for p in project.iterdir() if p.name in ['main.omg','build.omg','expected.txt','expected_native_exit.txt']})
def both(project,label):
 failure,raw=prepare(project,label)
 if failure:
  locks(project,label);return dict(project=str(project),passed=False,refusal=failure)
 command=[str(producer.cli),'run','--both',str(project/'main.omg')];r,seconds=run(command,label);lines=r.stderr.splitlines();passed=r.returncode==0 and 'native exit 0' in lines and 'interp exit: 0' in lines
 locks(project,label);return dict(project=str(project),command=command,returncode=r.returncode,seconds=seconds,native_zero='native exit 0' in lines,Terminal_zero='interp exit: 0' in lines,passed=passed,inputs={p.name:h(p) for p in project.iterdir() if p.name in ['main.omg','build.omg','expected_native_exit.txt']})
# Close the exact producer before restoring the authored program checkout.
# The runner is the only new Rust executable; the CLI is the retained P3 image.
import corpus_gate
from ci.compiler_pilot import validate_report,select_fixtures
candidate=git('rev-parse','HEAD');assert not git('status','--porcelain')
previous=json.loads((INPUT/'summary.json').read_text())['actual_producer']
source=json.loads((INPUT/'case-where-repair3/source-inputs.json').read_text())
assert source['revision']==previous['revision']=='70036176871cda9279a6a401987d0ca4325918e2'
inputs={p:h(ROOT/p) for p in git('ls-files','--','Cargo.toml','Cargo.lock','rust-toolchain.toml','omega-rust','source/library').splitlines()}
assert inputs==source['files']
save('source-inputs.json',dict(candidate=candidate,actual_compiler_revision=previous['revision'],files=inputs))
sdk={p:v for p,v in inputs.items() if p.startswith('source/library/')};save('sdk-inputs.json',sdk)
for p in sdk:
 dest=D/'sdk-snapshot'/p;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/p,dest)
retained=OUT/'producer-case-where-r3-pair';retained.mkdir()
cli=retained/'omega';shutil.copy2(INPUT/Path(previous['cli']).parent.name/'omega',cli)
profile=require_optimized(cli);assert profile['sha256']==previous['cli_profile']['sha256']
old_sha=os.environ.get('GITHUB_SHA');started=time.monotonic()
try:
 subprocess.run(['git','checkout','--detach',previous['revision']],cwd=ROOT,check=True)
 os.environ['GITHUB_SHA']=previous['revision']
 assert not git('status','--porcelain') and all(h(ROOT/p)==v for p,v in inputs.items())
 runner=corpus_gate.build_runner()
 runner_profile=require_optimized(runner)
 target_runner=retained/'corpus_runner';shutil.copy2(runner,target_runner)
 shutil.copy2(str(runner)+'.compiler-profile.json',str(target_runner)+'.compiler-profile.json')
 assert require_optimized(target_runner)['sha256']==runner_profile['sha256']
 producer=CompilerProducer(previous['revision'],git('rev-parse','HEAD^{tree}'),previous['target'],previous['toolchain'],cli.resolve(),target_runner.resolve(),profile,require_optimized(target_runner))
 producer.verify()
 save('runner-build.json',dict(source_revision=git('rev-parse','HEAD'),source_tree=git('rev-parse','HEAD^{tree}'),seconds=time.monotonic()-started,profile=runner_profile,CLI_rebuilt=False))
 assert all(h(ROOT/p)==v for p,v in inputs.items()) and not git('status','--porcelain')
finally:
 subprocess.run(['git','checkout','--detach',candidate],cwd=ROOT,check=True)
 if old_sha is None:os.environ.pop('GITHUB_SHA',None)
 else:os.environ['GITHUB_SHA']=old_sha
assert not git('status','--porcelain') and all(h(ROOT/p)==v for p,v in inputs.items())
(retained/'producer.json').write_text(json.dumps(producer.record(),indent=2)+'\n')
for binary,binary_profile in producer._artifacts():
 Path(str(binary)+'.verified-profile.json').write_text(json.dumps(dict(source_revision=producer.revision,source_tree=producer.tree,toolchain=producer.toolchain,observation=binary_profile),indent=2)+'\n')
os.environ['OMEGA_LIBRARY_ROOT']=str(ROOT/'source/library')
places=['root','local','field','parameter','call_result','match_join','const_name','other_state','other_machine','target_body']
roster=[];public=[];closures={}
for place in places:
 project=ROOT/'samples/cli/facts/case_where_equality'
 if place!='root':project=project/place
 for positive in [True,False]:
  folder=project if positive else project/'invalid'
  name='case_where_equality_root_'+('exit' if positive else 'rejected') if place=='root' else 'case_where_equality_through_'+place+('_exit' if positive else '_rejected')
  fixture=('pass' if positive else 'fail')+'/facts/'+name;pin=ROOT/'tests/omega'/fixture
  assert (folder/'main.omg').read_bytes()==(pin/'main.omg').read_bytes() and (folder/'build.omg').read_bytes()==(pin/'build.omg').read_bytes()
  roster.append(fixture);public.append((place,folder,positive))
  for source_folder in [folder,pin]:
   for path in source_folder.iterdir():
    if path.is_file() and path.name in ['main.omg','build.omg','expected.txt','expected_native_exit.txt','sample.json']:
     closures[str(path.relative_to(ROOT))]=dict(sha256=h(path),text=path.read_text())
select_fixtures(','.join(roster));save('program-inputs.json',closures)
reports=[]
for label,fixtures,both_leg in [('checking',roster,False),('both',[f for f in roster if f.startswith('pass/')],True)]:
 golden=D/(label+'-golden.txt');original=ROOT/'tests/omega'/('corpus_both_linux_x86_64.txt' if both_leg else 'corpus_outcomes.txt');shutil.copy2(original,golden)
 command=[sys.executable,'tools/corpus_gate.py','--runner',str(producer.runner),'--filter',','.join(fixtures),'--record','--golden',str(golden),'--json',str(D/(label+'.json'))]
 if both_leg:command+=['--both']
 result,seconds=run(command,label+'-gate');report=json.loads((D/(label+'.json')).read_text());validate_report(report,tuple(fixtures),both=both_leg,revision=candidate,target=producer.target)
 assert result.returncode==0
 reports.append(dict(label=label,command=command,returncode=result.returncode,seconds=seconds,rows=len(fixtures),passed=True));save('corpus.json',reports)
observed=[]
for place,project,positive in public:
 row=both(project,'public-'+place) if positive else check(project,'public-'+place+'-invalid',False)
 observed.append(dict(place=place,positive=positive,**row));save('public.json',observed)
# Preserve the independently valid closed-literal production, its opposite,
# and mutation/formation/case-access boundaries of the changed owners.
preservation=[]
for fixture,positive in [
 ('pass/facts/case_const_equal_literals_without_predicate',True),
 ('pass/facts/case_const_equal_literals_with_predicate',True),
 ('fail/facts/case_const_unequal_literals_subtraction_rejected',False),
 ('fail/dependent/case_where_payload_write_exit_open',False),
 ('fail/generics/extraction_inactive_case',False),
 ('fail/dependent/closed_case_projection_checks_sibling_arithmetic',False),
]:
 row=check(ROOT/'tests/omega'/fixture,'preservation-'+str(len(preservation)),positive)
 preservation.append(dict(fixture=fixture,**row));save('preservation.json',preservation)
# Counterfactuals retain the authored observation and corrupt one operation.
controls=[];control_inputs={}
target=ROOT/'samples/cli/facts/case_where_equality/target_body';match=ROOT/'samples/cli/facts/case_where_equality/match_join'
for label,source_folder,transform,build_transform,kind,expected in [
 ('selected-width',target,lambda s:s.replace('main.width = hi - lo;','main.width = hi - lo;\n        main.width = 1;'),lambda s:s,'trap',None),
 ('match-route',match,lambda s:s.replace('route = tag;','route = 17;'),lambda s:s,'trap',None),
 ('missing-selection',target,lambda s:s,lambda s:s.replace('    Build::select_provider<PointWidth, SelectedPointWidth>(builder, linux_x86_64);\n',''),'reject','no provider is selected'),
 ('unselected-invalid',target,lambda s:s+'\nUnselectedPointWidth satisfies PointWidth {\n machine run(main: &mut Main) reaches crashes Trap { let invalid: u64 = 0u64 - 1u64; }\n}\n',lambda s:s,'reject','may overflow `u64`'),
 ('alternate-selection',target,lambda s:s+'\nAlternatePointWidth satisfies PointWidth {\n machine run(main: &mut Main) reaches crashes Trap { crash Trap; }\n}\n',lambda s:s.replace('select_provider<PointWidth, SelectedPointWidth>(builder, linux_x86_64)','select_provider<PointWidth, AlternatePointWidth>(builder, linux_x86_64)'),'trap',None),
]:
 project=D/'controls'/label;project.mkdir(parents=True)
 (project/'main.omg').write_text(transform((source_folder/'main.omg').read_text()));(project/'build.omg').write_text(build_transform((source_folder/'build.omg').read_text()))
 control_inputs[label]={p.name:dict(sha256=h(p),text=p.read_text()) for p in project.iterdir() if p.is_file()}
 if kind=='reject':
  (project/'expected.txt').write_text(expected+'\n');row=check(project,label,False)
 else:
  admission=check(project,label+'-check',True)
  if admission['passed']:
   command=[str(producer.cli),'run','--both',str(project/'main.omg')];result,seconds=run(command,label);text=result.stdout+result.stderr
   row=dict(command=command,returncode=result.returncode,seconds=seconds,passed=bool(re.search(r'native signal [45]',text) and 'interp crash: Trap' in text and 'declined' not in text.lower()),Source=admission)
  else:row=dict(passed=False,Source=admission)
 controls.append(dict(label=label,kind=kind,**row));save('controls.json',controls);save('control-inputs.json',control_inputs)
artifacts=[]
for target_name in ['windows_x86_64','linux_x86_64','macos_arm64','linux_arm64']:
 failure,text=sample_native_gate.accept_review(str(producer.cli),str(target),target_name,180);(D/(target_name+'.review.txt')).write_text(text)
 assert failure is None,(target_name,failure,text)
 build=D/'artifacts'/target_name;command=[str(producer.cli),'--build-dir',str(build),'--report-file',str(D/(target_name+'.report.txt')),'--target',target_name,str(target/'main.omg')]
 result,seconds=run(command,target_name);image=build/('omega-program.exe' if target_name=='windows_x86_64' else 'omega-program');data=image.read_bytes() if image.is_file() else b''
 magic='PE' if data.startswith(b'MZ') else 'ELF' if data.startswith(b'\x7fELF') else 'Mach-O' if data[:4] in [b'\xcf\xfa\xed\xfe',b'\xfe\xed\xfa\xcf'] else 'unknown'
 artifacts.append(dict(target=target_name,command=command,returncode=result.returncode,seconds=seconds,path=str(image),sha256=h(image) if data else None,bytes=len(data),format=magic,executed=False,passed=result.returncode==0 and magic==('PE' if target_name=='windows_x86_64' else 'Mach-O' if target_name=='macos_arm64' else 'ELF')));save('artifacts.json',artifacts);locks(target,'artifact-'+target_name)
producer.verify();assert git('rev-parse','HEAD')==candidate and not git('status','--porcelain') and all(h(ROOT/p)==v for p,v in inputs.items())
assert all(h(ROOT/p)==row['sha256'] for p,row in closures.items())
summary=dict(candidate=candidate,actual_producer=producer.record(),CLI_Rust_build=False,runner_Rust_build=True,runner_source_revision=producer.revision,Source=len(roster),BOTH=len(places),public=len(observed),preservation=len(preservation),controls=len(controls),target_artifacts=len(artifacts),passed=all(r['passed'] for r in reports+observed+preservation+controls+artifacts))
(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');save('verdict.json',summary);print(json.dumps(summary),flush=True);sys.exit(0 if summary['passed'] else 1)
