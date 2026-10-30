from pathlib import Path
import hashlib,json,subprocess
ROOT=Path.cwd();OUT=ROOT/'build/proof-profile';WORK=OUT/'same-binary-graph';WORK.mkdir(exist_ok=False)
assert not subprocess.check_output(['git','status','--porcelain']).strip()
base=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
subprocess.run(['git','apply','wiki/drafts/measurements/proof_parallelism_2026_10_28/lazy_reference_graph/candidate.patch'],check=True)
name='omega-rust/psi/compilation/representations/terminal-psi/semantics/src/reference_origins/memberships.rs';path=ROOT/name
text=path.read_text()
needle='''        preserve_call_memberships(
            &self.module.structural_types,
            &self.module.structural_domains,
            || {
                self.graph
                    .get_or_init(|| descriptors::terminal_borrowed_storage_graph(self.module))
                    .as_slice()
            },'''
replacement='''        // Diagnostic control: both modes use this exact executable and query kernel.
        let eager = std::env::var_os("OMEGA_REFERENCE_GRAPH_EXPERIMENT_MODE")
            .is_some_and(|mode| mode == "eager");
        let counts = std::env::var_os("OMEGA_REFERENCE_GRAPH_EXPERIMENT_COUNTS").is_some();
        if counts {
            eprintln!("SAME_GRAPH_READY\\t{}\\t{}", if eager { "eager" } else { "lazy" }, self.module.machines.len());
        }
        let graph = || {
            self.graph.get_or_init(|| {
                let graph = descriptors::terminal_borrowed_storage_graph(self.module);
                if counts {
                    eprintln!("SAME_GRAPH_BUILD\\t{}\\t{}", self.module.machines.len(), graph.len());
                }
                graph
            }).as_slice()
        };
        if eager {
            let _ = graph();
        }
        preserve_call_memberships(
            &self.module.structural_types,
            &self.module.structural_domains,
            graph,'''
assert text.count(needle)==1;text=text.replace(needle,replacement)
path.write_text(text,encoding='utf-8',newline='\n')
subprocess.run(['rustfmt','--edition','2024','--config','skip_children=true',str(path)],check=True)
assert subprocess.check_output(['git','diff','--name-only','--','omega-rust'],text=True).splitlines()==[name]
patch=subprocess.check_output(['git','diff','--','omega-rust']);(WORK/'diagnostic.patch').write_bytes(patch)
(WORK/'source.json').write_text(json.dumps(dict(base=base,semantic_base='456c5e984452a45d977295e254b3ca9ae3cc7750',file=name,
    source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),patch_sha256=hashlib.sha256(patch).hexdigest()),indent=2)+'\n',encoding='utf-8',newline='\n')
print('Prepared single-file eager/lazy diagnostic and exact patch.')
