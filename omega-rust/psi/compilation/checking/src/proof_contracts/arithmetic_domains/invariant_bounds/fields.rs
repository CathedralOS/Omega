//! Declaration-owned storage bounds follow the shared readable field path.
//!
//! The bound belongs to the selected field, not the number of member hops in
//! its source spelling. Canonical coordinates and the structural declaration
//! walk retain each owner, reference crossing and field identity. This query
//! establishes no value, case membership or read permission; those remain
//! obligations of the observation. Mutable arrival facts are never recovered.

use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn type_reference(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    declared_mutable_leaves: bool,
) -> Option<TypeReferenceHandle> {
    let place = crate::canonical_place_from_expression(program, expression)?;
    let root = place.root.as_symbol()?;
    let parameters = program.state_parameters(state);
    let parameter = program.state_parameter_by_symbol(state, root)?;
    // Invocation-fixed queries retain the existing immutable-input rule.
    // The machine's attached storage is different: its own field writes owe
    // the declared predicates, independently of its exclusive binding.
    let owned_storage = program
        .data_binding_position(machine.attached_data_symbol, parameters)
        .is_some_and(|position| parameters[position].symbol == root);
    if parameter.is_const || (parameter.is_mutable && !declared_mutable_leaves && !owned_storage) {
        return None;
    }
    let field =
        crate::structural_scalar_field_at_coordinate(program, parameters, root, &place.segments)?;
    let reference = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        expression,
    )?;
    (program.primitive_type_reference(reference) == Some(field.primitive_type)).then_some(reference)
}
