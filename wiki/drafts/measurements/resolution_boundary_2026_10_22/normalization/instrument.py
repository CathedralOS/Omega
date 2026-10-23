"""Compare every normalized carrier against the frozen clone-based normalizer.

Temporary instrumentation only. The legacy source is read from the recorded
baseline revision, not reconstructed from the candidate. Save and restore the
candidate's original bytes; never discard its implementation edits.
"""
from pathlib import Path
import json
import subprocess

output = Path('build/resolved-normalization')
root = Path('omega-rust/psi/compilation/representations/symbol-resolved-trees/src')
owner = root/'symbol_resolved_trees.rs'
legacy = root/'symbol_resolved_trees/normalization_oracle.rs'
base = (output/'base.txt').read_text().strip()
legacy.write_bytes(subprocess.check_output(['git','show',base+':'+str(root/'symbol_resolved_trees/storage/tables.rs')]))
original = owner.read_text()
(output/'owner-before-instrumentation.rs').write_text(original)
text = original.replace('pub mod calls;', 'mod normalization_oracle;\npub mod calls;', 1)
for method, arguments in [('rebuild_tables',''), ('append_completed_body_tables','predecessor, completed_machines')]:
    start = text.index('    pub fn '+method+'(')
    body = text.index(' {\n',start)+3
    normalizer = 'from_body_completion' if arguments else 'from_symbol_resolved_trees_with_state_spans'
    suffix = ', '+arguments if arguments else ''
    setup = f'''        let measure = std::env::var_os("OMEGA_NORMALIZATION_VERIFY").is_some();
        let expected = if measure {{
            let mut expected = self.clone();
            let tables = normalization_oracle::SymbolResolvedTreeTables::{normalizer}(&mut expected{suffix});
            expected.tables.bodies.expressions = tables.bodies.expressions;
            expected.tables.bodies.statements = tables.bodies.statements;
            expected.tables.types.references = tables.types.references;
            Some(expected)
        }} else {{ None }};
        let removed_rows = self.tables.bodies.expressions.expression_count();
'''
    text = text[:body]+setup+text[body:]
    end = text.index('        self.tables.types.references = tables.types.references;',body)
    end += len('        self.tables.types.references = tables.types.references;')
    verify = f'''
        if let Some(expected) = expected {{
            assert!(self == &expected, "{method}: complete normalized carrier differs from baseline");
            eprintln!("NORMALIZATION\\t{method}\\t{{removed_rows}}\\tequal");
        }}'''
    text = text[:end]+verify+text[end:]
owner.write_text(text)
(output/'instrumentation.patch').write_bytes(subprocess.check_output(['git','diff','--',str(owner)]))
(output/'legacy-normalizer.rs').write_bytes(legacy.read_bytes())
print('Installed full-carrier normalization oracle',flush=True)
