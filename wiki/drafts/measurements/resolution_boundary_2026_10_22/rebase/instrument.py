"""Temporarily compare the full rebase Result with the frozen original methods."""
from pathlib import Path
import subprocess,json,hashlib
out=Path('build/resolved-rebase')
root=Path('omega-rust/psi/compilation/representations/symbol-resolved-trees/src')
base=(out/'base.txt').read_text().strip()
paths=[root/'symbol_resolved_trees.rs', root/'symbol_resolved_trees/values/expression.rs', root/'symbol_resolved_trees/control_flow/statement.rs']
originals={str(p):p.read_text() for p in paths}
(out/'before-instrumentation.json').write_text(json.dumps(originals,indent=2)+'\n')
def method(text,name):
    marker='fn '+name+'('
    position=text.index(marker)
    start=text.rfind('\n',0,position)+1
    end=text.index('\n    }',position)+6
    return text[start:end]+'\n\n'
legacy_sources={str(p):subprocess.check_output(['git','show',base+':'+str(p)],text=True) for p in paths}
for p in paths[1:]:
    original=legacy_sources[str(p)]
    text=originals[str(p)]
    legacy=method(original,'rebase_authored_selection_extension')
    insertion=text.index('    pub(crate) fn '+('prepare_authored_selection_rebase' if p.name=='expression.rs' else 'validate_authored_selection_rebase'))
    p.write_text(text[:insertion]+legacy+text[insertion:])
p=paths[0];text=originals[str(p)];original=legacy_sources[str(p)]
legacy=method(original,'rebase_authored_selection_extension').replace('pub fn rebase_authored_selection_extension(', 'fn rebase_authored_selection_extension_legacy(')
for name in ('rebase_occurrence_stores','rebase_tree_statement_occurrences','rebase_proof_fact_occurrences'):
    legacy+=method(original,name)
text=text.replace('pub fn rebase_authored_selection_extension(', 'fn rebase_authored_selection_extension_candidate(',1)
position=text.index('    fn rebase_authored_selection_extension_candidate(')
wrapper='''    pub fn rebase_authored_selection_extension(
        self,
        frontier: AuthoredSelectionExtensionFrontier,
        destination_base: &AuthoredDeclarationSelections,
    ) -> Result<Self, (Self, AuthoredSelectionExtensionRebaseError)> {
        if std::env::var_os("OMEGA_REBASE_VERIFY").is_none() {
            return self.rebase_authored_selection_extension_candidate(frontier, destination_base);
        }
        let rows = self.tables.bodies.expressions.expression_count();
        let expected = self.clone().rebase_authored_selection_extension_legacy(frontier, destination_base);
        let actual = self.rebase_authored_selection_extension_candidate(frontier, destination_base);
        assert!(actual == expected, "full transactional rebase result differs from baseline");
        let outcome = if actual.is_ok() { "accepted" } else { "rejected" };
        eprintln!("REBASE\\t{rows}\\t{outcome}\\tequal");
        actual
    }

'''
p.write_text(text[:position]+wrapper+legacy+text[position:])
(out/'instrumentation.patch').write_bytes(subprocess.check_output(['git','diff','--',*(str(p) for p in paths)]))
(out/'legacy-sources.json').write_text(json.dumps({name:hashlib.sha256(text.encode()).hexdigest() for name,text in legacy_sources.items()},indent=2)+'\n')
print('Installed complete success/failure rebase oracle',flush=True)
