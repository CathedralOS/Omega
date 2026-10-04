//! Payloads a transition arm's dispatch already copied out of its subject.
//!
//! A dispatch evaluates its subject once and extracts payload bindings from
//! that saved subject (`wiki/spec/language/patterns.md`). The parser rewrites
//! each destructured payload binding into a case-qualified projection of the
//! subject (`TableMemberExpression::case_variant`), and the arm's guard tests
//! the same subject expression for that case. A plain-copy payload is therefore
//! copied when the arm is selected, before the target evaluates any argument:
//! its row ends before access 0 instead of observing the subject alongside a
//! later loan of the subject's root.
//! Computed subjects have no source place: their payload reads must not replay
//! the subject's calls or operand loans as part of the selected arm.
//!
//! Only the arm target's own roster carries these selections. Authored
//! projections have no case qualifier, and loans of a payload or non-copy
//! payloads keep their ordinary accesses. Nested calls' own rosters, which
//! stage 05 replays independently, keep their argument-order ends.

use checked_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use checked_trees::statement::{StatementNode, TransitionGuardNode};
use symbols::SymbolHandle;

/// The saved subject expressions and cases a transition arm's guard selected.
#[derive(Default)]
pub struct DispatchExtraction {
    selected_cases: Vec<(ExpressionHandle, SymbolHandle)>,
}

impl DispatchExtraction {
    /// Read the case tests of the transition arm at `statement_index`.
    pub fn of_transition_arm(
        program: &typed_trees::TypedTrees,
        state: &typed_trees::state::State,
        statement_index: usize,
    ) -> Self {
        let mut extraction = Self::default();
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
                    if crate::exact_case_reference_owner(program, binary.right).is_none() {
                        continue;
                    }
                    extraction.selected_cases.push((binary.left, case.symbol));
                }
                _ => {}
            }
        }
        extraction
    }

    /// Whether `expression` reads within a plain-copy payload this arm's
    /// dispatch extracted from a selected case of its saved subject.
    pub fn reads_extracted_copy(
        &self,
        program: &typed_trees::TypedTrees,
        mut expression: ExpressionHandle,
        is_plain_copy: impl Fn(ExpressionHandle) -> bool,
    ) -> bool {
        if self.selected_cases.is_empty() {
            return false;
        }
        let (payload, member) = loop {
            match program.expression_table.expression(expression) {
                ExpressionNode::Member(member) if member.case_variant.is_some() => {
                    break (expression, member);
                }
                ExpressionNode::Member(member) => expression = member.receiver,
                ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                _ => return false,
            }
        };
        // A subject need not denote storage: a completed call or literal also
        // produces a saved value. The case-qualified projection is parser-
        // generated and must rejoin that exact selected subject, not merely a
        // call returning the same type. Lowering may clone its expression tree.
        self.selected_cases.iter().any(|(selected, case)| {
            if !program
                .expression_table
                .expressions_structurally_equal(*selected, member.receiver)
            {
                return false;
            }
            let Some(owner) = program.data_definition_by_symbol(program.symbols.get(*case).parent)
            else {
                return false;
            };
            let Some(variant) = program
                .data_members(owner)
                .iter()
                .find_map(|row| match row {
                    typed_trees::data::DataMember::Variant(variant) if variant.symbol == *case => {
                        Some(variant)
                    }
                    _ => None,
                })
            else {
                return false;
            };
            // Payload names can repeat in sibling cases. Resolve only within
            // the guard's exact selected case, retaining any bound field identity.
            member.case_variant.as_ref() == Some(&variant.name)
                && program.data_payload_fields(variant).iter().any(|field| {
                    field.name == member.member
                        && (!member.member_symbol.is_valid()
                            || member.member_symbol == field.symbol)
                })
        }) && is_plain_copy(payload)
    }
}
