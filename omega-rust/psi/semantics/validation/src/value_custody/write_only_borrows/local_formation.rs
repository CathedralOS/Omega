//! Non-observing projected captures; later uses retain ordinary borrow checks.
use super::{
    ExpressionNode, Machine, ReferenceAccess, State, TypeReferenceNode, TypedTrees, WriteOnlyRoot,
    receiver,
};
use typed_trees::statement::TableLocalData;

pub(super) fn admitted(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    local: &TableLocalData,
    roots: &[WriteOnlyRoot],
) -> bool {
    if local.is_mutable
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
    {
        return false;
    }
    let TypeReferenceNode::Reference {
        referee,
        access: ReferenceAccess::WriteOnly,
        ..
    } = program
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return false;
    };
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(local.initial_value)
    else {
        return false;
    };
    if borrow.access != ReferenceAccess::WriteOnly
        || !crate::place_has_builtin_coordinates(program, machine, Some(state), borrow.target)
    {
        return false;
    }
    // Mutable parameters and earlier `let mut` bindings may attenuate at
    // formation, and earlier immutable `&mut` carriers preserve mutable
    // authority until attenuation. None is a write-only root for ordinary
    // expression validation, where reading remains legal — the same sources
    // the checked-call subloan gate uses.
    let mut sources = roots.to_vec();
    sources.extend(super::mutable_formation_sources(
        program,
        machine,
        state,
        Some(local.symbol),
    ));
    receiver::captured_type(program, borrow.target, &sources).is_some_and(|actual| {
        crate::value_custody::type_references::type_references_match(program, actual, *referee)
    })
}
