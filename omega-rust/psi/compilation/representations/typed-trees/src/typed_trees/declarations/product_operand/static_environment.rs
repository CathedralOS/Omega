//! Source-owned actuals of an already checked generic helper activation.
//! Only its exact specialization receipt supplies bindings. Source requests
//! remain unchanged, and no current-world symbol/type handle escapes here.
use crate::{TypedTrees, expression as expr, types};
use arena::{Handle, HandleSpan};
use language_semantics::declaration_selection::product_operands::*;
use source::SourceSpan;
use std::sync::Arc;
use symbols::SymbolHandle;

pub fn capture_static_environment(
    program: &TypedTrees,
    machine: SymbolHandle,
) -> Result<Option<Arc<ProductStaticEnvironment>>, String> {
    let mut specializations = program
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.instance == machine);
    let Some(specialization) = specializations.next() else {
        return Ok(None);
    };
    if specializations.next().is_some() {
        return Err("Build helper has multiple exact specialization receipts".into());
    }
    if !specialization.binding_application.is_valid() {
        return Err(
            "Build helper specialization lost its original static binding application".into(),
        );
    }
    let application = program
        .expression_table
        .static_machine_application(specialization.binding_application);
    if application.binding_declaration != specialization.template {
        return Err(
            "Build helper specialization binding belongs to a different exact template".into(),
        );
    }
    let owner = program
        .symbols
        .symbol_provenance_source_span(application.lexical_owner)
        .ok_or("Build helper actuals lost their original lexical owner")?;
    let mut capture = Capture {
        program,
        environment: ProductStaticEnvironment {
            template: program
                .symbols
                .symbol_provenance_source_span(specialization.template),
            lexical_owner: Some(owner),
            ..Default::default()
        },
        owner,
        types: Vec::new(),
        active_types: Vec::new(),
        expressions: Vec::new(),
        active_expressions: Vec::new(),
    };
    for binding in program
        .expression_table
        .static_machine_application_bindings(specialization.binding_application)
    {
        let parameter = program
            .symbols
            .symbol_provenance_source_span(binding.parameter)
            .ok_or("Build helper static binder lost source identity")?;
        let actual = match &binding.value {
            crate::static_applications::StaticMachineBindingValue::Type(reference)
            | crate::static_applications::StaticMachineBindingValue::Const(reference) => program
                .static_operand_from_type_reference(*reference)
                .map_err(str::to_owned)?,
            crate::static_applications::StaticMachineBindingValue::Machine(argument)
            | crate::static_applications::StaticMachineBindingValue::Conformance(argument) => {
                argument.clone()
            }
            crate::static_applications::StaticMachineBindingValue::RuntimeValue { .. } => continue,
        };
        let argument = capture.operand(&actual)?;
        let declared_type = program
            .data_type_parameters
            .span_or_empty(specialization.template_parameters)
            .iter()
            .find(|formal| formal.symbol == binding.parameter)
            .and_then(|formal| match formal.kind {
                crate::data::TypeParameterKind::Const { type_reference } => Some(type_reference),
                _ => None,
            })
            .map(|reference| capture.ty(reference))
            .transpose()?
            .unwrap_or_default();
        capture
            .environment
            .bindings
            .insert(ProductStaticEnvironmentBinding {
                parameter,
                argument,
                lexical_owner: owner,
                declared_type,
            });
    }
    let template = program
        .machine_by_symbol(specialization.template)
        .ok_or("Build helper lost its source template")?;
    for binding in program
        .expression_table
        .static_machine_application_lifetimes(specialization.binding_application)
    {
        let ordinal = template
            .lifetime_parameters
            .iter()
            .position(|parameter| parameter.as_str() == binding.parameter.as_str())
            .ok_or("Build helper lifetime binding has no exact template position")?;
        let argument = capture.name(binding.argument.as_str(), SymbolHandle::invalid());
        capture
            .environment
            .lifetimes
            .insert(ProductStaticEnvironmentLifetime {
                parameter_owner: program
                    .symbols
                    .symbol_provenance_source_span(specialization.template)
                    .ok_or("Build helper lifetime binder lost its original declaration")?,
                parameter_ordinal: ordinal as u32,
                argument,
                lexical_owner: owner,
            });
    }
    let mut identity = b"omega-source-static-environment-v1\0".to_vec();
    let frame = |out: &mut Vec<u8>, value: &[u8]| {
        out.extend_from_slice(&(value.len() as u64).to_le_bytes());
        out.extend_from_slice(value);
    };
    frame(
        &mut identity,
        specialization.normalized_template_identity.as_bytes(),
    );
    for values in [
        &specialization.type_argument_identities,
        &specialization.const_argument_identities,
    ] {
        identity.extend_from_slice(&(values.len() as u64).to_le_bytes());
        for value in values {
            frame(&mut identity, value.as_bytes());
        }
    }
    for symbols in [
        &specialization.machine_arguments,
        &specialization.conformance_arguments,
    ] {
        identity.extend_from_slice(&(symbols.len() as u64).to_le_bytes());
        for symbol in symbols {
            frame(
                &mut identity,
                program
                    .normalized_hermetic_symbol_identity(*symbol)?
                    .as_bytes(),
            );
        }
    }
    identity
        .extend_from_slice(&(specialization.conformance_applications.len() as u64).to_le_bytes());
    for application in &specialization.conformance_applications {
        identity.extend_from_slice(&application.commitment.as_bytes());
    }
    for binding in program
        .expression_table
        .static_machine_application_lifetimes(specialization.binding_application)
    {
        let actual = crate::type_identity::portable_applied_lifetime_identity(
            program,
            binding.argument.as_str(),
            application.lexical_owner,
            &[],
        )
        .and_then(|identity| program.type_identity_text(identity))
        .ok_or("Build helper region actual lost its exact portable scope")?;
        frame(&mut identity, actual.as_bytes());
    }
    capture.environment.identity = identity;
    Ok(Some(Arc::new(capture.environment)))
}

/// Capture one already formed whole static operand into independent owned
/// arenas. Its source coordinates remain under the caller's retained map;
/// no current-program type, symbol, expression or member handle escapes.
pub fn capture_closed_operand(
    program: &TypedTrees,
    argument: &expr::StaticMachineArgument,
    lexical_owner: SymbolHandle,
) -> Result<ProductStaticOperandGraph, String> {
    let owner = program
        .symbols
        .symbol_provenance_source_span(lexical_owner)
        .ok_or("closed Product application lost its original lexical scope")?;
    let mut capture = Capture {
        program,
        environment: ProductStaticEnvironment::default(),
        owner,
        types: Vec::new(),
        active_types: Vec::new(),
        expressions: Vec::new(),
        active_expressions: Vec::new(),
    };
    capture.environment.graph.root = capture.operand(argument)?;
    Ok(capture.environment.graph)
}

struct Capture<'a> {
    program: &'a TypedTrees,
    environment: ProductStaticEnvironment,
    owner: SourceSpan,
    types: Vec<(types::TypeReferenceHandle, Handle<ProductOperandType>)>,
    active_types: Vec<types::TypeReferenceHandle>,
    expressions: Vec<(expr::ExpressionHandle, Handle<ProductOperandExpression>)>,
    active_expressions: Vec<expr::ExpressionHandle>,
}
impl Capture<'_> {
    fn name(&mut self, text: &str, symbol: SymbolHandle) -> Handle<ProductOperandName> {
        let program = self.program;
        let declaration = symbol
            .is_valid()
            .then(|| program.symbols.symbol_provenance_source_span(symbol))
            .flatten();
        self.environment.graph.names.insert(ProductOperandName {
            authored_selection: Handle::invalid(),
            text: text.to_owned(),
            source_span: declaration.unwrap_or(self.owner),
            declaration,
            builtin_type: symbol
                .is_valid()
                .then(|| program.symbols.builtin_type_atom(symbol))
                .flatten(),
            builtin_function: symbol
                .is_valid()
                .then(|| program.symbols.builtin_function_for_symbol(symbol))
                .flatten(),
        })
    }
    fn names(&mut self, names: &[crate::name::Identifier]) -> HandleSpan<ProductOperandName> {
        let values = names
            .iter()
            .map(|name| {
                let handle = self.name(name.as_str(), SymbolHandle::invalid());
                self.environment.graph.names.get(handle).clone()
            })
            .collect::<Vec<_>>();
        self.environment.graph.names.insert_many(values)
    }
    fn operand(
        &mut self,
        argument: &expr::StaticMachineArgument,
    ) -> Result<Handle<ProductStaticOperand>, String> {
        let program = self.program;
        if !argument.symbol.is_valid()
            && !argument.type_reference.is_valid()
            && argument.application.is_none()
            && argument.const_literal.is_none()
            && argument.path.len() == 1
            && let Some(value) = language_semantics::const_value::CanonicalConstValue::from_atom(
                argument.path[0].as_str(),
            )
        {
            return Ok(self
                .environment
                .graph
                .operands
                .insert(ProductStaticOperand {
                    structural_type: self
                        .environment
                        .graph
                        .types
                        .insert(ProductOperandType::CanonicalConst(value)),
                    ..Default::default()
                }));
        }
        let names = argument
            .path
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let h = self.name(
                    name.as_str(),
                    if index + 1 == argument.path.len() {
                        argument.symbol
                    } else {
                        SymbolHandle::invalid()
                    },
                );
                self.environment.graph.names.get(h).clone()
            })
            .collect::<Vec<_>>();
        let path = self.environment.graph.names.insert_many(names);
        let structural_type = if argument.type_reference.is_valid() {
            self.ty(argument.type_reference)?
        } else {
            Handle::invalid()
        };
        let application = if let Some(app) = &argument.application {
            let arguments = app
                .arguments
                .iter()
                .map(|argument| self.operand(argument))
                .collect::<Result<Vec<_>, _>>()?;
            let arguments = self
                .environment
                .graph
                .static_arguments
                .insert_many(arguments);
            let lifetimes = self.names(&app.lifetime_arguments);
            let member = if app.member.is_valid() {
                let member = program
                    .expression_table
                    .static_member(app.member)
                    .ok_or("Build static actual has an invalid member continuation")?;
                self.operand(member)?
            } else {
                Handle::invalid()
            };
            self.environment
                .graph
                .applications
                .insert(ProductStaticApplication {
                    arguments,
                    lifetimes,
                    member,
                })
        } else {
            Handle::invalid()
        };
        let projection = argument.evidence_projection.as_ref().map(|projection| {
            (
                self.name(projection.term.as_str(), SymbolHandle::invalid()),
                self.name(projection.member.as_str(), SymbolHandle::invalid()),
            )
        });
        Ok(self
            .environment
            .graph
            .operands
            .insert(ProductStaticOperand {
                authored_selection: Handle::invalid(),
                path,
                structural_type,
                application,
                integer: argument.const_literal.clone(),
                projection,
                declaration: argument
                    .symbol
                    .is_valid()
                    .then(|| {
                        program
                            .symbols
                            .symbol_provenance_source_span(argument.symbol)
                    })
                    .flatten(),
            }))
    }
    fn type_arguments(
        &mut self,
        arguments: HandleSpan<types::TypeReferenceHandle>,
    ) -> Result<HandleSpan<Handle<ProductOperandType>>, String> {
        let program = self.program;
        let values = program
            .type_reference_table
            .type_reference_handles(arguments)
            .iter()
            .map(|reference| self.ty(*reference))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.environment.graph.type_arguments.insert_many(values))
    }
    fn ty(
        &mut self,
        reference: types::TypeReferenceHandle,
    ) -> Result<Handle<ProductOperandType>, String> {
        let program = self.program;
        if let Some((_, value)) = self.types.iter().find(|(source, _)| *source == reference) {
            return Ok(*value);
        }
        if self.active_types.contains(&reference) {
            return Err("Build static actual type contains a cycle".into());
        }
        self.active_types.push(reference);
        let value = match program.type_reference_table.type_reference(reference) {
            types::TypeReferenceNode::Unit => ProductOperandType::Unit,
            types::TypeReferenceNode::Named { symbol, name } => {
                if !symbol.is_valid()
                    && let Some(value) =
                        language_semantics::const_value::CanonicalConstValue::from_atom(
                            name.as_str(),
                        )
                {
                    let value = self
                        .environment
                        .graph
                        .types
                        .insert(ProductOperandType::CanonicalConst(value));
                    self.active_types.pop();
                    self.types.push((reference, value));
                    return Ok(value);
                }
                if let Some(application) = symbol
                    .is_valid()
                    .then(|| program.data_definition_by_symbol(*symbol))
                    .flatten()
                    .and_then(|definition| definition.generic_instance)
                {
                    let value = self.ty(application)?;
                    self.active_types.pop();
                    self.types.push((reference, value));
                    return Ok(value);
                }
                if let Some(literal) = program.static_const_literal_from_type_reference(reference) {
                    let expression =
                        self.environment
                            .graph
                            .expressions
                            .insert(ProductOperandExpression {
                                source_span: self.owner,
                                authored_selections: HandleSpan::empty(),
                                kind: ProductExpressionKind::Integer(literal),
                            });
                    ProductOperandType::ConstExpression(expression)
                } else {
                    ProductOperandType::Named(self.name(name.as_str(), *symbol))
                }
            }
            types::TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            } => ProductOperandType::Generic {
                name: self.name(base_name.as_str(), *base_symbol),
                lifetimes: self.names(lifetime_arguments),
                arguments: self.type_arguments(*arguments)?,
            },
            types::TypeReferenceNode::DynamicTrait {
                symbol,
                name,
                arguments,
            } => ProductOperandType::Dynamic {
                name: self.name(name.as_str(), *symbol),
                arguments: self.type_arguments(*arguments)?,
            },
            types::TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => ProductOperandType::Reference {
                referee: self.ty(*referee)?,
                access: *access,
                lifetime: lifetime
                    .as_ref()
                    .map(|name| self.name(name.as_str(), SymbolHandle::invalid())),
            },
            types::TypeReferenceNode::Slice { element_type } => {
                ProductOperandType::Slice(self.ty(*element_type)?)
            }
            types::TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => ProductOperandType::Array {
                element: self.ty(*element_type)?,
                length: match length {
                    types::FixedArrayLength::Literal(value) => ProductArrayLength::Literal(*value),
                    types::FixedArrayLength::Expression(value) => {
                        ProductArrayLength::Expression(self.ty(*value)?)
                    }
                    types::FixedArrayLength::ConstParameter { symbol, name } => {
                        ProductArrayLength::Parameter(self.name(name.as_str(), *symbol))
                    }
                    types::FixedArrayLength::ConstCall { machine, name, .. } => {
                        ProductArrayLength::Call(self.name(name.as_str(), *machine))
                    }
                },
            },
            types::TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base = self.ty(*base_type)?;
                let constraints = program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .map(|constraint| self.constraint(constraint))
                    .collect::<Result<Vec<_>, _>>()?;
                ProductOperandType::Constrained {
                    base,
                    constraints: self.environment.graph.constraints.insert_many(constraints),
                }
            }
            types::TypeReferenceNode::ConstExpression(expression) => {
                ProductOperandType::ConstExpression(self.expression(*expression)?)
            }
        };
        let value = self.environment.graph.types.insert(value);
        self.active_types.pop();
        self.types.push((reference, value));
        Ok(value)
    }
    fn constraint(
        &mut self,
        value: &types::TypeConstraintNode,
    ) -> Result<ProductOperandConstraint, String> {
        Ok(match value {
            types::TypeConstraintNode::Named(name) => {
                ProductOperandConstraint::Named(self.name(name.as_str(), SymbolHandle::invalid()))
            }
            types::TypeConstraintNode::ArithmeticDomain(policy) => {
                ProductOperandConstraint::Arithmetic(*policy)
            }
            types::TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } => ProductOperandConstraint::Range {
                minimum: self.expression(*minimum)?,
                maximum: self.expression(*maximum)?,
                inclusive: *end_inclusive,
            },
            types::TypeConstraintNode::Domain(domain) => {
                let args = domain
                    .arguments
                    .iter()
                    .map(|reference| self.ty(*reference))
                    .collect::<Result<Vec<_>, _>>()?;
                let arguments = self.environment.graph.type_arguments.insert_many(args);
                ProductOperandConstraint::Domain {
                    name: self.name(domain.name.as_str(), domain.symbol),
                    arguments,
                }
            }
        })
    }
    fn expression_selections(
        &mut self,
        expression: expr::ExpressionHandle,
    ) -> Result<HandleSpan<ProductOperandSelection>, String> {
        let program = self.program;
        let ledger = program.authored_declaration_selections();
        let mut selections = Vec::new();
        for occurrence in program
            .expression_table
            .authored_selection_occurrences(expression)
        {
            let selection = ledger
                .get(occurrence)
                .ok_or("Build static actual retains an unknown authored selection")?;
            let rejoined = ledger
                .source_coordinate_occurrence(
                    selection.source_span(),
                    selection.exposure(),
                    selection.kind(),
                )
                .map_err(|reason| reason.to_string())?;
            if rejoined != occurrence {
                return Err(
                    "Build static actual changed its exact source selection association".into(),
                );
            }
            selections.push(ProductOperandSelection {
                source_span: selection.source_span(),
                exposure: selection.exposure(),
                kind: Some(selection.kind()),
            });
        }
        Ok(self.environment.graph.selections.insert_many(selections))
    }

    fn expression(
        &mut self,
        expression: expr::ExpressionHandle,
    ) -> Result<Handle<ProductOperandExpression>, String> {
        let program = self.program;
        if let Some((_, value)) = self
            .expressions
            .iter()
            .find(|(source, _)| *source == expression)
        {
            return Ok(*value);
        }
        if self.active_expressions.contains(&expression) {
            return Err("Build static actual expression contains a cycle".into());
        }
        self.active_expressions.push(expression);
        let kind = match program.expression_table.expression(expression) {
            expr::ExpressionNode::Integer(literal) => {
                ProductExpressionKind::Integer(literal.clone())
            }
            expr::ExpressionNode::Boolean(value) => ProductExpressionKind::Boolean(*value),
            expr::ExpressionNode::Float(value) => ProductExpressionKind::Float(value.clone()),
            expr::ExpressionNode::String(value) => ProductExpressionKind::Text(value.to_vec()),
            expr::ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                let values = members
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        let h = self.name(
                            name.as_str(),
                            if index + 1 == members.len() {
                                path.symbol
                            } else {
                                SymbolHandle::invalid()
                            },
                        );
                        self.environment.graph.names.get(h).clone()
                    })
                    .collect::<Vec<_>>();
                ProductExpressionKind::Name(self.environment.graph.names.insert_many(values))
            }
            expr::ExpressionNode::StructLiteral(literal) => {
                let name = self.name(literal.type_name.as_str(), literal.type_symbol);
                let case = literal
                    .case_name
                    .as_ref()
                    .map(|name| self.name(name.as_str(), literal.case_symbol.unwrap_or_default()));
                let fields = program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| {
                        Ok(ProductOperandField {
                            name: self.name(field.name.as_str(), field.field_symbol),
                            value: self.expression(field.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                ProductExpressionKind::Record {
                    name,
                    case,
                    fields: self.environment.graph.fields.insert_many(fields),
                }
            }
            expr::ExpressionNode::ArrayLiteral(elements) => {
                let values = program
                    .expression_table
                    .expression_handles(*elements)
                    .iter()
                    .map(|value| self.expression(*value))
                    .collect::<Result<Vec<_>, _>>()?;
                ProductExpressionKind::Array(
                    self.environment
                        .graph
                        .expression_arguments
                        .insert_many(values),
                )
            }
            expr::ExpressionNode::UndeterminedNaN(format) => {
                ProductExpressionKind::UndeterminedNaN(*format)
            }
            expr::ExpressionNode::ZeroValue(reference) => {
                ProductExpressionKind::ZeroValue(self.ty(*reference)?)
            }
            expr::ExpressionNode::TypeExpression(reference) => {
                ProductExpressionKind::TypeExpression(self.ty(*reference)?)
            }
            expr::ExpressionNode::Unary(unary) => ProductExpressionKind::Unary {
                operator: match unary.operator {
                    expr::UnaryOperator::BitwiseNot => ProductUnaryOperator::BitwiseNot,
                    expr::UnaryOperator::LogicalNot => ProductUnaryOperator::LogicalNot,
                },
                operand: self.expression(unary.operand)?,
            },
            expr::ExpressionNode::Binary(binary) => ProductExpressionKind::Binary {
                left: self.expression(binary.left)?,
                right: self.expression(binary.right)?,
                operator: match binary.operator {
                    expr::BinaryOperator::CaseMembership => ProductBinaryOperator::CaseMembership,
                    expr::BinaryOperator::Add => ProductBinaryOperator::Add,
                    expr::BinaryOperator::And => ProductBinaryOperator::And,
                    expr::BinaryOperator::BitwiseAnd => ProductBinaryOperator::BitwiseAnd,
                    expr::BinaryOperator::BitwiseOr => ProductBinaryOperator::BitwiseOr,
                    expr::BinaryOperator::BitwiseXor => ProductBinaryOperator::BitwiseXor,
                    expr::BinaryOperator::Divide => ProductBinaryOperator::Divide,
                    expr::BinaryOperator::Equal => ProductBinaryOperator::Equal,
                    expr::BinaryOperator::Greater => ProductBinaryOperator::Greater,
                    expr::BinaryOperator::GreaterOrEqual => ProductBinaryOperator::GreaterOrEqual,
                    expr::BinaryOperator::Less => ProductBinaryOperator::Less,
                    expr::BinaryOperator::LessOrEqual => ProductBinaryOperator::LessOrEqual,
                    expr::BinaryOperator::Modulo => ProductBinaryOperator::Modulo,
                    expr::BinaryOperator::Multiply => ProductBinaryOperator::Multiply,
                    expr::BinaryOperator::NotEqual => ProductBinaryOperator::NotEqual,
                    expr::BinaryOperator::Or => ProductBinaryOperator::Or,
                    expr::BinaryOperator::ShiftLeft => ProductBinaryOperator::ShiftLeft,
                    expr::BinaryOperator::ShiftRight => ProductBinaryOperator::ShiftRight,
                    expr::BinaryOperator::Subtract => ProductBinaryOperator::Subtract,
                },
            },
            expr::ExpressionNode::Member(member) => ProductExpressionKind::Member {
                receiver: self.expression(member.receiver)?,
                member: self.name(member.member.as_str(), member.member_symbol),
            },
            expr::ExpressionNode::Indexed(indexed) => ProductExpressionKind::Indexed {
                receiver: self.expression(indexed.collection)?,
                index: self.expression(indexed.index)?,
            },
            expr::ExpressionNode::Cast(cast) => {
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(cast.semantic_domain_arguments)
                    .iter()
                    .map(|reference| self.ty(*reference))
                    .collect::<Result<Vec<_>, _>>()?;
                let domain_arguments = self.environment.graph.type_arguments.insert_many(arguments);
                let members = program
                    .expression_table
                    .name_path_members(cast.semantic_domain);
                let domain = self.names(members);
                ProductExpressionKind::Cast {
                    value: self.expression(cast.value)?,
                    target: self.ty(cast.target_type)?,
                    policy: cast.domain,
                    domain,
                    domain_arguments,
                    form: cast.form,
                }
            }
            expr::ExpressionNode::Call(call) => {
                if call.native_code_layout.is_some() {
                    return Err("an executed static layout actual must retain its materialized value before Product capture".into());
                }
                let receiver = if call.receiver.is_valid() {
                    self.expression(call.receiver)?
                } else {
                    Handle::invalid()
                };
                let target = self.name(call.target.as_str(), call.target_symbol);
                if let Some(occurrence) = call.authored_call_selection {
                    let selection = program
                        .authored_declaration_selections()
                        .get(occurrence)
                        .ok_or("Build static call lost its authored Call occurrence")?;
                    if selection.kind()
                        != language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
                        || !program
                            .expression_table
                            .authored_selection_occurrences(expression)
                            .any(|attached| attached == occurrence)
                    {
                        return Err("Build static call changed its source-owned Call association".into());
                    }
                    // The call token and its selected declaration have separate
                    // coordinates. The original occurrence supplies the former;
                    // `name` above retains the latter for exact source rejoining.
                    self.environment.graph.names.get_mut(target).source_span =
                        selection.source_span();
                }
                let owner = if call.requirement_application.is_valid() {
                    self.ty(call.requirement_application)?
                } else {
                    Handle::invalid()
                };
                let static_arguments = call
                    .machine_arguments
                    .iter()
                    .map(|argument| self.operand(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                let static_arguments = self
                    .environment
                    .graph
                    .static_arguments
                    .insert_many(static_arguments);
                let arguments = program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|expression| self.expression(*expression))
                    .collect::<Result<Vec<_>, _>>()?;
                let arguments = self
                    .environment
                    .graph
                    .expression_arguments
                    .insert_many(arguments);
                let evidence = self.names(&call.evidence_arguments);
                ProductExpressionKind::Call {
                    receiver,
                    target,
                    owner,
                    static_arguments,
                    arguments,
                    evidence,
                    acknowledgement: call.operational_acknowledgement,
                }
            }
            _ => {
                return Err(
                    "Build specialization has a non-static or unmaterialized selected value".into(),
                );
            }
        };
        let authored_selections = self.expression_selections(expression)?;
        let value = self
            .environment
            .graph
            .expressions
            .insert(ProductOperandExpression {
                source_span: program.expression_table.source_span(expression),
                authored_selections,
                kind,
            });
        self.active_expressions.pop();
        self.expressions.push((expression, value));
        Ok(value)
    }
}
