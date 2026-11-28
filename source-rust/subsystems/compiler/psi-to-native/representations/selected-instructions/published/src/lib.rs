//! The completed selected-program handoff to register allocation.
//! Shared analyses consume only its facts, without body publication metadata.
#![forbid(unsafe_code)]
pub use selected_analysis_facts::{SelectedFacts, SelectedFactsParts};
use std::sync::Arc;

/// Only selected optimization may issue the completed input to allocation.
/// ```compile_fail,E0277
/// fn bypass(parts: selected_body::SelectedFactsParts) -> selected_body::SelectedBody {
///     parts.into()
/// }
/// ```
#[derive(Debug)]
pub struct SelectedBody {
    body: Arc<physical_program::NativeBodyFacts>,
    facts: SelectedFacts,
}
impl SelectedBody {
    pub fn new(
        issuer: selected_body_issuer::SelectedBodyIssuer,
        body: Arc<physical_program::NativeBodyFacts>,
        facts: SelectedFactsParts,
    ) -> Self {
        Self {
            body,
            facts: SelectedFacts::new(issuer, facts),
        }
    }
    pub fn into_parts(self) -> (Arc<physical_program::NativeBodyFacts>, SelectedFacts) {
        (self.body, self.facts)
    }
}
