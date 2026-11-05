//! Classify an operation independently of evaluating its operands. Discarding
//! a result may still require the operation; callers must separately inspect
//! every ordered child and any complete callee effect evidence.

use crate::CheckingInput;
use crate::frozen_program::expression_table;
use crate::value_custody::places::collection_operations::MeasureReceiver;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Discarding a value never discards an operation's own effects. Calls,
/// atomics, executable conversions and non-builtin operators keep their complete
/// owner. Representation recasts, construction and projection have only their
/// ordered children's effects; the recast checker owns compatibility and loans.
pub(crate) fn expression_has_no_own_effects(
    program: &dyn CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> bool {
    match expression_table(program).expression(expression) {
        ExpressionNode::Call(_) | ExpressionNode::Atomic(_) | ExpressionNode::Unary(_) => false,
        // A recast borrows existing backing; it does not execute a conversion.
        // Its completed reference may remain opaque without discarding the
        // prefix's independent premises or claiming new referent contents.
        ExpressionNode::Cast(cast) => cast.form.is_recast(),
        ExpressionNode::Binary(_) => {
            crate::has_builtin_binary_expression_meaning(program, machine, Some(state), expression)
        }
        ExpressionNode::Indexed(indexed) => {
            crate::has_builtin_subslice_meaning(program, machine, Some(state), expression)
                || crate::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    indexed.collection,
                )
                .is_some_and(|collection| {
                    crate::value_custody::places::value_element_has_builtin_coordinate(
                        program,
                        machine,
                        Some(state),
                        expression,
                        collection,
                    )
                })
        }
        ExpressionNode::Member(member) => {
            let Some(reference) =
                crate::expression_result_type_reference(program, machine, state, member.receiver)
            else {
                return false;
            };
            if crate::value_custody::places::collection_operations::collection_measure_member(
                program,
                member,
                MeasureReceiver::Declared(reference),
            )
            .is_some()
            {
                return true;
            }
            crate::value_custody::places::data_definition_for_type(program, reference)
                .and_then(|data| {
                    crate::value_custody::places::exact_data_member_field(
                        program,
                        data,
                        member.member_symbol,
                        member.member.as_str(),
                        member.case_variant.as_ref().map(|case| case.as_str()),
                    )
                })
                .is_some()
        }
        ExpressionNode::Match(_)
        | ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::String(_) => true,
        ExpressionNode::SavedSubject(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::TypeExpression(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}
