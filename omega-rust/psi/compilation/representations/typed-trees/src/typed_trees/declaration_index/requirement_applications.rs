//! Exact executable requirement symbols rejoin their retained applications.
//!
//! Signature lookup used to scan every applied callable row, including symbolic
//! trait views and machine selections, once per queried trait. This derived
//! index retains exact arena handles and the old first-match order. The sole
//! source arena revision covers insertion, replacement, removal and slot reuse;
//! payload projection still reads the current complete application record.

use super::relationships::SymbolRows;
use crate::typed_trees::callable_applications::AppliedCallableSignature;
use arena::Handle;
use symbols::SymbolHandle;

#[derive(Clone, Default)]
pub(super) struct RequirementApplicationIndex {
    rows: SymbolRows<Handle<AppliedCallableSignature>>,
    by_template: SymbolRows<Handle<AppliedCallableSignature>>,
}

impl RequirementApplicationIndex {
    pub(super) fn densify(&mut self) {
        self.rows.densify();
        self.by_template.densify();
    }
}

impl crate::TypedTrees {
    pub(super) fn with_requirement_application_index<R>(
        &self,
        run: impl FnOnce(&RequirementApplicationIndex) -> R,
    ) -> R {
        self.declaration_indexes.requirement_applications.with(
            self.applied_callable_signatures.revision(),
            || {
                let mut rows = Vec::new();
                let mut by_template = Vec::new();
                for (handle, application) in self.applied_callable_signatures.iter() {
                    if let Some(symbol) = application.requirement_instance_symbol() {
                        rows.push((symbol, handle));
                    }
                    if let Some(template) = application.requirement_template_symbol() {
                        by_template.push((template, handle));
                    }
                }
                RequirementApplicationIndex {
                    rows: SymbolRows::new(rows),
                    by_template: SymbolRows::new(by_template),
                }
            },
            run,
        )
    }

    pub(crate) fn closed_requirement_signature(
        &self,
        symbol: SymbolHandle,
    ) -> Option<&AppliedCallableSignature> {
        let handle =
            self.with_requirement_application_index(|index| index.rows.first(symbol).copied())?;
        Some(self.applied_callable_signatures.get(handle))
    }

    pub(crate) fn closed_requirement_instance_handles_for_template(
        &self,
        template: SymbolHandle,
    ) -> Vec<Handle<AppliedCallableSignature>> {
        self.with_requirement_application_index(|index| index.by_template.get(template).to_vec())
    }
}
