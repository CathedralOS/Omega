use crate::borrow::accesses::BorrowAccessCollection;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

/// Evaluate a loan's place without inventing a copied read of its referent.
/// Selectors and computed receivers still perform their ordinary accesses
/// before the loan is retained for invocation.
pub(super) fn collect_place_evaluation_reads(
    collection: &mut BorrowAccessCollection<'_>,
    expression: ExpressionHandle,
) {
    match collection.program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {}
        ExpressionNode::Member(member) => {
            collect_place_evaluation_reads(collection, member.receiver);
        }
        ExpressionNode::Indexed(indexed) => {
            collect_place_evaluation_reads(collection, indexed.collection);
            collect_read_accesses(collection, indexed.index);
        }
        ExpressionNode::Borrow(borrow) => {
            collect_place_evaluation_reads(collection, borrow.target);
        }
        ExpressionNode::Cast(cast) => {
            collect_place_evaluation_reads(collection, cast.value);
        }
        _ => collect_read_accesses(collection, expression),
    }
}

pub(super) fn collect_read_accesses(
    collection: &mut BorrowAccessCollection<'_>,
    expression: ExpressionHandle,
) {
    match collection.program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            collect_read_accesses(collection, dispatch.subject);
            // This is a may-read summary, not an execution or move schedule.
            // Each arm retains result loans; only copied reads and loans
            // internal to a completed nested call expire within that arm.
            for arm in collection
                .program
                .expression_table
                .match_arms(dispatch.arms)
            {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    collect_read_accesses(collection, pattern);
                }
                collect_read_accesses(collection, arm.value);
            }
        }
        ExpressionNode::Atomic(atomic) => collect_read_accesses(collection, atomic.value),
        ExpressionNode::ArrayLiteral(values) => {
            for value in collection
                .program
                .expression_table
                .expression_handles(*values)
            {
                collect_read_accesses(collection, *value);
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                collect_read_accesses(collection, range.start);
            }
            if range.end.is_valid() {
                collect_read_accesses(collection, range.end);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_read_accesses(collection, binary.left);
            collect_read_accesses(collection, binary.right);
        }
        ExpressionNode::Call(call) => {
            let first_access = collection.access_count();
            // Static qualification selects a declaration, not a runtime place.
            // Keep real receivers and every actual in evaluation order; the
            // resolved target determines whether a qualifier is only lexical.
            let no_runtime_receiver = collection
                .program
                .call_has_no_runtime_receiver_for_target(call);
            if call.receiver.is_valid() && !no_runtime_receiver {
                collect_read_accesses(collection, call.receiver);
                collection.retain_receiver_accesses(first_access);
            }

            super::collect_target_argument_accesses(
                collection,
                collection
                    .program
                    .expression_table
                    .expression_handles(call.arguments),
                call.target_symbol,
            );
            collection.finish_nested_call(first_access, expression);
        }
        ExpressionNode::Cast(cast) => collect_read_accesses(collection, cast.value),
        ExpressionNode::Indexed(indexed) => {
            if !append_read_access(collection, expression) {
                // A computed collection has no retained source place, but its
                // evaluation still observes the receiver and call operands.
                collect_read_accesses(collection, indexed.collection);
            }

            collect_read_accesses(collection, indexed.index);
        }
        ExpressionNode::Member(member) => {
            if !append_read_access(collection, expression)
                && !(member.case_variant.is_some() && collection.reads_extracted_copy(expression))
            {
                // A copied dispatch payload reads its saved subject, not a
                // fresh invocation of the expression that produced it. Place
                // subjects retain their completed observation row above;
                // computed subjects have no additional caller-storage read.
                collect_read_accesses(collection, member.receiver);
            }
        }
        ExpressionNode::Name(_) => {
            append_read_access(collection, expression);
        }
        ExpressionNode::Borrow(_) => super::collect_argument_accesses(collection, expression),
        ExpressionNode::Unary(unary) => collect_read_accesses(collection, unary.operand),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in collection
                .program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_read_accesses(collection, field.value);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn append_read_access(
    collection: &mut BorrowAccessCollection<'_>,
    expression: ExpressionHandle,
) -> bool {
    let Some(access_place) = collection.borrow_access_place(expression) else {
        return false;
    };

    collection.append_read_access(access_place, expression);
    true
}
