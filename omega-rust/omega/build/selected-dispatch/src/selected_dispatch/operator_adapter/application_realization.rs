//! D29 canonical-empty-application realization joins.
//!
//! This module derives review-projector input from facts already retained by
//! checking and selected dispatch. It creates no authority or execution
//! evidence: every returned identity is either a compiler-private checked
//! coordinate or an existing strong commitment.

use checked_trees::{CheckedBoundaryOperatorApplicationUseSite, CheckedTrees};
use diagnostics::Diagnostic;
use language_core::operator_spelling::OperatorSpelling;
use symbols::SymbolHandle;

mod checked_expressions;

/// The retained authored-use lane rejoined to one selected checked occurrence.
/// The expression and origin live in `application_site`; this
/// discriminant prevents a downstream projector from conflating named and
/// fixed-token applications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedOperatorAuthoredUseKind {
    Named,
    FixedToken(OperatorSpelling),
}

/// One canonical-empty operator application whose selected realization is a
/// nongeneric checked body.
///
/// This is read-only compiler custody for a downstream review projector. It
/// does not claim generic-family coverage, native execution, or admission.
/// The retained realization contract's private snapshots are equality-checked
/// here and intentionally are not exposed as stable projector input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedNongenericOperatorApplicationRealization {
    pub application_site: CheckedBoundaryOperatorApplicationUseSite,
    pub authored_use_kind: CheckedOperatorAuthoredUseKind,
    pub requirement_operator: SymbolHandle,
    pub requirement_overload_identity: String,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_contract_report_fingerprint: u64,
    pub realization_contract_commitment: checked_trees::MachineContractCommitment,
}

/// Rejoin actual selected scalar calls to their canonical empty application
/// and exact nongeneric checked body.
///
/// Attached-Unit planning supplies an independent join where available;
/// ordinary value-machine expressions derive from their checked application,
/// exact authored selection, selected plan, and independently replayed body
/// contract. Generic applications remain in their specialization derivation.
/// Once a selected call names a nongeneric checked adapter, absence,
/// ambiguity, or substitution rejects the derivation.
pub fn derive_checked_nongeneric_operator_application_realizations(
    checked: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<CheckedNongenericOperatorApplicationRealization>, Vec<Diagnostic>> {
    let independently_derived_realizations =
        typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(&checked.typed);
    let mut rows: Vec<CheckedNongenericOperatorApplicationRealization> = Vec::new();
    for row in checked_expressions::derive(
        checked,
        selected_provider_plans.plans(),
        &independently_derived_realizations,
    )? {
        match rows
            .iter()
            .find(|retained| retained.application_site == row.application_site)
        {
            Some(retained) if retained == &row => {}
            Some(_) => {
                return Err(vec![Diagnostic::error(
                    "attached-Unit and checked-expression boundary application derivations disagree",
                )]);
            }
            None => rows.push(row),
        }
    }
    Ok(rows)
}
