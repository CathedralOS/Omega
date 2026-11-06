//! Rejoin a requirement's normalized content equation to its satisfying body.
//!
//! The retained conformance selects the complete applied signature. Positional
//! formal correspondence changes subjects, not projection or operator meaning.
//! The resulting equation still needs proof at this exact completed return;
//! inheritance supplies neither the theorem nor custody of its subjects.

use checked_trees::{CheckFacts, ContractProofFactKind, ContractProofFactOwner, FlowExitFact};
use language_semantics::content::{
    ContentConservationEquation, ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm, ContentPlaceRoot, conservation_report_fingerprint,
};
use typed_trees::signature::StateParameter;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn proves_exit(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    fact: &facts::Fact,
    sources: &[validation::ContentConservationSourcePlan],
    available: &[ContentConservationPlan],
) -> bool {
    let facts::FactPayload::ContractBooleanExpression {
        fact: source_fact,
        expression,
        ..
    } = fact.payload
    else {
        return false;
    };
    if program.proof_facts.get(source_fact).as_expression() != Some(&expression) {
        return false;
    }
    let Some(machine) = program.machine_by_symbol(exit.machine_symbol) else {
        return false;
    };
    let Some(state) = program.state_in_machine(machine, exit.state_symbol) else {
        return false;
    };
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
        || !facts.flow.control.operands.is_valid(exit.returned_value)
    {
        return false;
    }
    let mut mapped: Option<ContentConservationPlan> = None;
    for (_, contract) in facts.proof.contract_facts.iter().filter(|(_, contract)| {
        contract.kind == ContractProofFactKind::Ensures
            && contract.fact == source_fact
            && contract.inherited_scope.is_some()
            && matches!(contract.owner, ContractProofFactOwner::MachineState {
                machine_symbol, ..
            } if machine_symbol == machine.symbol)
    }) {
        let Some((declaring_trait, requirement, satisfier, scope)) =
            crate::published::semantic::facts::inherited_contract_scope_parts(
                program,
                &facts.proof,
                contract,
            )
        else {
            return false;
        };
        // The completed-value judge's Entry revision is this state's arrival.
        // A root-origin row cannot transport old contents across intervening
        // state writes. Cross-state guarantees keep their existing composed
        // proof route until a complete entry-revision mapping is available.
        if satisfier.symbol != state.symbol
            || program
                .machine_states(machine)
                .first()
                .map(|state| state.symbol)
                != Some(state.symbol)
        {
            return false;
        }
        let Some(applied) = crate::published::semantic::facts::applied_inherited_requirement(
            program,
            requirement,
            scope,
        ) else {
            return false;
        };
        let Some(parameters) = validation::inherited_satisfier_parameters(
            program,
            declaring_trait,
            requirement,
            satisfier,
        ) else {
            return false;
        };
        let declared = program.state_signature_parameters(requirement);
        let selected = program.state_signature_parameters(applied);
        // A normalized path retains its declaring field and qualification
        // identities. Until those identities themselves can be instantiated,
        // do not transport an equation whose geometry changes under application.
        if !same_type(program, requirement.return_type, applied.return_type)
            || !same_type(program, applied.return_type, satisfier.return_type)
            || declared.len() != selected.len()
            || selected.len() != parameters.len()
            || !declared.iter().zip(selected).zip(parameters).all(
                |((declared, selected), actual)| {
                    same_type(program, declared.type_reference, selected.type_reference)
                        && same_type(program, selected.type_reference, actual.type_reference)
                },
            )
        {
            return false;
        }
        let mut matching = sources.iter().filter(|source| {
            source.source_expression == expression
                && source.plan.owner_kind == ContentConservationOwnerKind::TraitRequirement
                && source.plan.owner == scope.declaring_trait
                && source.plan.callable == scope.requirement
        });
        let Some(source) = matching.next() else {
            return false;
        };
        if matching.next().is_some() {
            return false;
        }
        let mut plan = source.plan.clone();
        let mut left = plan.equation.left().clone();
        let mut right = plan.equation.right().clone();
        for term in [&mut left, &mut right] {
            if rebind(term, declared, parameters, program.state_parameters(state)).is_none() {
                return false;
            }
        }
        plan.owner_kind = ContentConservationOwnerKind::Machine;
        plan.owner = machine.symbol;
        plan.callable = state.symbol;
        plan.equation = ContentConservationEquation::new(left, right);
        plan.report_fingerprint = conservation_report_fingerprint(&plan.algebra, &plan.equation);
        if mapped.as_ref().is_some_and(|known| known != &plan) {
            return false;
        }
        mapped = Some(plan);
    }
    let Some(plan) = mapped else {
        return false;
    };
    let completed = facts.flow.control.operands.get(exit.returned_value);
    completed.state_symbol == state.symbol
        && completed.statement_index == exit.statement_index
        && validation::completed_value_proves_content_conservation(
            program,
            machine,
            state,
            completed.statement_index,
            completed.expression,
            completed.type_reference,
            &plan,
            &facts.qualifications.content.plans,
            available,
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

fn rebind(
    term: &mut ContentConservationTerm,
    declared: &[StateParameter],
    actual: &[StateParameter],
    current: &[StateParameter],
) -> Option<()> {
    match term {
        ContentConservationTerm::Separate(children) => {
            for child in children {
                rebind(child, declared, actual, current)?;
            }
        }
        ContentConservationTerm::Projection { subject, .. } => {
            if let ContentPlaceRoot::Parameter {
                position,
                symbol,
                name,
            } = &mut subject.root
            {
                let ordinal = usize::try_from(*position).ok()?;
                if declared.get(ordinal)?.symbol != *symbol {
                    return None;
                }
                let target = actual.get(ordinal)?.symbol;
                let (ordinal, parameter) = current
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == target)?;
                *position = u32::try_from(ordinal).ok()?;
                *symbol = target;
                *name = parameter.name.as_str().to_owned();
            }
        }
    }
    Some(())
}
