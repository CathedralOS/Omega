//! Payloads a transition arm's dispatch already copied out of its subject.
//!
//! A dispatch evaluates its subject once and extracts payload bindings from
//! that saved subject (`wiki/spec/language/patterns.md`). The parser rewrites
//! each destructured payload binding into a case-qualified projection of the
//! subject (`TableMemberExpression::case_variant`), and the arm's guard tests
//! the same subject place for that case. A plain-copy payload is therefore
//! copied when the arm is selected, before the target evaluates any argument:
//! its row ends before access 0 instead of observing the subject alongside a
//! later loan of the subject's root.
//!
//! Only the arm target's own roster carries these selections. Authored
//! projections have no case qualifier, and loans of a payload or non-copy
//! payloads keep their ordinary accesses. Nested calls' own rosters, which
//! stage 05 replays independently, keep their argument-order ends.

use super::BorrowAccessPlace;
use super::collection::BorrowAccessCollection;
use checked_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use checked_trees::statement::{StatementNode, TransitionGuardNode};
use symbols::SymbolHandle;

/// The subject places and cases a transition arm's guard selected.
#[derive(Default)]
pub(super) struct DispatchExtraction {
    selected_cases: Vec<(BorrowAccessPlace, SymbolHandle)>,
}

impl DispatchExtraction {
    /// Read the case tests of the transition arm at `statement_index`.
    pub(super) fn of_transition_arm(
        program: &typed_trees::TypedTrees,
        state_symbol: SymbolHandle,
        statement_index: usize,
        machine_symbol: SymbolHandle,
    ) -> Self {
        let mut extraction = Self::default();
        let Some(state) = crate::semantic::calls::find_state(program, state_symbol) else {
            return extraction;
        };
        let Some(StatementNode::Transition(transition)) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index)
        else {
            return extraction;
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            return extraction;
        };
        // Destructure arms conjoin each subject's case test with field
        // equalities and the authored `if` guard; only the case tests select.
        let mut conjuncts = vec![guard];
        while let Some(conjunct) = conjuncts.pop() {
            let ExpressionNode::Binary(binary) = program.expression_table.expression(conjunct)
            else {
                continue;
            };
            match binary.operator {
                BinaryOperator::And => conjuncts.extend([binary.left, binary.right]),
                BinaryOperator::Equal => {
                    let ExpressionNode::Name(case) =
                        program.expression_table.expression(binary.right)
                    else {
                        continue;
                    };
                    if validation::exact_case_reference_owner(program, binary.right).is_none() {
                        continue;
                    }
                    if let Some(subject) = super::borrow_access_place(
                        program,
                        state_symbol,
                        statement_index,
                        binary.left,
                        machine_symbol,
                    ) {
                        extraction.selected_cases.push((subject, case.symbol));
                    }
                }
                _ => {}
            }
        }
        extraction
    }

    /// Whether `expression` reads within a plain-copy payload this arm's
    /// dispatch extracted from a selected case of its saved subject.
    pub(super) fn reads_extracted_copy(
        &self,
        collection: &BorrowAccessCollection<'_>,
        mut expression: ExpressionHandle,
    ) -> bool {
        if self.selected_cases.is_empty() {
            return false;
        }
        let program = collection.program;
        let (payload, receiver) = loop {
            match program.expression_table.expression(expression) {
                ExpressionNode::Member(member) if member.case_variant.is_some() => {
                    break (expression, member.receiver);
                }
                ExpressionNode::Member(member) => expression = member.receiver,
                ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                _ => return false,
            }
        };
        let (Some(subject), Some(payload_place)) = (
            collection.borrow_access_place(receiver),
            collection.borrow_access_place(payload),
        ) else {
            return false;
        };
        let Some(
            [
                facts::PlaceSegment::Case { variant },
                facts::PlaceSegment::Field { .. },
            ],
        ) = payload_place
            .segments
            .strip_prefix(subject.segments.as_slice())
        else {
            return false;
        };
        payload_place.root_symbol == subject.root_symbol
            && self
                .selected_cases
                .iter()
                .any(|(selected, case)| case == variant && *selected == subject)
            && collection.is_plain_copy(payload)
    }
}
