//! Rebuild typed constant-position operations from the owned source graph.
//! Every operation remains ordinary typed source for later formation checking.
use super::*;
use expr::{ExpressionHandle, ExpressionNode as Node};
impl Lower<'_> {
    fn expression_optional(
        &mut self,
        handle: Handle<ProductOperandExpression>,
    ) -> Result<ExpressionHandle, Vec<Diagnostic>> {
        if handle.is_valid() {
            self.expression(handle)
        } else {
            Ok(Handle::invalid())
        }
    }
    fn expression_arguments(
        &mut self,
        span: HandleSpan<Handle<ProductOperandExpression>>,
    ) -> Result<HandleSpan<ExpressionHandle>, Vec<Diagnostic>> {
        let values = self
            .graph
            .expression_arguments
            .span_or_empty(span)
            .iter()
            .copied()
            .map(|value| self.expression(value))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self
            .program
            .expression_table
            .insert_expression_handles(values))
    }
    fn source_subject(
        &mut self,
        handle: Handle<ProductMatchSubject>,
    ) -> Result<Handle<expr::CompletedValueCapture>, Vec<Diagnostic>> {
        if let Some((_, capture)) = self.subjects.iter().find(|(source, _)| *source == handle) {
            return Ok(*capture);
        }
        let subject = self.expression(self.graph.match_subjects.get(handle).subject)?;
        let capture = self.program.expression_table.insert_match_subject(subject);
        self.subjects.push((handle, capture));
        Ok(capture)
    }
    pub(super) fn expression(
        &mut self,
        handle: Handle<ProductOperandExpression>,
    ) -> Result<ExpressionHandle, Vec<Diagnostic>> {
        if let Some((_, target)) = self
            .expressions
            .iter()
            .find(|(source, _)| *source == handle)
        {
            return Ok(*target);
        }
        if self.active_expressions.contains(&handle) {
            return Err(self.error("designated constant expression graph contains a cycle"));
        }
        self.active_expressions.push(handle);
        let value = self.graph.expressions.get(handle).clone();
        let node = match value.kind {
            ProductExpressionKind::Invalid => {
                return Err(self.error("designated constant expression has an invalid node"));
            }
            ProductExpressionKind::Integer(value) => Node::Integer(value),
            ProductExpressionKind::Boolean(value) => Node::Boolean(value),
            ProductExpressionKind::Float(value) => Node::Float(value),
            ProductExpressionKind::UndeterminedNaN(value) => Node::UndeterminedNaN(value),
            ProductExpressionKind::Text(value) => Node::String(value.into()),
            ProductExpressionKind::Name(names) => {
                let path = self.path(names)?;
                let last = names
                    .handles()
                    .last()
                    .ok_or_else(|| self.error("designated name has no source path"))?;
                let name = self.graph.names.get(last);
                let target = if name.declaration.is_some()
                    || name.builtin_type.is_some()
                    || name.builtin_function.is_some()
                {
                    self.symbol(last)?
                } else {
                    self.program
                        .symbols
                        .find_build_subject_declaration_from_source(
                            &path,
                            name.source_span,
                            self.candidates(),
                            &[],
                        )
                        .ok_or_else(|| {
                            self.error("designated constant name has no exact visible declaration")
                        })?
                };
                let owned = expr::Expression::Name(expr::NamePath::resolved(
                    self.identifiers(names),
                    target,
                    target,
                ));
                let target = self.program.expression_table.insert_tree(&owned);
                self.program
                    .expression_table
                    .set_source_span(target, value.source_span);
                self.active_expressions.pop();
                self.expressions.push((handle, target));
                return Ok(target);
            }
            ProductExpressionKind::Binary {
                left,
                operator,
                right,
            } => Node::Binary(expr::TableBinaryExpression {
                left: self.expression(left)?,
                operator: binary(operator),
                right: self.expression(right)?,
            }),
            ProductExpressionKind::Unary { operator, operand } => {
                Node::Unary(expr::TableUnaryExpression {
                    operator: match operator {
                        ProductUnaryOperator::BitwiseNot => expr::UnaryOperator::BitwiseNot,
                        ProductUnaryOperator::LogicalNot => expr::UnaryOperator::LogicalNot,
                    },
                    operand: self.expression(operand)?,
                })
            }
            ProductExpressionKind::Array(values) => {
                Node::ArrayLiteral(self.expression_arguments(values)?)
            }
            ProductExpressionKind::Indexed { receiver, index } => {
                Node::Indexed(expr::TableIndexedExpression {
                    collection: self.expression(receiver)?,
                    index: self.expression(index)?,
                })
            }
            ProductExpressionKind::Member { receiver, member } => {
                Node::Member(expr::TableMemberExpression {
                    receiver: self.expression(receiver)?,
                    member_symbol: if self.graph.names.get(member).declaration.is_some() {
                        self.symbol(member)?
                    } else {
                        SymbolHandle::invalid()
                    },
                    member: self.identifier(member),
                    case_variant: None,
                })
            }
            ProductExpressionKind::Borrow { target, access } => {
                Node::Borrow(expr::TableBorrowExpression {
                    target: self.expression(target)?,
                    access,
                })
            }
            ProductExpressionKind::Atomic {
                value,
                result,
                ordering,
                result_custody,
            } => Node::Atomic(expr::TableAtomicExpression {
                value: self.expression(value)?,
                result: self.expression_optional(result)?,
                ordering,
                result_custody,
            }),
            ProductExpressionKind::Range {
                start,
                end,
                inclusive,
            } => Node::Range(expr::TableRangeExpression {
                start: self.expression_optional(start)?,
                end: self.expression_optional(end)?,
                end_inclusive: inclusive,
            }),
            ProductExpressionKind::ZeroValue(target) => Node::ZeroValue(self.ty(target)?),
            ProductExpressionKind::Cast {
                value,
                target,
                policy,
                domain,
                domain_arguments,
                form,
            } => {
                let mut semantic_domain = HandleSpan::empty();
                for name in self.identifiers(domain) {
                    self.program
                        .expression_table
                        .push_name_path_member(&mut semantic_domain, name);
                }
                Node::Cast(expr::TableCastExpression {
                    value: self.expression(value)?,
                    target_type: self.ty(target)?,
                    result_type: Handle::invalid(),
                    target_label: HandleSpan::empty(),
                    domain: policy,
                    semantic_domain,
                    semantic_domain_arguments: self.type_arguments(domain_arguments)?,
                    semantic_domain_symbol: domain
                        .handles()
                        .last()
                        .filter(|name| self.graph.names.get(*name).declaration.is_some())
                        .map(|name| self.symbol(name))
                        .transpose()?
                        .unwrap_or_default(),
                    semantic_domain_id: Default::default(),
                    form,
                })
            }
            ProductExpressionKind::Call {
                receiver,
                target,
                owner,
                static_arguments,
                arguments,
                evidence,
                acknowledgement,
            } => {
                let requirement_application = self.ty_optional(owner)?;
                let target_symbol = self.symbol(target)?;
                let machine_arguments = self
                    .graph
                    .static_arguments
                    .span_or_empty(static_arguments)
                    .iter()
                    .copied()
                    .map(|value| self.operand(value))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice();
                Node::Call(expr::TableCallExpression {
                    authored_call_binding: Some(
                        language_semantics::declaration_selection::ResolvedAuthoredCallBinding {
                            selected: target_symbol,
                            original: target_symbol,
                        },
                    ),
                    exact_coercion_target: Handle::invalid(),
                    requirement_application,
                    receiver: self.expression_optional(receiver)?,
                    target_symbol,
                    target: self.identifier(target),
                    arguments: self.expression_arguments(arguments)?,
                    evidence_arguments: self.identifiers(evidence).into_boxed_slice(),
                    machine_arguments,
                    operational_acknowledgement: acknowledgement,
                    build_selection: Handle::invalid(),
                    authored_machine_application: Handle::invalid(),
                    static_machine_parameter: SymbolHandle::invalid(),
                    static_requirement_dispatch: None,
                    quotient_operation: None,
                    private_layout_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                })
            }
            ProductExpressionKind::Record { name, case, fields } => {
                let type_symbol = self.symbol(name)?;
                let fields = self
                    .graph
                    .fields
                    .span_or_empty(fields)
                    .iter()
                    .cloned()
                    .map(|field| {
                        Ok(expr::TableStructLiteralField {
                            name: self.identifier(field.name),
                            field_symbol: self
                                .graph
                                .names
                                .get(field.name)
                                .declaration
                                .is_some()
                                .then(|| self.symbol(field.name))
                                .transpose()?
                                .unwrap_or_default(),
                            value: self.expression(field.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                Node::StructLiteral(expr::TableStructLiteral {
                    type_name: self.identifier(name),
                    type_symbol,
                    case_name: case.map(|name| self.identifier(name)),
                    case_symbol: case
                        .filter(|name| self.graph.names.get(*name).declaration.is_some())
                        .map(|name| self.symbol(name))
                        .transpose()?,
                    fields: self.program.expression_table.insert_struct_fields(fields),
                })
            }
            ProductExpressionKind::SavedSubject {
                capture,
                pattern_head,
            } => Node::SavedSubject(expr::TableSavedSubject {
                capture: self.source_subject(capture)?,
                pattern_head: self.expression_optional(pattern_head)?,
            }),
            ProductExpressionKind::Match {
                subject,
                capture,
                arms,
            } => {
                let subject = self.expression(subject)?;
                let capture = self.source_subject(capture)?;
                let arms = self
                    .graph
                    .match_arms
                    .span_or_empty(arms)
                    .iter()
                    .cloned()
                    .map(|arm| {
                        Ok(expr::TableMatchArm {
                            pattern: self.pattern(arm.pattern)?,
                            value: self.expression(arm.value)?,
                            source_span: arm.source_span,
                        })
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                Node::Match(expr::TableMatchExpression {
                    subject,
                    capture,
                    arms: self.program.expression_table.insert_match_arms(arms),
                })
            }
            // Membership normalization owns source-domain lookup and executable
            // predicates. Rejoin its exact domain before ordinary normalization.
            ProductExpressionKind::Membership { .. } => return Err(self.error(
                "designated constant membership must be normalized by its source operation owner",
            )),
        };
        let target = self.program.expression_table.insert(node);
        self.program
            .expression_table
            .set_source_span(target, value.source_span);
        self.active_expressions.pop();
        self.expressions.push((handle, target));
        Ok(target)
    }
    fn pattern(
        &mut self,
        pattern: ProductMatchPattern,
    ) -> Result<expr::MatchPattern, Vec<Diagnostic>> {
        Ok(match pattern {
            ProductMatchPattern::Wildcard => expr::MatchPattern::Wildcard,
            ProductMatchPattern::Value(value) => expr::MatchPattern::Value(self.expression(value)?),
            ProductMatchPattern::Destructure {
                head,
                fields,
                has_rest,
                guard,
                condition,
                proof_selectors,
            } => {
                let fields = self
                    .graph
                    .match_fields
                    .span_or_empty(fields)
                    .iter()
                    .cloned()
                    .map(|field| {
                        Ok(expr::TableMatchField {
                            member: self.identifier(field.member),
                            field_symbol: SymbolHandle::invalid(),
                            action: match field.action {
                                ProductMatchFieldAction::Bind(name) => {
                                    expr::MatchFieldAction::Bind(self.identifier(name))
                                }
                                ProductMatchFieldAction::Equal(value) => {
                                    expr::MatchFieldAction::Equal(self.expression(value)?)
                                }
                                ProductMatchFieldAction::Waive => expr::MatchFieldAction::Waive,
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                let selectors = self
                    .graph
                    .proof_selectors
                    .span_or_empty(proof_selectors)
                    .iter()
                    .map(|selector| typed_trees::statement::OutcomeProofSelector {
                        output_field: self.identifier(selector.output),
                        binding: self.identifier(selector.binding),
                    })
                    .collect::<Vec<_>>();
                expr::MatchPattern::Destructure(expr::TableDestructurePattern {
                    head: self.expression(head)?,
                    fields: self.program.expression_table.insert_match_fields(fields),
                    has_rest,
                    guard: self.expression_optional(guard)?,
                    condition: self.expression_optional(condition)?,
                    proof_selectors: self
                        .program
                        .expression_table
                        .insert_match_proof_selectors(selectors),
                })
            }
        })
    }
}
fn binary(value: ProductBinaryOperator) -> expr::BinaryOperator {
    match value {
        ProductBinaryOperator::Add => expr::BinaryOperator::Add,
        ProductBinaryOperator::And => expr::BinaryOperator::And,
        ProductBinaryOperator::BitwiseAnd => expr::BinaryOperator::BitwiseAnd,
        ProductBinaryOperator::BitwiseOr => expr::BinaryOperator::BitwiseOr,
        ProductBinaryOperator::BitwiseXor => expr::BinaryOperator::BitwiseXor,
        ProductBinaryOperator::Divide => expr::BinaryOperator::Divide,
        ProductBinaryOperator::Equal => expr::BinaryOperator::Equal,
        ProductBinaryOperator::Greater => expr::BinaryOperator::Greater,
        ProductBinaryOperator::GreaterOrEqual => expr::BinaryOperator::GreaterOrEqual,
        ProductBinaryOperator::Less => expr::BinaryOperator::Less,
        ProductBinaryOperator::LessOrEqual => expr::BinaryOperator::LessOrEqual,
        ProductBinaryOperator::Modulo => expr::BinaryOperator::Modulo,
        ProductBinaryOperator::Multiply => expr::BinaryOperator::Multiply,
        ProductBinaryOperator::NotEqual => expr::BinaryOperator::NotEqual,
        ProductBinaryOperator::Or => expr::BinaryOperator::Or,
        ProductBinaryOperator::ShiftLeft => expr::BinaryOperator::ShiftLeft,
        ProductBinaryOperator::ShiftRight => expr::BinaryOperator::ShiftRight,
        ProductBinaryOperator::Subtract => expr::BinaryOperator::Subtract,
    }
}
