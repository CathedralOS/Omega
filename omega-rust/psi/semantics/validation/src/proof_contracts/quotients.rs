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
//! Termination eligibility of a representative or selected theorem is read
//! through [`CheckedTerminationOracle`]: ordinary validation supplies the
//! typed machine summaries it has (no guarantee before the checked stage
//! proves one), and the Terminal producer supplies the checked termination
//! facts, so a real-route batch can admit once the checked stage has proved
//! its machines terminate.
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

/// Checked termination summaries the quotient bridge consults for
/// representative and selected theorem eligibility.
///
/// The bridge never proves termination itself; it only asks whether a
/// machine already carries an unconditional checked guarantee. Which stage
/// answers decides what the bridge can admit: the typed program answers with
/// the summaries validation has (nothing before the checked stage runs), the
/// checked facts answer with what that stage proved.
pub trait CheckedTerminationOracle {
    /// The checked termination summary recorded for `machine`, when one is.
    fn checked_termination(
        &self,
        machine: SymbolHandle,
    ) -> Option<language_semantics::TerminationGuarantee>;

    /// Whether `machine` terminates unconditionally: a checked guarantee with
    /// no progress-profile premises. Premises are observable admission
    /// dependencies the initial quotient wrapper cannot discharge silently.
    fn unconditionally_terminates(&self, machine: SymbolHandle) -> bool {
        matches!(
            self.checked_termination(machine),
            Some(language_semantics::TerminationGuarantee::Terminates { premises })
                if premises.is_empty()
        )
    }
}

/// The typed program's own machine summaries: what ordinary validation has.
/// The unconditional judgment stays the one denotational-call helper so
/// quotient eligibility and normal-return eligibility read one rule.
impl CheckedTerminationOracle for TypedTrees {
    fn checked_termination(
        &self,
        machine: SymbolHandle,
    ) -> Option<language_semantics::TerminationGuarantee> {
        self.machines()
            .iter()
            .find(|candidate| candidate.symbol == machine)
            .map(|candidate| candidate.termination_plan.checked_summary.clone())
    }

    fn unconditionally_terminates(&self, machine: SymbolHandle) -> bool {
        crate::machine_calls::denotational_calls::unconditionally_terminates(self, machine)
    }
}

/// A lookup closure, for producers whose summaries live beside rather than on
/// the typed machine (the checked stage's termination facts).
impl<Lookup> CheckedTerminationOracle for Lookup
where
    Lookup: Fn(SymbolHandle) -> Option<language_semantics::TerminationGuarantee>,
{
    fn checked_termination(
        &self,
        machine: SymbolHandle,
    ) -> Option<language_semantics::TerminationGuarantee> {
        self(machine)
    }
}

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
/// `define` or argument-adapted forward-transport `lift` bridge, no partial
/// aggregate is returned.
pub fn extract_non_executable_quotient_correspondences(
    program: &TypedTrees,
) -> Result<NonExecutableQuotientCorrespondenceBatch, Vec<Diagnostic>> {
    extract_non_executable_quotient_correspondences_with_termination(program, program)
}

/// [`extract_non_executable_quotient_correspondences`] with termination
/// eligibility answered by `termination` instead of the typed machine
/// summaries. The Terminal producer passes the checked stage's termination
/// facts here; every other judgment still reads the typed program.
pub fn extract_non_executable_quotient_correspondences_with_termination(
    program: &TypedTrees,
    termination: &dyn CheckedTerminationOracle,
) -> Result<NonExecutableQuotientCorrespondenceBatch, Vec<Diagnostic>> {
    let proof_only = typed_trees::proof_only::classify(program);
    let mut diagnostics = Vec::new();
    collect_validated_quotient_formations(program, &proof_only, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    terminal_bridge::extract(program, termination)
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

/// Where a program's sealed `Quotient::define`/`Quotient::lift` requests are
/// judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotientRequestAdmission {
    /// Ordinary validation judges every request against the typed machine
    /// summaries it has. No checked termination guarantee exists yet, so on
    /// the compiler route every request rejects at the termination fence.
    AtValidation,
    /// Validation checks quotient formations only and leaves every request to
    /// [`admit_checked_quotient_requests`], which the checked stage must call
    /// once its termination facts exist. Selecting this without that call
    /// would admit requests nothing judged; the compiler route pairs them.
    AfterCheckedFacts,
}

pub(crate) fn validate_quotients_with_admission(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    admission: QuotientRequestAdmission,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match admission {
        QuotientRequestAdmission::AtValidation => {
            reject_quotient_operation_requests(program, program, diagnostics);
        }
        QuotientRequestAdmission::AfterCheckedFacts => {}
    }

    collect_validated_quotient_formations(program, proof_only, diagnostics);
}

/// Judge the program's sealed quotient requests against checked termination.
///
/// This is the [`QuotientRequestAdmission::AfterCheckedFacts`] half: the
/// checked stage calls it once its termination facts exist, answering
/// `termination` from them. A program without requests admits trivially. A
/// program whose whole request batch the proof-only bridge extracts admits;
/// the batch is all-or-nothing, so any other request keeps every request's
/// rejection diagnostic, rendered against the same checked termination.
/// Admission is proof-only: the extracted rows reach Terminal only through
/// the producer's own installer, and a nonempty correspondence table is still
/// refused by Terminal execution.
pub fn admit_checked_quotient_requests(
    program: &TypedTrees,
    termination: &dyn CheckedTerminationOracle,
) -> Result<(), Vec<Diagnostic>> {
    if !program
        .expression_table
        .iter_expressions()
        .any(|(_, expression)| {
            matches!(
                expression,
                typed_trees::expression::ExpressionNode::Call(call)
                    if call.quotient_operation.is_some()
            )
        })
    {
        return Ok(());
    }
    if matches!(terminal_bridge::extract(program, termination), Ok(rows) if !rows.is_empty()) {
        return Ok(());
    }
    let mut diagnostics = Vec::new();
    reject_quotient_operation_requests(program, termination, &mut diagnostics);
    if diagnostics.is_empty() {
        // The bridge and the rejection walk judge the same plans; a batch the
        // bridge refused must name its reason rather than pass silently.
        diagnostics.push(Diagnostic::error(
            "quotient operation requests were not admitted by the proof-only bridge".to_owned(),
        ));
    }
    Err(diagnostics)
}
