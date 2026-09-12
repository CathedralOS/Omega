//! Abstract signatures retain their parameter declarations without an executable
//! machine. Global value searches omit these parameters, and spelling-based
//! place paths can substitute a foreign declaration. Resolve roots from the
//! supplied telescope, then reuse nominal field and call-result queries.

use super::{exact_data_member_field, unwrapped_type_reference};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

pub(crate) fn parameter_scoped_type_reference(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    expression_type(program, parameters, expression, 0)
}

fn expression_type(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    depth: usize,
) -> Option<TypeReferenceHandle> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let child_type = |expression| expression_type(program, parameters, expression, depth + 1);
    let reference = match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let [name] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            if !path.symbol.is_valid()
                || path.head_symbol != path.symbol
                || program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols)
                    != [path.symbol]
            {
                return None;
            }
            let mut candidates = parameters
                .iter()
                .filter(|parameter| parameter.symbol == path.symbol && parameter.name == *name);
            let parameter = candidates.next()?;
            if candidates.next().is_some() {
                return None;
            }
            parameter.type_reference
        }
        ExpressionNode::Borrow(borrow) => child_type(borrow.target)?,
        ExpressionNode::Member(member) => {
            let receiver = unwrapped_type_reference(program, child_type(member.receiver)?)?;
            let owner = match program.type_reference_table.type_reference(receiver) {
                TypeReferenceNode::Named { symbol, .. } => *symbol,
                TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                _ => return None,
            };
            let data = program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == owner)?;
            exact_data_member_field(
                program,
                data,
                member.member_symbol,
                member.member.as_str(),
                member.case_variant.as_ref().map(|case| case.as_str()),
            )?
            .type_reference
        }
        ExpressionNode::Indexed(indexed) => {
            if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) {
                return None;
            }
            let collection_type = child_type(indexed.collection)?;
            // Element projection describes builtin indexing only. An authored
            // [] operator can return another carrier, even for an array or
            // slice accepted through implicit shared collection adaptation.
            // Its result requires an exact application, not the element type.
            let operands = [Some(collection_type), child_type(indexed.index)];
            if !has_retained_builtin_index_meaning(program, expression)
                || !typed_trees::operator::resolve_indexed_spelling_for_operands(
                    program,
                    language_core::OperatorSpelling::Index,
                    &operands,
                )
                .is_empty()
            {
                return None;
            }
            let collection = unwrapped_type_reference(program, collection_type)?;
            match program.type_reference_table.type_reference(collection) {
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => *element_type,
                _ => return None,
            }
        }
        ExpressionNode::Call(call) => crate::calls::resolved_call_result_type(program, call)?,
        ExpressionNode::Cast(cast) => cast.target_type,
        ExpressionNode::ZeroValue(reference) => *reference,
        _ => return None,
    };
    program
        .type_reference_table
        .contains_type_reference(reference)
        .then_some(reference)
}

fn has_retained_builtin_index_meaning(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
    use typed_trees::{AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget};

    let mut occurrences = program
        .expression_table
        .authored_selection_occurrences(expression);
    let Some(occurrence) = occurrences.next() else {
        return false;
    };
    occurrences.next().is_none()
        && program
            .authored_declaration_selections()
            .get(occurrence)
            .is_some_and(|selection| {
                selection.kind() == AuthoredDeclarationSelectionKind::Operator
                    && selection.target()
                        == AuthoredDeclarationSelectionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                        )
            })
}
