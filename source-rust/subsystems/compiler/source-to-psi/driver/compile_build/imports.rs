//! Bind each unavailable Build import to the declarations that require it.
use super::PendingBuildImport;
use diagnostics::Diagnostic;
use source_text::SourceSpan;
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Default)]
pub(in crate::driver) struct DeferredBuildImports {
    by_declaration: HashMap<SourceSpan, Vec<usize>>,
    imports: Vec<PendingBuildImport>,
}

impl DeferredBuildImports {
    pub(super) fn new(imports: Vec<PendingBuildImport>) -> Result<Self, Vec<Diagnostic>> {
        let mut by_declaration: HashMap<_, Vec<_>> = HashMap::new();
        let mut errors = Vec::new();
        for (index, import) in imports.iter().enumerate() {
            if import.declarations.is_empty() {
                errors.push(import.refusal());
            }
            for span in &import.declarations {
                by_declaration
                    .entry(SourceSpan::new(import.source, *span))
                    .or_default()
                    .push(index);
            }
        }
        if errors.is_empty() {
            Ok(Self {
                by_declaration,
                imports,
            })
        } else {
            Err(errors)
        }
    }

    pub(in crate::driver) fn require(
        &self,
        declarations: impl Iterator<Item = SourceSpan>,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.imports.is_empty() {
            return Ok(());
        }
        let missing: BTreeSet<_> = declarations
            .filter_map(|span| self.by_declaration.get(&span))
            .flatten()
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing
                .into_iter()
                .map(|&index| self.imports[index].refusal())
                .collect())
        }
    }
}

impl PendingBuildImport {
    fn refusal(&self) -> Diagnostic {
        let message = if self.target.root.is_none() {
            format!("no admitted logical file `{}`", self.target.path)
        } else {
            format!("Build import `{}` has no supplied source", self.target)
        };
        Diagnostic::error(message).with_source_span(SourceSpan::new(self.source, self.span))
    }
}
