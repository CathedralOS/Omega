use crate::expression::lower_expression_handle_from_table_in_fact_position;
use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

pub(crate) fn lower_proposition_definition(
    lowerer: &mut Lowerer,
    proposition: &resolved::proposition::PropositionDefinition,
) -> Result<typed::proposition::PropositionDefinition, Diagnostic> {
    let mut typed_proposition = typed::proposition::PropositionDefinition {
        symbol: proposition.symbol,
        name: crate::name::lower_name(&proposition.name),
        is_public: proposition.is_public,
        binders: Default::default(),
        parameters: Default::default(),
        transparent_formula_source_span: proposition.transparent_formula_source_span,
        body: typed::proposition::PropositionBody::Primitive,
    };

    for binder in lowerer
        .source_trees
        .tables
        .declarations
        .proposition_binders
        .span_or_empty(proposition.binders)
    {
        let kind = match &binder.kind {
            resolved::proposition::PropositionBinderKind::Type => {
                typed::proposition::PropositionBinderKind::Type
            }
            resolved::proposition::PropositionBinderKind::Const { type_reference } => {
                typed::proposition::PropositionBinderKind::Const {
                    type_reference: lower_type_reference_into_table(lowerer, type_reference)?,
                }
            }
            resolved::proposition::PropositionBinderKind::Machine => {
                typed::proposition::PropositionBinderKind::Machine
            }
        };
        lowerer.typed_trees.push_proposition_binder(
            &mut typed_proposition,
            typed::proposition::PropositionBinder {
                symbol: binder.symbol,
                name: crate::name::lower_name(&binder.name),
                kind,
                bounds: typed::data::DataProperties {
                    carry: binder.bounds.carry,
                    multiplicity: binder.bounds.multiplicity,
                },
            },
        );
    }

    for parameter in lowerer
        .source_trees
        .state_parameters(proposition.parameters)
    {
        let parameter = crate::state::lower_state_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .push_proposition_parameter(&mut typed_proposition, parameter);
    }

    typed_proposition.body = match &proposition.body {
        resolved::proposition::PropositionBody::Primitive => {
            typed::proposition::PropositionBody::Primitive
        }
        resolved::proposition::PropositionBody::Witness { evidence } => {
            typed::proposition::PropositionBody::Witness {
                evidence: lower_type_reference_into_table(lowerer, evidence)?,
            }
        }
        resolved::proposition::PropositionBody::Transparent { proposition } => {
            let formula = if let resolved::expression::ExpressionNode::Call(call) = lowerer
                .source_trees
                .tables
                .bodies
                .expressions
                .expression(*proposition)
                && call.target_symbol.is_valid()
                && matches!(
                    lowerer.source_trees.symbols.get(call.target_symbol).kind,
                    psi_symbols::SymbolKind::Proposition
                        | psi_symbols::SymbolKind::PropositionParameter
                ) {
                typed::proposition::PropositionFormula::Application(lower_proposition_application(
                    lowerer, call,
                )?)
            } else {
                typed::proposition::PropositionFormula::BooleanExpression(
                    lower_expression_handle_from_table_in_fact_position(
                        lowerer.source_trees,
                        &lowerer.source_trees.tables.bodies.expressions,
                        &mut lowerer.typed_trees,
                        *proposition,
                    )?,
                )
            };
            typed::proposition::PropositionBody::Transparent {
                proposition: formula,
            }
        }
    };

    Ok(typed_proposition)
}

pub(crate) fn lower_proposition_application(
    lowerer: &mut Lowerer,
    call: &resolved::expression::TableCallExpression,
) -> Result<typed::proposition::PropositionApplication, Diagnostic> {
    let declaration = lowerer
        .source_trees
        .propositions
        .iter()
        .find(|proposition| proposition.symbol == call.target_symbol);
    let proposition_parameter = lowerer
        .source_trees
        .tables
        .declarations
        .data_type_parameters
        .iter()
        .map(|(_, parameter)| parameter)
        .find(|parameter| {
            parameter.symbol == call.target_symbol
                && matches!(
                    parameter.kind,
                    resolved::data::TypeParameterKind::Proposition { .. }
                )
        });
    if declaration.is_none() && proposition_parameter.is_none() {
        return Err(Diagnostic::error(format!(
            "proposition application `{}` has no resolved declaration or generic proposition parameter",
            call.target.as_str()
        )));
    }
    let binders = declaration
        .map(|declaration| {
            lowerer
                .source_trees
                .tables
                .declarations
                .proposition_binders
                .span_or_empty(declaration.binders)
        })
        .unwrap_or(&[]);
    if binders.len() != call.machine_arguments.len() {
        return Err(Diagnostic::error(format!(
            "proposition `{}` expects {} proof-static binder argument(s), received {}",
            call.target.as_str(),
            binders.len(),
            call.machine_arguments.len()
        )));
    }
    let mut typed_binder_arguments = Vec::with_capacity(binders.len());
    for (binder, argument) in binders.iter().zip(&call.machine_arguments) {
        let (kind, symbol) = match &binder.kind {
            resolved::proposition::PropositionBinderKind::Type => {
                if argument.const_literal.is_some() {
                    return Err(Diagnostic::error(format!(
                        "proposition `{}` type binder `{}` received a const literal",
                        call.target.as_str(),
                        binder.name.as_str()
                    )));
                }
                let symbol = resolve_proposition_static_path(lowerer, argument);
                if classify_proposition_static_symbol(lowerer, symbol)
                    != Some(typed::proposition::PropositionBinderArgumentKind::Type)
                {
                    return Err(Diagnostic::error(format!(
                        "proposition `{}` type binder `{}` received a non-type argument",
                        call.target.as_str(),
                        binder.name.as_str()
                    )));
                }
                (
                    typed::proposition::PropositionBinderArgumentKind::Type,
                    symbol,
                )
            }
            resolved::proposition::PropositionBinderKind::Const { type_reference } => {
                let symbol = resolve_proposition_static_path(lowerer, argument);
                if let Some(literal) = &argument.const_literal {
                    validate_const_literal_argument(
                        call.target.as_str(),
                        binder.name.as_str(),
                        type_reference,
                        literal,
                    )?;
                } else if classify_proposition_static_symbol(lowerer, symbol)
                    != Some(typed::proposition::PropositionBinderArgumentKind::Const)
                {
                    return Err(Diagnostic::error(format!(
                        "proposition `{}` const binder `{}` received a non-const argument",
                        call.target.as_str(),
                        binder.name.as_str()
                    )));
                } else if !proposition_const_symbol_type(lowerer, symbol)
                    .is_some_and(|actual| proposition_const_types_match(type_reference, actual))
                {
                    return Err(Diagnostic::error(format!(
                        "proposition `{}` const binder `{}` received a const argument with a different declared type",
                        call.target.as_str(),
                        binder.name.as_str()
                    )));
                }
                (
                    typed::proposition::PropositionBinderArgumentKind::Const,
                    symbol,
                )
            }
            resolved::proposition::PropositionBinderKind::Machine => {
                if argument.evidence_projection.is_some() {
                    (
                        typed::proposition::PropositionBinderArgumentKind::Machine,
                        psi_symbols::SymbolHandle::invalid(),
                    )
                } else {
                    let symbol = resolve_proposition_static_path(lowerer, argument);
                    if argument.const_literal.is_some()
                        || classify_proposition_static_symbol(lowerer, symbol)
                            != Some(typed::proposition::PropositionBinderArgumentKind::Machine)
                    {
                        return Err(Diagnostic::error(format!(
                            "proposition `{}` machine binder `{}` received a non-machine argument",
                            call.target.as_str(),
                            binder.name.as_str()
                        )));
                    }
                    (
                        typed::proposition::PropositionBinderArgumentKind::Machine,
                        symbol,
                    )
                }
            }
        };
        typed_binder_arguments.push(typed::proposition::PropositionBinderArgument {
            kind,
            path: argument
                .path
                .iter()
                .map(crate::name::lower_name)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            const_literal: argument.const_literal.clone(),
            evidence_projection: argument.evidence_projection.as_ref().map(|projection| {
                typed::expression::EvidenceProjection {
                    term: crate::name::lower_name(&projection.term),
                    member: crate::name::lower_name(&projection.member),
                }
            }),
            symbol,
        });
    }
    let parameters = if let Some(declaration) = declaration {
        lowerer
            .source_trees
            .state_parameters(declaration.parameters)
    } else if let Some(resolved::data::TypeParameterKind::Proposition { contract }) =
        proposition_parameter.map(|parameter| &parameter.kind)
    {
        lowerer.source_trees.state_parameters(contract.parameters)
    } else {
        &[]
    };
    if parameters.len() != call.arguments.len() {
        return Err(Diagnostic::error(format!(
            "proposition `{}` expects {} value argument(s), received {}",
            call.target.as_str(),
            parameters.len(),
            call.arguments.len()
        )));
    }

    let arguments = lowerer
        .source_trees
        .tables
        .bodies
        .expressions
        .expression_handles(call.arguments)
        .iter()
        .map(|argument| {
            lower_expression_handle_from_table_in_fact_position(
                lowerer.source_trees,
                &lowerer.source_trees.tables.bodies.expressions,
                &mut lowerer.typed_trees,
                *argument,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let arguments = lowerer
        .typed_trees
        .expression_table
        .insert_expression_handles(arguments);

    Ok(typed::proposition::PropositionApplication {
        proposition: call.target_symbol,
        name: crate::name::lower_name(&call.target),
        binder_arguments: typed_binder_arguments.into_boxed_slice(),
        arguments,
    })
}

fn resolve_proposition_static_path(
    lowerer: &Lowerer,
    argument: &resolved::expression::StaticMachineArgument,
) -> psi_symbols::SymbolHandle {
    if argument.symbol.is_valid() {
        return argument.symbol;
    }
    lowerer
        .source_trees
        .symbols
        .find_descendant_by_path(
            lowerer.source_trees.symbols.root(),
            argument.path.iter().map(|member| member.as_str()),
        )
        .unwrap_or_else(psi_symbols::SymbolHandle::invalid)
}

fn classify_proposition_static_symbol(
    lowerer: &Lowerer,
    symbol: psi_symbols::SymbolHandle,
) -> Option<typed::proposition::PropositionBinderArgumentKind> {
    if !symbol.is_valid() {
        return None;
    }
    match lowerer.source_trees.symbols.get(symbol).kind {
        psi_symbols::SymbolKind::BuiltinType | psi_symbols::SymbolKind::Data => {
            Some(typed::proposition::PropositionBinderArgumentKind::Type)
        }
        psi_symbols::SymbolKind::State
        | psi_symbols::SymbolKind::MachineParameter
        | psi_symbols::SymbolKind::PropositionMachineParameter => {
            Some(typed::proposition::PropositionBinderArgumentKind::Machine)
        }
        psi_symbols::SymbolKind::TypeParameter => lowerer
            .source_trees
            .tables
            .declarations
            .proposition_binders
            .iter()
            .map(|(_, binder)| binder)
            .find(|binder| binder.symbol == symbol)
            .map(|binder| match binder.kind {
                resolved::proposition::PropositionBinderKind::Type => {
                    typed::proposition::PropositionBinderArgumentKind::Type
                }
                resolved::proposition::PropositionBinderKind::Const { .. } => {
                    typed::proposition::PropositionBinderArgumentKind::Const
                }
                resolved::proposition::PropositionBinderKind::Machine => {
                    typed::proposition::PropositionBinderArgumentKind::Machine
                }
            })
            .or_else(|| {
                lowerer
                    .source_trees
                    .tables
                    .declarations
                    .data_type_parameters
                    .iter()
                    .map(|(_, parameter)| parameter)
                    .find(|parameter| parameter.symbol == symbol)
                    .and_then(|parameter| match parameter.kind {
                        resolved::data::TypeParameterKind::Type => {
                            Some(typed::proposition::PropositionBinderArgumentKind::Type)
                        }
                        resolved::data::TypeParameterKind::Const { .. } => {
                            Some(typed::proposition::PropositionBinderArgumentKind::Const)
                        }
                        resolved::data::TypeParameterKind::Machine { .. } => {
                            Some(typed::proposition::PropositionBinderArgumentKind::Machine)
                        }
                        resolved::data::TypeParameterKind::Proposition { .. } => None,
                    })
            }),
        _ => None,
    }
}

fn validate_const_literal_argument(
    proposition: &str,
    binder: &str,
    type_reference: &resolved::types::TypeReference,
    literal: &psi_numerics::literals::IntegerLiteral,
) -> Result<(), Diagnostic> {
    let Some(primitive) = type_reference.primitive_type() else {
        return Err(Diagnostic::error(format!(
            "proposition `{proposition}` const binder `{binder}` has a non-primitive type and cannot receive integer literal `{}`",
            literal.text()
        )));
    };
    if !primitive.accepts_integer_literal() || !integer_literal_fits(literal, primitive) {
        return Err(Diagnostic::error(format!(
            "proposition `{proposition}` const binder `{binder}` cannot receive integer literal `{}` as `{}`",
            literal.text(),
            primitive.name()
        )));
    }
    Ok(())
}

fn proposition_const_symbol_type<'a>(
    lowerer: &'a Lowerer,
    symbol: psi_symbols::SymbolHandle,
) -> Option<&'a resolved::types::TypeReference> {
    lowerer
        .source_trees
        .tables
        .declarations
        .proposition_binders
        .iter()
        .map(|(_, binder)| binder)
        .find(|binder| binder.symbol == symbol)
        .and_then(|binder| match &binder.kind {
            resolved::proposition::PropositionBinderKind::Const { type_reference } => {
                Some(type_reference)
            }
            resolved::proposition::PropositionBinderKind::Type
            | resolved::proposition::PropositionBinderKind::Machine => None,
        })
        .or_else(|| {
            lowerer
                .source_trees
                .tables
                .declarations
                .data_type_parameters
                .iter()
                .map(|(_, parameter)| parameter)
                .find(|parameter| parameter.symbol == symbol)
                .and_then(|parameter| match &parameter.kind {
                    resolved::data::TypeParameterKind::Const { type_reference } => {
                        Some(type_reference)
                    }
                    resolved::data::TypeParameterKind::Type
                    | resolved::data::TypeParameterKind::Machine { .. }
                    | resolved::data::TypeParameterKind::Proposition { .. } => None,
                })
        })
}

fn proposition_const_types_match(
    expected: &resolved::types::TypeReference,
    actual: &resolved::types::TypeReference,
) -> bool {
    match (expected.primitive_type(), actual.primitive_type()) {
        (Some(expected), Some(actual)) => expected == actual,
        _ => match (expected, actual) {
            (
                resolved::types::TypeReference::Named {
                    symbol: expected, ..
                },
                resolved::types::TypeReference::Named { symbol: actual, .. },
            ) => expected == actual,
            _ => expected == actual,
        },
    }
}

fn integer_literal_fits(
    literal: &psi_numerics::literals::IntegerLiteral,
    primitive: resolved::types::PrimitiveType,
) -> bool {
    use resolved::types::PrimitiveType;
    let signed_width = match primitive {
        PrimitiveType::I8 => Some(8),
        PrimitiveType::I16 => Some(16),
        PrimitiveType::I32 => Some(32),
        PrimitiveType::I64 => Some(64),
        _ => None,
    };
    if let Some(width) = signed_width {
        return literal.value_i64().is_some_and(|value| {
            width == 64 || (-(1i64 << (width - 1))..=(1i64 << (width - 1)) - 1).contains(&value)
        });
    }
    let unsigned_width = match primitive {
        PrimitiveType::U8 => Some(8),
        PrimitiveType::U16 => Some(16),
        PrimitiveType::U32 => Some(32),
        PrimitiveType::U64 | PrimitiveType::Addr => Some(64),
        _ => None,
    };
    unsigned_width.is_some_and(|width| {
        literal
            .value_u64()
            .is_some_and(|value| width == 64 || value <= (1u64 << width) - 1)
    })
}
