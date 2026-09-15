//! N6 proof quotient migration boundary.
//!
//! Quotient formation selects one exact proposition relation and one exact
//! named `Equivalence` conformance from the declaration's static `where`
//! surface. Structural law-machine discovery is not an authority path.
//!
//! Sealed `Quotient::lift`/`define` requests retain their exact source-selected
//! operation and conformance identities, but this module rejects executable
//! admission until formation, correspondence, and contract obligations are
//! checked. Bare representative calls never discover structural respect proof
//! machines and never acquire lift authority.
//!
//! This file validates quotient formations and extracts correspondences.
//! `formation_collection.rs` collects validated formations and rejects
//! operation requests, `equivalence_selection.rs` validates equivalence
//! selections and exact relation applications, `legacy_candidates.rs`
//! recognizes legacy quotient call candidates and `carrier_fence.rs`,
//! `relation_plan.rs` and `terminal_bridge.rs` carry the fence, plan and
//! bridge; `tests.rs` holds the quotient tests.

mod carrier_fence;
mod equivalence_selection;
mod formation_collection;
mod legacy_candidates;
mod relation_plan;
mod terminal_bridge;
#[cfg(test)]
mod tests;

pub(crate) use equivalence_selection::exact_relation_application_matches;
pub(crate) use legacy_candidates::{
    legacy_attached_quotient_call_candidate, legacy_quotient_call_candidate,
};

use diagnostics::Diagnostic;
use std::collections::HashSet;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::proof_only::ProofOnlyClassification;
use typed_trees::types::TypeReferenceHandle;

use crate::proof_contracts::quotients::formation_collection::{
    collect_validated_quotient_formations, reject_quotient_operation_requests,
};
use carrier_fence::first_forbidden_carrier_content;

pub(crate) fn type_has_forbidden_denotational_content(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    type_reference: TypeReferenceHandle,
) -> bool {
    first_forbidden_carrier_content(program, proof_only, type_reference, &mut HashSet::new())
        .is_some()
}

const CORE_EQUIVALENCE_SOURCE: &str = "relation.omg";

/// Complete source-free quotient-correspondence batch admitted by the narrow
/// proof-only extraction seam.
///
/// The inner rows are intentionally private. Downstream Terminal producers
/// may inspect or consume a batch, but only this validation crate can construct
/// one after checking every retained quotient request. This preserves the
/// extractor's all-or-nothing guarantee across the source-erasure boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonExecutableQuotientCorrespondenceBatch {
    correspondences:
        Vec<language_semantics::quotient_correspondence::CanonicalQuotientCorrespondence>,
}

impl NonExecutableQuotientCorrespondenceBatch {
    /// Consume the validated batch at the source-erasure boundary.
    pub fn into_correspondences(
        self,
    ) -> Vec<language_semantics::quotient_correspondence::CanonicalQuotientCorrespondence> {
        self.correspondences
    }
}

impl std::ops::Deref for NonExecutableQuotientCorrespondenceBatch {
    type Target = [language_semantics::quotient_correspondence::CanonicalQuotientCorrespondence];

    fn deref(&self) -> &Self::Target {
        &self.correspondences
    }
}

/// Exact quotient-formation identity rederived from the complete typed graph.
///
/// The selected equivalence conformance licenses formation but is deliberately
/// absent: changing one valid proof implementation does not change quotient
/// identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedQuotientFormation {
    pub data_symbol: SymbolHandle,
    pub carrier: TypeReferenceHandle,
    pub relation_symbol: SymbolHandle,
}

/// Rerun the complete quotient-formation judgment without considering any
/// quotient-operation requests. Package review uses this instead of trusting
/// the retained `QuotientDefinition` metadata as checked evidence.
pub fn validate_quotient_formations(
    program: &TypedTrees,
) -> Result<Vec<ValidatedQuotientFormation>, Vec<Diagnostic>> {
    let proof_only = typed_trees::proof_only::classify(program);
    let mut diagnostics = Vec::new();
    let formations = collect_validated_quotient_formations(program, &proof_only, &mut diagnostics);
    if diagnostics.is_empty() {
        Ok(formations)
    } else {
        Err(diagnostics)
    }
}

/// Derive the first source-handle-free quotient correspondence aggregate.
///
/// This API is deliberately separate from ordinary validation. It grants no
/// checked or executable authority, and [`validate_program`] continues to
/// reject every quotient operation request. The extraction is all-or-nothing:
/// if any retained request lies outside the narrow total direct faithful
/// `define` or position-preserving forward-transport `lift` bridge, no partial
/// aggregate is returned.
pub fn extract_non_executable_quotient_correspondences(
    program: &TypedTrees,
) -> Result<NonExecutableQuotientCorrespondenceBatch, Vec<Diagnostic>> {
    let proof_only = typed_trees::proof_only::classify(program);
    let mut diagnostics = Vec::new();
    collect_validated_quotient_formations(program, &proof_only, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    terminal_bridge::extract(program)
        .map(|correspondences| NonExecutableQuotientCorrespondenceBatch { correspondences })
        .map_err(|errors| {
            errors
                .into_iter()
                .map(|error| {
                    Diagnostic::error(format!(
                        "non-executable quotient correspondence extraction failed: {error}"
                    ))
                })
                .collect()
        })
}

pub(crate) fn validate_quotients(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
) {
    reject_quotient_operation_requests(program, diagnostics);

    collect_validated_quotient_formations(program, proof_only, diagnostics);
}
