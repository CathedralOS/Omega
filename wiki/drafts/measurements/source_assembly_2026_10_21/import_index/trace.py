from pathlib import Path
import subprocess,shutil,json
f=Path('build/import-index');root=Path('omega-rust/omega/packages/compilation-inputs/src');paths=[root/'source_imports.rs',root/'source_imports/import_queue.rs'];originals={p:p.read_bytes() for p in paths}
try:
 p=paths[0];s=p.read_text();s=s.replace('        for (ordinal, root_item) in parsed_source.root_items.iter().enumerate() {','        let import_ordinals = parsed_source.root_items.iter().enumerate().filter_map(|(ordinal, item)| matches!(syntax_trees.root_item(*item), Item::Use(_)).then_some(ordinal)).collect::<Vec<_>>(); eprintln!("DISCOVERY\\t{}\\t{}\\t{}", parsed_source.root_items.len(), import_ordinals.len(), import_ordinals.last().zip(import_ordinals.first()).map_or(0, |(last, first)| last-first+1));\n        for (ordinal, root_item) in parsed_source.root_items.iter().enumerate() {');p.write_text(s)
 p=paths[1];s=p.read_text().replace('        self.seen.insert((path.to_path_buf(), scope), ());','        eprintln!("QUEUE_MARK\\t{}",path.as_os_str().len());\n        self.seen.insert((path.to_path_buf(), scope), ());').replace('        if let Entry::Vacant(entry) = self.seen.entry((path, scope)) {','        eprintln!("QUEUE_PUSH\\t{}",path.as_os_str().len());\n        if let Entry::Vacant(entry) = self.seen.entry((path, scope)) {\n            eprintln!("QUEUE_NEW\\t{}",entry.key().0.as_os_str().len());');p.write_text(s)
 with (f/'build-trace.log').open('w') as out:subprocess.run(['mbx','build','--manifest-path',str(f/'harness/Cargo.toml'),'--bin','assembly-measurement'],stdout=out,stderr=subprocess.STDOUT,check=True)
 shutil.copy2(f/'harness/target/debug/assembly-measurement',f/'harness-trace')
 with (f/'trace.stdout').open('w') as out,(f/'trace.stderr').open('w') as err:subprocess.run([str(f/'harness-trace'),str(f/'inputs.txt')],stdout=out,stderr=err,check=True)
finally:
 for p,data in originals.items():p.write_bytes(data)
rows=[];counts=dict(roots=0,imports=0,range_roots=0,pushes=0,push_path_bytes=0,unique=0,unique_path_bytes=0,marks=0,mark_path_bytes=0)
for line in (f/'trace.stderr').read_text().splitlines():
 parts=line.split('\t')
 if parts[0]=='DISCOVERY':counts['roots']+=int(parts[1]);counts['imports']+=int(parts[2]);counts['range_roots']+=int(parts[3])
 if parts[0] in ['QUEUE_PUSH','QUEUE_NEW','QUEUE_MARK']:
  a,b={'QUEUE_PUSH':('pushes','push_path_bytes'),'QUEUE_NEW':('unique','unique_path_bytes'),'QUEUE_MARK':('marks','mark_path_bytes')}[parts[0]];counts[a]+=1;counts[b]+=int(parts[1])
 if parts[0]=='ROOTS':rows.append(dict(path=parts[-1],**counts));counts=dict.fromkeys(counts,0)
assert len(rows)==289
summary=dict(totals={k:sum(r[k] for r in rows) for k in counts},length=[r for r in rows if '/length/local/' in r['path']],rows=rows)
(f/'trace.json').write_text(json.dumps(summary,indent=2)+'\n');print({k:v for k,v in summary.items() if k!='rows'},flush=True)
