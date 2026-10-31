//! Rebuild typed constant-position operations from the owned source graph.
//! Every operation remains ordinary typed source for later formation checking.
use super::*;
use crate::tables::expressions_of_mut;
use expr::{ExpressionHandle, ExpressionNode as Node};
impl Lower<'_> {
    /// Rejoin only coordinates captured from this source node's original
    /// attachments. Every materialized node, including substituted literals,
    /// retains those obligations for ordinary checking and finalization.
    fn retain_expression_source(
        &mut self,
        expression: ExpressionHandle,
        source_span: source::SourceSpan,
        selections: HandleSpan<ProductOperandSelection>,
        call_target_span: Option<source::SourceSpan>,
    ) -> Result<(), Vec<Diagnostic>> {
        let retained = if selections.is_empty() {
            &[][..]
        } else {
            self.graph.selections.span(selections).ok_or_else(|| {
                self.error("designated expression has a foreign authored-selection span")
            })?
        };
        let mut occurrences = Vec::with_capacity(retained.len());
        let call_target_span =
            call_target_span.map(|span| self.program.symbols.rejoin_product_occurrence(span));
        let mut call_selection = None;
        for selection in retained {
            let kind = selection.kind.ok_or_else(|| {
                self.error("designated expression lost its authored selection kind")
            })?;
            let span = self
                .program
                .symbols
                .rejoin_product_occurrence(selection.source_span);
            let occurrence = self
                .program
                .authored_declaration_selections()
                .source_coordinate_occurrence(span, selection.exposure, kind)
                .map_err(|reason| {
                    vec![Diagnostic::error(format!("designated expression cannot rejoin its {kind:?} selection ({:?}): {reason}", selection.exposure)).with_source_span(span)]
                })?;
            if kind
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
                && call_target_span == Some(span)
            {
                if call_selection.is_some_and(|previous| previous != occurrence) {
                    return Err(
                        self.error("designated call has conflicting source-owned Call occurrences")
                    );
                }
                call_selection = Some(occurrence);
            }
            occurrences.push(occurrence);
        }
        let span = self.program.symbols.rejoin_product_occurrence(source_span);
        expressions_of_mut(self.program).set_source_span(expression, span);
        expressions_of_mut(self.program)
            .attach_authored_selection_occurrences(expression, occurrences);
        if let Node::Call(call) = expressions_of_mut(self.program).expression_mut(expression) {
            call.authored_call_selection = call_selection;
        }
        Ok(())
    }
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
        Ok(expressions_of_mut(self.program).insert_expression_handles(values))
    }
    fn source_subject(
        &mut self,
        handle: Handle<ProductMatchSubject>,
    ) -> Result<Handle<expr::CompletedValueCapture>, Vec<Diagnostic>> {
        if let Some((_, capture)) = self.subjects.iter().find(|(source, _)| *source == handle) {
            return Ok(*capture);
        }
        let subject = self.expression(self.graph.match_subjects.get(handle).subject)?;
        let capture = expressions_of_mut(self.program).insert_match_subject(subject);
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
        if let ProductExpressionKind::Name(names) = &value.kind
            && names.count() == 1
            && let Some(name) = names.handles().next()
            && let Some(actual) = self.environment_argument(name)?
        {
            let declared = actual.declared_type.ok_or_else(|| {
                self.error("a static type or callable actual cannot become a const value")
            })?;
            let reference = self
                .program
                .static_operand_type_reference_in_scope(&actual.argument, actual.owner)
                .map_err(|reason| self.error(reason))?;
            let node = crate::monomorphization::materialize_static_value(
                self.program,
                reference,
                declared,
            )
            .ok_or_else(|| self.error("completed static actual has no ordinary const value"))?;
            let result = expressions_of_mut(self.program).insert(node);
            self.retain_expression_source(
                result,
                value.source_span,
                value.authored_selections,
                None,
            )?;
            self.expressions.push((handle, result));
            self.active_expressions.pop();
            return Ok(result);
        }
        let call_target_span = match &value.kind {
            ProductExpressionKind::Call { target, .. } => {
                Some(self.graph.names.get(*target).source_span)
            }
            _ => None,
        };
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
                let target = expressions_of_mut(self.program).insert_tree(&owned);
                self.retain_expression_source(
                    target,
                    value.source_span,
                    value.authored_selections,
                    None,
                )?;
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
            ProductExpressionKind::TypeExpression(target) => Node::TypeExpression(self.ty(target)?),
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
                    expressions_of_mut(self.program)
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
                // Nested calls own their own static telescope. Their actual
                // positions must not be appended to an outer nominal input.
                let outer_path = std::mem::take(&mut self.input_path);
                let outer_selections = std::mem::take(&mut self.input_selections);
                let machine_arguments = self
                    .graph
                    .static_arguments
                    .span_or_empty(static_arguments)
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(ordinal, value)| self.argument(ordinal, value))
                    .collect::<Result<Vec<_>, _>>();
                let input_selections =
                    self.program
                        .product_input_selections
                        .insert_many(std::mem::replace(
                            &mut self.input_selections,
                            outer_selections,
                        ));
                self.input_path = outer_path;
                let machine_arguments = machine_arguments?.into_boxed_slice();
                let target_name = self.identifier(target);
                let authored_application = expressions_of_mut(self.program)
                    .retain_static_machine_application(
                        target_symbol,
                        target_name,
                        &machine_arguments,
                    );
                let source_application = expressions_of_mut(self.program)
                    .static_machine_application_mut(authored_application);
                source_application.lexical_owner = self.author;
                source_application.binding_declaration = target_symbol;
                source_application.input_selections = input_selections;
                Node::Call(expr::TableCallExpression {
                    authored_call_selection: None,
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
                    authored_machine_application: authored_application,
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
                    fields: expressions_of_mut(self.program).insert_struct_fields(fields),
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
                            source_span: self
                                .program
                                .symbols
                                .rejoin_product_occurrence(arm.source_span),
                        })
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                Node::Match(expr::TableMatchExpression {
                    subject,
                    capture,
                    arms: expressions_of_mut(self.program).insert_match_arms(arms),
                })
            }
            // Membership normalization owns source-domain lookup and executable
            // predicates. Rejoin its exact domain before ordinary normalization.
            ProductExpressionKind::Membership { .. } => return Err(self.error(
                "designated constant membership must be normalized by its source operation owner",
            )),
        };
        let target = expressions_of_mut(self.program).insert(node);
        self.retain_expression_source(
            target,
            value.source_span,
            value.authored_selections,
            call_target_span,
        )?;
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
                    fields: expressions_of_mut(self.program).insert_match_fields(fields),
                    has_rest,
                    guard: self.expression_optional(guard)?,
                    condition: self.expression_optional(condition)?,
                    proof_selectors: expressions_of_mut(self.program)
                        .insert_match_proof_selectors(selectors),
                })
            }
        })
    }
}
fn binary(value: ProductBinaryOperator) -> expr::BinaryOperator {
    match value {
        ProductBinaryOperator::CaseMembership => expr::BinaryOperator::CaseMembership,
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
