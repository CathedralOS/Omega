//! Resolve inherited obligations through their retained declaration and application.
//!
//! A public parameter or result keeps its authored identity when an ordinary
//! implementation owes its contract. Consumers share this exact application
//! lookup; declaration names and equal carrier shapes cannot select an edge.

use checked_trees::{ContractProofFact, ContractProofFactOwner, ProofFacts};
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::AppliedTraitScope;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::TypeReferenceHandle;

/// Resolve one inherited contract fact's migrated proposition endpoint: the
/// declaring trait, its requirement signature, and the satisfying state the
/// fact is owned by. `None` fails closed -- the fact keeps its authored schema
/// identity rather than publishing a partial substitution.
pub(crate) fn inherited_contract_scope_parts<'program>(
    program: &'program dyn ::validation::CheckingInput,
    proof: &'program ProofFacts,
    contract: &ContractProofFact,
) -> Option<(
    &'program typed_trees::trait_definition::TraitDefinition,
    &'program typed_trees::signature::StateSignature,
    &'program typed_trees::state::State,
    &'program checked_trees::InheritedContractScope,
)> {
    let scope = proof
        .inherited_contract_scopes
        .get(contract.inherited_scope?);
    let trait_definition = program.trait_definition_by_symbol(scope.declaring_trait)?;
    let requirement =
        program.trait_machine_signature_by_symbol(trait_definition, scope.requirement)?;
    let ContractProofFactOwner::MachineState { state_symbol, .. } = contract.owner else {
        return None;
    };
    let satisfier_state = program
        .machines()
        .iter()
        .find_map(|machine| program.state_in_machine(machine, state_symbol))?;
    Some((trait_definition, requirement, satisfier_state, scope))
}

/// A declaration and its complete applied telescope remain distinct. Select
/// the exact retained conformance, rejecting ambiguous inherited applications.
pub(crate) fn applied_inherited_requirement<'program>(
    program: &'program dyn validation::CheckingInput,
    requirement: &'program StateSignature,
    scope: &checked_trees::InheritedContractScope,
) -> Option<&'program StateSignature> {
    let owner = program.machine_by_symbol(scope.conformance_machine)?;
    let mut application = None;
    for conformance in program.machine_trait_conformances(owner) {
        let root = AppliedTraitScope::from_machine_conformance(
            program.typed_program(),
            owner,
            conformance,
        )?;
        let Some(candidate) = root.declaring_scope(program.typed_program(), scope.declaring_trait)
        else {
            continue;
        };
        if let Some(known) = &application
            && !candidate.same_application(program.typed_program(), known)
        {
            return None;
        }
        application = Some(candidate);
    }
    let application = application?;
    let binders = program.trait_type_parameters(application.declaration);
    if binders.len() != scope.trait_arguments.len()
        || !binders
            .iter()
            .zip(&scope.trait_arguments)
            .all(|(binder, actual)| {
                same_type(
                    program,
                    application.parameter_argument(program.typed_program(), binder.symbol),
                    *actual,
                )
            })
    {
        return None;
    }
    program
        .applied_callable_signature(&application, requirement.symbol)
        .or_else(|| {
            (requirement.type_parameters.is_empty()
                && application.substitutions.is_empty()
                && application.declaration.lifetime_parameters.is_empty())
            .then_some(requirement)
        })
}

fn same_type(
    program: &dyn validation::CheckingInput,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    left.is_valid()
        && right.is_valid()
        && program.type_identity(TypeIdentityRequest::ordinary(left))
            == program.type_identity(TypeIdentityRequest::ordinary(right))
}
