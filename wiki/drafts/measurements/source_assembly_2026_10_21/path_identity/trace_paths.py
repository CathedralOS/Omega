from pathlib import Path
import subprocess,shutil,json
f=Path('build/path-identity');queue=Path('omega-rust/omega/packages/compilation-inputs/src/source_imports/import_queue.rs');lex=Path('omega-rust/psi/compilation/pipeline/00_source-files-to-tokens/src/lex_sources.rs');parse=Path('omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/parser.rs');frontier=Path('omega-rust/omega/packages/compilation-inputs/src/source_imports/frontier.rs');harness=f/'harness/src/main.rs'
original={p:p.read_bytes() for p in [queue,lex,parse,frontier,harness]}
try:
 for p,expression,label in [(lex,'loaded_source.path.clone()','lex'),(parse,'lexed_source.path.clone()','parse'),(frontier,'parsed_source.path.clone()','storage')]:
  s=p.read_text().replace(expression,'trace_path_copy(&'+expression.split('.path')[0]+'.path)')
  s+='\nfn trace_path_copy(path: &std::path::PathBuf) -> std::path::PathBuf { eprintln!("COPY\\t'+label+'\\t{}\\t{}",path.as_os_str().len(),path.capacity());path.clone() }\n';p.write_text(s)
 s=queue.read_text().replace('HashMap<(PathBuf, DependencyScope), ()>','HashMap<(PathBuf, DependencyScope), (), TraceHashing>')
 s=s.replace('        self.seen.insert((path.to_path_buf(), scope), ());','        eprintln!("COPY\\tmark\\t{}\\t{}",path.as_os_str().len(),path.as_os_str().len());\n        self.seen.insert((path.to_path_buf(), scope), ());')
 s=s.replace('            self.pending.push_back(entry.key().clone());','            eprintln!("COPY\\tqueue\\t{}\\t{}",entry.key().0.as_os_str().len(),entry.key().0.capacity());\n            self.pending.push_back(entry.key().clone());')
 s+='''
use std::hash::{BuildHasher, Hasher};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
#[derive(Debug, Default)]
struct TraceHashing { state: std::collections::hash_map::RandomState, hashes: AtomicUsize, bytes: Arc<AtomicUsize> }
struct TraceHasher { inner: std::collections::hash_map::DefaultHasher, bytes: Arc<AtomicUsize> }
impl BuildHasher for TraceHashing {
 type Hasher=TraceHasher;
 fn build_hasher(&self)->TraceHasher { self.hashes.fetch_add(1,Ordering::Relaxed);TraceHasher{inner:self.state.build_hasher(),bytes:self.bytes.clone()} }
}
impl Hasher for TraceHasher {
 fn finish(&self)->u64 {self.inner.finish()}
 fn write(&mut self,bytes:&[u8]) {self.bytes.fetch_add(bytes.len(),Ordering::Relaxed);self.inner.write(bytes)}
'''
 for ty in ['u8','u16','u32','u64','u128','usize','i8','i16','i32','i64','i128','isize']:
  s+=f'fn write_{ty}(&mut self,value:{ty}) {{self.bytes.fetch_add(std::mem::size_of::<{ty}>(),Ordering::Relaxed);self.inner.write_{ty}(value)}}\n'
 s+='}\nimpl Drop for ImportQueue {fn drop(&mut self) {eprintln!("HASH\\t{}\\t{}",self.seen.hasher().hashes.load(Ordering::Relaxed),self.seen.hasher().bytes.load(Ordering::Relaxed));}}\n';queue.write_text(s)
 s=harness.read_text().replace('  COUNTS.with(|state|state.set((true,0,0)));','  eprintln!("START\\t{}",path.display());\n  COUNTS.with(|state|state.set((true,0,0)));').replace('  let mut root_metrics=', '  eprintln!("END");\n  let mut root_metrics=');harness.write_text(s)
 with (f/'build-trace.log').open('w') as out:subprocess.run(['mbx','build','--manifest-path',str(f/'harness/Cargo.toml'),'--bin','assembly-measurement'],stdout=out,stderr=subprocess.STDOUT,check=True)
 shutil.copy2(f/'harness/target/debug/assembly-measurement',f/'harness-trace')
 with (f/'trace.tsv').open('w') as out,(f/'trace.stderr').open('w') as err:subprocess.run([str(f/'harness-trace'),str(f/'inputs.txt')],stdout=out,stderr=err,check=True)
finally:
 for p,b in original.items():p.write_bytes(b)
rows=[];current=None
for line in (f/'trace.stderr').read_text().splitlines():
 fields=line.split('\t')
 if fields[0]=='START':current=dict(path=fields[1],copies={},hashes=0,hash_bytes=0)
 elif fields[0]=='END':rows.append(current);current=None
 elif current is not None:
  if fields[0]=='HASH':current['hashes']+=int(fields[1]);current['hash_bytes']+=int(fields[2])
  elif fields[0]=='COPY':
   v=current['copies'].setdefault(fields[1],dict(calls=0,bytes=0,capacity=0));v['calls']+=1;v['bytes']+=int(fields[2]);v['capacity']+=int(fields[3])
assert len(rows)==289
summary=dict(hashes=sum(r['hashes'] for r in rows),hash_bytes=sum(r['hash_bytes'] for r in rows),copies={label:{key:sum(r['copies'].get(label,{}).get(key,0) for r in rows) for key in ['calls','bytes','capacity']} for label in ['lex','parse','storage','queue','mark']})
r=dict(summary=summary,length=[r for r in rows if '/length/local/' in r['path']],rows=rows);(f/'path-trace.json').write_text(json.dumps(r,indent=2)+'\n');print(summary);print(r['length'])
