from pathlib import Path
import subprocess,shutil,json
out=Path('build/import-assembly')
def run(command,label):
 with (out/label).open('w') as log: result=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT)
 print(label,result.returncode,flush=True);assert result.returncode==0,label

def census(name,shared=False):
 suffix='-shared' if shared else ''
 command=[str(out/f'harness-{name}'),str(out/'inputs.txt')]+(['--shared'] if shared else [])
 with (out/f'{name}{suffix}.tsv').open('w') as stdout,(out/f'{name}{suffix}-roots.tsv').open('w') as stderr:
  result=subprocess.run(command,stdout=stdout,stderr=stderr)
 assert result.returncode==0
 print(name,suffix,'census finished',flush=True)
shutil.copy2(out/'harness/target/debug/assembly-measurement',out/'harness-baseline')
census('baseline');census('baseline',True)
p=Path('omega-rust/omega/packages/compilation-inputs/src/source_imports/frontier.rs')
s=p.read_text().replace('        parsed: ParsedSources,\n        resolution_stratum:', '        mut parsed: ParsedSources,\n        resolution_stratum:')
s=s.replace('for parsed_source in parsed.sources.span_or_empty(parsed.batch) {','for parsed_source in parsed.sources.span_mut_or_empty(parsed.batch) {')
s=s.replace('            self.files.append(SourceFile {','            // Import discovery has finished with this consumed batch. Keep its\n            // per-source associations without copying their owned buffers.\n            self.files.append(SourceFile {')
s=s.replace('                path: parsed_source.path.clone(),\n                root_items: parsed_source.root_items.clone(),','                path: std::mem::take(&mut parsed_source.path),\n                root_items: std::mem::take(&mut parsed_source.root_items),')
p.write_text(s)
run(['rustfmt','--edition','2024',str(p)],'format.log')
run(['mbx','build','-p','omega'],'build-candidate.log')
shutil.copy2('target/debug/omega',out/'omega-candidate')
run(['mbx','build','--manifest-path',str(out/'harness/Cargo.toml')],'build-harness-candidate.log')
shutil.copy2(out/'harness/target/debug/assembly-measurement',out/'harness-candidate')
census('candidate');census('candidate',True)
for suffix in ['', '-shared']:
 before=[row.split('\t') for row in (out/f'baseline{suffix}.tsv').read_text().splitlines()]
 after=[row.split('\t') for row in (out/f'candidate{suffix}.tsv').read_text().splitlines()]
 assert len(before)==len(after)==289
 assert all(left[3:]==right[3:] for left,right in zip(before,after))
 changed=[dict(path=l[-1],calls=int(l[0])-int(r[0]),bytes=int(l[1])-int(r[1]),status=l[3]) for l,r in zip(before,after)]
 result=dict(inputs=len(before),assembled=sum(row[3]=='assembled' for row in before),rejected=sum(row[3]=='rejected' for row in before),calls_removed=sum(row['calls'] for row in changed),bytes_removed=sum(row['bytes'] for row in changed),changed=changed)
 (out/f'comparison{suffix}.json').write_text(json.dumps(result,indent=2)+'\n')
 print({key:value for key,value in result.items() if key!='changed'},flush=True)
