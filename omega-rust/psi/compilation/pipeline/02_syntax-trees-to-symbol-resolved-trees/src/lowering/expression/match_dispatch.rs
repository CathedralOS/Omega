//! Resolve one capture definition and selected-arm aliases without introducing
//! synthetic locals. Alias definitions are scoped by generational handles;
//! equal spellings or equivalent-looking subjects cannot select another arm.

use super::{expression_table, lower_expression_into_table, lower_name};
use crate::resolution::lowerer::{Lowerer, MatchArmCapture};
use arena::Handle;
use diagnostics::Diagnostic;
use symbol_resolved_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableDestructurePattern, TableMatchArm,
    TableMatchExpression, TableMatchField, TableSavedSubject,
};
use syntax_trees::{SyntaxTrees, expression as syntax};

pub(super) fn lower_match(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    dispatch: syntax::TableMatchExpression,
) -> Result<ExpressionHandle, Diagnostic> {
    let subject = lower_expression_into_table(lowerer, syntax_trees, dispatch.subject)?;
    let capture = if dispatch.capture.is_valid() {
        if syntax_trees
            .expressions
            .match_subject(dispatch.capture)
            .subject
            != dispatch.subject
        {
            return Err(Diagnostic::error(
                "match subject capture does not name its defining subject",
            ));
        }
        expression_table(lowerer).insert_match_subject(subject)
    } else {
        Handle::invalid()
    };
    let mut arms = Vec::new();
    for arm in syntax_trees.expressions.match_arms(dispatch.arms) {
        let pattern = match arm.pattern {
            syntax::MatchPattern::Value(value) => {
                MatchPattern::Value(lower_expression_into_table(lowerer, syntax_trees, value)?)
            }
            syntax::MatchPattern::Wildcard => MatchPattern::Wildcard,
            syntax::MatchPattern::Destructure(pattern) => {
                let head = lower_expression_into_table(lowerer, syntax_trees, pattern.head)?;
                let mut fields = Vec::new();
                for field in syntax_trees.expressions.match_fields(pattern.fields) {
                    use symbol_resolved_trees::expression::MatchFieldAction;
                    let action = match &field.action {
                        syntax::MatchFieldAction::Bind(binding) => {
                            MatchFieldAction::Bind(lower_name(syntax_trees, binding))
                        }
                        syntax::MatchFieldAction::Equal(value) => MatchFieldAction::Equal(
                            lower_expression_into_table(lowerer, syntax_trees, *value)?,
                        ),
                        syntax::MatchFieldAction::Waive => MatchFieldAction::Waive,
                    };
                    fields.push(TableMatchField {
                        member: lower_name(syntax_trees, &field.member),
                        field_symbol: Handle::invalid(),
                        action,
                    });
                }
                let selectors = syntax_trees
                    .statements
                    .outcome_proof_selectors(pattern.proof_selectors)
                    .iter()
                    .map(
                        |selector| symbol_resolved_trees::statement::OutcomeProofSelector {
                            output_field: lower_name(syntax_trees, &selector.output_field),
                            binding: lower_name(syntax_trees, &selector.binding),
                        },
                    );
                let proof_selectors =
                    expression_table(lowerer).insert_match_proof_selectors(selectors);
                lowerer.expression_match_captures.push(MatchArmCapture {
                    syntax_capture: dispatch.capture,
                    syntax_head: pattern.head,
                    resolved_capture: capture,
                    resolved_head: head,
                });
                let guard = lower_expression_into_table(lowerer, syntax_trees, pattern.guard);
                lowerer.expression_match_captures.pop();
                let guard = guard?;
                let saved = expression_table(lowerer).insert(ExpressionNode::SavedSubject(
                    TableSavedSubject {
                        capture,
                        pattern_head: head,
                    },
                ));
                let mut condition = expression_table(lowerer).insert(ExpressionNode::Boolean(true));
                for field in &fields {
                    if let symbol_resolved_trees::expression::MatchFieldAction::Equal(expected) =
                        field.action
                    {
                        let projected = expression_table(lowerer).insert(ExpressionNode::Member(
                            symbol_resolved_trees::expression::TableMemberExpression {
                                receiver: saved,
                                member_symbol: field.field_symbol,
                                member: field.member.clone(),
                                case_variant: None,
                            },
                        ));
                        let equality = expression_table(lowerer).insert(ExpressionNode::Binary(
                            symbol_resolved_trees::expression::TableBinaryExpression {
                                left: projected,
                                operator: symbol_resolved_trees::expression::BinaryOperator::Equal,
                                right: expected,
                            },
                        ));
                        condition = expression_table(lowerer).insert(ExpressionNode::Binary(
                            symbol_resolved_trees::expression::TableBinaryExpression {
                                left: condition,
                                operator: symbol_resolved_trees::expression::BinaryOperator::And,
                                right: equality,
                            },
                        ));
                    }
                }
                if guard.is_valid() {
                    condition = expression_table(lowerer).insert(ExpressionNode::Binary(
                        symbol_resolved_trees::expression::TableBinaryExpression {
                            left: condition,
                            operator: symbol_resolved_trees::expression::BinaryOperator::And,
                            right: guard,
                        },
                    ));
                }
                let fields = expression_table(lowerer).insert_match_fields(fields);
                MatchPattern::Destructure(TableDestructurePattern {
                    head,
                    fields,
                    has_rest: pattern.has_rest,
                    guard,
                    condition,
                    proof_selectors,
                })
            }
        };
        let has_aliases = if let (
            syntax::MatchPattern::Destructure(source),
            MatchPattern::Destructure(resolved),
        ) = (arm.pattern, pattern)
        {
            lowerer.expression_match_captures.push(MatchArmCapture {
                syntax_capture: dispatch.capture,
                syntax_head: source.head,
                resolved_capture: capture,
                resolved_head: resolved.head,
            });
            true
        } else {
            false
        };
        let value = lower_expression_into_table(lowerer, syntax_trees, arm.value);
        if has_aliases {
            lowerer.expression_match_captures.pop();
        }
        arms.push(TableMatchArm {
            pattern,
            value: value?,
            source_span: arm.source_span,
        });
    }
    let arms = expression_table(lowerer).insert_match_arms(arms);
    Ok(
        expression_table(lowerer).insert(ExpressionNode::Match(TableMatchExpression {
            subject,
            capture,
            arms,
        })),
    )
}

pub(super) fn lower_saved_subject(
    lowerer: &mut Lowerer,
    saved: syntax::TableSavedSubject,
) -> Result<ExpressionHandle, Diagnostic> {
    let binding = lowerer
        .expression_match_captures
        .iter()
        .rev()
        .find(|binding| {
            binding.syntax_capture == saved.capture && binding.syntax_head == saved.pattern_head
        })
        .copied()
        .ok_or_else(|| {
            Diagnostic::error("saved match subject read has no enclosing arm capture")
        })?;
    Ok(
        expression_table(lowerer).insert(ExpressionNode::SavedSubject(TableSavedSubject {
            capture: binding.resolved_capture,
            pattern_head: binding.resolved_head,
        })),
    )
}
