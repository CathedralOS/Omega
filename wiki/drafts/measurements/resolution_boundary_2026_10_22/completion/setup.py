from pathlib import Path
import subprocess
out=Path('build/deferred-completion-storage')
root=Path('omega-rust/psi/compilation/representations/symbol-resolved-trees/src')
base=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
(out/'base.txt').write_text(base+'\n')
owner=root/'symbol_resolved_trees.rs'; tables=root/'symbol_resolved_trees/storage/tables.rs'
(out/'owner-original.rs').write_bytes(owner.read_bytes())
(out/'tables-original.rs').write_bytes(tables.read_bytes())
original=tables.read_text()
text=original.replace('    /// successor expression table already contains newly resolved body graphs;\n    /// only normalized statements/types are seeded from the predecessor.','    /// successor owns a clone of the predecessor, extended with resolved body\n    /// graphs in declaration storage. Its normalized statements and types are\n    /// still the exact predecessor tables: final routing runs after this append.\n    /// Transfer those owned tables instead of copying the predecessor twice.')
text=text.replace('statements: predecessor.tables.bodies.statements.clone(),','statements: std::mem::take(&mut symbol_resolved_trees.tables.bodies.statements),').replace('references: predecessor.tables.types.references.clone(),','references: std::mem::take(&mut symbol_resolved_trees.tables.types.references),')
tables.write_text(text)
(out/'candidate.patch').write_bytes(subprocess.check_output(['git','diff','--','omega-rust']))
legacy=root/'symbol_resolved_trees/completion_oracle.rs';legacy.write_text(original)
text=owner.read_text().replace('pub mod calls;', 'mod completion_oracle;\npub mod calls;',1)
start=text.index('    pub fn append_completed_body_tables('); body=text.index(' {\n',start)+3
setup='''        let baseline = std::env::var("OMEGA_COMPLETION_EXPERIMENT").unwrap_or_default() == "baseline";
        let statements = self.tables.bodies.statements.statement_count();
        let references = self.tables.types.references.type_reference_count();
        assert!(self.tables.bodies.statements == predecessor.tables.bodies.statements, "completion changed retained statement table");
        assert!(self.tables.types.references == predecessor.tables.types.references, "completion changed retained type table");
        let mut expected = self.clone();
        let previous = completion_oracle::SymbolResolvedTreeTables::from_body_completion(&mut expected, predecessor, completed_machines);
        expected.tables.bodies.expressions = previous.bodies.expressions;
        expected.tables.bodies.statements = previous.bodies.statements;
        expected.tables.types.references = previous.types.references;
'''
text=text[:body]+setup+text[body:]
end=text.index('        self.tables.types.references = tables.types.references;',body)+len('        self.tables.types.references = tables.types.references;')
text=text[:end]+'''
        assert!(self == &expected, "completion changed complete normalized carrier");
        eprintln!("COMPLETION\\t{}\\t{statements}\\t{references}\\tequal", completed_machines.len());
        if baseline { *self = expected; }
'''+text[end:]
owner.write_text(text)
(out/'legacy-normalizer.rs').write_text(original)
(out/'instrumented.patch').write_bytes(subprocess.check_output(['git','diff','--','omega-rust']))
