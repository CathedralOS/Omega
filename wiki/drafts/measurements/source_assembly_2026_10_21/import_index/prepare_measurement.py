from pathlib import Path
f=Path('build/import-index');p=f/'harness/src/bin/all_threads.rs';p.parent.mkdir(exist_ok=True)
s=(f/'harness/src/main.rs').read_text()
s=s.replace('thread_local! {static COUNTS:Cell<(bool,u64,u64)> = const { Cell::new((false,0,0)) };}', 'use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};\nstatic ENABLED:AtomicBool=AtomicBool::new(false); static CALLS:AtomicU64=AtomicU64::new(0); static BYTES:AtomicU64=AtomicU64::new(0);')
s=s.replace('fn count(bytes:usize) {COUNTS.with(|state| {let (enabled,calls,total)=state.get();if enabled {state.set((true,calls+1,total+bytes as u64));}});}', 'fn count(bytes:usize) {if ENABLED.load(Ordering::Relaxed) {CALLS.fetch_add(1,Ordering::Relaxed);BYTES.fetch_add(bytes as u64,Ordering::Relaxed);}}')
s=s.replace('COUNTS.with(|state|state.set((true,0,0)));','CALLS.store(0,Ordering::Relaxed);BYTES.store(0,Ordering::Relaxed);ENABLED.store(true,Ordering::Relaxed);')
s=s.replace('let counts=COUNTS.with(|state|{let result=state.get();state.set((false,0,0));result});','ENABLED.store(false,Ordering::Relaxed);let counts=(false,CALLS.load(Ordering::Relaxed),BYTES.load(Ordering::Relaxed));')
s=s.replace(' for path in paths.lines(){', ' let warm_path=PathBuf::from(paths.lines().next().unwrap()).canonicalize().unwrap(); let mut warm_timings=CompileTimings::default(); black_box(ImmutableSourceParseCheckpoint::prepare(&warm_path,None,false,&mut warm_timings).and_then(|checkpoint|checkpoint.assemble(None,&mut warm_timings))).ok();\n for path in paths.lines(){')
p.write_text(s)
