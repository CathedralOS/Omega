from pathlib import Path
import hashlib,json,subprocess
ROOT=Path.cwd();WORK=ROOT/'build/proof-profile/lazy-reference-graph'
source=json.loads((WORK/'candidate-source.json').read_text());path=ROOT/source['file']
assert hashlib.sha256(path.read_bytes()).hexdigest()==source['source_sha256']
text=path.read_text()
needle='''        preserve_call_memberships(
            &self.module.structural_types,'''
replacement='''        let profiling = std::env::var_os("OMEGA_LAZY_REFERENCE_GRAPH_COUNTS").is_some();
        if profiling {
            eprintln!("LAZY_REFERENCE_READY\\t{:?}\\t{}", std::thread::current().id(), self.module.machines.len());
        }
        preserve_call_memberships(
            &self.module.structural_types,'''
assert text.count(needle)==1;text=text.replace(needle,replacement)
needle='''.get_or_init(|| descriptors::terminal_borrowed_storage_graph(self.module))'''
replacement='''.get_or_init(|| {
                        let graph = descriptors::terminal_borrowed_storage_graph(self.module);
                        if profiling {
                            eprintln!("LAZY_REFERENCE_GRAPH_BUILD\\t{:?}\\t{}\\t{}", std::thread::current().id(), self.module.machines.len(), graph.len());
                        }
                        graph
                    })'''
assert text.count(needle)==1;text=text.replace(needle,replacement)
path.write_text(text,encoding='utf-8',newline='\n')
subprocess.run(['rustfmt','--edition','2024','--config','skip_children=true',str(path)],check=True)
tracked=subprocess.check_output(['git','diff','--name-only','--','omega-rust'],text=True).splitlines()
untracked=subprocess.check_output(['git','ls-files','--others','--exclude-standard','--','omega-rust'],text=True).splitlines()
assert tracked==[source['file']] and not untracked
content=subprocess.check_output(['git','diff','--','omega-rust'])
for name in untracked:
    lines=(ROOT/name).read_text().splitlines();content+=('--- /dev/null\n+++ b/'+name+'\n@@ -0,0 +1,'+str(len(lines))+' @@\n'+''.join('+'+line+'\n' for line in lines)).encode()
(WORK/'diagnostic.patch').write_bytes(content)
(WORK/'diagnostic-source.json').write_text(json.dumps(dict(base=source['base'],semantic_base=source['semantic_base'],candidate_patch_sha256=source['patch_sha256'],patch_sha256=hashlib.sha256(content).hexdigest(),files=tracked+untracked),indent=2)+'\n',encoding='utf-8',newline='\n')
print('Prepared one-file counter overlay; pure candidate frozen separately')
