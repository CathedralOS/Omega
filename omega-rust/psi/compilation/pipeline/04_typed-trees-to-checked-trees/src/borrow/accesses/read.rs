use crate::borrow::accesses::BorrowAccessCollection;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

/// Evaluate a loan's place without inventing a copied read of its referent.
/// Selectors and computed receivers still perform their ordinary accesses
/// before the loan is retained for invocation.
pub(super) fn collect_place_evaluation_reads(
    collection: &mut BorrowAccessCollection<'_>,
    expression: ExpressionHandle,
) {
    let program = collection.program;
    // Shared child traversal reads cast operands as values. A loan instead
    // evaluates the place beneath its cast, retaining selectors and computed
    // receivers without inventing another read of the lent referent.
    match program.expression_table.expression(expression) {
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
        _ => {
            if !super::super::children::recurse_shared_children(program, expression, &mut |child| {
                collect_read_accesses(collection, child)
            }) {
                collect_read_accesses(collection, expression);
            }
        }
    }
}

pub(super) fn collect_read_accesses(
    collection: &mut BorrowAccessCollection<'_>,
    expression: ExpressionHandle,
) {
    if super::super::children::recurse_shared_children(
        collection.program,
        expression,
        &mut |child| {
            collect_read_accesses(collection, child);
        },
    ) {
        return;
    }
    match collection.program.expression_table.expression(expression) {
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
                collection.state_symbol,
                collection.statement_index,
                validation::call_target_symbol(collection.program, call),
            );
            collection.finish_nested_call(first_access, expression);
        }
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
        ExpressionNode::Borrow(_) => {
            super::collect_single_expression_argument_access(collection, expression)
        }
        _ => {}
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
