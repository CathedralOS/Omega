//! Lexical payload aliases shared by both dispatch forms.
//! Names are substituted only in their arm scope. The receiver may be a saved
//! subject read; it is never a newly inferred owner or reference grant.

use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableAtomicExpression, TableBinaryExpression,
    TableCallExpression, TableCastExpression, TableIndexedExpression, TableMemberExpression,
    TableMembershipExpression, TableRangeExpression, TableStructLiteral, TableStructLiteralField,
    TableUnaryExpression,
};
use syntax_trees::identifier::Identifier;

type SubjectCopies = Vec<(
    Handle<syntax_trees::expression::MatchSubjectCapture>,
    Handle<syntax_trees::expression::MatchSubjectCapture>,
)>;

#[derive(Clone)]
pub(crate) struct DestructureBinding {
    pub(crate) binding: Identifier,
    pub(crate) member: Identifier,
    pub(crate) case_variant: Option<Identifier>,
}

pub(crate) fn rewrite_destructure_guard_expression(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
    fields: &[DestructureBinding],
) -> ExpressionHandle {
    rewrite_expression(syntax_trees, expression, subject, fields, &mut Vec::new())
}

fn rewrite_expression(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
    fields: &[DestructureBinding],
    captures: &mut SubjectCopies,
) -> ExpressionHandle {
    let rewritten = match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Match(dispatch) => {
            let saved_subject =
                rewrite_expression(syntax_trees, dispatch.subject, subject, fields, captures);
            let outer_captures = captures.len();
            let capture = if dispatch.capture.is_valid() {
                let copied = syntax_trees.expressions.insert_match_subject(saved_subject);
                captures.push((dispatch.capture, copied));
                copied
            } else {
                Handle::invalid()
            };
            let mut arms = syntax_trees.expressions.match_arms(dispatch.arms).to_vec();
            for arm in &mut arms {
                use syntax_trees::expression::{MatchFieldAction, MatchPattern};
                let visible_fields = match &mut arm.pattern {
                    MatchPattern::Value(pattern) => {
                        *pattern =
                            rewrite_expression(syntax_trees, *pattern, subject, fields, captures);
                        fields.to_vec()
                    }
                    MatchPattern::Destructure(pattern) => {
                        // Heads are type/case names, not value uses. Equality
                        // operands see the outer scope; the guard and body see
                        // this arm's bindings, which shadow outer aliases.
                        let mut pattern_fields = syntax_trees
                            .expressions
                            .match_fields(pattern.fields)
                            .to_vec();
                        let visible = fields.iter().filter(|outer| {
                            !pattern_fields.iter().any(|inner| matches!(&inner.action, MatchFieldAction::Bind(binding) if binding.as_str(syntax_trees) == outer.binding.as_str(syntax_trees)))
                                && !syntax_trees.statements.outcome_proof_selectors(pattern.proof_selectors).iter().any(|selector| selector.binding.as_str(syntax_trees) == outer.binding.as_str(syntax_trees))
                        }).cloned().collect::<Vec<_>>();
                        for field in &mut pattern_fields {
                            if let MatchFieldAction::Equal(value) = &mut field.action {
                                *value = rewrite_expression(
                                    syntax_trees,
                                    *value,
                                    subject,
                                    fields,
                                    captures,
                                );
                            }
                        }
                        pattern.fields =
                            syntax_trees.expressions.insert_match_fields(pattern_fields);
                        pattern.guard = rewrite_optional_expression(
                            syntax_trees,
                            pattern.guard,
                            subject,
                            &visible,
                            captures,
                        );
                        visible
                    }
                    MatchPattern::Wildcard => fields.to_vec(),
                };
                arm.value =
                    rewrite_expression(syntax_trees, arm.value, subject, &visible_fields, captures);
            }
            captures.truncate(outer_captures);
            let arms = syntax_trees.expressions.insert_match_arms(arms);
            ExpressionNode::Match(syntax_trees::expression::TableMatchExpression {
                subject: saved_subject,
                arms,
                capture,
            })
        }
        ExpressionNode::SavedSubject(saved) => {
            ExpressionNode::SavedSubject(syntax_trees::expression::TableSavedSubject {
                capture: captures
                    .iter()
                    .rev()
                    .find_map(|(source, copied)| (*source == saved.capture).then_some(*copied))
                    .unwrap_or(saved.capture),
                pattern_head: saved.pattern_head,
            })
        }
        ExpressionNode::Atomic(atomic) => ExpressionNode::Atomic(TableAtomicExpression {
            value: rewrite_expression(syntax_trees, atomic.value, subject, fields, captures),
            result: if atomic.result.is_valid() {
                rewrite_expression(syntax_trees, atomic.result, subject, fields, captures)
            } else {
                ExpressionHandle::invalid()
            },
            ordering: atomic.ordering,
            result_custody: atomic.result_custody,
        }),
        ExpressionNode::ArrayLiteral(values) => {
            let source_values = syntax_trees.expressions.expression_handles(values).to_vec();
            let values = source_values
                .iter()
                .map(|value| rewrite_expression(syntax_trees, *value, subject, fields, captures))
                .collect::<Vec<_>>();
            ExpressionNode::ArrayLiteral(syntax_trees.expressions.insert_expression_handles(values))
        }
        ExpressionNode::Binary(binary) => ExpressionNode::Binary(TableBinaryExpression {
            left: rewrite_expression(syntax_trees, binary.left, subject, fields, captures),
            operator: binary.operator,
            right: rewrite_expression(syntax_trees, binary.right, subject, fields, captures),
        }),
        ExpressionNode::Call(call) => ExpressionNode::Call(TableCallExpression {
            target_is_static: call.target_is_static,
            receiver: if call.target_is_static {
                call.receiver
            } else {
                rewrite_optional_expression(syntax_trees, call.receiver, subject, fields, captures)
            },
            target: call.target,
            machine_arguments: call.machine_arguments,
            arguments: rewrite_expression_span(
                syntax_trees,
                call.arguments,
                subject,
                fields,
                captures,
            ),
            evidence_arguments: call.evidence_arguments,
            operational_acknowledgement: call.operational_acknowledgement,
        }),
        ExpressionNode::Cast(cast) => ExpressionNode::Cast(TableCastExpression {
            value: rewrite_expression(syntax_trees, cast.value, subject, fields, captures),
            target_type: cast.target_type,
            target_label: cast.target_label,
            domain: cast.domain,
            semantic_domain: cast.semantic_domain,
            semantic_domain_arguments: cast.semantic_domain_arguments,
            form: cast.form,
        }),
        ExpressionNode::Indexed(indexed) => ExpressionNode::Indexed(TableIndexedExpression {
            collection: rewrite_expression(
                syntax_trees,
                indexed.collection,
                subject,
                fields,
                captures,
            ),
            index: rewrite_expression(syntax_trees, indexed.index, subject, fields, captures),
        }),
        ExpressionNode::Membership(membership) => {
            ExpressionNode::Membership(TableMembershipExpression {
                value: rewrite_expression(
                    syntax_trees,
                    membership.value,
                    subject,
                    fields,
                    captures,
                ),
                domain: membership.domain,
            })
        }
        ExpressionNode::Member(member) => ExpressionNode::Member(TableMemberExpression {
            receiver: rewrite_expression(syntax_trees, member.receiver, subject, fields, captures),
            member: member.member,
            case_variant: member.case_variant,
        }),
        ExpressionNode::Borrow(inner) => {
            ExpressionNode::Borrow(syntax_trees::expression::TableBorrowExpression {
                target: rewrite_expression(syntax_trees, inner.target, subject, fields, captures),
                access: inner.access,
            })
        }
        ExpressionNode::Name(path) => {
            if let Some((field, case_variant)) =
                single_destructured_field_name(syntax_trees, path, fields)
            {
                ExpressionNode::Member(TableMemberExpression {
                    receiver: subject,
                    member: field,
                    case_variant,
                })
            } else {
                ExpressionNode::Name(path)
            }
        }
        ExpressionNode::Range(range) => ExpressionNode::Range(TableRangeExpression {
            start: rewrite_optional_expression(
                syntax_trees,
                range.start,
                subject,
                fields,
                captures,
            ),
            end: rewrite_optional_expression(syntax_trees, range.end, subject, fields, captures),
            end_inclusive: range.end_inclusive,
        }),
        ExpressionNode::StructLiteral(struct_literal) => {
            let source_fields = syntax_trees
                .expressions
                .struct_fields(struct_literal.fields)
                .to_vec();
            let struct_fields = source_fields
                .iter()
                .map(|field| TableStructLiteralField {
                    name: field.name.clone(),
                    value: rewrite_expression(syntax_trees, field.value, subject, fields, captures),
                })
                .collect::<Vec<_>>();
            ExpressionNode::StructLiteral(TableStructLiteral {
                constructor_name: struct_literal.constructor_name,
                fields: syntax_trees.expressions.insert_struct_fields(struct_fields),
            })
        }
        ExpressionNode::Unary(unary) => ExpressionNode::Unary(TableUnaryExpression {
            operator: unary.operator,
            operand: rewrite_expression(syntax_trees, unary.operand, subject, fields, captures),
        }),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::TypeExpression(_)
        | ExpressionNode::ZeroValue(_) => syntax_trees.expressions.expression(expression).clone(),
    };

    let source_span = syntax_trees.expressions.source_span(expression);
    let rewritten = syntax_trees.expressions.insert(rewritten);
    syntax_trees
        .expressions
        .set_source_span(rewritten, source_span);
    rewritten
}

fn rewrite_optional_expression(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
    fields: &[DestructureBinding],
    captures: &mut SubjectCopies,
) -> ExpressionHandle {
    if expression.is_valid() {
        rewrite_expression(syntax_trees, expression, subject, fields, captures)
    } else {
        expression
    }
}

fn rewrite_expression_span(
    syntax_trees: &mut SyntaxTrees,
    expressions: HandleSpan<ExpressionHandle>,
    subject: ExpressionHandle,
    fields: &[DestructureBinding],
    captures: &mut SubjectCopies,
) -> HandleSpan<ExpressionHandle> {
    let source_expressions = syntax_trees
        .expressions
        .expression_handles(expressions)
        .to_vec();
    let expressions = source_expressions
        .iter()
        .map(|expression| rewrite_expression(syntax_trees, *expression, subject, fields, captures))
        .collect::<Vec<_>>();
    syntax_trees
        .expressions
        .insert_expression_handles(expressions)
}

/// The `(field member name, case variant)` a single-name reference resolves to
/// when it names a destructure binding -- so the rewritten `subject.member`
/// access carries the variant for correct payload-field offset resolution.
fn single_destructured_field_name(
    syntax_trees: &SyntaxTrees,
    path: HandleSpan<Identifier>,
    fields: &[DestructureBinding],
) -> Option<(Identifier, Option<Identifier>)> {
    let members = syntax_trees.expressions.identifier_path_members(path);
    let [member] = members else {
        return None;
    };
    fields
        .iter()
        .find(|field| field.binding.as_str(syntax_trees) == member.as_str(syntax_trees))
        .map(|field| (field.member.clone(), field.case_variant.clone()))
}
