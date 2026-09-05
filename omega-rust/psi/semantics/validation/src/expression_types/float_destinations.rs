//! A landed float's format survives delivery to storage, arguments, and results.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub(super) fn report_mismatch(
    program: &TypedTrees,
    machine: Option<&Machine>,
    state: Option<&State>,
    value: ExpressionHandle,
    target: PrimitiveType,
    slot_context: &str,
    slot_noun: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !matches!(target, PrimitiveType::F32 | PrimitiveType::F64) {
        return false;
    }
    // Direct literals retain the existing directed suffix diagnostic. This
    // guard extends that rule to values whose format comes from a declaration,
    // conversion, or call result.
    let mut direct = value;
    while let ExpressionNode::Borrow(borrow) = program.expression_table.expression(direct) {
        direct = borrow.target;
    }
    if matches!(
        program.expression_table.expression(direct),
        ExpressionNode::Float(_)
    ) {
        return false;
    }

    let source = match program.expression_table.expression(direct) {
        // A cast's output format, not its input, reaches this destination.
        ExpressionNode::Cast(cast) => primitive(program, cast.target_type),
        ExpressionNode::Call(call) => crate::calls::resolved_call_result_type(program, call)
            .or_else(|| {
                typed_trees::operator::resolve_named_expression_call(program, call)
                    .map(|operator| operator.return_type)
            })
            .and_then(|reference| primitive(program, reference)),
        ExpressionNode::Name(path) => super::named_value_type_reference(program, path)
            .or_else(|| {
                machine.and_then(|machine| {
                    crate::places::declared_place_type(program, machine, state, direct)
                })
            })
            .and_then(|reference| primitive(program, reference)),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => machine
            .and_then(|machine| crate::places::declared_place_type(program, machine, state, direct))
            .and_then(|reference| primitive(program, reference)),
        // A Binary's operands do not establish its result type: authored
        // heterogeneous operators can return a different format. Checked
        // operator selection owns that result; do not guess it here.
        _ => None,
    };
    if let Some(source @ (PrimitiveType::F32 | PrimitiveType::F64)) = source
        && source != target
    {
        diagnostics.push(Diagnostic::error(format!(
                "{slot_context} delivers a `{}` value to a `{}` {slot_noun}; a landed float retains its format, so changing format requires an explicit conversion",
                source.name(), target.name(),
            )));
        return true;
    }
    false
}

fn primitive(program: &TypedTrees, reference: TypeReferenceHandle) -> Option<PrimitiveType> {
    let reference = crate::places::unwrapped_type_reference(program, reference)?;
    program.primitive_type_reference(reference)
}
