//! Exact executable requirement symbols rejoin their retained applications.
//!
//! Signature lookup used to scan every applied callable row, including symbolic
//! trait views and machine selections, once per queried trait. This derived
//! index retains exact arena handles and the old first-match order. The sole
//! source arena revision covers insertion, replacement, removal and slot reuse;
//! payload projection still reads the current complete application record.

use super::relationships::{first_symbol_row, sort_symbol_rows};
use crate::typed_trees::callable_applications::AppliedCallableSignature;
use arena::Handle;
use symbols::SymbolHandle;

#[derive(Clone, Default)]
pub(super) struct RequirementApplicationIndex {
    rows: Vec<(SymbolHandle, Handle<AppliedCallableSignature>)>,
}

impl crate::TypedTrees {
    pub(crate) fn closed_requirement_signature(
        &self,
        symbol: SymbolHandle,
    ) -> Option<&AppliedCallableSignature> {
        let handle = self.declaration_indexes.requirement_applications.with(
            self.applied_callable_signatures.revision(),
            || {
                let mut rows = self
                    .applied_callable_signatures
                    .iter()
                    .filter_map(|(handle, application)| {
                        application
                            .requirement_instance_symbol()
                            .map(|symbol| (symbol, handle))
                    })
                    .collect::<Vec<_>>();
                sort_symbol_rows(&mut rows);
                RequirementApplicationIndex { rows }
            },
            |index| first_symbol_row(&index.rows, symbol).map(|(_, handle)| *handle),
        )?;
        Some(self.applied_callable_signatures.get(handle))
    }
}
