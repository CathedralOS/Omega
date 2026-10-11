//! Prepare and check one complete typed program under fresh invocation authority.
//!
//! Product and Build consumers supply admission data, not a preparation/checking
//! sequence. Retained language judgments are matched here against their exact
//! preparation inputs; package authority is admitted even when work is reused.
//! Product root selection consumes the successful result, so it cannot hide an
//! invalid authored body. Host effects and provider dispatch are not inputs.

use diagnostics::Diagnostic;
use std::sync::Arc;

pub use typed_trees_to_checked_trees::{CheckedTreeProductSelection, RetainedCheck};

/// Evaluated source permissions and representation facts for this invocation.
/// These are not provider selections or execution authority.
#[derive(Default)]
pub struct SourceAdmission<'request> {
    pub opaque_properties: &'request [validation::OpaqueDataPropertyReceipt],
    pub asm_authority: Option<validation::AsmAuthorityAdmission>,
    pub wire_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    pub declarations: Option<&'request dyn validation::DeclarationSelectionAuthority>,
}

/// Consume a complete typed program. Previous judgments belong to the same
/// invocation; the first is the incremental predecessor, followed by siblings.
pub struct CheckRequest<'request> {
    pub program: typed_trees::TypedTrees,
    pub admission: SourceAdmission<'request>,
    pub previous: &'request [&'request RetainedCheck],
    pub retain: bool,
}

/// Successful language judgment, before optional whole-product root selection.
pub struct CheckedSource {
    pub program: checked_trees::CheckedTrees,
    pub retained: Option<RetainedCheck>,
    pub reused: bool,
}

/// Shared source product with the exact optional root-selection evidence.
pub struct SelectedCheckedSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    pub retained: Option<RetainedCheck>,
    pub selection: Option<CheckedTreeProductSelection>,
}

impl CheckRequest<'_> {
    pub fn check(self) -> Result<CheckedSource, Vec<Diagnostic>> {
        let mut preparation = typed_trees_to_typed_trees::PreparationRequest::settled()
            .with_opaque_property_receipts(self.admission.opaque_properties)
            .with_wire_compatibility_demands(self.admission.wire_demands);
        if let Some(authority) = self.admission.asm_authority {
            preparation = preparation.with_asm_authority(authority);
        }
        let checking = typed_trees_to_checked_trees::CheckingRequest::new()
            .with_declaration_authority(self.admission.declarations);
        let admission = preparation.admission();
        let matching = self
            .previous
            .iter()
            .copied()
            .find(|prior| prior.judges_preparation_input(&self.program, &admission));
        let outcome = if let Some(prior) = matching {
            typed_trees_to_checked_trees::reuse_retained_check(prior, &checking, self.retain)?
        } else {
            let retained_input = self
                .retain
                .then(|| (Arc::new(self.program.clone()), admission));
            let (program, preparation) =
                typed_trees_to_typed_trees::prepare_typed_trees(self.program, &preparation)?;
            typed_trees_to_checked_trees::lower_typed_trees_reusing(
                program,
                preparation,
                retained_input,
                &checking,
                self.previous.first().copied(),
                self.retain,
            )?
        };
        Ok(CheckedSource {
            program: outcome.checked,
            retained: outcome.retained,
            reused: outcome.took_prior,
        })
    }
}

impl CheckedSource {
    /// Select all authored Product roots after checking, or retain the complete
    /// checked program when no pruning was requested. An explicit empty root set
    /// remains an invalid request, not permission to skip checking or selection.
    pub fn select_product(
        self,
        roots: Option<Vec<symbols::SymbolHandle>>,
    ) -> Result<SelectedCheckedSource, Vec<Diagnostic>> {
        let (program, selection) = typed_trees_to_checked_trees::select_checked_tree_product(
            Arc::new(self.program),
            roots,
        )?;
        Ok(SelectedCheckedSource {
            program,
            retained: self.retained,
            selection,
        })
    }
}
