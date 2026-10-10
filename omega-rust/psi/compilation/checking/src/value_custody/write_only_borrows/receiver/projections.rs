//! Content-independent receiver addresses through closed fields and fixed arrays.

use super::super::{
    TypeReferenceHandle, direct_write_only_root, fixed_unrestricted_write_only_array_shape,
    is_admitted_checked_referee,
};
use super::{ExpressionHandle, ExpressionNode, WriteOnlyRoot, bare_field, write_only_record};

/// Local formation freezes only literal coordinates. Runtime receiver dispatch
/// retains its existing separate selector-evaluation and range checks.
pub(in crate::value_custody::write_only_borrows) fn captured_type(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> Option<TypeReferenceHandle> {
    let (_, referee, attached_root) =
        projected(program, expression, roots, &mut |selector, length| {
            let Some(integer) = program.expression_table.integer_node(selector) else {
                return false;
            };
            integer
                .value_bignum()
                .and_then(|value| value.to_u64())
                .and_then(|value| usize::try_from(value).ok())
                .is_some_and(|position| position < length)
        })?;
    (!attached_root && is_admitted_checked_referee(program, referee)).then_some(referee)
}

/// Resolve the address without observing its contents. The consumer owns
/// selector admission: a persistent capture needs frozen coordinates, while
/// a call evaluates its selectors and owes the ordinary range obligations.
/// Neither policy changes field eligibility, access or the exact referee.
pub(in crate::value_custody::write_only_borrows) fn projected<'roots>(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    roots: &'roots [WriteOnlyRoot],
    admit_selector: &mut dyn FnMut(ExpressionHandle, usize) -> bool,
) -> Option<(&'roots WriteOnlyRoot, TypeReferenceHandle, bool)> {
    let (projections, cursor) =
        crate::expression_traversal::projection_climb(program, expression, usize::MAX)?;
    let (root, mut referee, mut attached_root) =
        if let Some(root) = direct_write_only_root(program, cursor, roots) {
            (root, root.referee, root.receiver_machine.is_valid())
        } else {
            let (root, field) = bare_field(program, cursor, roots)?;
            if field.relevance.is_erased() {
                return None;
            }
            (root, field.type_reference, false)
        };
    for projection in projections.into_iter().rev() {
        match program.expression_table.expression(projection) {
            ExpressionNode::Member(member) => {
                let field = if attached_root {
                    super::field(program, root, projection)?
                } else {
                    let owner = write_only_record(program, referee)?;
                    crate::value_custody::places::exact_data_member_field(
                        program,
                        owner,
                        member.member_symbol,
                        member.member.as_str(),
                        None,
                    )?
                };
                if field.relevance.is_erased() {
                    return None;
                }
                referee = field.type_reference;
            }
            ExpressionNode::Indexed(indexed) => {
                // Do not peel a reference stored in the referent: following
                // that pointer would observe prior contents. The array shape
                // and its element address must come from the declared type.
                let (element, length) =
                    fixed_unrestricted_write_only_array_shape(program, referee)?;
                if !program.expression_table.expression_is_valid(indexed.index)
                    || !admit_selector(indexed.index, length)
                {
                    return None;
                }
                let (machine, state) = crate::machine_calls::calls::machine_state_by_symbol(
                    program,
                    program.symbols.get(root.symbol).parent,
                )?;
                if !crate::place_has_builtin_coordinates(program, machine, Some(state), projection)
                {
                    return None;
                }
                referee = element;
            }
            _ => return None,
        }
        attached_root = false;
    }
    Some((root, referee, attached_root))
}
