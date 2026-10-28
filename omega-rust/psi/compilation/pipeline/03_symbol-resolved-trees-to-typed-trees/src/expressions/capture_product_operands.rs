//! Capture only the selected static operand's reachable source graph.
use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::product_operands::*;
use symbol_resolved_trees::{
    SymbolResolvedTrees, expression as source_expression, types as source_types,
};
use symbols::SymbolHandle;

pub(super) fn capture(
    source: &SymbolResolvedTrees,
    typed: &mut typed_trees::TypedTrees,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
    occurrences: &[language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId],
    argument: &source_expression::StaticMachineArgument,
) -> Result<ProductStaticOperandGraph, Diagnostic> {
    let mut capture = Capture {
        source,
        typed,
        exposure,
        graph: ProductStaticOperandGraph::default(),
        expressions: Vec::new(),
        captures: Vec::new(),
        active: Vec::new(),
    };
    capture.graph.root = capture.operand(argument, occurrences, true)?;
    Ok(capture.graph)
}
struct Capture<'a> {
    source: &'a SymbolResolvedTrees,
    typed: &'a mut typed_trees::TypedTrees,
    exposure:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
    graph: ProductStaticOperandGraph,
    expressions: Vec<(
        source_expression::ExpressionHandle,
        Handle<ProductOperandExpression>,
    )>,
    captures: Vec<(
        Handle<source_expression::MatchSubjectCapture>,
        Handle<ProductMatchSubject>,
    )>,
    active: Vec<source_expression::ExpressionHandle>,
}
impl Capture<'_> {
    fn retain_coordinate(
        &mut self,
        occurrence: language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        from_type_producer: bool,
    ) -> Result<Handle<ProductOperandSelection>, Diagnostic> {
        let ledger = if from_type_producer {
            self.typed.authored_declaration_selections()
        } else {
            self.source.authored_declaration_selections()
        };
        let selection = ledger.get(occurrence).ok_or_else(|| {
            Diagnostic::error("original static input retains an unknown selection")
        })?;
        let rejoined = ledger
            .source_coordinate_occurrence(
                selection.source_span(),
                selection.exposure(),
                selection.kind(),
            )
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "captured static operand cannot rejoin its {:?} selection ({:?}): {reason}",
                    selection.kind(),
                    selection.exposure()
                ))
                .with_source_span(selection.source_span())
            })?;
        if rejoined != occurrence {
            return Err(Diagnostic::error(
                "original static input changed its exact selection attachment",
            ));
        }
        Ok(self.graph.selections.insert(ProductOperandSelection {
            source_span: selection.source_span(),
            exposure: selection.exposure(),
            kind: Some(selection.kind()),
        }))
    }
    fn type_name(
        &mut self,
        name: &symbol_resolved_trees::name::DiagnosticName,
        symbol: SymbolHandle,
        kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
    ) -> Result<Handle<ProductOperandName>, Diagnostic> {
        let occurrence = crate::type_reference::retain_type_reference_recipe_selection(
            self.source,
            self.typed,
            name,
            symbol,
            self.exposure,
            kind,
        )?;
        let mut retained = self.named(name, symbol);
        if let Some(occurrence) = occurrence {
            retained.authored_selection = self.retain_coordinate(occurrence, true)?;
        }
        Ok(self.graph.names.insert(retained))
    }
    fn operand_selection(
        &mut self,
        argument: &source_expression::StaticMachineArgument,
        occurrences: &[language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId],
    ) -> Result<Handle<ProductOperandSelection>, Diagnostic> {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind as Kind;
        if argument.path.is_empty() || !argument.path.iter().any(|name| name.is_source_backed()) {
            return Ok(Handle::invalid());
        }
        if argument.symbol.is_valid()
            && matches!(
                self.source.symbols.get(argument.symbol).kind,
                symbols::SymbolKind::Local
                    | symbols::SymbolKind::Parameter
                    | symbols::SymbolKind::TypeParameter
                    | symbols::SymbolKind::ConformanceParameter
                    | symbols::SymbolKind::MachineParameter
                    | symbols::SymbolKind::PropositionParameter
                    | symbols::SymbolKind::PropositionMachineParameter
            )
        {
            return Ok(Handle::invalid());
        }
        if !argument.symbol.is_valid()
            && argument.application.is_none()
            && argument.evidence_projection.is_none()
            && let [name] = argument.path.as_ref()
            && language_semantics::const_value::boolean_literal_spelling(name.as_str()).is_some()
        {
            return Ok(Handle::invalid());
        }
        let first = argument.path.first().expect("nonempty path").source_span();
        let last = argument.path.last().expect("nonempty path").source_span();
        if first.source_id != last.source_id {
            return Err(Diagnostic::error(
                "original static input path crosses source owners",
            ));
        }
        let span = source::SourceSpan::new(
            first.source_id,
            source::Span::new(first.span.start, last.span.end),
        );
        let mut matching = occurrences.iter().copied().filter(|occurrence| {
            self.source
                .authored_declaration_selections()
                .get(*occurrence)
                .is_some_and(|selection| {
                    selection.source_span() == span
                        && matches!(selection.kind(), Kind::StaticArgument | Kind::Conformance)
                })
        });
        let occurrence = matching.next().ok_or_else(|| {
            Diagnostic::error(
                "original static input has no exact original-call selection attachment",
            )
            .with_source_span(span)
        })?;
        if matching.next().is_some() {
            return Err(Diagnostic::error(
                "original static input has ambiguous original-call selection attachments",
            )
            .with_source_span(span));
        }
        self.retain_coordinate(occurrence, false)
    }
    fn declaration(&self, symbol: SymbolHandle) -> Option<source::SourceSpan> {
        symbol
            .is_valid()
            .then(|| self.source.symbols.symbol_provenance_source_span(symbol))
            .flatten()
    }
    fn named(
        &self,
        name: &symbol_resolved_trees::name::DiagnosticName,
        symbol: SymbolHandle,
    ) -> ProductOperandName {
        ProductOperandName {
            authored_selection: Handle::invalid(),
            text: name.as_str().to_owned(),
            source_span: name.source_span(),
            declaration: self.declaration(symbol),
            builtin_type: symbol
                .is_valid()
                .then(|| self.source.symbols.builtin_type_atom(symbol))
                .flatten(),
            builtin_function: symbol
                .is_valid()
                .then(|| self.source.symbols.builtin_function_for_symbol(symbol))
                .flatten(),
        }
    }
    fn name(
        &mut self,
        name: &symbol_resolved_trees::name::DiagnosticName,
        symbol: SymbolHandle,
    ) -> Handle<ProductOperandName> {
        let value = self.named(name, symbol);
        self.graph.names.insert(value)
    }
    fn names(
        &mut self,
        names: &[symbol_resolved_trees::name::DiagnosticName],
        last_symbol: SymbolHandle,
    ) -> HandleSpan<ProductOperandName> {
        let names = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                self.named(
                    name,
                    if index + 1 == names.len() {
                        last_symbol
                    } else {
                        SymbolHandle::invalid()
                    },
                )
            })
            .collect::<Vec<_>>();
        self.graph.names.insert_many(names)
    }
    fn operand(
        &mut self,
        argument: &source_expression::StaticMachineArgument,
        occurrences: &[language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId],
        descriptor: bool,
    ) -> Result<Handle<ProductStaticOperand>, Diagnostic> {
        let path = self.names(&argument.path, argument.symbol);
        let structural_type = if argument.type_reference.is_valid() {
            self.ty_handle(argument.type_reference)?
        } else {
            Handle::invalid()
        };
        let application = if let Some(application) = &argument.application {
            let source = self.source;
            let lifetimes = self.names(&application.lifetime_arguments, SymbolHandle::invalid());
            let arguments = application
                .arguments
                .iter()
                .map(|argument| self.operand(argument, occurrences, false))
                .collect::<Result<Vec<_>, _>>()?;
            let arguments = self.graph.static_arguments.insert_many(arguments);
            let member = if application.member.is_valid() {
                let member = source
                    .tables
                    .bodies
                    .expressions
                    .static_member(application.member)
                    .ok_or_else(|| {
                        Diagnostic::error("designated operand has a foreign member handle")
                    })?;
                self.operand(member, occurrences, false)?
            } else {
                Handle::invalid()
            };
            self.graph.applications.insert(ProductStaticApplication {
                lifetimes,
                arguments,
                member,
            })
        } else {
            Handle::invalid()
        };
        let projection = argument.evidence_projection.as_ref().map(|projection| {
            (
                self.name(&projection.term, SymbolHandle::invalid()),
                self.name(&projection.member, SymbolHandle::invalid()),
            )
        });
        let authored_selection = if descriptor {
            Handle::invalid()
        } else {
            self.operand_selection(argument, occurrences)?
        };
        Ok(self.graph.operands.insert(ProductStaticOperand {
            authored_selection,
            path,
            application,
            structural_type,
            integer: argument.const_literal.clone(),
            projection,
            declaration: self.declaration(argument.symbol),
        }))
    }
    fn ty_handle(
        &mut self,
        handle: Handle<source_types::TypeReference>,
    ) -> Result<Handle<ProductOperandType>, Diagnostic> {
        let source = self.source;
        self.ty(source.child_type_reference(handle))
    }
    fn types(
        &mut self,
        span: HandleSpan<source_types::TypeReference>,
    ) -> Result<HandleSpan<Handle<ProductOperandType>>, Diagnostic> {
        let source = self.source;
        let values = source
            .child_type_references(span)
            .iter()
            .map(|reference| self.ty(reference))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.graph.type_arguments.insert_many(values))
    }
    fn ty(
        &mut self,
        reference: &source_types::TypeReference,
    ) -> Result<Handle<ProductOperandType>, Diagnostic> {
        use source_types::TypeReference as Source;
        let source = self.source;
        let value = match reference {
            Source::Unit => ProductOperandType::Unit,
            Source::Named { name, symbol, .. } => {
                ProductOperandType::Named(self.type_name(name, *symbol, language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference)?)
            }
            Source::Generic(value) => ProductOperandType::Generic {
                name: self.type_name(&value.base_name, value.base_symbol, language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference)?,
                lifetimes: self.names(&value.lifetime_arguments, SymbolHandle::invalid()),
                arguments: self.types(value.arguments)?,
            },
            Source::Reference(value) => ProductOperandType::Reference {
                referee: self.ty_handle(value.referee)?,
                access: value.access,
                lifetime: value
                    .lifetime
                    .as_ref()
                    .map(|name| self.name(name, SymbolHandle::invalid())),
            },
            Source::Slice(value) => ProductOperandType::Slice(self.ty_handle(value.element_type)?),
            Source::FixedArray(value) => ProductOperandType::Array {
                element: self.ty_handle(value.element_type)?,
                length: match &value.length {
                    source_types::FixedArrayLength::Literal(value) => {
                        ProductArrayLength::Literal(*value)
                    }
                    source_types::FixedArrayLength::Expression(value) => {
                        ProductArrayLength::Expression(self.ty_handle(*value)?)
                    }
                    source_types::FixedArrayLength::ConstParameter { symbol, name } => {
                        ProductArrayLength::Parameter(self.type_name(name, *symbol, language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticPathSegment)?)
                    }
                    source_types::FixedArrayLength::ConstCall { machine, name } => {
                        ProductArrayLength::Call(self.type_name(name, *machine, language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticPathSegment)?)
                    }
                },
            },
            Source::Constrained(value) => {
                let base = self.ty_handle(value.base_type)?;
                let constraints = source
                    .tables
                    .types
                    .constraints
                    .span_or_empty(value.constraints)
                    .iter()
                    .map(|constraint| self.constraint(constraint))
                    .collect::<Result<Vec<_>, _>>()?;
                ProductOperandType::Constrained {
                    base,
                    constraints: self.graph.constraints.insert_many(constraints),
                }
            }
            Source::DynamicTrait {
                symbol,
                name,
                arguments,
                ..
            } => ProductOperandType::Dynamic {
                name: self.type_name(name, *symbol, language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference)?,
                arguments: self.types(*arguments)?,
            },
            Source::ConstExpression(value) => {
                ProductOperandType::ConstExpression(self.expression(*value)?)
            }
        };
        Ok(self.graph.types.insert(value))
    }
    fn constraint(
        &mut self,
        value: &source_types::TypeConstraint,
    ) -> Result<ProductOperandConstraint, Diagnostic> {
        Ok(match value {
            source_types::TypeConstraint::Named(name) => {
                ProductOperandConstraint::Named(self.name(name, SymbolHandle::invalid()))
            }
            source_types::TypeConstraint::ArithmeticDomain(policy) => {
                ProductOperandConstraint::Arithmetic(*policy)
            }
            source_types::TypeConstraint::Domain(value) => ProductOperandConstraint::Domain {
                name: self.name(&value.name, SymbolHandle::invalid()),
                arguments: self.types(value.arguments)?,
            },
            source_types::TypeConstraint::Range {
                minimum,
                maximum,
                end_inclusive,
            } => ProductOperandConstraint::Range {
                minimum: self.expression(*minimum)?,
                maximum: self.expression(*maximum)?,
                inclusive: *end_inclusive,
            },
        })
    }
    fn expression_span(
        &mut self,
        span: HandleSpan<source_expression::ExpressionHandle>,
    ) -> Result<HandleSpan<Handle<ProductOperandExpression>>, Diagnostic> {
        let source = self.source;
        let values = source
            .tables
            .bodies
            .expressions
            .expression_handles(span)
            .iter()
            .map(|value| self.expression(*value))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.graph.expression_arguments.insert_many(values))
    }
    fn capture_subject(
        &mut self,
        capture: Handle<source_expression::MatchSubjectCapture>,
    ) -> Result<Handle<ProductMatchSubject>, Diagnostic> {
        if let Some((_, value)) = self
            .captures
            .iter()
            .find(|(original, _)| *original == capture)
        {
            return Ok(*value);
        }
        let subject = self
            .source
            .tables
            .bodies
            .expressions
            .match_subject(capture)
            .subject;
        let subject = self.expression(subject)?;
        let value = self
            .graph
            .match_subjects
            .insert(ProductMatchSubject { subject });
        self.captures.push((capture, value));
        Ok(value)
    }
    fn expression_selections(
        &mut self,
        expression: source_expression::ExpressionHandle,
    ) -> Result<HandleSpan<ProductOperandSelection>, Diagnostic> {
        let source = self.source;
        let ledger = source.authored_declaration_selections();
        let mut selections = Vec::new();
        for occurrence in source
            .tables
            .bodies
            .expressions
            .authored_selection_occurrences(expression)
        {
            let selection = ledger.get(occurrence).ok_or_else(|| {
                Diagnostic::error(
                    "designated source expression retains an unknown authored selection",
                )
                .with_source_span(source.tables.bodies.expressions.source_span(expression))
            })?;
            let rejoined = ledger
                .source_coordinate_occurrence(
                    selection.source_span(),
                    selection.exposure(),
                    selection.kind(),
                )
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "captured static operand cannot rejoin its {:?} selection ({:?}): {reason}",
                        selection.kind(),
                        selection.exposure()
                    ))
                    .with_source_span(selection.source_span())
                })?;
            if rejoined != occurrence {
                return Err(Diagnostic::error(
                    "designated source expression selection changed its exact ledger association",
                )
                .with_source_span(selection.source_span()));
            }
            selections.push(ProductOperandSelection {
                source_span: selection.source_span(),
                exposure: selection.exposure(),
                kind: Some(selection.kind()),
            });
        }
        Ok(self.graph.selections.insert_many(selections))
    }

    fn expression(
        &mut self,
        expression: source_expression::ExpressionHandle,
    ) -> Result<Handle<ProductOperandExpression>, Diagnostic> {
        if !expression.is_valid() {
            return Ok(Handle::invalid());
        }
        if let Some((_, value)) = self
            .expressions
            .iter()
            .find(|(original, _)| *original == expression)
        {
            return Ok(*value);
        }
        let source = self.source;
        let span = source.tables.bodies.expressions.source_span(expression);
        if self.active.contains(&expression) {
            return Err(Diagnostic::error(
                "designated static operand has a cyclic source expression",
            )
            .with_source_span(span));
        }
        self.active.push(expression);
        use source_expression::ExpressionNode as Source;
        let kind = match source.tables.bodies.expressions.expression(expression) {
            Source::Integer(value) => ProductExpressionKind::Integer(value.clone()),
            Source::Float(value) => ProductExpressionKind::Float(value.clone()),
            Source::UndeterminedNaN(value) => ProductExpressionKind::UndeterminedNaN(*value),
            Source::Boolean(value) => ProductExpressionKind::Boolean(*value),
            Source::String(value) => ProductExpressionKind::Text(value.to_vec()),
            Source::Name(value) => ProductExpressionKind::Name(
                self.names(
                    source
                        .tables
                        .bodies
                        .expressions
                        .name_path_members(value.members),
                    value.symbol,
                ),
            ),
            Source::ArrayLiteral(values) => {
                ProductExpressionKind::Array(self.expression_span(*values)?)
            }
            Source::Binary(value) => ProductExpressionKind::Binary {
                left: self.expression(value.left)?,
                operator: binary(value.operator),
                right: self.expression(value.right)?,
            },
            Source::Unary(value) => ProductExpressionKind::Unary {
                operand: self.expression(value.operand)?,
                operator: match value.operator {
                    source_expression::UnaryOperator::BitwiseNot => {
                        ProductUnaryOperator::BitwiseNot
                    }
                    source_expression::UnaryOperator::LogicalNot => {
                        ProductUnaryOperator::LogicalNot
                    }
                },
            },
            Source::Member(value) => ProductExpressionKind::Member {
                receiver: self.expression(value.receiver)?,
                member: self.name(&value.member, value.member_symbol),
            },
            Source::Indexed(value) => ProductExpressionKind::Indexed {
                receiver: self.expression(value.collection)?,
                index: self.expression(value.index)?,
            },
            Source::Borrow(value) => ProductExpressionKind::Borrow {
                target: self.expression(value.target)?,
                access: value.access,
            },
            Source::Atomic(value) => ProductExpressionKind::Atomic {
                value: self.expression(value.value)?,
                result: self.expression(value.result)?,
                ordering: value.ordering,
                result_custody: value.result_custody,
            },
            Source::Range(value) => ProductExpressionKind::Range {
                start: self.expression(value.start)?,
                end: self.expression(value.end)?,
                inclusive: value.end_inclusive,
            },
            Source::Membership(value) => ProductExpressionKind::Membership {
                value: self.expression(value.value)?,
                domain: self.names(
                    source
                        .tables
                        .bodies
                        .expressions
                        .name_path_members(value.domain),
                    value.domain_symbol,
                ),
                declaration: self.declaration(value.domain_symbol),
                case_type: self.declaration(value.case_type_symbol),
                case: self.declaration(value.case_symbol),
            },
            Source::ZeroValue(value) => ProductExpressionKind::ZeroValue(self.ty_handle(*value)?),
            Source::TypeExpression(value) => {
                ProductExpressionKind::TypeExpression(self.ty_handle(*value)?)
            }
            Source::Call(value) => {
                let receiver = self.expression(value.receiver)?;
                let target = self.name(&value.target, value.target_symbol);
                let owner = if value.requirement_application.is_valid() {
                    self.ty_handle(value.requirement_application)?
                } else {
                    Handle::invalid()
                };
                let static_arguments = value
                    .machine_arguments
                    .iter()
                    .map(|value| {
                        self.operand(
                            value,
                            &source
                                .tables
                                .bodies
                                .expressions
                                .authored_selection_occurrences(expression)
                                .collect::<Vec<_>>(),
                            false,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let static_arguments = self.graph.static_arguments.insert_many(static_arguments);
                let arguments = self.expression_span(value.arguments)?;
                let evidence = self.names(&value.evidence_arguments, SymbolHandle::invalid());
                ProductExpressionKind::Call {
                    receiver,
                    target,
                    owner,
                    static_arguments,
                    arguments,
                    evidence,
                    acknowledgement: value.operational_acknowledgement,
                }
            }
            Source::Cast(value) => ProductExpressionKind::Cast {
                value: self.expression(value.value)?,
                target: self.ty_handle(value.target_type)?,
                policy: value.domain,
                domain: self.names(
                    source
                        .tables
                        .bodies
                        .expressions
                        .name_path_members(value.semantic_domain),
                    value.semantic_domain_symbol,
                ),
                domain_arguments: self.types(value.semantic_domain_arguments)?,
                form: value.form,
            },
            Source::StructLiteral(value) => {
                let name = self.name(&value.type_name, value.type_symbol);
                let case = value
                    .case_name
                    .as_ref()
                    .map(|name| self.name(name, value.case_symbol.unwrap_or_default()));
                let fields = source
                    .tables
                    .bodies
                    .expressions
                    .struct_fields(value.fields)
                    .iter()
                    .map(|field| {
                        Ok(ProductOperandField {
                            name: self.name(&field.name, field.field_symbol),
                            value: self.expression(field.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                ProductExpressionKind::Record {
                    name,
                    case,
                    fields: self.graph.fields.insert_many(fields),
                }
            }
            Source::SavedSubject(value) => ProductExpressionKind::SavedSubject {
                capture: self.capture_subject(value.capture)?,
                pattern_head: self.expression(value.pattern_head)?,
            },
            Source::Match(value) => {
                let subject = self.expression(value.subject)?;
                let capture = self.capture_subject(value.capture)?;
                let arms = source
                    .tables
                    .bodies
                    .expressions
                    .match_arms(value.arms)
                    .iter()
                    .map(|arm| {
                        let pattern = self.pattern(arm.pattern)?;
                        Ok(ProductMatchArm {
                            pattern,
                            value: self.expression(arm.value)?,
                            source_span: arm.source_span,
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                ProductExpressionKind::Match {
                    subject,
                    capture,
                    arms: self.graph.match_arms.insert_many(arms),
                }
            }
        };
        let authored_selections = self.expression_selections(expression)?;
        let value = self.graph.expressions.insert(ProductOperandExpression {
            source_span: span,
            authored_selections,
            kind,
        });
        self.active.pop();
        self.expressions.push((expression, value));
        Ok(value)
    }
    fn pattern(
        &mut self,
        pattern: source_expression::MatchPattern,
    ) -> Result<ProductMatchPattern, Diagnostic> {
        let source = self.source;
        Ok(match pattern {
            source_expression::MatchPattern::Wildcard => ProductMatchPattern::Wildcard,
            source_expression::MatchPattern::Value(value) => {
                ProductMatchPattern::Value(self.expression(value)?)
            }
            source_expression::MatchPattern::Destructure(value) => {
                let head = self.expression(value.head)?;
                let fields = source
                    .tables
                    .bodies
                    .expressions
                    .match_fields(value.fields)
                    .iter()
                    .map(|field| {
                        Ok(ProductMatchField {
                            member: self.name(&field.member, field.field_symbol),
                            action: match &field.action {
                                source_expression::MatchFieldAction::Bind(name) => {
                                    ProductMatchFieldAction::Bind(
                                        self.name(name, SymbolHandle::invalid()),
                                    )
                                }
                                source_expression::MatchFieldAction::Equal(value) => {
                                    ProductMatchFieldAction::Equal(self.expression(*value)?)
                                }
                                source_expression::MatchFieldAction::Waive => {
                                    ProductMatchFieldAction::Waive
                                }
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                let fields = self.graph.match_fields.insert_many(fields);
                let selectors = source
                    .tables
                    .bodies
                    .expressions
                    .match_proof_selectors(value.proof_selectors)
                    .iter()
                    .map(|selector| ProductProofSelector {
                        output: self.name(&selector.output_field, SymbolHandle::invalid()),
                        binding: self.name(&selector.binding, SymbolHandle::invalid()),
                    })
                    .collect::<Vec<_>>();
                ProductMatchPattern::Destructure {
                    head,
                    fields,
                    has_rest: value.has_rest,
                    guard: self.expression(value.guard)?,
                    condition: self.expression(value.condition)?,
                    proof_selectors: self.graph.proof_selectors.insert_many(selectors),
                }
            }
        })
    }
}
fn binary(value: source_expression::BinaryOperator) -> ProductBinaryOperator {
    use source_expression::BinaryOperator as Source;
    match value {
        Source::Add => ProductBinaryOperator::Add,
        Source::And => ProductBinaryOperator::And,
        Source::BitwiseAnd => ProductBinaryOperator::BitwiseAnd,
        Source::BitwiseOr => ProductBinaryOperator::BitwiseOr,
        Source::BitwiseXor => ProductBinaryOperator::BitwiseXor,
        Source::Divide => ProductBinaryOperator::Divide,
        Source::Equal => ProductBinaryOperator::Equal,
        Source::Greater => ProductBinaryOperator::Greater,
        Source::GreaterOrEqual => ProductBinaryOperator::GreaterOrEqual,
        Source::Less => ProductBinaryOperator::Less,
        Source::LessOrEqual => ProductBinaryOperator::LessOrEqual,
        Source::Modulo => ProductBinaryOperator::Modulo,
        Source::Multiply => ProductBinaryOperator::Multiply,
        Source::NotEqual => ProductBinaryOperator::NotEqual,
        Source::Or => ProductBinaryOperator::Or,
        Source::ShiftLeft => ProductBinaryOperator::ShiftLeft,
        Source::ShiftRight => ProductBinaryOperator::ShiftRight,
        Source::Subtract => ProductBinaryOperator::Subtract,
    }
}
