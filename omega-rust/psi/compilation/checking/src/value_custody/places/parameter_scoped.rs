//! Abstract signatures retain their parameter declarations without an executable
//! machine. Global value searches omit these parameters, and spelling-based
//! place paths can substitute a foreign declaration. Resolve roots from the
//! supplied telescope, then reuse nominal field and call-result queries.

use super::unwrapped_type_reference;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn parameter_scoped_type_reference(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    expression_type(program, parameters, expression, 0)
}

fn expression_type(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    depth: usize,
) -> Option<TypeReferenceHandle> {
    let node = program
        .expression_table
        .expression_at_depth(expression, depth)?;
    let child_type = |expression| expression_type(program, parameters, expression, depth + 1);
    let reference = match node {
        ExpressionNode::SavedSubject(saved) => child_type(program.saved_value_source(saved)?)?,
        ExpressionNode::Name(path) => {
            // One member: a bare binding occurrence, never a chain. The
            // telescope membership test below is the resolved symbol's, so
            // the spelling contributes nothing a foreign declaration could
            // substitute past.
            let [_] = program.expression_table.name_path_members(path.members) else {
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
                .filter(|parameter| parameter.symbol == path.symbol);
            let parameter = candidates.next()?;
            if candidates.next().is_some() {
                return None;
            }
            parameter.type_reference
        }
        ExpressionNode::Borrow(borrow) => child_type(borrow.target)?,
        ExpressionNode::Member(member) => {
            let receiver = unwrapped_type_reference(program, child_type(member.receiver)?)?;
            super::projected_members::applied_data_member_field(program, receiver, member)?
                .type_reference
        }
        ExpressionNode::Indexed(indexed) => {
            if program.expression_table.indexed_on_range(indexed) {
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
                    program.expression_table.source_span(expression),
                )
                .is_empty()
            {
                return None;
            }
            super::collection_element_type_reference(program, collection_type)?
        }
        ExpressionNode::Call(_) => {
            crate::machine_calls::calls::call_result_type(program, expression)?
        }
        ExpressionNode::Cast(cast) => cast.target_type,
        ExpressionNode::ZeroValue(reference) => *reference,
        _ => return None,
    };
    program
        .type_reference_table
        .contains_type_reference(reference)
        .then_some(reference)
}

pub(crate) fn has_retained_builtin_index_meaning(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
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
