"""Temporary optimized CLI qualification of the copied private-arrival repair."""
from pathlib import Path
import hashlib,json,os,sys,subprocess,shutil,time
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'tools'))
import sample_native_gate
OUT=ROOT/'build/compiler-pilot';OUT.mkdir(parents=True,exist_ok=True);D=OUT/'generic-case-migration1';D.mkdir()
def h(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=ROOT,text=True).strip()
def save(n,v):(D/n).write_text(json.dumps(v,indent=2)+'\n')
def run(command,label,timeout=240):
 started=time.monotonic();r=subprocess.run(command,cwd=ROOT,capture_output=True,text=True,stdin=subprocess.DEVNULL,timeout=timeout);(D/(label+'.stdout')).write_text(r.stdout);(D/(label+'.stderr')).write_text(r.stderr);return r,time.monotonic()-started
def locks(project,label):
 target=D/'consumed-acceptance'/label;target.mkdir(parents=True,exist_ok=True);files={}
 for name in ['omega.lock','omega.admissions']:
  p=project/name
  if p.is_file():to=target/(name+'.txt');shutil.copy2(p,to);files[name]=dict(sha256=h(to),bytes=to.stat().st_size,retained=str(to.relative_to(D)))
 save(str(Path('consumed-acceptance')/label/'manifest.json'),dict(project=str(project),files=files))
def check(project,label,positive):
 failure,raw=sample_native_gate.accept_review(str(producer.cli),str(project),producer.target,180);(D/(label+'.review.txt')).write_text(raw);code=None;seconds=0;checked=False;phase='review';command=[str(producer.cli),'--check','--target',producer.target,'--report-file',str(D/(label+'.report.txt')),str(project/'main.omg')]
 if failure is None:
  r,seconds=run(command,label);code=r.returncode;raw=r.stdout+r.stderr;phase='Source check';p=D/(label+'.report.txt');checked=code==0 and p.is_file() and 'product: check\n' in p.read_text()
 expected=[] if positive else [x for x in (project/'expected.txt').read_text().splitlines() if x.strip()];passed=checked if positive else bool((failure or code!=0) and expected and all(x in raw for x in expected));locks(project,label);return dict(label=label,project=str(project),command=command,phase=phase,returncode=code,seconds=seconds,positive=positive,checked_product=checked,expected_fragments=expected,passed=passed,inputs={p.name:dict(sha256=h(p),text=p.read_text()) for p in project.iterdir() if p.is_file() and p.name in ['main.omg','build.omg','expected.txt','expected_native_exit.txt','expected_interpreter_exit.txt']})
def both(project,label):
 failure,raw=sample_native_gate.accept_review(str(producer.cli),str(project),producer.target,180);(D/(label+'.review.txt')).write_text(raw)
 if failure:locks(project,label);return dict(label=label,project=str(project),refusal=failure,passed=False)
 command=[str(producer.cli),'run','--both',str(project/'main.omg')];r,seconds=run(command,label);lines=r.stderr.splitlines();locks(project,label);return dict(label=label,project=str(project),command=command,returncode=r.returncode,seconds=seconds,native_zero='native exit 0' in lines,Terminal_zero='interp exit: 0' in lines,passed=r.returncode==0 and 'native exit 0' in lines and 'interp exit: 0' in lines,inputs={p.name:h(p) for p in project.iterdir() if p.is_file() and p.name in ['main.omg','build.omg','expected_native_exit.txt','expected_interpreter_exit.txt']})

import importlib.util,re,corpus_gate
from compiler_profile import require_optimized
from ci.compiler_pilot import validate_report
INPUT=ROOT/'build/generic-retained-input';candidate=git('rev-parse','HEAD');assert not git('status','--porcelain')
previous = json.loads((INPUT / 'summary.json').read_text())['actual_producer']
assert previous['revision'] == '1f3c4da0217eac2b7401cc0d3b3e0186d954e7e2'
inputs = json.loads((INPUT / 'copied-case-repair2/source-inputs.json').read_text())['files']
frozen = ROOT / 'build/verified-generic-producer-source1f3'
subprocess.run(['git', 'worktree', 'add', '--detach', str(frozen), previous['revision']], cwd=ROOT, check=True)
assert all(h(frozen / p) == value for p, value in inputs.items())
spec = importlib.util.spec_from_file_location('generic_frozen_owner', frozen / 'tools/ci/compiler_pilot.py')
owner = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = owner
spec.loader.exec_module(owner)
retained = OUT / 'producer-generic-case-migration1'
retained.mkdir()
for name in ['omega', 'corpus_runner', 'corpus_runner.compiler-profile.json']:
    shutil.copy2(INPUT / Path(previous['cli']).parent.name / name, retained / name)
cli, runner = retained / 'omega', retained / 'corpus_runner'
for binary in [cli, runner]: binary.chmod(binary.stat().st_mode | 0o111)
cli_profile, runner_profile = require_optimized(cli), require_optimized(runner)
assert cli_profile['sha256'] == previous['cli_profile']['sha256']
assert runner_profile['sha256'] == previous['runner_profile']['sha256']
producer = owner.CompilerProducer(previous['revision'], previous['tree'], previous['target'], previous['toolchain'], cli.resolve(), runner.resolve(), cli_profile, runner_profile)
producer.verify()
(retained / 'producer.json').write_text(json.dumps(producer.record(), indent=2) + '\n')
for binary, profile in [(cli, cli_profile), (runner, runner_profile)]:
    Path(str(binary) + '.verified-profile.json').write_text(json.dumps(dict(source_revision=producer.revision, source_tree=producer.tree, toolchain=producer.toolchain, observation=profile), indent=2) + '\n')
os.environ['OMEGA_LIBRARY_ROOT'] = str(frozen / 'source/library')

current={p:h(ROOT/p) for p in git('ls-files','--','Cargo.toml','Cargo.lock','rust-toolchain.toml','omega-rust','source/library').splitlines()}
save('source-inputs.json',dict(candidate=candidate,actual_compiler_revision=producer.revision,files=inputs,current_candidate_files=current))
sdk={p:v for p,v in inputs.items() if p.startswith('source/library/')};assert all(h(ROOT/p)==v for p,v in sdk.items());save('sdk-inputs.json',sdk)
base=ROOT/'samples/cli/facts/generic_case_subject';places=['local','field','parameter','call_result','match_join','const_name','other_state','other_machine','target_body'];roster=[];closures={}
for place in places:
 for positive in [True,False]:
  folder=base/place if positive else base/place/'invalid';fixture=('pass' if positive else 'fail')+'/facts/generic_case_subject_through_'+place+('_exit' if positive else '_rejected');pin=ROOT/'tests/omega'/fixture;roster.append(fixture)
  for name in ['main.omg','build.omg']:assert (folder/name).read_bytes()==(pin/name).read_bytes()
  for p in [folder,pin]:
   for f in p.iterdir():
    if f.is_file() and f.suffix in ['.omg','.txt','.json']:closures[str(f.relative_to(ROOT))]=dict(sha256=h(f),text=f.read_text())
save('program-inputs.json',closures);reports=[]
for label,fixtures,both_leg in [('checking',roster,False),('both',[f for f in roster if f.startswith('pass/')],True)]:
 golden=D/(label+'-golden.txt');shutil.copy2(ROOT/'tests/omega'/('corpus_both_linux_x86_64.txt' if both_leg else 'corpus_outcomes.txt'),golden);command=[sys.executable,'tools/corpus_gate.py','--runner',str(runner),'--filter',','.join(fixtures),'--record','--golden',str(golden),'--json',str(D/(label+'.json'))]
 if both_leg:command.append('--both')
 result,seconds=run(command,label+'-gate',600);report=json.loads((D/(label+'.json')).read_text());validate_report(report,tuple(fixtures),both=both_leg,revision=candidate,target=producer.target);assert result.returncode==0;reports.append(dict(label=label,command=command,returncode=result.returncode,seconds=seconds,rows=len(fixtures),passed=True));save('corpus.json',reports)
public=[]
for place in places:
 public.append(dict(place=place,**both(base/place,'public-'+place)));save('public.json',public)
controls=[]
for label,place,old,new in [('selected-payload','target_body','LinuxChoiceObservation satisfies CaseObservation {\n    machine observe() -> u64 {\n        transition word() {\n            Choice::Value { value } -> value','LinuxChoiceObservation satisfies CaseObservation {\n    machine observe() -> u64 {\n        transition word() {\n            Choice::Value { value } -> 16u64'),('boolean-payload','local','machine flag() -> Choice<bool> { wrap(true) }','machine flag() -> Choice<bool> { wrap(false) }')]:
 original=base/place;p=D/'controls'/label;p.mkdir(parents=True);text=(original/'main.omg').read_text();assert text.count(old)==1,(label,old);(p/'main.omg').write_text(text.replace(old,new));(p/'build.omg').write_bytes((original/'build.omg').read_bytes());admission=check(p,label+'-check',True)
 if admission['passed']:
  command=[str(cli),'run','--both',str(p/'main.omg')];result,seconds=run(command,label);raw=result.stdout+result.stderr;locks(p,label);row=dict(label=label,Source=admission,command=command,returncode=result.returncode,seconds=seconds,passed=bool(re.search(r'native signal [45]',raw) and 'interp crash: Trap' in raw and 'declined' not in raw.lower()))
 else:row=dict(label=label,Source=admission,passed=False)
 controls.append(row);save('controls.json',controls)
providers=[];target=base/'target_body'
for label,absent in [('absent-candidates',True),('competing-candidates',False)]:
 p=D/'controls'/label;p.mkdir(parents=True);text=(target/'main.omg').read_text();build=(target/'build.omg').read_text()
 if absent:
  text,count=re.subn(r'^(WindowsChoiceObservation|LinuxChoiceObservation|ArmChoiceObservation|DarwinChoiceObservation) satisfies CaseObservation \{\n.*?^\}\n','',text,flags=re.M|re.S);assert count==4;build=re.sub(r'^    Build::select_provider<CaseObservation,.*\n','',build,flags=re.M)
 else:
  old='    Build::select_provider<CaseObservation, LinuxChoiceObservation>(builder, linux_x86_64);\n';assert build.count(old)==1;build=build.replace(old,'')
 (p/'main.omg').write_text(text);(p/'build.omg').write_text(build);failure,raw=sample_native_gate.accept_review(str(cli),str(p),producer.target,180);(D/(label+'.review.txt')).write_text(raw);command=None;code=None;seconds=0
 if failure is None:
  command=[str(cli),'--build-dir',str(D/(label+'-native')),'--target',producer.target,str(p/'main.omg')];result,seconds=run(command,label);raw=result.stdout+result.stderr;code=result.returncode
 text_lower=raw.lower();reason=(('no provider is selected' in text_lower or 'missing provider' in text_lower) if absent else ('ambiguous' in text_lower or ('multiple' in text_lower and any(x in text_lower for x in ['candidate','provider','implementation']))));locks(p,label);providers.append(dict(label=label,command=command,returncode=code,seconds=seconds,phase='review' if failure else 'native realization',runtime_executed=False,passed=bool((failure or code!=0) and reason)));save('providers.json',providers)
artifacts=[]
for target_name in ['windows_x86_64','linux_x86_64','macos_arm64','linux_arm64']:
 failure,raw=sample_native_gate.accept_review(str(cli),str(target),target_name,180);(D/(target_name+'.review.txt')).write_text(raw);assert failure is None,(target_name,failure);build=D/'artifacts'/target_name;command=[str(cli),'--build-dir',str(build),'--report-file',str(D/(target_name+'.report.txt')),'--target',target_name,str(target/'main.omg')];result,seconds=run(command,target_name);image=build/('omega-program.exe' if target_name=='windows_x86_64' else 'omega-program');data=image.read_bytes() if image.is_file() else b'';magic='PE' if data.startswith(b'MZ') else 'ELF' if data.startswith(b'\x7fELF') else 'Mach-O' if data[:4] in [b'\xcf\xfa\xed\xfe',b'\xfe\xed\xfa\xcf'] else 'unknown';artifacts.append(dict(target=target_name,command=command,returncode=result.returncode,seconds=seconds,image=str(image),sha256=h(image) if data else None,bytes=len(data),format=magic,executed=False,passed=result.returncode==0 and magic!='unknown'));save('artifacts.json',artifacts)
producer.verify();assert git('rev-parse','HEAD')==candidate and not git('status','--porcelain') and all(h(ROOT/p)==v for p,v in current.items()) and all(h(ROOT/p)==v['sha256'] for p,v in closures.items())
summary=dict(candidate=candidate,actual_producer=producer.record(),Rust_build=False,current_Main_execution_claim=False,corpus=reports,public=public,controls=controls,providers=providers,artifacts=artifacts,passed=all(row['passed'] for row in reports+public+controls+providers+artifacts));save('verdict.json',summary);(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary),flush=True);sys.exit(0 if summary['passed'] else 1)
