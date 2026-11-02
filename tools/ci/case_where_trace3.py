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
D=OUT/'case-where-trace3';D.mkdir(exist_ok=False)
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
revision=git('rev-parse','HEAD');assert revision=='a9b91b42f08c5d6fade690b5e979c2192d32dd4e' and not git('status','--porcelain')
previous=json.loads((INPUT/'summary.json').read_text())['actual_producer'];original=json.loads((INPUT/'case-where-repair2/source-inputs.json').read_text());assert previous['revision']==revision
inputs={p:h(ROOT/p) for p in git('ls-files','--','Cargo.toml','Cargo.lock','rust-toolchain.toml','omega-rust','source/library').splitlines()};assert original['files']==inputs
save('source-inputs.json',dict(revision=revision,tree=git('rev-parse','HEAD^{tree}'),files=inputs));sdk={k:v for k,v in inputs.items() if k.startswith('source/library/')};save('sdk-inputs.json',sdk)
for p in sdk:
 dest=D/'sdk-snapshot'/p;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/p,dest)
local=OUT/Path(previous['cli']).parent.name;local.mkdir();cli=local/'omega';shutil.copy2(INPUT/local.name/'omega',cli);profile=require_optimized(cli);assert profile['sha256']==previous['cli_profile']['sha256'];producer=CompilerProducer(previous['revision'],previous['tree'],previous['target'],previous['toolchain'],cli,None,profile,None);producer.verify()
(local/'producer.json').write_text(json.dumps(producer.record(),indent=2)+'\n');(local/'omega.verified-profile.json').write_text(json.dumps(dict(source_revision=revision,source_tree=producer.tree,toolchain=producer.toolchain,observation=profile),indent=2)+'\n')
os.environ['OMEGA_LIBRARY_ROOT']=str(ROOT/'source/library')
import gzip
os.environ['OMEGA_STRUCT_TRACE']='1'
PROTOTYPE={'expected_native_exit.txt': '0\n', 'build.omg': 'machine build(builder: &mut Build) {\n    Build::application(builder, "case_where_equality_target_body");\n    Build::bind(builder, windows_x86_64::ProgramEntry, entry_start);\n    Build::bind(builder, linux_x86_64::ProgramEntry, entry_start);\n    Build::bind(builder, linux_arm64::ProgramEntry, entry_start);\n    Build::bind(builder, macos_arm64::ProgramEntry, entry_start);\n\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, windows_x86_64);\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, linux_x86_64);\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, macos_arm64);\n    Build::select_provider<PointWidth, SelectedPointWidth>(builder, linux_arm64);\n}\n', 'main.omg': '// Case Point(lo, hi) where lo == hi makes hi - lo exact in u64, without a guard or Wrapping.\n// Expected exit: 0\ndata Interval [copy] \n    requires (self in Interval::Empty) || ((self in Interval::Point) && (self.lo == self.hi))\n{\n    case Empty;\n    case Point(lo: u64, hi: u64);\n}\ndata Main { interval: Interval; width: u64; flag: bool; }\n\n// Preserve the original setup before an independently checked Main body.\nmachine run_main(main: &mut Main) crashes Trap {\n    main.width = 99;\n    main.interval = Interval::Point { lo: 17, hi: 17 };\n    PointWidth::run(main);\n}\n\nboundary trait PointWidth {\n    machine run(main: &mut Main) reaches crashes Trap;\n}\nSelectedPointWidth satisfies PointWidth {\n    machine run(main: &mut Main) reaches crashes Trap {\n    transition main.interval {\n        Interval::Point { lo, hi } -> take(lo, hi, main)\n        Interval::Empty -> fail()\n    }\n    machine take(lo: u64, hi: u64, main: &mut Main) {\n        main.width = hi - lo;\n        transition main.width == 0 {\n            true -> pass()\n            _ -> fail()\n        }\n    }\n    machine pass() { }\n    machine fail() { crash Trap; }\n\n    }\n}\n\n// Observe the original caller home and both adjacent fields after the call.\nmachine entry_start() crashes Trap {\n    let mut entry_storage: Main = Main {\n        interval: Interval::Empty {}, width: 99, flag: false\n    };\n    run_main(&mut entry_storage);\n    transition entry_storage.interval {\n        Interval::Point { lo, hi } -> observe(&mut entry_storage, lo, hi)\n        Interval::Empty -> bad()\n    }\n    machine observe(main: &mut Main, lo: u64, hi: u64) {\n        transition main.width == 0 && main.flag == false && lo == 17 && hi == 17 {\n            true -> done()\n            false -> bad()\n        }\n    }\n    machine done() {}\n    machine bad() { crash Trap; }\n}\n'}
for name,text in PROTOTYPE.items():
 p=D/'prototype'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text)
EXTRA={'local/main.omg': "// Case Point(lo, hi) where lo == hi makes hi - lo exact in u64, without a guard or Wrapping.\n// Expected exit: 0\ndata Interval [copy] \n    requires (self in Interval::Empty) || ((self in Interval::Point) && (self.lo == self.hi))\n{\n    case Empty;\n    case Point(lo: u64, hi: u64);\n}\ndata Main { interval: Interval; left_guard: u64; width: u64; right_guard: u64; flag: bool; }\n\nmachine run_main(main: &mut Main) crashes Trap {\n    main.width = 99;\n    main.interval = Interval::Point { lo: 17, hi: 17 };\n    guarded_width(main);\n}\n\nmachine guarded_width(main: &mut Main) crashes Trap {\n\n    let interval: Interval = main.interval;\n    transition interval {\n        Interval::Point { lo, hi } -> take(lo, hi, main)\n        Interval::Empty -> fail()\n    }\n    machine take(lo: u64, hi: u64, main: &mut Main) {\n        main.width = hi - lo;\n        transition main.width == 0 {\n            true -> pass()\n            _ -> fail()\n        }\n    }\n    machine pass() { }\n    machine fail() { crash Trap; }\n}\n\n\n// Observe the caller's original home and adjacent cells after return.\nmachine entry_start() crashes Trap {\n    let mut entry_storage: Main = Main { interval: Interval::Empty {}, left_guard: 101, width: 99, right_guard: 107, flag: false };\n    run_main(&mut entry_storage);\n    transition entry_storage.width == 0 && entry_storage.left_guard == 101 && entry_storage.right_guard == 107 && entry_storage.flag == false {\n        true -> observed()\n        false -> writeback_failed()\n    }\n    machine observed() {}\n    machine writeback_failed() { crash Trap; }\n}\n", 'local/build.omg': 'machine build(builder: &mut Build) {\n    Build::application(builder, "case_where_equality_local");\n    Build::bind(builder, windows_x86_64::ProgramEntry, entry_start);\n    Build::bind(builder, linux_x86_64::ProgramEntry, entry_start);\n    Build::bind(builder, linux_arm64::ProgramEntry, entry_start);\n    Build::bind(builder, macos_arm64::ProgramEntry, entry_start);\n}\n', 'local/expected_native_exit.txt': '0\n', 'match_join/main.omg': "// Case Point(lo, hi) where lo == hi makes hi - lo exact in u64, without a guard or Wrapping.\n// Expected exit: 0\ndata Interval [copy] \n    requires (self in Interval::Empty) || ((self in Interval::Point) && (self.lo == self.hi))\n{\n    case Empty;\n    case Point(lo: u64, hi: u64);\n}\ndata Main { interval: Interval; other: Interval; left_guard: u64; width: u64; right_guard: u64; flag: bool; route: u64; }\n\nmachine mark_interval(interval: Interval, route: &mut u64, tag: u64) -> Interval {\n    route = tag;\n    interval\n}\n\nmachine run_main(main: &mut Main) crashes Trap {\n    main.width = 99;\n    main.flag = true;\n    main.interval = Interval::Point { lo: 17, hi: 17 };\n    main.other = Interval::Point { lo: 17, hi: 17 };\n    guarded_width(main);\n    transition main.width == 0 && main.route == 17 {\n        true -> second(main)\n        false -> route_failed()\n    }\n    machine second(main: &mut Main) {\n        main.flag = false;\n        main.width = 99;\n        guarded_width(main);\n        transition main.width == 0 && main.route == 23 {\n            true -> both_routes()\n            false -> route_failed()\n        }\n    }\n    machine both_routes() {}\n    machine route_failed() { crash Trap; }\n}\n\nmachine guarded_width(main: &mut Main) crashes Trap {\n\n    let interval: Interval = match main.flag {\n        true -> mark_interval(main.interval, &mut main.route, 17),\n        false -> mark_interval(main.other, &mut main.route, 23)\n    };\n    transition interval {\n        Interval::Point { lo, hi } -> take(lo, hi, main)\n        Interval::Empty -> fail()\n    }\n    machine take(lo: u64, hi: u64, main: &mut Main) {\n        main.width = hi - lo;\n        transition main.width == 0 {\n            true -> pass()\n            _ -> fail()\n        }\n    }\n    machine pass() { }\n    machine fail() { crash Trap; }\n}\n\n\n// Observe the caller's original home and adjacent cells after return.\nmachine entry_start() crashes Trap {\n    let mut entry_storage: Main = Main { interval: Interval::Empty {}, left_guard: 101, width: 99, right_guard: 107, flag: false, other: Interval::Empty {}, route: 0 };\n    run_main(&mut entry_storage);\n    transition entry_storage.width == 0 && entry_storage.left_guard == 101 && entry_storage.right_guard == 107 && entry_storage.flag == false && entry_storage.route == 23 {\n        true -> observed()\n        false -> writeback_failed()\n    }\n    machine observed() {}\n    machine writeback_failed() { crash Trap; }\n}\n", 'match_join/build.omg': 'machine build(builder: &mut Build) {\n    Build::application(builder, "case_where_equality_match_join");\n    Build::bind(builder, windows_x86_64::ProgramEntry, entry_start);\n    Build::bind(builder, linux_x86_64::ProgramEntry, entry_start);\n    Build::bind(builder, linux_arm64::ProgramEntry, entry_start);\n    Build::bind(builder, macos_arm64::ProgramEntry, entry_start);\n}\n', 'match_join/expected_native_exit.txt': '0\n'}
for name,text in EXTRA.items():
 p=D/'extra'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text)
records=[]
for label,project in [('const',ROOT/'tests/omega/pass/facts/case_where_equality_through_const_name_exit'),('parameter',ROOT/'tests/omega/pass/facts/case_where_equality_through_parameter_exit')]:
 failure,raw=prepare(project,label);parts=[('review',raw)]
 if failure is None:
  result,seconds=run([str(producer.cli),'--check','--target',producer.target,str(project/'main.omg')],label+'-check');parts.append(('check',result.stdout+result.stderr))
 streams=[];selected=[]
 for phase,text in parts:
  data=text.encode();p=D/(label+'.'+phase+'.raw.txt.gz');p.write_bytes(gzip.compress(data));streams.append(dict(phase=phase,raw_bytes=len(data),raw_sha256=hashlib.sha256(data).hexdigest(),gzip_sha256=h(p),retained=p.name))
  selected.extend(line for line in text.splitlines() if any(x in line for x in ['STRUCT case-domain','STRUCT predicate-case','exact arithmetic','STRUCT invariant','STRUCT normal-call','STRUCT resolve','STRUCT call-entry']))
 (D/(label+'.selected.txt')).write_text('\n'.join(selected)+'\n')
 for p in D.glob(label+'*.txt'):
  if p.name.endswith('.review.txt'):p.unlink()
 for suffix in ['.stdout','.stderr']:
  p=D/(label+'-check'+suffix)
  if p.is_file():p.unlink()
 locks(project,label);records.append(dict(label=label,project=str(project),review_failure=failure,streams=streams,source_main_sha256=h(project/'main.omg'),source_build_sha256=h(project/'build.omg')));save('observations.json',records)
producer.verify();assert all(h(ROOT/p)==v for p,v in inputs.items()) and not git('status','--porcelain')
summary=dict(actual_producer=producer.record(),harness_revision=os.environ['GITHUB_SHA'],Rust_build=False,diagnostic_only=True,OMEGA_STRUCT_TRACE='1',records=records);(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');save('verdict.json',summary);print(json.dumps(summary),flush=True)
