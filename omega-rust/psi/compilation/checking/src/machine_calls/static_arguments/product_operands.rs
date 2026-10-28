//! Product nominal applications use the same source arithmetic, parameter
//! correspondence and lexical region owners as ordinary static invocations.
//! A closed selected tuple never supplies the facts needed to form its recipe.
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::ProductOperandRole;
use typed_trees::{
    data::TypeParameterKind,
    product_operand::{ProductOperandApplication, ProductSelectionOccurrence},
};

struct InputTelescope<'a> {
    name: &'a str,
    parameters: &'a [typed_trees::data::TypeParameter],
    argument: &'a typed_trees::expression::StaticMachineArgument,
}

fn parameters<'a>(
    program: &'a dyn crate::CheckingInput,
    application: &'a ProductOperandApplication,
) -> Option<Vec<InputTelescope<'a>>> {
    let argument = &application.argument;
    match application.role? {
        ProductOperandRole::BoundarySubject => {
            let definition = program.trait_definition_by_symbol(application.declaration)?;
            Some(vec![InputTelescope {
                name: definition.name.as_str(),
                parameters: program.trait_type_parameters(definition),
                argument,
            }])
        }
        ProductOperandRole::Provider => {
            let definition = program.conformance_by_symbol(application.declaration)?;
            Some(vec![InputTelescope {
                name: program.symbols.name(definition.symbol),
                parameters: program.conformance_type_parameters(definition),
                argument,
            }])
        }
        ProductOperandRole::Requirement => {
            let mut result = Vec::new();
            let mut member = argument;
            if let Some(outer) = &argument.application
                && outer.member.is_valid()
            {
                let definition = program.trait_definition_by_symbol(argument.symbol)?;
                result.push(InputTelescope {
                    name: definition.name.as_str(),
                    parameters: program.trait_type_parameters(definition),
                    argument,
                });
                member = program.expression_table.static_member(outer.member)?;
            }
            // Omission retains the member's own universal telescope. Only a
            // real supplied application introduces source input obligations.
            if member.application.is_some() {
                if let Some((_, signature)) =
                    program.trait_machine_signature(application.declaration)
                {
                    result.push(InputTelescope {
                        name: signature.name.as_str(),
                        parameters: program.state_signature_type_parameters(signature),
                        argument: member,
                    });
                } else {
                    let machine =
                        program.machine_by_own_or_state_symbol(application.declaration)?;
                    result.push(InputTelescope {
                        name: machine.name.as_str(),
                        parameters: program.machine_type_parameters(machine),
                        argument: member,
                    });
                }
            }
            Some(result)
        }
        _ => None,
    }
}

fn occurrence<'a>(
    program: &dyn crate::CheckingInput,
    application: &ProductOperandApplication,
    occurrences: &'a [ProductSelectionOccurrence],
) -> Option<&'a ProductSelectionOccurrence> {
    let mut matching = occurrences.iter().filter(|occurrence| {
        // Source formation always belongs to the original template. A generated
        // clone's state cannot replace the template's own entry assumptions.
        occurrence.machine == application.owner
            && occurrence.request.operands.iter().any(|operand| {
                program
                    .typed_program()
                    .same_product_operand_source(operand, &application.operand)
            })
    });
    let first = matching.next()?;
    if matching.any(|other| other.state != first.state || other.expression != first.expression) {
        return None;
    }
    Some(first)
}

pub(super) fn validate_parameter_correspondence(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let typed = program.typed_program();
    let occurrences = typed_trees::product_operand::collect_product_selection_occurrences(typed);
    for handle in typed.live_product_operand_applications() {
        let application = typed.product_operand_applications.get(handle);
        let Some(telescopes) = parameters(program, application) else {
            diagnostics.push(
                Diagnostic::error("Product application lost its declared input telescopes")
                    .with_source_span(application.operand.source_span),
            );
            continue;
        };
        let state = occurrence(program, application, &occurrences)
            .and_then(|occurrence| program.state_by_symbol(occurrence.state));
        let source = typed
            .expression_table
            .static_machine_application(application.supplying_application);
        for telescope in telescopes {
            let Some(arguments) = telescope.argument.application.as_ref() else {
                diagnostics.push(
                    Diagnostic::error("Product application lost its original actuals")
                        .with_source_span(application.operand.source_span),
                );
                continue;
            };
            if telescope.parameters.len() != arguments.arguments.len() {
                diagnostics.push(
                    Diagnostic::error(
                        "Product application must supply every declaration parameter",
                    )
                    .with_source_span(application.operand.source_span),
                );
                continue;
            }
            if let Err(error) = super::validate_arguments(
                program,
                telescope.name,
                telescope.parameters,
                &arguments.arguments,
                state,
            ) {
                diagnostics.push(error.with_source_span(application.operand.source_span));
            }
            if let Err(error) =
                crate::conformance_applications::validate_conformance_lifetime_arguments(
                    program,
                    telescope.argument,
                    source.lexical_owner,
                )
            {
                diagnostics.push(error.with_source_span(application.operand.source_span));
            }
        }
    }
}

pub(super) fn validate_source_recipes(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    let typed = program.typed_program();
    let occurrences = typed_trees::product_operand::collect_product_selection_occurrences(typed);
    for handle in typed.live_product_operand_applications() {
        let application = typed.product_operand_applications.get(handle);
        let Some(telescopes) = parameters(program, application) else {
            continue;
        };
        if application.environment.is_none() {
            let Some(context) = occurrence(program, application, &occurrences) else {
                diagnostics.push(
                    Diagnostic::error("Product const recipe lost its exact original source state")
                        .with_source_span(application.operand.source_span),
                );
                continue;
            };
            let Some(machine) = program.machine(context.machine) else {
                continue;
            };
            let Some(state) = program.state_by_symbol(context.state) else {
                continue;
            };
            for telescope in telescopes {
                let Some(arguments) = &telescope.argument.application else {
                    continue;
                };
                crate::value_custody::const_expressions::validate_argument_recipes(
                    program,
                    machine,
                    state,
                    telescope.parameters,
                    &arguments.arguments,
                    diagnostics,
                    formation_obligations,
                );
            }
        } else {
            // Reached helper actuals are checked independently in their exact
            // supplying owner. A template-state goal cannot form a closed value.
            let source = typed
                .expression_table
                .static_machine_application(application.supplying_application);
            for telescope in telescopes {
                let Some(arguments) = &telescope.argument.application else {
                    continue;
                };
                for (parameter, argument) in
                    telescope.parameters.iter().zip(arguments.arguments.iter())
                {
                    let TypeParameterKind::Const { type_reference } = parameter.kind else {
                        continue;
                    };
                    let Some(expression) = super::const_expression_argument(program, argument)
                    else {
                        continue;
                    };
                    match crate::check_closed_const_expression(
                        program,
                        source.lexical_owner,
                        expression,
                        type_reference,
                    ) {
                        Ok(Some(_)) => {}
                        Ok(None) => diagnostics.push(
                            Diagnostic::error(
                                "executed Product const operand is not independently closed",
                            )
                            .with_source_span(application.operand.source_span),
                        ),
                        Err(errors) => diagnostics.extend(errors),
                    }
                }
            }
        }
    }
}
