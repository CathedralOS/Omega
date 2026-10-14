//! Prepare all map-owned callable demands independently of method execution.
//! A callback's complete provider tuple and a map's complete capture tuple have
//! separate retained applications. Preparation grants no callable admission.
use super::super::{Diagnostic, TypedTrees};
use typed_trees::data::TypeParameterKind;
use typed_trees::static_applications::{
    StaticMachineBinding, StaticMachineBindingValue, StaticMachineLifetimeBinding,
};

pub(crate) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let pending = program
        .closed_conformance_application_sites()
        .into_iter()
        .filter(|(_, _, _, app)| !app.callable_application.is_valid())
        .map(|(site, _, supplying, app)| (site, supplying, app.clone()))
        .collect::<Vec<_>>();
    for (site, supplying, application) in pending {
        let declaration = program
            .conformance_by_symbol(application.declaration)
            .cloned()
            .ok_or_else(|| failure("captured callable demand lost its map declaration"))?;
        let parameters = program.conformance_type_parameters(&declaration).to_vec();
        if !parameters
            .iter()
            .any(|p| matches!(p.kind, TypeParameterKind::Machine { .. }))
        {
            continue;
        }
        if parameters.len() != application.arguments.len()
            || declaration.lifetime_parameters.len() != application.lifetime_arguments.len()
        {
            return Err(failure(
                "captured callable demand requires its complete map telescope",
            ));
        }

        let source = program
            .expression_table
            .static_machine_application(supplying)
            .clone();
        let invocation = program.expression_table.retain_static_machine_application(
            declaration.symbol,
            declaration.alias.clone().unwrap_or_default(),
            &application.arguments,
        );
        let record = program
            .expression_table
            .static_machine_application_mut(invocation);
        record.lexical_owner = source.lexical_owner;
        record.binding_declaration = declaration.symbol;
        let mut operands = Vec::new();
        let mut bindings = Vec::new();
        for (parameter, argument) in parameters.iter().zip(application.arguments.iter()) {
            let value = match &parameter.kind {
                TypeParameterKind::Type
                | TypeParameterKind::Const { .. }
                | TypeParameterKind::Value { .. } => {
                    let actual = program
                        .static_operand_type_reference(argument)
                        .map_err(failure)?;
                    operands.push((parameter.symbol, actual));
                    if matches!(parameter.kind, TypeParameterKind::Type) {
                        StaticMachineBindingValue::Type(actual)
                    } else {
                        StaticMachineBindingValue::Const(actual)
                    }
                }
                TypeParameterKind::Machine { .. } => {
                    StaticMachineBindingValue::Machine(argument.clone())
                }
                _ => {
                    return Err(failure(
                        "captured callable demand has unsupported declaration operand",
                    ));
                }
            };
            bindings.push(StaticMachineBinding {
                parameter: parameter.symbol,
                value,
            });
        }
        program
            .expression_table
            .set_static_machine_application_bindings(invocation, &bindings);
        let lifetimes = declaration
            .lifetime_parameters
            .iter()
            .cloned()
            .zip(application.lifetime_arguments.iter())
            .map(|(parameter, argument)| StaticMachineLifetimeBinding {
                parameter,
                argument: typed_trees::name::Identifier::generated(argument),
            })
            .collect::<Vec<_>>();
        program
            .expression_table
            .set_static_machine_application_lifetimes(invocation, &lifetimes);
        let mut selected = Vec::new();
        for (parameter, argument) in parameters.iter().zip(application.arguments.iter()) {
            let TypeParameterKind::Machine { contract } = &parameter.kind else {
                continue;
            };
            let signature = program
                .machine_parameter_contract_view(contract)
                .map(|view| view.signature().clone())
                .ok_or_else(|| failure("captured callable demand lost its required signature"))?;
            let ordinal = selected.len();
            let closed = super::providers::close(
                program,
                argument,
                parameter.symbol,
                &signature,
                &operands,
                invocation,
                ordinal,
            )?;
            let mut executable = argument.clone();
            if let Some(entry) = closed {
                executable.symbol = entry;
                executable.application = None;
            } else if program.symbols.get(argument.symbol).kind
                == symbols::SymbolKind::MachineParameter
            {
                let provider = program.expression_table.retain_static_machine_application(
                    argument.symbol,
                    typed_trees::name::Identifier::generated(argument.display_name()),
                    &[],
                );
                let descriptor = program
                    .expression_table
                    .static_machine_application_mut(provider);
                descriptor.binding_declaration = argument.symbol;
                descriptor.lexical_owner = source.lexical_owner;
                program
                    .prepare_applied_machine_callable(
                        invocation,
                        ordinal,
                        provider,
                        argument.symbol,
                        parameter.symbol,
                        &signature,
                        &operands,
                        &lifetimes,
                    )
                    .map_err(failure)?;
            } else {
                return Err(failure(
                    "captured Machine argument requires a complete selected provider tuple",
                ));
            }
            selected.push(executable);
        }
        program
            .expression_table
            .set_static_machine_application_selected_machines(invocation, &selected);
        program
            .closed_conformance_application_at_mut(site)
            .ok_or_else(|| failure("captured demand lost its source occurrence"))?
            .callable_application = invocation;
    }
    Ok(())
}
fn failure(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}
