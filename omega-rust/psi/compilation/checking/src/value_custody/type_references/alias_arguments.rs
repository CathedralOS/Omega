//! An expanded alias retains its own declaration's argument obligations.
//!
//! Validation runs in the same use-site formation context as the ordinary
//! expanded type; erased parameters receive no exemption from their contract.

use super::*;

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    owner: TypeReferenceOwner<'_>,
    scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    for application in program
        .type_reference_table
        .type_alias_applications(reference)
    {
        let Some(alias) = program
            .type_aliases()
            .iter()
            .find(|alias| alias.symbol == application.declaration)
        else {
            diagnostics.push(Diagnostic::error(
                "type alias application has no retained declaration header",
            ));
            continue;
        };
        if program.symbols.symbol_source_span(alias.symbol) != Some(application.declaration_source)
        {
            diagnostics.push(Diagnostic::error(
                "type alias application has different declaration source custody",
            ));
            continue;
        }
        let mut scope = scope;
        if application.supplied_operand_owner.is_valid() {
            scope.operand_lifetime_owner = application.supplied_operand_owner;
        }
        let arguments = program
            .type_reference_table
            .type_reference_handles(application.arguments);
        let parameters = program
            .tables
            .data_type_parameters
            .span_or_empty(alias.type_parameters);
        if parameters.len() != arguments.len()
            || alias.lifetime_parameters.len() != application.lifetime_arguments.len()
        {
            diagnostics.push(Diagnostic::error(format!(
                "type alias `{}` requires its complete declared argument telescope",
                alias.name
            )));
            continue;
        }
        for (slot, lifetime) in application.lifetime_arguments.iter().enumerate() {
            if !scope.contains_operand_lifetime(
                program,
                application.application,
                slot,
                lifetime.as_str(),
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses undeclared lifetime argument `'{}' in type alias `{}`",
                    lifetime.as_str(),
                    alias.name
                )));
            }
        }
        validate_parameter_bounds(
            program,
            alias.name.as_str(),
            parameters,
            arguments,
            scope,
            diagnostics,
        );
        crate::declarations::traits::validate_implementation_application(
            program,
            alias.symbol,
            parameters,
            &alias.lifetime_parameters,
            &alias.conformance_bounds,
            arguments,
            &application.lifetime_arguments,
            if scope.operand_lifetime_owner.is_valid() {
                scope.operand_lifetime_owner
            } else {
                scope.lexical_owner
            },
            &format!("type alias `{}`", alias.name),
            diagnostics,
        );
        for (parameter, argument) in parameters.iter().zip(arguments) {
            match &parameter.kind {
                TypeParameterKind::Implementation => implementation_arguments::validate_operand_formation(program, *argument, symbols, owner.generic_argument(), scope, diagnostics, obligations),
                TypeParameterKind::Const { type_reference } | TypeParameterKind::Value { type_reference } => validate_const_data_argument(
                    program, alias.name.as_str(), parameter, AppliedValueCarrier::applied(*type_reference, parameters, arguments),
                    *argument, scope, &owner.value_formation_facts(program), owner.local_value_formation(), diagnostics, obligations,
                ),
                TypeParameterKind::Machine { contract } => validate_machine_data_argument(program, alias.symbol, alias.name.as_str(), parameter, contract, *argument, scope, diagnostics),
                TypeParameterKind::Proposition { .. } => diagnostics.push(Diagnostic::error(format!("proposition parameter `{}` of type alias `{}` requires a proposition-family argument", parameter.name, alias.name))),
                TypeParameterKind::Type => {
                    if machine_argument_name(program, *argument, scope).is_some() {
                        diagnostics.push(Diagnostic::error(format!("type parameter `{}` of type alias `{}` requires a type argument, not a machine symbol", parameter.name, alias.name)));
                    } else {
                        validate_type_reference_node(program, *argument, symbols, diagnostics, owner.generic_argument(), scope, obligations);
                    }
                }
            }
        }
    }
}
