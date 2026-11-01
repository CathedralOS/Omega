//! A by-value loan carrier transports existing debt, not referent ownership.
//! Content supply separately retains the exact owner projection. Enclosing
//! data applications keep their original operands and supplying scopes here;
//! a prepared field's erased type or lifetime spelling is insufficient.

use super::CallableBorrowContract;
use super::carried_lifetimes::{
    CarriedLifetime, LifetimeFrontier, RegionIdentity, scoped_carried_lifetimes,
};
use symbols::SymbolHandle;
use typed_trees::type_identity::{IndexIdentityArgument, NormalizedTypeIdentity};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// A source-availability judgment, not a claim correspondence or issuance rule.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OwnedCarrierLoanRelation {
    ViewFree,
    Preserved,
    Unproven,
}

/// Resolve the complete applied source once. Loan-bearing owned supply also
/// requires an identical result carrier and complete unchanged debt frontier;
/// the content consumer independently retains the exact owner projection.
pub fn owned_carrier_loan_relation(
    program: &dyn crate::CheckingInput,
    callable: SymbolHandle,
    source: TypeReferenceHandle,
    source_applications: &[TypeReferenceHandle],
    result: TypeReferenceHandle,
    result_applications: &[TypeReferenceHandle],
) -> OwnedCarrierLoanRelation {
    let Some(source) = applied_frontier(program, callable, source, source_applications) else {
        return OwnedCarrierLoanRelation::Unproven;
    };
    if source.borrowed {
        return OwnedCarrierLoanRelation::Unproven;
    }
    if source.loans.is_empty() {
        return OwnedCarrierLoanRelation::ViewFree;
    }
    if !source.linear {
        return OwnedCarrierLoanRelation::Unproven;
    }
    let Some(result) = applied_frontier(program, callable, result, result_applications) else {
        return OwnedCarrierLoanRelation::Unproven;
    };
    if source.identity == result.identity
        && source.loans.len() == result.loans.len()
        && source
            .loans
            .iter()
            .zip(&result.loans)
            .all(|(source, result)| {
                source.lifetime_only == result.lifetime_only
                    && source.retained_contract == result.retained_contract
                    && source.owner_path == result.owner_path
                    && source.lifetime == result.lifetime
                    && source.access == result.access
                    && source.source == result.source
            })
    {
        OwnedCarrierLoanRelation::Preserved
    } else {
        OwnedCarrierLoanRelation::Unproven
    }
}

struct AppliedCarrierFrontier {
    identity: NormalizedTypeIdentity,
    loans: Vec<CarriedLifetime>,
    borrowed: bool,
    linear: bool,
}

fn applied_frontier(
    program: &dyn crate::CheckingInput,
    callable: SymbolHandle,
    reference: TypeReferenceHandle,
    applications: &[TypeReferenceHandle],
) -> Option<AppliedCarrierFrontier> {
    if !reference.is_valid() {
        return None;
    }
    let reference = crate::peel_declared_constraints(program, reference);
    let (reference, frames) =
        program.project_enclosing_data_type_reference(reference, applications)?;
    let applications = &applications[..frames];
    let linear = program.type_multiplicity(reference) == language_semantics::Multiplicity::Linear;
    let borrowed = matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Reference { .. }
    );
    program
        .with_enclosing_data_identity_scopes(applications, |scopes| {
            let identity = typed_trees::type_identity::applied_type_identity_in_scope(
                program.typed_program(),
                reference,
                callable,
                callable,
                scopes,
            )?;
            let mut substitutions = Vec::new();
            let mut starts = Vec::new();
            for scope in scopes {
                starts.push(substitutions.len());
                substitutions.extend(scope.arguments.iter().filter_map(|(parameter, argument)| {
                    match argument {
                        IndexIdentityArgument::TypeArgument(reference) => {
                            Some((*parameter, *reference))
                        }
                        _ => None,
                    }
                }));
            }
            let contract = CallableBorrowContract {
                lexical_owner: callable,
                lifetime_parameters: &[],
                parameters: &[],
                return_type: reference,
                substitutions: &substitutions,
                substitution_scope_starts: &starts,
                application_scopes: scopes,
                binders: &[],
                lifetime_applications: &[],
                argument_owner: callable,
            };
            let regions = super::callable_regions::resolve(program, &contract).ok()?;
            let frontier = scoped_carried_lifetimes(
                program,
                reference,
                &substitutions,
                &starts,
                &[],
                &regions.names,
                &regions.arguments,
                &regions.applications,
                &regions.bindings,
                callable,
                true,
                LifetimeFrontier::Value,
            )?;
            (!frontier
                .iter()
                .any(|leaf| matches!(leaf.lifetime, Some(RegionIdentity::Declared(_)))))
            .then_some(AppliedCarrierFrontier {
                identity,
                loans: frontier,
                borrowed,
                linear,
            })
        })
        .flatten()
}
