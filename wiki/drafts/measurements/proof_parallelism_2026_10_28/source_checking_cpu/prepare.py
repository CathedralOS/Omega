from pathlib import Path
import hashlib,json,re,subprocess
ROOT=Path.cwd();WORK=ROOT/'build/proof-profile/source-checking-cpu';WORK.mkdir(exist_ok=False)
assert not subprocess.check_output(['git','status','--porcelain']).strip()
base=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
assert base=='cec7b6657299525e62aa6fc2168d3768b4d33b6a'
driver='omega-rust/psi/compilation/driver/src/source_checking.rs'
path=ROOT/driver;text=path.read_text()
text=text.replace('let representations = self.admission.representations.admit(&program)?;', '''let representations = phase_profile::scope("05.cpu.representation_admission", || {
            self.admission.representations.admit(&program)
        })?;''')
text=text.replace('constant_entries::retain_evaluation_entries(&mut program)?;', '''phase_profile::scope("05.cpu.constant_entries", || {
            constant_entries::retain_evaluation_entries(&mut program)
        })?;''')
path.write_text(text,encoding='utf-8',newline='\n')
pipeline='omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/'
scope_files=[driver]+[pipeline+x for x in ['checking.rs','facts.rs','execution/finalize_execution.rs','checks/mod.rs']]
names=sorted(set(n for f in scope_files for n in re.findall(r'phase_profile::(?:scope|guard)\(\s*"([^"]+)"',(ROOT/f).read_text())))
extra=['omega-rust/psi/compilation/pipeline/03b_typed-trees-to-typed-trees/src/preparation.rs',pipeline+'checking/program_validation.rs','omega-rust/psi/compilation/checking/src/program_validation.rs']
names=sorted(set(names)|{n for f in extra for n in re.findall(r'phase_profile::(?:scope|guard)\(\s*"([^"]+)"',(ROOT/f).read_text()) if not any(x in n for x in ['05.d.2.a.2.machine.','05.d.2.a.2a.','05.d.2.a.2b.'])})
assert '05' in names and len(names)>45
profile='omega-rust/psi/foundation/phase-profile/src/lib.rs';path=ROOT/profile;text=path.read_text()
text=text.replace('    if !enabled() {\n        return work();','    let _cpu = CpuWindow::begin(name);\n    if !enabled() {\n        return work();',1)
text=text.replace('    started: Option<Instant>,','    started: Option<Instant>,\n    _cpu: Option<CpuWindow>,',1)
text=text.replace('        name,\n        started: enabled().then(Instant::now),','        name,\n        started: enabled().then(Instant::now),\n        _cpu: CpuWindow::begin(name),',1)
text+='''
// Temporary selective CPU attribution. The host owns platform calls.
static CPU_SAMPLER: std::sync::OnceLock<fn() -> [u64; 4]> = std::sync::OnceLock::new();
static CPU_ORIGIN: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
static CPU_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn install_cpu_sampler(sample: fn() -> [u64; 4]) {
    assert!(CPU_SAMPLER.set(sample).is_ok());
}
struct CpuWindow { name: &'static str, start: u128, ticks: [u64; 4], id: u64 }
impl CpuWindow {
    fn begin(name: &'static str) -> Option<Self> {
        let sample = CPU_SAMPLER.get()?;
        if !matches!(name, NAMES) { return None; }
        let origin = CPU_ORIGIN.get_or_init(Instant::now);
        Some(Self { name, start: origin.elapsed().as_nanos(), ticks: sample(), id: CPU_SEQUENCE.fetch_add(1, Ordering::Relaxed) })
    }
}
impl Drop for CpuWindow {
    fn drop(&mut self) {
        let ticks = CPU_SAMPLER.get().unwrap()();
        let end = CPU_ORIGIN.get().unwrap().elapsed().as_nanos();
        eprintln!("CHECK_CPU\\t{}\\t{:?}\\t{}\\t{}\\t{}\\t{}\\t{}\\t{}\\t{}\\t{}\\t{}\\t{}\\t{}", self.name, std::thread::current().id(), self.id, self.start, end, self.ticks[0],self.ticks[1],self.ticks[2],self.ticks[3],ticks[0],ticks[1],ticks[2],ticks[3]);
    }
}
'''.replace('NAMES',' | '.join(json.dumps(n) for n in names))
path.write_text(text,encoding='utf-8',newline='\n')
host='omega-rust/omega/foundation/artifacts/src/compile_timings/mod.rs';path=ROOT/host
path.write_text(path.read_text().replace('pub mod counters;','pub mod counters;\n#[cfg(windows)]\npub mod check_cpu;'),encoding='utf-8',newline='\n')
probe=path.with_name('check_cpu.rs');assert not probe.exists()
probe.write_text('''//! Temporary host CPU snapshots; never an input to compilation.
use std::ffi::c_void;
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct FileTime { low: u32, high: u32 }
impl FileTime { fn ticks(self) -> u64 { (u64::from(self.high) << 32) | u64::from(self.low) } }
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn GetCurrentThread() -> *mut c_void;
    fn GetProcessTimes(handle: *mut c_void, created: *mut FileTime, exited: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
    fn GetThreadTimes(handle: *mut c_void, created: *mut FileTime, exited: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
}
pub fn snapshot() -> [u64; 4] {
    let mut created=FileTime::default(); let mut exited=FileTime::default();
    let mut pk=FileTime::default(); let mut pu=FileTime::default();
    let mut tk=FileTime::default(); let mut tu=FileTime::default();
    // Pseudohandles identify this process/thread; all output buffers are live and correctly aligned.
    let process = unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut pk, &mut pu) };
    assert_ne!(process, 0, "process CPU observation failed: {}",std::io::Error::last_os_error());
    let thread = unsafe { GetThreadTimes(GetCurrentThread(), &mut created, &mut exited, &mut tk, &mut tu) };
    assert_ne!(thread, 0, "thread CPU observation failed: {}",std::io::Error::last_os_error());
    [pk.ticks(),pu.ticks(),tk.ticks(),tu.ticks()]
}
#[cfg(test)]
mod tests {
    #[test]
    fn filetime_and_monotonic_snapshots() {
        assert_eq!(super::FileTime {low:7, high:3}.ticks(), (3u64<<32)+7);
        let a=super::snapshot(); let b=super::snapshot();
        assert!(a.into_iter().zip(b).all(|(a,b)| b>=a));
    }
}
''',encoding='utf-8',newline='\n')
cli='omega-rust/omega/application/cli/src/main.rs';path=ROOT/cli;text=path.read_text()
text=text.replace('fn main() -> ExitCode {','''fn main() -> ExitCode {
    #[cfg(windows)]
    if std::env::var_os("OMEGA_SOURCE_CHECK_CPU").is_some_and(|value| value != "0") {
        phase_profile::install_cpu_sampler(artifacts::compile_timings::check_cpu::snapshot);
    }''',1)
path.write_text(text,encoding='utf-8',newline='\n')
tracked=[driver,profile,host,cli];new=[probe.relative_to(ROOT).as_posix()]
subprocess.run(['rustfmt','--edition','2024','--config','skip_children=true',*tracked,*new],check=True)
assert sorted(subprocess.check_output(['git','diff','--name-only','--','omega-rust'],text=True).splitlines())==sorted(tracked)
patch=subprocess.check_output(['git','diff','--','omega-rust'])
for name in new:
    lines=(ROOT/name).read_text().splitlines()
    patch+=('diff --git a/'+name+' b/'+name+'\nnew file mode 100644\n--- /dev/null\n+++ b/'+name+'\n@@ -0,0 +1,'+str(len(lines))+' @@\n'+''.join('+'+line+'\n' for line in lines)).encode()
(WORK/'diagnostic.patch').write_bytes(patch)
(WORK/'source.json').write_text(json.dumps(dict(base=base,patch_sha256=hashlib.sha256(patch).hexdigest(),files={f:hashlib.sha256((ROOT/f).read_bytes()).hexdigest() for f in tracked+new},new_files=new,selected_scopes=names),indent=2)+'\n',encoding='utf-8')
(WORK/'research.md').write_text('''# Source-checking CPU attribution research

Question: Which passes own the remaining source-checking process CPU on the merged source?
Fixed source: cec7b6657299525e62aa6fc2168d3768b4d33b6a plus archived temporary probes.
Fixed workload: authored Dungeon program and EOF, windows_x86_64.
Bound: six interleaved cold trials (off,on,on,off,on,on), two warm profiles, one BOTH control.
No optimization or before/after speedup claim. Keep all valid observations.

Query: site.learn.microsoft.com GetThreadTimes GetCurrentThread thread user kernel time FILETIME

Primary sources read:
- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getthreadtimes
- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentthread
- GetProcessTimes/GetSystemTimes research retained in ../graph_phase_cpu/research.md and ../cold_variation/research.md.

Thread times are kernel/user execution durations in 100ns units. Pseudohandles refer to the calling thread and need no close. Process times include other threads. Capture both; actual wall coordinates determine nested ownership. Subtract disjoint children only, report residuals, reject ambiguous overlap. CPU quantization is not zero work. API failure aborts observations. PSI retains forbid(unsafe_code); host artifacts owns FFI. No new dependencies.

Instrumentation boundary: selective coarse pass names only; ordinary fine wall profiling disabled. Same executable probe-off controls check gross observer disturbance, not a precise correction. Endpoint system CPU records external contention without pausing other work.
''',encoding='utf-8')
print('Prepared',len(names),'coarse scopes; base',base)
