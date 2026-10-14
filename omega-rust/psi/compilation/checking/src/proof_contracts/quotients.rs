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
//! proves one), and checked-tree admission supplies the checked termination
//! facts, so a real-route batch can admit once the checked stage has proved
//! its machines terminate.
//!
//! This file validates quotient formations and extracts correspondences.
//! `formation_collection.rs` collects validated formations and rejects
//! operation requests, `equivalence_selection.rs` validates equivalence
//! selections and exact relation applications, and `carrier_fence.rs`,
//! `relation_plan.rs` and `terminal_bridge.rs` carry the fence, plan and
//! bridge. Source fixtures under `tests/omega/*/proofs/quotient_*` exercise
//! admission, executable realization and invalid controls.

mod carrier_fence;
mod equivalence_selection;
mod formation_collection;
mod relation_plan;
mod terminal_bridge;

pub(crate) use equivalence_selection::exact_relation_application_matches;

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
    program: &dyn crate::CheckingInput,
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
        self.machine(machine)
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

/// Exact quotient-formation identity rederived from the complete typed graph.
///
/// The selected equivalence conformance licenses formation but is deliberately
/// absent: changing one valid proof implementation does not change quotient
/// identity.
pub type ValidatedQuotientFormation = checked_trees::CheckedQuotientFormation;

/// An admitted source occurrence and its normalized executable correspondence.
/// The request is still sealed in typed trees: execution consumes this checked
/// join, never the mere presence of a source-selected representative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedQuotientOperation {
    pub request: typed_trees::expression::ExpressionHandle,
    pub public_machine: SymbolHandle,
    pub public_state: SymbolHandle,
    pub representative_state: SymbolHandle,
    pub correspondence:
        language_semantics::quotient_correspondence::CanonicalQuotientCorrespondence,
    /// Entry-state symbols of the request's selected theorem machines, in
    /// role order.
    pub theorems: Box<[SymbolHandle]>,
}

/// Rerun the complete quotient-formation judgment without considering any
/// quotient-operation requests. Package review uses this instead of trusting
/// the retained `QuotientDefinition` metadata as checked evidence.
pub fn validate_quotient_formations(
    program: &dyn crate::CheckingInput,
) -> Result<Vec<ValidatedQuotientFormation>, Vec<Diagnostic>> {
    let proof_only = crate::proof_only_classification(program);
    let mut diagnostics = Vec::new();
    let formations = collect_validated_quotient_formations(program, &proof_only, &mut diagnostics);
    if diagnostics.is_empty() {
        Ok(formations)
    } else {
        Err(diagnostics)
    }
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

pub(crate) fn validate_quotients(
    program: &dyn crate::CheckingInput,
    proof_only: &ProofOnlyClassification,
    admission: QuotientRequestAdmission,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedQuotientFormation> {
    match admission {
        QuotientRequestAdmission::AtValidation => {
            reject_quotient_operation_requests(program, program.typed_program(), diagnostics);
        }
        QuotientRequestAdmission::AfterCheckedFacts => {}
    }

    collect_validated_quotient_formations(program, proof_only, diagnostics)
}

/// Judge the program's sealed quotient requests against checked termination.
///
/// This is the [`QuotientRequestAdmission::AfterCheckedFacts`] half: the
/// checked stage calls it once its termination facts exist, answering
/// `termination` from them. A program without requests admits trivially. A
/// program whose whole request batch the proof-only bridge can express admits;
/// the batch is all-or-nothing, so any other request keeps every request's
/// rejection diagnostic, rendered against the same checked termination.
/// Admission publishes the exact checked correspondence. Runtime demand must
/// still elaborate its representative call and retain the executable joins.
pub fn admit_checked_quotient_requests(
    program: &dyn crate::CheckingInput,
    termination: &dyn CheckedTerminationOracle,
) -> Result<Vec<ValidatedQuotientOperation>, Vec<Diagnostic>> {
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
        return Ok(Vec::new());
    }
    if let Ok(rows) = terminal_bridge::extract(program, termination)
        && !rows.is_empty()
    {
        return Ok(rows);
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

/// Render the exact nominal application from a retained formation judgment.
pub fn checked_quotient_relation_identity(
    program: &dyn crate::CheckingInput,
    quotient_type: TypeReferenceHandle,
    formations: &[ValidatedQuotientFormation],
) -> Result<language_semantics::quotient_correspondence::QuotientRelationIdentity, String> {
    let symbol = equivalence_selection::base_data_symbol(program, quotient_type)
        .ok_or_else(|| "quotient application has no nominal declaration".to_owned())?;
    let formation = formations
        .iter()
        .find(|row| row.data_symbol == symbol)
        .ok_or_else(|| "quotient application has no checked formation".to_owned())?;
    relation_plan::relation_identity(
        program,
        relation_plan::ExactQuotientRelation {
            quotient_symbol: symbol,
            quotient_type,
            relation_symbol: formation.relation_symbol,
        },
    )
}
