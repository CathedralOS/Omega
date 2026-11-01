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
    let receiving_owner = scope.lexical_owner;
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
        let supplier = application.supplied_operand_owner;
        let supplied_parameters = supplier
            .is_valid()
            .then(|| declaration_parameters(program, supplier));
        let scope = TypeParameterScope {
            type_parameters: supplied_parameters
                .as_deref()
                .unwrap_or(scope.type_parameters),
            lexical_owner: if supplier.is_valid() {
                supplier
            } else {
                scope.lexical_owner
            },
            operand_lifetime_owner: if supplier.is_valid() {
                supplier
            } else {
                scope.operand_lifetime_owner
            },
            ..scope
        };
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
            // A tuple's supplier is only the default: separately copied actuals
            // retain their own complete declaration telescope and cannot acquire
            // a same-spelled binder from this alias's receiving context.
            let argument_supplier = program
                .type_reference_table
                .operand_lifetime_owner(*argument);
            let argument_parameters = argument_supplier
                .is_valid()
                .then(|| declaration_parameters(program, argument_supplier));
            let scope = TypeParameterScope {
                type_parameters: argument_parameters
                    .as_deref()
                    .unwrap_or(scope.type_parameters),
                lexical_owner: if argument_supplier.is_valid() {
                    argument_supplier
                } else {
                    scope.lexical_owner
                },
                operand_lifetime_owner: if argument_supplier.is_valid() {
                    argument_supplier
                } else {
                    scope.operand_lifetime_owner
                },
                ..scope
            };
            // Callable materialization can create a closed expression after
            // the original source cohort's canonical-publication pass. Replay
            // the same formation and exact-carrier judgment here; no receiver
            // facts or deferred obligations may turn it into a closed value.
            if let TypeParameterKind::Const { type_reference } = parameter.kind
                && let TypeReferenceNode::ConstExpression(expression) =
                    program.type_reference_table.type_reference(*argument)
                && !program.pending_const_range_endpoints.contains(expression)
            {
                match check_closed_const_expression_in_carrier(
                    program,
                    scope.lexical_owner,
                    *expression,
                    AppliedValueCarrier::applied(type_reference, parameters, arguments),
                ) {
                    Ok(Some(_)) => continue,
                    Err(mut errors) => {
                        diagnostics.append(&mut errors);
                        continue;
                    }
                    Ok(None) => {}
                }
            }
            let argument_owner = if scope.lexical_owner == receiving_owner {
                owner.generic_argument()
            } else {
                // Keep this formation provenance through every nested type or
                // implementation argument. Reusing the receiver owner would
                // make its facts/local values available again during recursion.
                TypeReferenceOwner::SuppliedOperand {
                    declaration: program.symbols.name(scope.lexical_owner),
                    generic_depth: 1,
                }
            };
            let prior_facts = argument_owner.value_formation_facts(program);
            let local_formation = argument_owner.local_value_formation();
            match &parameter.kind {
                TypeParameterKind::Implementation => implementation_arguments::validate_operand_formation(program, *argument, symbols, argument_owner, scope, diagnostics, obligations),
                TypeParameterKind::Const { type_reference } | TypeParameterKind::Value { type_reference } => validate_const_data_argument(
                    program, alias.name.as_str(), parameter, AppliedValueCarrier::applied(*type_reference, parameters, arguments),
                    *argument, scope, &prior_facts, local_formation, diagnostics, obligations,
                ),
                TypeParameterKind::Machine { contract } => validate_machine_data_argument(program, alias.symbol, alias.name.as_str(), parameter, contract, *argument, scope, diagnostics),
                TypeParameterKind::Proposition { .. } => diagnostics.push(Diagnostic::error(format!("proposition parameter `{}` of type alias `{}` requires a proposition-family argument", parameter.name, alias.name))),
                TypeParameterKind::Type => {
                    if machine_argument_name(program, *argument, scope).is_some() {
                        diagnostics.push(Diagnostic::error(format!("type parameter `{}` of type alias `{}` requires a type argument, not a machine symbol", parameter.name, alias.name)));
                    } else {
                        validate_type_reference_node(program, *argument, symbols, diagnostics, argument_owner, scope, obligations);
                    }
                }
            }
        }
    }
}

/// Read the actual supplier's declared telescope, including captured enclosing
/// declarations. Full symbols distinguish same-spelled fresh and captured
/// binders; this is source formation scope, never execution or a value grant.
fn declaration_parameters(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
) -> Vec<TypeParameter> {
    if let Some(machine) = program.machine_declaration_for_lexical_scope(owner) {
        return program
            .machine_declaration_scope(machine, |left, right| left.symbol == right.symbol)
            .0;
    }
    let mut parameters = Vec::new();
    let mut seen = Vec::new();
    let mut declaration = owner;
    while declaration.is_valid() && !seen.contains(&declaration) {
        seen.push(declaration);
        for parameter in program.declaration_type_parameters(declaration) {
            if !parameters
                .iter()
                .any(|known: &TypeParameter| known.symbol == parameter.symbol)
            {
                parameters.push(parameter.clone());
            }
        }
        declaration = program.declaration_scope_parent(declaration);
    }
    parameters
}
