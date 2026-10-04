use crate::authored_selections::is_boundary_acceptance_marker;
use crate::borrow::calls::BorrowCallCollection;
use crate::lookup::{
    call_receiver_parts, receiver_can_dispatch_to_machine, resolve_state_call_target,
};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn collect_expression_borrow_calls(
    collection: &mut BorrowCallCollection<'_>,
    expression: ExpressionHandle,
) {
    match collection.program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            collect_expression_borrow_calls(collection, dispatch.subject);
            // These rows name authored calls, not a simultaneous execution.
            // Keep even covered suffixes so lexical call ordinals stay stable.
            for arm in collection
                .program
                .expression_table
                .match_arms(dispatch.arms)
            {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    collect_expression_borrow_calls(collection, pattern);
                }
                collect_expression_borrow_calls(collection, arm.value);
            }
        }
        ExpressionNode::Atomic(atomic) => collect_expression_borrow_calls(collection, atomic.value),
        ExpressionNode::ArrayLiteral(values) => {
            for value in collection
                .program
                .expression_table
                .expression_handles(*values)
            {
                collect_expression_borrow_calls(collection, *value);
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                collect_expression_borrow_calls(collection, range.start);
            }
            if range.end.is_valid() {
                collect_expression_borrow_calls(collection, range.end);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_borrow_calls(collection, binary.left);
            collect_expression_borrow_calls(collection, binary.right);
        }
        ExpressionNode::Call(call) => {
            let (receiver_symbol, receiver_path) =
                call_receiver_parts(collection.program, call.receiver);
            let boundary_operator =
                typed_trees::operator::resolve_named_expression_call(collection.program, call)
                    .filter(|operator| operator.is_boundary);
            let resolved_target = resolve_state_call_target(
                collection.program,
                collection.machine,
                collection.state,
                receiver_symbol,
                boundary_operator.map_or(call.target_symbol, |operator| operator.symbol),
                receiver_path.as_deref(),
                &call.target,
            );
            let is_machine_call = resolved_target.is_valid()
                || receiver_can_dispatch_to_machine(
                    collection.program,
                    collection.machine,
                    collection.state,
                    receiver_symbol,
                    receiver_path.as_deref(),
                );

            // `accept_boundary#...` is a statically harvested build
            // declaration, not an operational machine call. Keep nested
            // expression traversal below, but do not mint flow-call evidence.
            // A retained `size_of`/`align_of` request is the compiler's
            // layout query, not an invocation: it names no callee to dispatch
            // to, and a receiverless spelling is dispatch-eligible by shape
            // alone, so it would mint a call fact with an invalid target and
            // the terminal-unit call sequencer would omit the whole unit
            // plan. Selected-execution settlement folds every query to a
            // `u64` literal before settled checking, but preliminary checking
            // and build-time evaluation still admit the request node here.
            if is_machine_call
                && !is_boundary_acceptance_marker(call.target.as_str())
                && call.layout_query.is_none()
            {
                let accesses = collection.collect_call_argument_accesses(
                    collection
                        .program
                        .expression_table
                        .expression_handles(call.arguments),
                    if resolved_target.is_valid() {
                        resolved_target
                    } else {
                        call.target_symbol
                    },
                );
                collection.append_borrow_call(
                    receiver_symbol,
                    if resolved_target.is_valid() {
                        resolved_target
                    } else {
                        call.target_symbol
                    },
                    boundary_operator.map_or_else(
                        || {
                            call.receiver.is_valid()
                                && collection.program.symbols.get(receiver_symbol).kind
                                    != symbols::SymbolKind::ConformanceParameter
                        },
                        |operator| {
                            // A qualified requirement namespace is not a
                            // runtime operand. Only the declaration's extra
                            // receiver position establishes receiver custody.
                            call.receiver.is_valid()
                                && collection.program.operator_parameters(operator).len()
                                    == collection
                                        .program
                                        .expression_table
                                        .expression_handles(call.arguments)
                                        .len()
                                        + 1
                        },
                    ),
                    accesses,
                );
            }

            if call.receiver.is_valid() {
                collect_expression_borrow_calls(collection, call.receiver);
            }
            for argument in collection
                .program
                .expression_table
                .expression_handles(call.arguments)
            {
                collect_expression_borrow_calls(collection, *argument);
            }
        }
        ExpressionNode::Cast(cast) => collect_expression_borrow_calls(collection, cast.value),
        ExpressionNode::Indexed(indexed) => {
            collect_expression_borrow_calls(collection, indexed.collection);
            collect_expression_borrow_calls(collection, indexed.index);
        }
        ExpressionNode::Member(member) => {
            collect_expression_borrow_calls(collection, member.receiver)
        }
        ExpressionNode::Borrow(inner_expression) => {
            collect_expression_borrow_calls(collection, inner_expression.target)
        }
        ExpressionNode::Unary(unary) => collect_expression_borrow_calls(collection, unary.operand),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in collection
                .program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_expression_borrow_calls(collection, field.value);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
