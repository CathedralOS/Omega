//! Prove each exit obligation in the declaration that authored it.
//!
//! An ordinary implementation can owe several public contracts with distinct
//! parameter and result symbols. The retained conformance supplies their exact
//! correspondence; the flow graph still supplies every original-to-current
//! value mapping. Neither the public goal nor a same-spelled parameter proves
//! anything about the completed result.

use checked_trees::{CheckFacts, ContractProofFactKind, ContractProofFactOwner, FlowExitFact};
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::{machine::Machine, state::State};

#[allow(clippy::too_many_arguments)]
pub(super) fn proves(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    machine: &Machine,
    state: &State,
    source_fact: arena::Handle<typed_trees::domain::ProofFact>,
    expression: ExpressionHandle,
    origins: &[(symbols::SymbolHandle, symbols::SymbolHandle)],
) -> bool {
    if program.proof_facts.get(source_fact).as_expression() != Some(&expression) {
        return false;
    }
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let Some(returned) = super::super::return_values::exit_return_completion(program, exit) else {
        return false;
    };
    let mut inherited = false;
    for (_, contract) in facts.proof.contract_facts.iter().filter(|(_, contract)| {
        contract.kind == ContractProofFactKind::Ensures
            && contract.fact == source_fact
            && contract.inherited_scope.is_some()
            && matches!(contract.owner, ContractProofFactOwner::MachineState { machine_symbol, .. }
                if machine_symbol == machine.symbol)
    }) {
        inherited = true;
        let Some((declaration, requirement, supplying, scope)) =
            crate::published::semantic::facts::inherited_contract_scope_parts(
                program,
                &facts.proof,
                contract,
            )
        else {
            return false;
        };
        if scope.conformance_machine != machine.symbol || supplying.symbol != entry.symbol {
            return false;
        }
        let Some(applied) = crate::published::semantic::facts::applied_inherited_requirement(
            program,
            requirement,
            scope,
        ) else {
            return false;
        };
        let Some(actual) = validation::inherited_satisfier_parameters(
            program,
            declaration,
            requirement,
            supplying,
        ) else {
            return false;
        };
        let declared = program.state_signature_parameters(requirement);
        let selected = program.state_signature_parameters(applied);
        if declared.len() != selected.len()
            || selected.len() != actual.len()
            || !same_type(program, applied.return_type, supplying.return_type)
            || !selected.iter().zip(actual).all(|(formal, actual)| {
                formal.is_const == actual.is_const
                    && formal.is_mutable == actual.is_mutable
                    && formal.relevance == actual.relevance
                    && same_type(program, formal.type_reference, actual.type_reference)
            })
        {
            return false;
        }
        // Applied types do not change the authored value-formal identities.
        let parameters = declared
            .iter()
            .zip(selected)
            .map(|(declared, selected)| StateParameter {
                type_reference: selected.type_reference,
                ..declared.clone()
            })
            .collect::<Vec<_>>();
        let type_bindings = program
            .trait_type_parameters(declaration)
            .iter()
            .zip(&scope.trait_arguments)
            .map(|(formal, actual)| (formal.symbol, *actual))
            .collect::<Vec<_>>();
        if !validation::normal_exit_proves(
            program,
            machine,
            state,
            exit.statement_index,
            returned,
            validation::NormalExitContract {
                expression,
                callable: requirement.symbol,
                parameters: &parameters,
                supplying_parameters: actual,
                type_bindings: &type_bindings,
            },
            origins,
            &facts.termination.rank_arrivals,
        ) {
            return false;
        }
    }
    // Repeated applications of one authored fact must each prove their goal;
    // no first-match choice may hide a distinct inherited application.
    inherited
        || validation::normal_exit_proves(
            program,
            machine,
            state,
            exit.statement_index,
            returned,
            validation::NormalExitContract {
                expression,
                callable: machine.symbol,
                parameters: program.state_parameters(entry),
                supplying_parameters: program.state_parameters(entry),
                type_bindings: &[],
            },
            origins,
            &facts.termination.rank_arrivals,
        )
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
