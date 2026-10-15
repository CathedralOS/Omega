//! The authored parameter roster as the structural plans see it: roots, binders,
//! ABI positions, and the erased scalar and proof formals that own no slot.

use super::*;

pub(in crate::execution::terminal_unit) fn state_flow(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<&checked_trees::FlowStateFact> {
    facts.flow.control.state(machine, state)
}

/// Which authored parameter an argument or contract place roots at. A
/// parameter owns its place root outright, so the question is its own symbol
/// and nothing else.
///
/// Before static machines this was `parameter_root_symbol`, which answered the
/// attached machine's symbol for a `self` receiver, because an implicit
/// receiver place rooted at the machine rather than at any parameter. Every
/// caller therefore spelled the question as a disjunction over both roots. A
/// qualified machine name now contributes no hidden receiver and
/// `StateParameter` carries no receiver flag at all, so the machine-rooted
/// alternative is not representable and the disjunction collapses to this one
/// comparison. Keep the question here rather than re-deriving it at each
/// place-rooting site, so a later root spelling has one place to grow.
pub(crate) fn parameter_is_root(parameter: &StateParameter, root: SymbolHandle) -> bool {
    parameter.symbol == root
}

pub(in crate::execution::terminal_unit) fn machine_binders(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<(SymbolHandle, String)> {
    program
        .machine_type_parameters(machine)
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.symbol, format!("$T{index}")))
        .collect()
}

/// Whether the calling plan strips this parameter as an `[erased]` binding
/// occurrence (contracts.md#explicit-erased-bindings): it receives no
/// structural or scalar plan entry and no ABI position, while the retained
/// parameters keep their authored `position`/`source_position` indices. Only
/// a plain immutable value binding is strippable. `const` and
/// mutable bindings carry runtime custody or entry storage that erasure does
/// not define away, so an erased one refuses the plan (`None`) instead of
/// silently vanishing or silently keeping a slot.
pub(crate) fn strips_erased_parameter(parameter: &StateParameter) -> Option<bool> {
    if !parameter.relevance.is_erased() {
        return Some(false);
    }
    (!parameter.is_const && !parameter.is_mutable).then_some(true)
}

/// Storage for each `mut` scalar formal among `scalar_parameters`. Checking
/// reads such a formal as `StorageRead` of its symbol, never as the incoming
/// value, so the body must own initialized storage for it from entry. The
/// storage takes the formal's declared type, as a `let mut` local's does; a
/// carrier checking gives no current-value storage refuses the plan.
pub(in crate::execution::terminal_unit) fn parameter_storage_plans(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    scalar_parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
) -> Option<Vec<checked_trees::CheckedUnitParameterStoragePlan>> {
    let parameters = program.state_parameters(state);
    scalar_parameters
        .iter()
        .filter_map(|scalar| {
            let parameter = parameters.get(usize::try_from(scalar.source_position).ok()?);
            parameter
                .filter(|parameter| parameter.is_mutable)
                .map(|parameter| (scalar, parameter))
        })
        .map(|(scalar, parameter)| {
            let primitive_type = crate::values::mutable_scalar_parameter_type(program, parameter)?;
            if primitive_type != scalar.primitive_type
                || !super::supports_primitive_storage(program, parameter.type_reference)
            {
                return None;
            }
            Some(checked_trees::CheckedUnitParameterStoragePlan {
                source_position: scalar.source_position,
                symbol: parameter.symbol,
                type_identity: shapes.add_type(parameter.type_reference, binders, &[])?,
                primitive_type,
            })
        })
        .collect()
}

/// The authored `[erased]` scalar formals in authored order, each retaining
/// its authored parameter position. Contract-term erased formals belong to
/// the proof lane (`erased_proof_parameter_plans`), so this plan skips
/// them. Erased formals with any other non-primitive type refuse the plan.
pub(crate) fn erased_scalar_parameter_plans(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
                type_identity: base_type_identity(program, parameter.type_reference, &[], &[])?,
            })
        })
        .collect()
}

/// Whether an authored parameter owns a slot in the caller's structural
/// argument namespace once the unit plan is sealed. Erased bindings and
/// `const` formals carry no runtime slot, and scalar primitives live in the
/// scalar namespace. Borrowed placed descriptors retain a structural slot;
/// the separate placement roster still requires established backing. A
/// structural parameter retains its authored position independently of the
/// machine's qualification.
pub(crate) fn structural_parameter_candidate(
    program: &dyn ::validation::CheckingInput,
    parameter: &StateParameter,
) -> bool {
    if parameter.is_const || strips_erased_parameter(parameter) == Some(true) {
        return false;
    }
    program
        .primitive_type_reference(parameter.type_reference)
        .is_none()
}

/// Resolve the authored slot before the final structural roster is available.
/// Callers with a completed roster must use that roster instead.
pub(crate) fn dense_structural_parameter_index(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    authored_position: usize,
) -> Option<u32> {
    parameters.get(authored_position)?;
    u32::try_from(
        parameters
            .iter()
            .take(authored_position)
            .filter(|parameter| structural_parameter_candidate(program, parameter))
            .count(),
    )
    .ok()
}
