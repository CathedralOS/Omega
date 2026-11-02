"""Temporary fixed-producer case-where baseline and selected-body prototype."""
from pathlib import Path
import hashlib,json,os,re,shutil,subprocess,sys,time
ROOT=Path(os.environ['OMEGA_CASE_REPLAY_ROOT']).resolve()
INPUT=Path(os.environ['OMEGA_CASE_RETAINED_INPUT']).resolve()
sys.path.insert(0,str(ROOT/'tools'))
from compiler_profile import require_optimized
from ci.compiler_pilot import CompilerProducer
import sample_native_gate
OUT=ROOT/'build/compiler-pilot';OUT.mkdir(parents=True,exist_ok=True)
D=OUT/'case-where-baseline';D.mkdir(exist_ok=False)
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
revision=git('rev-parse','HEAD');assert revision=='08b7c22717e58f422e770c848287927ac9099f64' and not git('status','--porcelain')
previous=json.loads((INPUT/'summary.json').read_text())['producer'];original=json.loads((INPUT/'compound-index-join/source-inputs.json').read_text());assert previous['revision']==revision
inputs={p:h(ROOT/p) for p in git('ls-files','--','Cargo.toml','Cargo.lock','rust-toolchain.toml','omega-rust','source/library').splitlines()};assert original['files']==inputs
save('source-inputs.json',dict(revision=revision,tree=git('rev-parse','HEAD^{tree}'),files=inputs));sdk={k:v for k,v in inputs.items() if k.startswith('source/library/')};save('sdk-inputs.json',sdk)
for p in sdk:
 dest=D/'sdk-snapshot'/p;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/p,dest)
local=OUT/Path(previous['cli']).parent.name;local.mkdir();cli=local/'omega';shutil.copy2(INPUT/local.name/'omega',cli);profile=require_optimized(cli);assert profile['sha256']==previous['cli_profile']['sha256'];producer=CompilerProducer(previous['revision'],previous['tree'],previous['target'],previous['toolchain'],cli,None,profile,None);producer.verify()
(local/'producer.json').write_text(json.dumps(producer.record(),indent=2)+'\n');(local/'omega.verified-profile.json').write_text(json.dumps(dict(source_revision=revision,source_tree=producer.tree,toolchain=producer.toolchain,observation=profile),indent=2)+'\n')
os.environ['OMEGA_LIBRARY_ROOT']=str(ROOT/'source/library')
ROSTER=['pass/facts/case_where_equality_through_local_exit', 'fail/facts/case_where_equality_through_local_rejected', 'pass/facts/case_where_equality_through_field_exit', 'fail/facts/case_where_equality_through_field_rejected', 'pass/facts/case_where_equality_through_parameter_exit', 'fail/facts/case_where_equality_through_parameter_rejected', 'pass/facts/case_where_equality_through_call_result_exit', 'fail/facts/case_where_equality_through_call_result_rejected', 'pass/facts/case_where_equality_through_match_join_exit', 'fail/facts/case_where_equality_through_match_join_rejected', 'pass/facts/case_where_equality_through_const_name_exit', 'fail/facts/case_where_equality_through_const_name_rejected', 'pass/facts/case_where_equality_through_other_state_exit', 'fail/facts/case_where_equality_through_other_state_rejected', 'pass/facts/case_where_equality_through_other_machine_exit', 'fail/facts/case_where_equality_through_other_machine_rejected', 'pass/facts/case_where_equality_through_target_body_exit', 'fail/facts/case_where_equality_through_target_body_rejected']
source=[]
for index,fixture in enumerate(ROSTER):
 source.append(dict(fixture=fixture,**check(ROOT/'tests/omega'/fixture,'source-'+str(index),fixture.startswith('pass/'))));save('checking.json',source)
runtime=[]
for index,fixture in enumerate(f for f in ROSTER if f.startswith('pass/')):
 runtime.append(dict(fixture=fixture,**both(ROOT/'tests/omega'/fixture,'both-'+str(index))));save('both.json',runtime)
public=both(ROOT/'samples/cli/facts/case_where_equality','public-root');save('public-root.json',public)
PROTOTYPE={'positive/expected_native_exit.txt': '0\n', 'positive/build.omg': 'machine build(builder: &mut Build) {\n    Build::application(builder, "case_where_equality_target_body");\n    Build::bind(builder, windows_x86_64::ProgramEntry, entry_start);\n    Build::bind(builder, linux_x86_64::ProgramEntry, entry_start);\n    Build::bind(builder, linux_arm64::ProgramEntry, entry_start);\n    Build::bind(builder, macos_arm64::ProgramEntry, entry_start);\n\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, windows_x86_64);\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, linux_x86_64);\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, macos_arm64);\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, linux_arm64);\n}\n', 'positive/main.omg': '// Case Point(lo, hi) where lo == hi makes hi - lo exact in u64, without a guard or Wrapping.\n// Expected exit: 0\ndata Interval [copy] \n    requires (self in Interval::Empty) || ((self in Interval::Point) && (self.lo == self.hi))\n{\n    case Empty;\n    case Point(lo: u64, hi: u64);\n}\ndata Main { interval: Interval; width: u64; flag: bool; }\n\n// Preserve the original setup before an independently checked Main body.\nmachine run_main(main: &mut Main) crashes Trap {\n    main.width = 99;\n    main.interval = Interval::Point { lo: 17, hi: 17 };\n    PointWidth::run(main);\n}\n\nboundary trait PointWidth {\n    machine run(main: &mut Main) reaches crashes Trap;\n}\nSelectedPointWidth satisfies PointWidth {\n    machine run(main: &mut Main) reaches crashes Trap {\n    transition main.interval {\n        Interval::Point { lo, hi } -> take(lo, hi, main)\n        Interval::Empty -> fail()\n    }\n    machine take(lo: u64, hi: u64, main: &mut Main) {\n        main.width = hi - lo;\n        transition main.width == 0 {\n            true -> pass()\n            _ -> fail()\n        }\n    }\n    machine pass() { }\n    machine fail() { crash Trap; }\n\n    }\n}\n\n// Observe the original caller home and both adjacent fields after the call.\nmachine entry_start() crashes Trap {\n    let mut entry_storage: Main = Main {\n        interval: Interval::Empty {}, width: 99, flag: false\n    };\n    run_main(&mut entry_storage);\n    transition entry_storage.interval {\n        Interval::Point { lo, hi } -> observe(&mut entry_storage, lo, hi)\n        Interval::Empty -> bad()\n    }\n    machine observe(main: &mut Main, lo: u64, hi: u64) {\n        transition main.width == 0 && main.flag == false && lo == 17 && hi == 17 {\n            true -> done()\n            false -> bad()\n        }\n    }\n    machine done() {}\n    machine bad() { crash Trap; }\n}\n', 'negative/build.omg': 'machine build(builder: &mut Build) {\n    Build::application(builder, "case_where_equality_target_body");\n    Build::bind(builder, windows_x86_64::ProgramEntry, entry_start);\n    Build::bind(builder, linux_x86_64::ProgramEntry, entry_start);\n    Build::bind(builder, linux_arm64::ProgramEntry, entry_start);\n    Build::bind(builder, macos_arm64::ProgramEntry, entry_start);\n\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, windows_x86_64);\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, linux_x86_64);\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, macos_arm64);\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, linux_arm64);\n}\n', 'negative/main.omg': '// Case Point(lo, hi) where lo == hi makes hi - lo exact in u64, without a guard or Wrapping.\n// Expected exit: 0\ndata Interval [copy] \n    requires (self in Interval::Empty) || ((self in Interval::Point))\n{\n    case Empty;\n    case Point(lo: u64, hi: u64);\n}\ndata Main { interval: Interval; width: u64; flag: bool; }\n\n// Preserve the original setup before an independently checked Main body.\nmachine run_main(main: &mut Main) crashes Trap {\n    main.width = 99;\n    main.interval = Interval::Point { lo: 17, hi: 17 };\n    PointWidth::run(main);\n}\n\nboundary trait PointWidth {\n    machine run(main: &mut Main) reaches crashes Trap;\n}\nSelectedPointWidth satisfies PointWidth {\n    machine run(main: &mut Main) reaches crashes Trap {\n    transition main.interval {\n        Interval::Point { lo, hi } -> take(lo, hi, main)\n        Interval::Empty -> fail()\n    }\n    machine take(lo: u64, hi: u64, main: &mut Main) {\n        main.width = hi - lo;\n        transition main.width == 0 {\n            true -> pass()\n            _ -> fail()\n        }\n    }\n    machine pass() { }\n    machine fail() { crash Trap; }\n\n    }\n}\n\n// Observe the original caller home and both adjacent fields after the call.\nmachine entry_start() crashes Trap {\n    let mut entry_storage: Main = Main {\n        interval: Interval::Empty {}, width: 99, flag: false\n    };\n    run_main(&mut entry_storage);\n    transition entry_storage.interval {\n        Interval::Point { lo, hi } -> observe(&mut entry_storage, lo, hi)\n        Interval::Empty -> bad()\n    }\n    machine observe(main: &mut Main, lo: u64, hi: u64) {\n        transition main.width == 0 && main.flag == false && lo == 17 && hi == 17 {\n            true -> done()\n            false -> bad()\n        }\n    }\n    machine done() {}\n    machine bad() { crash Trap; }\n}\n', 'negative/expected.txt': 'may overflow `u64`\n', 'negative/sample.json': '{\n  "role": "intended-invalid",\n  "entry": "main.omg",\n  "rejection_contains": [\n    "may overflow `u64`"\n  ]\n}\n'}
for name,text in PROTOTYPE.items():
 p=D/'prototypes'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text)
probes=[]
for name,positive in [('positive',True),('negative',False)]:
 project=D/'prototypes'/name;row=check(project,'prototype-'+name,positive)
 if positive and row['passed']:row['runtime']=both(project,'prototype-positive-both');row['passed']=row['runtime']['passed']
 probes.append(dict(name=name,**row));save('prototype.json',probes)
producer.verify();assert all(h(ROOT/p)==v for p,v in inputs.items()) and not git('status','--porcelain')
summary=dict(actual_producer=producer.record(),source_revision=revision,harness_revision=os.environ['GITHUB_SHA'],Rust_build=False,Source=len(source),Source_checked=sum(r['positive'] and r['passed'] for r in source),Source_proper_rejects=sum(not r['positive'] and r['passed'] for r in source),baseline_BOTH=len(runtime),baseline_passed=all(r['passed'] for r in source+runtime) and public['passed'],prototype_passed=all(r['passed'] for r in probes),source_inputs=len(inputs),SDK_files=len(sdk))
summary['passed']=summary['baseline_passed'] and summary['prototype_passed'];(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');save('verdict.json',summary);print(json.dumps(summary),flush=True);sys.exit(0 if summary['passed'] else 1)
