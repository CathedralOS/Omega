//! Product nominal applications use the same source arithmetic, parameter
//! correspondence and lexical region owners as ordinary static invocations.
//! A closed selected tuple never supplies the facts needed to form its recipe.
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::ProductOperandRole;
use typed_trees::{
    data::TypeParameterKind,
    product_operand::{ProductOperandApplication, ProductSelectionOccurrence},
};

fn parameters<'a>(
    program: &'a dyn crate::CheckingInput,
    application: &ProductOperandApplication,
) -> Option<(&'a str, &'a [typed_trees::data::TypeParameter])> {
    match application.role? {
        ProductOperandRole::BoundarySubject => {
            let definition = program.trait_definition_by_symbol(application.declaration)?;
            Some((
                definition.name.as_str(),
                program.trait_type_parameters(definition),
            ))
        }
        ProductOperandRole::Provider => {
            let definition = program.conformance_by_symbol(application.declaration)?;
            Some((
                program.symbols.name(definition.symbol),
                program.conformance_type_parameters(definition),
            ))
        }
        _ => None,
    }
}

fn occurrence<'a>(
    _program: &dyn crate::CheckingInput,
    application: &ProductOperandApplication,
    occurrences: &'a [ProductSelectionOccurrence],
) -> Option<&'a ProductSelectionOccurrence> {
    let mut matching = occurrences.iter().filter(|occurrence| {
        // Source formation always belongs to the original template. A generated
        // clone's state cannot replace the template's own entry assumptions.
        occurrence.machine == application.owner
            && occurrence
                .request
                .operands
                .iter()
                .any(|operand| operand == &application.operand)
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
        let Some((name, parameters)) = parameters(program, application) else {
            diagnostics.push(
                Diagnostic::error("Product nominal application lost its declared telescope")
                    .with_source_span(application.operand.source_span),
            );
            continue;
        };
        let Some(arguments) = application.argument.application.as_ref() else {
            diagnostics.push(
                Diagnostic::error("Product nominal application lost its original actuals")
                    .with_source_span(application.operand.source_span),
            );
            continue;
        };
        let state = occurrence(program, application, &occurrences)
            .and_then(|occurrence| program.state_by_symbol(occurrence.state));
        if parameters.len() != arguments.arguments.len() {
            diagnostics.push(
                Diagnostic::error(
                    "Product nominal application must supply every declaration parameter",
                )
                .with_source_span(application.operand.source_span),
            );
            continue;
        }
        if let Err(error) =
            super::validate_arguments(program, name, parameters, &arguments.arguments, state)
        {
            diagnostics.push(error.with_source_span(application.operand.source_span));
        }
        let source = typed
            .expression_table
            .static_machine_application(application.supplying_application);
        if let Err(error) = crate::conformance_applications::validate_conformance_lifetime_arguments(
            program,
            &application.argument,
            source.lexical_owner,
        ) {
            diagnostics.push(error.with_source_span(application.operand.source_span));
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
        let Some((_, parameters)) = parameters(program, application) else {
            continue;
        };
        let Some(arguments) = application.argument.application.as_ref() else {
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
            crate::value_custody::const_expressions::validate_argument_recipes(
                program,
                machine,
                state,
                parameters,
                &arguments.arguments,
                diagnostics,
                formation_obligations,
            );
        } else {
            // The original recipe and the ordinary helper invocation are judged
            // independently. A reached substitution must now be closed; no
            // unrelated template-state fact can form a supplying expression.
            let source = typed
                .expression_table
                .static_machine_application(application.supplying_application);
            for (parameter, argument) in parameters.iter().zip(arguments.arguments.iter()) {
                let TypeParameterKind::Const { type_reference } = parameter.kind else {
                    continue;
                };
                let Some(expression) = super::const_expression_argument(program, argument) else {
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
