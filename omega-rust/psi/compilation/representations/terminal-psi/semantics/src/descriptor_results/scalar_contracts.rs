//! Immutable scalar contracts participate in current payload history. Storage
//! propositions do not: mutations invalidate those through the payload owner.
//! Call adapters supply exact typed formal/result bindings from the current
//! checked signature, never implementation-private facts or receiving domains.
use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};
use std::collections::BTreeMap;

pub(super) fn scalar_conjuncts(propositions: &[Proposition]) -> Vec<Proposition> {
    let mut pending = propositions.iter().rev().collect::<Vec<_>>();
    let mut result = Vec::new();
    while let Some(proposition) = pending.pop() {
        if let Proposition::Conjunction(children) = proposition {
            pending.extend(children.iter().rev());
        } else if proposition.visit_value_ids(|_| {}) {
            result.push(proposition.clone());
        }
    }
    result
}

/// Rebind facts simultaneously. A retired incoming value may only survive as
/// the new formal receiving it; numeric SSA reuse never equates two iterations.
pub(super) fn arrival_requirements(
    requirements: &[Proposition],
    bindings: &[(ValueId, ScalarTerm)],
    parameters: &[ValueId],
) -> Vec<Proposition> {
    let mut substitutions = BTreeMap::new();
    for (parameter, actual) in bindings {
        if let ScalarTerm::Value { id, scalar_type } = actual {
            substitutions
                .entry(*id)
                .or_insert_with(|| ScalarTerm::value(*parameter, *scalar_type));
        }
    }
    let surviving = |value| !parameters.contains(&value) || substitutions.contains_key(&value);
    let mut result = requirements
        .iter()
        .filter(|premise| !premise.any_value_id(|value| !surviving(value)))
        .map(|premise| crate::substitution::substitute_proposition_values(premise, &substitutions))
        .collect::<Vec<_>>();
    for (parameter, actual) in bindings {
        if actual.any_value_id(|value| !surviving(value)) {
            continue;
        }
        let value = crate::substitution::substitute_scalar_term_values(actual, &substitutions);
        result.push(Proposition::Equal(
            ScalarTerm::value(*parameter, value.scalar_type()),
            value,
        ));
        // Non-rebound names still denote the same captured incoming value.
        if !actual.any_value_id(|value| parameters.contains(&value)) {
            result.push(Proposition::Equal(
                ScalarTerm::value(*parameter, actual.scalar_type()),
                actual.clone(),
            ));
        }
    }
    result
}

/// Project immutable, fully bound scalar guarantees into the caller namespace.
/// Unsupported or unbound conjuncts supply no evidence; a matching numeric
/// ValueId in the caller is never a substitute for an actual/formal binding.
pub fn instantiate_payload_scalar_guarantees(
    contract: &terminal_psi::MachineContract,
    bindings: &[(ValueId, ScalarTerm)],
) -> Vec<Proposition> {
    let substitutions = bindings.iter().cloned().collect::<BTreeMap<_, _>>();
    if substitutions.len() != bindings.len() {
        return Vec::new();
    }
    let guarantees = contract
        .ensures
        .iter()
        .map(|clause| clause.proposition.clone())
        .collect::<Vec<_>>();
    scalar_conjuncts(&guarantees)
        .into_iter()
        .filter_map(|proposition| {
            let mut bound = true;
            proposition.visit_value_ids(|value| bound &= substitutions.contains_key(&value));
            bound.then(|| {
                crate::substitution::substitute_proposition_values(&proposition, &substitutions)
            })
        })
        .collect()
}

pub(super) fn terminal_guarantees(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<Vec<Proposition>> {
    let (callee, arguments) = match &operation.kind {
        terminal_psi::OperationKind::Call {
            callee, arguments, ..
        }
        | terminal_psi::OperationKind::CallStructuralScalar {
            callee, arguments, ..
        } => (*callee, arguments),
        _ => return None,
    };
    let mut matching = module
        .machines
        .iter()
        .filter(|machine| machine.id == callee);
    let callee = matching.next()?;
    let signature = callee.result.scalar()?;
    let result = operation.result.scalar()?;
    if matching.next().is_some()
        || signature.scalar_type != result.scalar_type
        || arguments.len() != callee.parameters.len()
        || arguments.contains(&result.id)
        || !callee.contract.erased_scalar_formals.is_empty()
        || !callee.contract.erased_proof_formals.is_empty()
    {
        return None;
    }
    let types = machine
        .parameters
        .iter()
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            machine
                .operations()
                .filter_map(|operation| operation.result.scalar_ref()),
        )
        .collect::<Vec<_>>();
    let mut bindings = Vec::new();
    for (formal, actual) in callee.parameters.iter().zip(arguments) {
        if !types
            .iter()
            .any(|value| value.id == *actual && value.scalar_type == formal.scalar_type)
            || types
                .iter()
                .any(|value| value.id == *actual && value.scalar_type != formal.scalar_type)
        {
            return None;
        }
        bindings.push((formal.id, ScalarTerm::value(*actual, formal.scalar_type)));
    }
    bindings.push((
        signature.id,
        ScalarTerm::value(result.id, result.scalar_type),
    ));
    Some(instantiate_payload_scalar_guarantees(
        &callee.contract,
        &bindings,
    ))
}
