from pathlib import Path
import hashlib,json,subprocess
ROOT=Path.cwd();OUT=ROOT/'build/proof-profile';WORK=OUT/'graph-phase-cpu';WORK.mkdir(exist_ok=False)
assert not subprocess.check_output(['git','status','--porcelain']).strip()
base=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
subprocess.run(['git','apply','wiki/drafts/measurements/proof_parallelism_2026_10_28/same_binary_graph/diagnostic.patch'],check=True)
member=json.loads((ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/same_binary_graph/source.json').read_text())
assert hashlib.sha256((ROOT/member['file']).read_bytes()).hexdigest()==member['source_sha256']
name='omega-rust/omega/foundation/artifacts/src/compile_timings/mod.rs';path=ROOT/name;text=path.read_text()
text=text.replace('pub mod counters;','pub mod counters;\n\n#[cfg(windows)]\nmod cpu_probe;',1)
needle='''        let time_start = Instant::now();
        let result = work();
        let microseconds = time_start.elapsed().as_micros();'''
replacement='''        #[cfg(windows)]
        let cpu_window = cpu_probe::begin(stage);
        let time_start = Instant::now();
        let result = work();
        let microseconds = time_start.elapsed().as_micros();
        #[cfg(windows)]
        cpu_probe::finish(cpu_window);'''
assert text.count(needle)==1;path.write_text(text.replace(needle,replacement),encoding='utf-8',newline='\n')
probe=path.with_name('cpu_probe.rs')
assert not probe.exists()
probe.write_text('''//! Temporary process-CPU observations; never an input to a compiler judgment.
use super::StageMeta;
use std::ffi::c_void;
use std::sync::OnceLock;
use std::time::Instant;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct FileTime { low: u32, high: u32 }
impl FileTime { fn ticks(self) -> u64 { (u64::from(self.high) << 32) | u64::from(self.low) } }

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn GetProcessTimes(process: *mut c_void, created: *mut FileTime, exited: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
}

#[derive(Clone, Copy)]
struct Snapshot { wall: u128, kernel: u64, user: u64 }
static ORIGIN: OnceLock<Instant> = OnceLock::new();

fn snapshot() -> Snapshot {
    let origin = ORIGIN.get_or_init(Instant::now);
    let mut created = FileTime::default();
    let mut exited = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    // The current-process pseudohandle remains valid; all four output buffers are live.
    let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user) };
    assert_ne!(ok, 0, "CPU boundary observation failed: {}", std::io::Error::last_os_error());
    Snapshot { wall: origin.elapsed().as_nanos(), kernel: kernel.ticks(), user: user.ticks() }
}

pub(super) struct Window { stage: &'static str, start: Snapshot }

pub(super) fn begin(stage: StageMeta) -> Option<Window> {
    if !matches!(stage.name, "Stage 05" | "terminal-production" | "terminal-production/lowering")
        || std::env::var_os("OMEGA_GRAPH_PHASE_CPU").is_none() { return None; }
    Some(Window { stage: stage.name, start: snapshot() })
}

pub(super) fn finish(window: Option<Window>) {
    if let Some(window) = window {
        let end = snapshot();
        eprintln!("GRAPH_CPU\\t{}\\t{:?}\\t{}\\t{}\\t{}\\t{}\\t{}\\t{}", window.stage, std::thread::current().id(), window.start.wall, end.wall, window.start.kernel, window.start.user, end.kernel, end.user);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn filetime_preserves_both_words() {
        assert_eq!(super::FileTime { low: 7, high: 3 }.ticks(), (3u64 << 32) + 7);
    }
    #[test]
    fn process_snapshots_are_monotonic() {
        let first = super::snapshot();
        let second = super::snapshot();
        assert!(second.wall >= first.wall && second.kernel >= first.kernel && second.user >= first.user);
    }
}
''',encoding='utf-8',newline='\n')
subprocess.run(['rustfmt','--edition','2024','--config','skip_children=true',str(path),str(probe)],check=True)
tracked=subprocess.check_output(['git','diff','--name-only','--','omega-rust'],text=True).splitlines();untracked=subprocess.check_output(['git','ls-files','--others','--exclude-standard','--','omega-rust'],text=True).splitlines()
assert sorted(tracked)==sorted([member['file'],name]) and untracked==[probe.relative_to(ROOT).as_posix()]
patch=subprocess.check_output(['git','diff','--','omega-rust'])
for file in untracked:
    lines=(ROOT/file).read_text().splitlines()
    patch+=('diff --git a/'+file+' b/'+file+'\nnew file mode 100644\n--- /dev/null\n+++ b/'+file+'\n@@ -0,0 +1,'+str(len(lines))+' @@\n'+''.join('+'+line+'\n' for line in lines)).encode()
(WORK/'diagnostic.patch').write_bytes(patch)
(WORK/'source.json').write_text(json.dumps(dict(base=base,semantic_base=member['semantic_base'],membership_unchanged_sha256=member['source_sha256'],
    patch_sha256=hashlib.sha256(patch).hexdigest(),files={f:hashlib.sha256((ROOT/f).read_bytes()).hexdigest() for f in tracked+untracked},new_files=untracked),indent=2)+'\n',encoding='utf-8',newline='\n')
print('Prepared exact same graph kernel plus host-recorder CPU probes; no unsafe prohibition or dependency changed.')
