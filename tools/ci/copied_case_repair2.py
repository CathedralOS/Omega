"""Temporary optimized CLI qualification of the copied private-arrival repair."""
from pathlib import Path
import hashlib,json,os,sys,subprocess,shutil,time
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'tools'))
from ci.compiler_pilot import prepare_producer
import sample_native_gate
OUT=ROOT/'build/compiler-pilot';OUT.mkdir(parents=True,exist_ok=True);D=OUT/'copied-case-repair2';D.mkdir()
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
import re,dataclasses,corpus_gate
from ci.compiler_pilot import validate_report
candidate=git('rev-parse','HEAD');assert not git('status','--porcelain')
inputs={p:h(ROOT/p) for p in git('ls-files','--','Cargo.toml','Cargo.lock','rust-toolchain.toml','omega-rust','source/library').splitlines()}
save('source-inputs.json',dict(candidate=candidate,files=inputs))
sdk={p:v for p,v in inputs.items() if p.startswith('source/library/')};save('sdk-inputs.json',sdk)
for name in sdk:
 to=D/'sdk-snapshot'/name;to.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/name,to)
for label,command in [('P05-check',['cargo','check','--locked','-p','typed-trees-to-checked-trees']),('P05-clippy',['cargo','clippy','--locked','-p','typed-trees-to-checked-trees','--lib','--no-deps'])]:
 r,seconds=run(command,label,600);save(label+'.json',dict(command=command,returncode=r.returncode,seconds=seconds));assert r.returncode==0
producer=prepare_producer(OUT,need_runner=False);os.environ['OMEGA_LIBRARY_ROOT']=str(ROOT/'source/library')
places=['local','field','parameter','call_result','match_join','const_name','other_state','other_machine','target_body']
base=ROOT/'samples/cli/facts/copied_case_before_later_argument'
critical=[check(base/'target_body','selected-positive',True)]
if critical[-1]['passed']:critical.append(both(base/'target_body','selected-both'))
critical.append(check(base/'target_body/invalid','selected-negative',False))
critical.append(check(base/'other_state','private-positive',True))
if critical[-1]['passed']:critical.append(both(base/'other_state','private-both'))
for name in ['second-absent-arrival','unknown-root-arrival','earlier-write']:
 critical.append(check(base/'private_arrivals'/name,name,False))
save('decisive.json',critical)
if not all(row['passed'] for row in critical):
 summary=dict(candidate=candidate,actual_producer=producer.record(),CLI_Rust_build=True,runner_Rust_build=False,decisive=critical,passed=False);save('verdict.json',summary);(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');sys.exit(1)
# The runner is built only after the actual changed owned-argument path succeeds.
runner=corpus_gate.build_runner();dest=producer.cli.parent/'corpus_runner';shutil.copy2(runner,dest);shutil.copy2(str(runner)+'.compiler-profile.json',str(dest)+'.compiler-profile.json');profile=corpus_gate.require_optimized(dest)
producer=dataclasses.replace(producer,runner=dest.resolve(),runner_profile=profile);producer.verify();(dest.parent/'producer.json').write_text(json.dumps(producer.record(),indent=2)+'\n');Path(str(dest)+'.verified-profile.json').write_text(json.dumps(dict(source_revision=candidate,source_tree=producer.tree,toolchain=producer.toolchain,observation=profile),indent=2)+'\n')
roster=[];closures={}
for place in places:
 for positive in [True,False]:
  project=base/place if positive else base/place/'invalid';fixture=('pass' if positive else 'fail')+'/facts/copied_case_before_later_argument_through_'+place+('_exit' if positive else '_rejected');pin=ROOT/'tests/omega'/fixture
  assert (pin/'main.omg').read_bytes()==(project/'main.omg').read_bytes() and (pin/'build.omg').read_bytes()==(project/'build.omg').read_bytes();roster.append(fixture)
  for folder in [project,pin]:
   for p in folder.iterdir():
    if p.is_file() and p.suffix in ['.omg','.txt','.json']:closures[str(p.relative_to(ROOT))]=dict(sha256=h(p),text=p.read_text())
for name in ['second-absent-arrival','unknown-root-arrival','earlier-write']:
 fixture='fail/facts/copied_case_private_'+name.replace('-','_')+'_rejected';roster.append(fixture)
 for folder in [base/'private_arrivals'/name,ROOT/'tests/omega'/fixture]:
  for p in folder.iterdir():
   if p.is_file() and p.suffix in ['.omg','.txt','.json']:closures[str(p.relative_to(ROOT))]=dict(sha256=h(p),text=p.read_text())
save('program-inputs.json',closures);reports=[]
for label,fixtures,both_leg in [('checking',roster,False),('both',[f for f in roster if f.startswith('pass/')],True)]:
 golden=D/(label+'-golden.txt');shutil.copy2(ROOT/'tests/omega'/('corpus_both_linux_x86_64.txt' if both_leg else 'corpus_outcomes.txt'),golden)
 command=[sys.executable,'tools/corpus_gate.py','--runner',str(producer.runner),'--filter',','.join(fixtures),'--record','--golden',str(golden),'--json',str(D/(label+'.json'))]
 if both_leg:command.append('--both')
 r,seconds=run(command,label+'-gate',600);report=json.loads((D/(label+'.json')).read_text());validate_report(report,tuple(fixtures),both=both_leg,revision=candidate,target=producer.target);assert r.returncode==0
 reports.append(dict(label=label,command=command,returncode=r.returncode,seconds=seconds,rows=len(fixtures),passed=True));save('corpus.json',reports)
public=[]
for place in places:
 public.append(dict(place=place,**both(base/place,'public-'+place)));save('public.json',public)
controls=[]
for label,place,old,new in [('omitted-replacement','target_body','    slot = Slot::Absent {};','    // Mutant omits replacement.'),('false-branch-route','match_join','route: 23','route: 17')]:
 original=base/place;p=D/'controls'/label;p.mkdir(parents=True);text=(original/'main.omg').read_text();assert text.count(old)==1;(p/'main.omg').write_text(text.replace(old,new));(p/'build.omg').write_bytes((original/'build.omg').read_bytes())
 admission=check(p,label+'-check',True)
 if admission['passed']:
  command=[str(producer.cli),'run','--both',str(p/'main.omg')];r,seconds=run(command,label);raw=r.stdout+r.stderr;locks(p,label);row=dict(label=label,command=command,returncode=r.returncode,seconds=seconds,Source=admission,passed=bool(re.search(r'native signal [45]',raw) and 'interp crash: Trap' in raw and 'declined' not in raw.lower()))
 else:row=dict(label=label,Source=admission,passed=False)
 controls.append(row);save('controls.json',controls)
artifacts=[];project=base/'target_body'
for target in ['windows_x86_64','linux_x86_64','macos_arm64','linux_arm64']:
 failure,raw=sample_native_gate.accept_review(str(producer.cli),str(project),target,180);(D/(target+'.review.txt')).write_text(raw);assert failure is None,(target,failure)
 build=D/'artifacts'/target;command=[str(producer.cli),'--build-dir',str(build),'--report-file',str(D/(target+'.report.txt')),'--target',target,str(project/'main.omg')];r,seconds=run(command,target);image=build/('omega-program.exe' if target=='windows_x86_64' else 'omega-program');data=image.read_bytes() if image.is_file() else b'';magic='PE' if data.startswith(b'MZ') else 'ELF' if data.startswith(b'\x7fELF') else 'Mach-O' if data[:4] in [b'\xcf\xfa\xed\xfe',b'\xfe\xed\xfa\xcf'] else 'unknown';row=dict(target=target,command=command,returncode=r.returncode,seconds=seconds,image=str(image),sha256=h(image) if data else None,bytes=len(data),format=magic,executed=False,passed=r.returncode==0 and magic!='unknown');artifacts.append(row);save('artifacts.json',artifacts)
producer.verify();assert git('rev-parse','HEAD')==candidate and not git('status','--porcelain') and all(h(ROOT/p)==v for p,v in inputs.items()) and all(h(ROOT/p)==v['sha256'] for p,v in closures.items())
summary=dict(candidate=candidate,actual_producer=producer.record(),CLI_Rust_build=True,runner_Rust_build=True,decisive=critical,corpus=reports,public=public,controls=controls,artifacts=artifacts,passed=all(r['passed'] for r in critical+reports+public+controls+artifacts));save('verdict.json',summary);(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary),flush=True);sys.exit(0 if summary['passed'] else 1)
