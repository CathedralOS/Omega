//! Resolve one capture definition and selected-arm aliases without introducing
//! synthetic locals. Alias definitions are scoped by generational handles;
//! equal spellings or equivalent-looking subjects cannot select another arm.

use super::{expression_table, lower_expression_into_table, lower_name};
use crate::resolution::lowerer::{Lowerer, MatchArmCapture};
use crate::tables::syntax_expressions;
use arena::Handle;
use diagnostics::Diagnostic;
use symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern,
    TableBinaryExpression, TableDestructurePattern, TableMatchArm, TableMatchExpression,
    TableMatchField, TableMemberExpression, TableSavedSubject,
};
use symbol_resolved_trees::statement::OutcomeProofSelector;
use syntax_trees::{SyntaxTrees, expression as syntax};

pub(super) fn lower_match(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    dispatch: syntax::TableMatchExpression,
) -> Result<ExpressionHandle, Diagnostic> {
    let subject = lower_expression_into_table(lowerer, syntax_trees, dispatch.subject)?;
    let capture = if dispatch.capture.is_valid() {
        if syntax_expressions(syntax_trees)
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
            syntax::MatchPattern::Destructure(pattern) => lower_destructure_pattern(
                lowerer,
                syntax_trees,
                pattern,
                dispatch.capture,
                capture,
            )?,
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

/// Declaration-input preparation lowers selected calls before executable
/// bodies. Preserve the current transition's lexical aliases while doing so;
/// this installs no statements and does not complete or admit the machine.
pub(in crate::lowering) fn with_transition_pattern_captures<T>(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: syntax_trees::statement::StatementHandle,
    operation: impl FnOnce(&mut Lowerer) -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    let outer = lowerer.expression_match_captures.len();
    let result = (|| {
        if let syntax_trees::statement::StatementNode::Transition(transition) =
            syntax_trees.statements.statement(statement)
        {
            for retained in syntax_trees
                .statements
                .transition_patterns(transition.patterns)
            {
                let subject = lower_expression_into_table(lowerer, syntax_trees, retained.subject)?;
                let head =
                    lower_expression_into_table(lowerer, syntax_trees, retained.pattern.head)?;
                let capture = expression_table(lowerer).insert_transition_subject(subject);
                lowerer.expression_match_captures.push(MatchArmCapture {
                    syntax_capture: retained.capture,
                    syntax_head: retained.pattern.head,
                    resolved_capture: capture,
                    resolved_head: head,
                });
            }
        }
        operation(lowerer)
    })();
    lowerer.expression_match_captures.truncate(outer);
    result
}

pub(super) fn lower_saved_subject(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
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
            let head = match syntax_trees.expressions.expression(saved.pattern_head) {
                syntax::ExpressionNode::Name(path) => syntax_trees.expressions
                    .identifier_path_members(*path).iter()
                    .map(|name| name.as_str(syntax_trees)).collect::<Vec<_>>().join("::"),
                _ => "<non-name>".to_owned(),
            };
            Diagnostic::error(format!(
                "saved match subject read has no enclosing arm capture in `{}::{}` for `{head}`: capture {:?}, head {:?}, purpose {:?}, enclosing {:?}",
                lowerer.current_machine_name.as_deref().unwrap_or("<declaration>"),
                lowerer.current_state_name.as_deref().unwrap_or("<signature>"),
                saved.capture, saved.pattern_head,
                syntax_trees.expressions.match_subject(saved.capture).purpose,
                lowerer.expression_match_captures.iter().map(|binding| (binding.syntax_capture,binding.syntax_head)).collect::<Vec<_>>(),
            ))
        })?;
    Ok(
        expression_table(lowerer).insert(ExpressionNode::SavedSubject(TableSavedSubject {
            capture: binding.resolved_capture,
            pattern_head: binding.resolved_head,
        })),
    )
}

pub(crate) fn lower_destructure_pattern(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    pattern: syntax::TableDestructurePattern,
    syntax_capture: Handle<syntax::MatchSubjectCapture>,
    capture: Handle<symbol_resolved_trees::expression::MatchSubjectCapture>,
) -> Result<MatchPattern, Diagnostic> {
    let application_type = if pattern.application_type.is_valid() {
        let reference = crate::lowering::type_reference::lower_type_reference_handle(
            lowerer,
            syntax_trees,
            pattern.application_type,
        )?;
        lowerer
            .declaration_child_type_references()
            .append(reference)
    } else {
        Default::default()
    };
    let head = lower_expression_into_table(lowerer, syntax_trees, pattern.head)?;
    let mut fields = Vec::new();
    for field in syntax_trees.expressions.match_fields(pattern.fields) {
        use symbol_resolved_trees::expression::MatchFieldAction;
        let action = match &field.action {
            syntax::MatchFieldAction::Bind(binding) => {
                MatchFieldAction::Bind(lower_name(syntax_trees, binding))
            }
            syntax::MatchFieldAction::Equal(value) => {
                MatchFieldAction::Equal(lower_expression_into_table(lowerer, syntax_trees, *value)?)
            }
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
        .map(|selector| OutcomeProofSelector {
            output_field: lower_name(syntax_trees, &selector.output_field),
            binding: lower_name(syntax_trees, &selector.binding),
        });
    let proof_selectors = expression_table(lowerer).insert_match_proof_selectors(selectors);
    lowerer.expression_match_captures.push(MatchArmCapture {
        syntax_capture,
        syntax_head: pattern.head,
        resolved_capture: capture,
        resolved_head: head,
    });
    let guard = lower_expression_into_table(lowerer, syntax_trees, pattern.guard);
    lowerer.expression_match_captures.pop();
    let guard = guard?;
    let saved = expression_table(lowerer).insert(ExpressionNode::SavedSubject(TableSavedSubject {
        capture,
        pattern_head: head,
    }));
    let mut condition = ExpressionHandle::invalid();
    for field in &fields {
        if let MatchFieldAction::Equal(expected) = field.action {
            let projected =
                expression_table(lowerer).insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: saved,
                    member_symbol: field.field_symbol,
                    member: field.member.clone(),
                    case_variant: None,
                }));
            let equality =
                expression_table(lowerer).insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: projected,
                    operator: BinaryOperator::Equal,
                    right: expected,
                }));
            condition = if condition.is_valid() {
                expression_table(lowerer).insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: condition,
                    operator: BinaryOperator::And,
                    right: equality,
                }))
            } else {
                equality
            };
        }
    }
    if guard.is_valid() {
        condition = if condition.is_valid() {
            expression_table(lowerer).insert(ExpressionNode::Binary(TableBinaryExpression {
                left: condition,
                operator: BinaryOperator::And,
                right: guard,
            }))
        } else {
            guard
        };
    }
    if !condition.is_valid() {
        condition = expression_table(lowerer).insert(ExpressionNode::Boolean(true));
    }
    let fields = expression_table(lowerer).insert_match_fields(fields);
    Ok(MatchPattern::Destructure(TableDestructurePattern {
        application_type,
        head,
        fields,
        has_rest: pattern.has_rest,
        guard,
        condition,
        proof_selectors,
    }))
}
