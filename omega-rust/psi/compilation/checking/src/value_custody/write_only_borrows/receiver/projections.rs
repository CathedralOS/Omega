//! Content-independent receiver addresses through closed fields and fixed arrays.

use super::super::{
    TypeReferenceHandle, direct_write_only_root, fixed_unrestricted_write_only_array_shape,
    is_supported_checked_referee,
};
use super::{
    ExpressionHandle, ExpressionNode, TypedTrees, WriteOnlyRoot, bare_field, write_only_record,
};

/// How far a non-observing projected address may descend through fixed-array
/// elements. `MembersOnly` serves content-independent record-field places:
/// every receiver is an admitted plain record and every selected field is
/// relevant. `LiteralIndexes` additionally freezes each element hop at a
/// literal in-bounds index; local formation admits only frozen coordinates.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::value_custody::write_only_borrows) enum ProjectionAdmission {
    MembersOnly,
    LiteralIndexes,
}

/// Local formation freezes only literal coordinates. Runtime receiver dispatch
/// retains its existing separate selector-evaluation and range checks.
pub(in crate::value_custody::write_only_borrows) fn captured_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> Option<TypeReferenceHandle> {
    let (_, referee, attached_root) = projected(
        program,
        expression,
        roots,
        ProjectionAdmission::LiteralIndexes,
    )?;
    (!attached_root && is_supported_checked_referee(program, referee)).then_some(referee)
}

pub(in crate::value_custody::write_only_borrows) fn projected<'roots>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &'roots [WriteOnlyRoot],
    admission: ProjectionAdmission,
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
                if admission == ProjectionAdmission::MembersOnly {
                    return None;
                }
                // Do not peel a reference stored in the referent: following
                // that pointer would observe prior contents. The array shape
                // and its element address must come from the declared type.
                let (element, length) =
                    fixed_unrestricted_write_only_array_shape(program, referee)?;
                if admission == ProjectionAdmission::LiteralIndexes {
                    if !program.expression_table.expression_is_valid(indexed.index) {
                        return None;
                    }
                    let ExpressionNode::Integer(index) =
                        program.expression_table.expression(indexed.index)
                    else {
                        return None;
                    };
                    let index = usize::try_from(index.value_bignum()?.to_u64()?).ok()?;
                    if index >= length {
                        return None;
                    }
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
