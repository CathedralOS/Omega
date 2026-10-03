//! The authored parameter roster as the structural plans see it: roots, binders,
//! ABI positions, and the erased scalar and proof formals that own no slot.

use super::*;

pub(in crate::execution::terminal_unit) fn state_flow(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<&checked_trees::FlowStateFact> {
    facts.flow.control.states.iter().find_map(|(_, candidate)| {
        (candidate.machine_symbol == machine && candidate.state_symbol == state)
            .then_some(candidate)
    })
}

pub(in crate::execution::terminal_unit) fn machine_binders(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<(SymbolHandle, String)> {
    program
        .machine_type_parameters(machine)
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.symbol, format!("$T{index}")))
        .collect()
}

pub(in crate::execution::terminal_unit) fn parameter_root_symbol(
    machine: SymbolHandle,
    parameter: &StateParameter,
) -> SymbolHandle {
    if parameter.is_self {
        machine
    } else {
        parameter.symbol
    }
}

/// Whether the calling plan strips this parameter as an `[erased]` binding
/// occurrence (contracts.md#explicit-erased-bindings): it receives no
/// structural or scalar plan entry and no ABI position, while the retained
/// parameters keep their authored `position`/`source_position` indices. Only
/// a plain immutable value binding is strippable. `self`, `const`, and
/// mutable bindings carry runtime custody or entry storage that erasure does
/// not define away, so an erased one refuses the plan (`None`) instead of
/// silently vanishing or silently keeping a slot.
pub(crate) fn strips_erased_parameter(parameter: &StateParameter) -> Option<bool> {
    if !parameter.relevance.is_erased() {
        return Some(false);
    }
    (!parameter.is_self && !parameter.is_const && !parameter.is_mutable).then_some(true)
}

/// The authored parameters that own an ABI position. An `[erased]` binding
/// occurrence stays in the typed signature (its `position` index is still the
/// authored one) but never receives a structural or scalar plan entry, so
/// every "every parameter is planned" consistency check counts this instead
/// of `state_parameters(state).len()`.
pub(in crate::execution::terminal_unit) fn abi_parameter_count(
    parameters: &[StateParameter],
) -> usize {
    parameters
        .iter()
        .filter(|parameter| !parameter.relevance.is_erased())
        .count()
}

/// The authored `[erased]` scalar formals in authored order, each retaining
/// its authored parameter position. Contract-term erased formals belong to
/// the proof lane (`erased_proof_parameter_plans`), so this plan skips
/// them. Erased formals with any other non-primitive type refuse the plan.
pub(crate) fn erased_scalar_parameter_plans(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<Vec<checked_trees::CheckedStructuralScalarParameterPlan>> {
    // Classifying proof-only data walks every data definition; a state
    // without an erased formal selects nothing and needs no classification.
    if !program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.relevance.is_erased())
    {
        return Some(Vec::new());
    }
    let proof_only = validation::proof_only_classification(program);
    program
        .state_parameters(state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.relevance.is_erased())
        .filter(|(_, parameter)| {
            !proof_only.contract_term_carrier(program, parameter.type_reference)
        })
        .map(|(position, parameter)| {
            Some(checked_trees::CheckedStructuralScalarParameterPlan {
                source_position: u32::try_from(position).ok()?,
                primitive_type: program.primitive_type_reference(parameter.type_reference)?,
            })
        })
        .collect()
}

/// The authored `[erased]` contract-term formals in authored order, each
/// retaining its authored parameter position and canonical semantic type
/// identity (`Nat`, `Proof`). These carriers admit no scalar lane; the
/// contract term lane carries them by identity instead.
pub(crate) fn erased_proof_parameter_plans(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<Vec<checked_trees::CheckedErasedProofParameterPlan>> {
    // Classifying proof-only data walks every data definition; a state
    // without an erased formal selects nothing and needs no classification.
    if !program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.relevance.is_erased())
    {
        return Some(Vec::new());
    }
    let proof_only = validation::proof_only_classification(program);
    program
        .state_parameters(state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.relevance.is_erased())
        .filter(|(_, parameter)| {
            proof_only.contract_term_carrier(program, parameter.type_reference)
        })
        .map(|(position, parameter)| {
            Some(checked_trees::CheckedErasedProofParameterPlan {
                source_position: u32::try_from(position).ok()?,
                parameter_symbol: parameter.symbol,
                type_identity: base_type_identity(program, parameter.type_reference, &[])?,
            })
        })
        .collect()
}

/// Whether an authored parameter owns a slot in the caller's structural
/// argument namespace once the unit plan is sealed. Erased bindings and
/// `const` formals carry no runtime slot, direct `Placed` view parameters
/// stay custody-only, and scalar primitives live in the scalar namespace. A
/// borrowed `self` is excluded here because it joins only when the plan
/// retains it; receivers that read `self` go through `reads_receiver`, which
/// decides retention before this index is committed.
pub(crate) fn structural_parameter_candidate(
    program: &TypedTrees,
    parameter: &StateParameter,
) -> bool {
    if parameter.is_const || strips_erased_parameter(parameter) == Some(true) {
        return false;
    }
    if parameter.is_self {
        return !is_reference(program, parameter.type_reference);
    }
    if matches!(
        program.type_reference_table.type_reference(parameter.type_reference),
        TypeReferenceNode::Reference { referee, .. }
            if program.placed_view_plan_for_type_reference(*referee).is_some()
    ) {
        return false;
    }
    program
        .primitive_type_reference(parameter.type_reference)
        .is_none()
}

/// Resolve the authored slot before the final structural roster is available.
/// Borrowed self is retained by a later receiver-observation decision; a
/// following parameter cannot acquire a definite dense index until then.
/// Callers with a completed roster must use that roster instead.
pub(crate) fn dense_structural_parameter_index(
    program: &TypedTrees,
    parameters: &[StateParameter],
    authored_position: usize,
) -> Option<u32> {
    let subject = parameters.get(authored_position)?;
    if !subject.is_self
        && parameters
            .iter()
            .take(authored_position)
            .any(|parameter| parameter.is_self && is_reference(program, parameter.type_reference))
    {
        return None;
    }
    u32::try_from(
        parameters
            .iter()
            .take(authored_position)
            .filter(|parameter| structural_parameter_candidate(program, parameter))
            .count(),
    )
    .ok()
}
