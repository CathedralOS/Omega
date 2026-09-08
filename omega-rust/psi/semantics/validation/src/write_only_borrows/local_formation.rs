//! Non-observing projected captures; later uses retain ordinary borrow checks.

use super::*;
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
    // Mutable parameters may attenuate at formation. They are not write-only
    // roots for ordinary expression validation, where reading remains legal.
    let mut sources = roots.to_vec();
    sources.extend(
        program
            .state_parameters(state)
            .iter()
            .filter_map(|parameter| {
                let TypeReferenceNode::Reference {
                    referee,
                    access: ReferenceAccess::Mutable,
                    ..
                } = program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                else {
                    return None;
                };
                Some(WriteOnlyRoot {
                    symbol: parameter.symbol,
                    receiver_machine: if parameter.is_self {
                        machine.symbol
                    } else {
                        SymbolHandle::invalid()
                    },
                    name: parameter.name.as_str().to_owned(),
                    referee: *referee,
                    is_parameter: true,
                })
            }),
    );
    receiver::captured_type(program, borrow.target, &sources).is_some_and(|actual| {
        crate::type_references::type_references_match(program, actual, *referee)
    })
}
