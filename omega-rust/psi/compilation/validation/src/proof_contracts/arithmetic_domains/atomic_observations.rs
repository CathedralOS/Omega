//! Declaration-owned bounds of fresh atomic snapshots, never cached residents.
use super::{ExpressionHandle, Interval, Machine, State};

pub(crate) fn atomic_resident_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    place: ExpressionHandle,
) -> Option<Interval> {
    let reference = crate::declared_place_type_raw(program, machine, state, place)?;
    let primitive = program.primitive_type_reference(reference)?;
    let mut bounds = super::primitive_range(primitive)?;
    if let Some(declared) = super::enforced_declared_range(program, reference) {
        bounds = bounds.intersect(declared);
    }
    if let Some(stated) = state.and_then(|state| {
        crate::proof_contracts::default_domains::stored_field_where_interval(
            program, machine, state, place,
        )
    }) {
        bounds = bounds.intersect(stated);
    }
    Some(bounds)
}

pub(crate) fn atomic_result_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<Interval> {
    let name = program.expression_table.name_path_node(expression)?;
    let (_, local) = program.state_local_declaration(state, name.symbol)?;
    let event = crate::atomic_result_placeholder(program, state, local)?;
    atomic_resident_interval(program, machine, Some(state), event.place)
}
