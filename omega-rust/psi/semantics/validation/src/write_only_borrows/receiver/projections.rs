//! Content-independent receiver addresses through closed fields and fixed arrays.

use super::*;

#[cfg(test)]
mod tests;

pub(super) fn record<'program>(
    program: &'program TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> Option<&'program DataDefinition> {
    let (root, referee, attached_root) = projected(program, expression, roots, false)?;
    if attached_root {
        super::record(program, root)
    } else {
        write_only_record(program, referee)
    }
}

/// Local formation freezes only literal coordinates. Runtime receiver dispatch
/// retains its existing separate selector-evaluation and range checks.
pub(in crate::write_only_borrows) fn captured_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> Option<TypeReferenceHandle> {
    let (_, referee, attached_root) = projected(program, expression, roots, true)?;
    (!attached_root && is_supported_checked_referee(program, referee)).then_some(referee)
}

fn projected<'roots>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &'roots [WriteOnlyRoot],
    literal_indexes: bool,
) -> Option<(&'roots WriteOnlyRoot, TypeReferenceHandle, bool)> {
    let mut cursor = expression;
    let mut projections = Vec::new();
    loop {
        if projections.contains(&cursor) || !program.expression_table.expression_is_valid(cursor) {
            return None;
        }
        cursor = match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                projections.push(cursor);
                member.receiver
            }
            ExpressionNode::Indexed(indexed) => {
                projections.push(cursor);
                indexed.collection
            }
            _ => break,
        };
    }
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
                    crate::places::exact_data_member_field(
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
                if literal_indexes {
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
                let (machine, state) = crate::calls::machine_state_by_symbol(
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

/// Admitting an address does not exempt selector evaluation from the ordinary
/// non-observation check. The range checker separately establishes index bounds.
pub(in crate::write_only_borrows) fn validate_operands(
    program: &TypedTrees,
    machine: &str,
    state: &str,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = expression;
    loop {
        cursor = match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => {
                validate_expression(program, machine, state, indexed.index, roots, diagnostics);
                indexed.collection
            }
            _ => break,
        };
    }
}
