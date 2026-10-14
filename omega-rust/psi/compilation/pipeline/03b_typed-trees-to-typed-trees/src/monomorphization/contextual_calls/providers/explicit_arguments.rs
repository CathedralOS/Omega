//! Bind an explicitly applied provider through ordinary static argument proposals.
//! Authored evidence and unused operands remain part of the same complete tuple.

use super::super::super::{Candidate, Diagnostic, StaticMachineArgument, SymbolHandle, TypedTrees};
use typed_trees::state::State;

pub(super) fn bind(
    program: &TypedTrees,
    provider: &mut Candidate<'_>,
    state: &State,
    selected: SymbolHandle,
    arguments: &[StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    let callee = super::super::super::CalleeState {
        symbol: selected,
        candidate_index: 0,
        return_type: state.return_type,
        parameter_types: program
            .state_parameters(state)
            .iter()
            .map(|parameter| parameter.type_reference)
            .collect(),
    };
    let mut machines = Vec::new();
    let mut evidence = Vec::new();
    let mut types = Vec::new();
    let mut constants = Vec::new();
    let mut runtime_values = Vec::new();
    if super::super::super::selection::collect_machine_proposals_for_callee(
        program,
        std::slice::from_ref(provider),
        &callee,
        arguments,
        None,
        &mut machines,
        &mut evidence,
        &mut types,
        &mut constants,
        &mut runtime_values,
    )
    .is_err()
    {
        return Err(vec![Diagnostic::error(
            "nested machine application supplies an argument beyond its declared telescope",
        )]);
    }
    let supplied = super::super::super::selection::proposal_bindings(
        program,
        provider,
        machines,
        evidence,
        types,
        constants,
        runtime_values,
    );
    provider.type_bindings = supplied.type_bindings;
    provider.const_bindings = supplied.const_bindings;
    provider.runtime_value_bindings = supplied.runtime_value_bindings;
    provider.machine_bindings = supplied.machine_bindings;
    provider.evidence_bindings = supplied.evidence_bindings;
    provider.conflicted = supplied.conflicted;
    Ok(())
}
