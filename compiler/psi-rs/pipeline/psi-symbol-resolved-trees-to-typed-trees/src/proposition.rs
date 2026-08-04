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
        binders: Default::default(),
        parameters: Default::default(),
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
                    copy: binder.bounds.copy,
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
    for (binder, argument) in binders.iter().zip(&call.machine_arguments) {
        if !matches!(
            binder.kind,
            resolved::proposition::PropositionBinderKind::Machine
        ) {
            return Err(Diagnostic::error(format!(
                "proposition `{}` binder `{}` is not a machine index; type/const proposition arguments are not implemented yet",
                call.target.as_str(),
                binder.name.as_str()
            )));
        }
        if !argument.symbol.is_valid() {
            return Err(Diagnostic::error(format!(
                "proposition `{}` received an unresolved machine-index argument for binder `{}`",
                call.target.as_str(),
                binder.name.as_str()
            )));
        }
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
        binder_arguments: call
            .machine_arguments
            .iter()
            .map(|argument| typed::proposition::PropositionBinderArgument {
                path: argument
                    .path
                    .iter()
                    .map(crate::name::lower_name)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                symbol: argument.symbol,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        arguments,
    })
}
