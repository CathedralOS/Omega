use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchSubjectCapture, TableAtomicExpression,
    TableBinaryExpression, TableBlockExpression, TableBorrowExpression, TableCallExpression,
    TableCastExpression, TableIndexedExpression, TableMemberExpression, TableMembershipExpression,
    TableRangeExpression, TableSavedSubject, TableStructLiteral, TableStructLiteralField,
    TableUnaryExpression,
};
use syntax_trees::identifier::Identifier;

pub(crate) fn bare_binding(trees: &SyntaxTrees, head: ExpressionHandle) -> Option<Identifier> {
    if !head.is_valid() {
        return None;
    }
    let ExpressionNode::Name {
        members,
        application: None,
    } = trees.expressions.expression(head)
    else {
        return None;
    };
    let [name] = trees.expressions.identifier_path_members(*members) else {
        return None;
    };
    Some(name.clone())
}

type SubjectCopies = Vec<(Handle<MatchSubjectCapture>, Handle<MatchSubjectCapture>)>;

#[derive(Clone)]
pub(crate) struct DestructureBinding {
    pub(crate) component: language_core::PatternComponent,
    pub(crate) binding: Identifier,
    pub(crate) member: Option<Identifier>,
    /// Set when a positional pattern introduced the binding: each use reads
    /// the field at this place, and `member` names no field.
    pub(crate) position: Option<u32>,
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
                    MatchPattern::Value(pattern) | MatchPattern::Guard(pattern) => {
                        *pattern =
                            rewrite_expression(syntax_trees, *pattern, subject, fields, captures);
                        fields.to_vec()
                    }
                    MatchPattern::Destructure(pattern) => {
                        let mut pattern_fields = syntax_trees
                            .expressions
                            .match_fields(pattern.fields)
                            .to_vec();
                        let whole = bare_binding(syntax_trees, pattern.subject_binding);
                        let visible = fields.iter().filter(|outer| {
                            !whole.as_ref().is_some_and(|inner| inner.as_str(syntax_trees) == outer.binding.as_str(syntax_trees))
                                && !pattern_fields.iter().any(|inner| matches!(&inner.action, MatchFieldAction::Bind(binding) if binding.as_str(syntax_trees) == outer.binding.as_str(syntax_trees)))
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
        ExpressionNode::Block(block) => {
            rewrite_block_statements(syntax_trees, block, subject, fields, captures)
        }
        ExpressionNode::SavedSubject(saved) => ExpressionNode::SavedSubject(TableSavedSubject {
            component: saved.component,
            capture: captures
                .iter()
                .rev()
                .find_map(|(source, copied)| (*source == saved.capture).then_some(*copied))
                .unwrap_or(saved.capture),
            pattern_head: saved.pattern_head,
        }),
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
            policy: binary.policy,
            negation: binary.negation,
            left: rewrite_expression(syntax_trees, binary.left, subject, fields, captures),
            operator: binary.operator,
            right: rewrite_expression(syntax_trees, binary.right, subject, fields, captures),
        }),
        ExpressionNode::Call(call) => ExpressionNode::Call(TableCallExpression {
            callee: rewrite_optional_expression(
                syntax_trees,
                call.callee,
                subject,
                fields,
                captures,
            ),
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
            access_source_span: member.access_source_span,
            member: member.member,
            position: member.position,
            case_variant: member.case_variant,
        }),
        ExpressionNode::Borrow(inner) => ExpressionNode::Borrow(TableBorrowExpression {
            target: rewrite_expression(syntax_trees, inner.target, subject, fields, captures),
            access: inner.access,
        }),
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } => {
            if let Some(bound) = path_application
                .is_none()
                .then(|| single_destructured_field_name(syntax_trees, path.to_owned(), fields))
                .flatten()
            {
                match (bound.member.clone(), bound.component) {
                    (Some(member), language_core::PatternComponent::Fields) => {
                        ExpressionNode::Member(TableMemberExpression {
                            receiver: subject,
                            access_source_span: Default::default(),
                            member,
                            position: bound.position,
                            case_variant: bound.case_variant.clone(),
                        })
                    }
                    (_, component) => {
                        // Both the whole binding and its case view reuse the
                        // dispatch capture. Neither evaluates the subject again.
                        let ExpressionNode::SavedSubject(mut saved) =
                            *syntax_trees.expressions.expression(subject)
                        else {
                            unreachable!("pattern aliases are produced from a saved subject")
                        };
                        saved.component = component;
                        ExpressionNode::SavedSubject(saved)
                    }
                }
            } else {
                ExpressionNode::Name {
                    members: path,
                    application: path_application,
                }
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
                zero_omitted_fields: struct_literal.zero_omitted_fields,
                application_type: struct_literal.application_type,
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

/// A pattern binding is visible throughout a block arm, so each statement and
/// the final value read it from the saved subject. The block owns its
/// statements, so they are rewritten in place. A block-local `let` that
/// reuses a binding's name is the declaration error resolution reports; from
/// that statement on the name is left as written rather than read from the
/// subject.
fn rewrite_block_statements(
    syntax_trees: &mut SyntaxTrees,
    block: TableBlockExpression,
    subject: ExpressionHandle,
    fields: &[DestructureBinding],
    captures: &mut SubjectCopies,
) -> ExpressionNode {
    use syntax_trees::statement::{StatementNode, TableAssignment};
    let mut visible = fields.to_vec();
    let statements = syntax_trees
        .statements
        .block_statements(block.statements)
        .to_vec();
    for statement in statements {
        let rewritten =
            match syntax_trees.statements.statement(statement).clone() {
                StatementNode::LocalData(mut local) => {
                    local.initial_value = rewrite_optional_expression(
                        syntax_trees,
                        local.initial_value,
                        subject,
                        &visible,
                        captures,
                    );
                    let name = local.name.as_str(syntax_trees).to_owned();
                    visible.retain(|field| field.binding.as_str(syntax_trees) != name);
                    StatementNode::LocalData(local)
                }
                StatementNode::Assignment(assignment) => {
                    StatementNode::Assignment(TableAssignment {
                        target: rewrite_expression(
                            syntax_trees,
                            assignment.target,
                            subject,
                            &visible,
                            captures,
                        ),
                        value: rewrite_expression(
                            syntax_trees,
                            assignment.value,
                            subject,
                            &visible,
                            captures,
                        ),
                    })
                }
                StatementNode::Call(mut call) => {
                    call.expression = rewrite_expression(
                        syntax_trees, call.expression, subject, &visible, captures,
                    );
                    StatementNode::Call(call)
                }
                StatementNode::Expression(expression) => StatementNode::Expression(
                    rewrite_expression(syntax_trees, expression, subject, &visible, captures),
                ),
                other => other,
            };
        syntax_trees
            .statements
            .replace_statement(statement, rewritten);
    }
    ExpressionNode::Block(TableBlockExpression {
        statements: block.statements,
        tail: rewrite_optional_expression(syntax_trees, block.tail, subject, &visible, captures),
    })
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

fn single_destructured_field_name<'fields>(
    syntax_trees: &SyntaxTrees,
    path: HandleSpan<Identifier>,
    fields: &'fields [DestructureBinding],
) -> Option<&'fields DestructureBinding> {
    let members = syntax_trees.expressions.identifier_path_members(path);
    let [member] = members else {
        return None;
    };
    fields
        .iter()
        .find(|field| field.binding.as_str(syntax_trees) == member.as_str(syntax_trees))
}
