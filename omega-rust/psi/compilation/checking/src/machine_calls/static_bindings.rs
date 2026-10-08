//! Admit derived static tuples against their original declaration once, after
//! typed closure. Exact arena operands survive executable retargeting; clone
//! names and display fingerprints are not evidence of a satisfied bound.
//! Open calls owe judgments for their known type and const operands. Evidence
//! fitting waits for a closed tuple, rather than judging a forwarded binder as
//! a named implementation. Runtime Value carriers keep ordinary value checking.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::machine::{GenericConformanceBound, Machine};
use typed_trees::static_applications::{
    StaticMachineApplication, StaticMachineBinding, StaticMachineBindingValue,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub fn validate_static_machine_bindings(
    program: &dyn crate::CheckingInput,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = crate::TopLevelSymbols::build(program, &mut diagnostics);
    for (handle, application) in program.expression_table.static_machine_applications() {
        let bindings = program
            .expression_table
            .static_machine_application_bindings(handle);
        if bindings.is_empty() {
            continue;
        }
        let Some(machine) = program
            .machine_by_symbol(application.registration_operation)
            .or_else(|| program.machine_holding_state(application.registration_operation))
        else {
            diagnostics.push(Diagnostic::error(
                "static tuple lost its original machine declaration",
            ));
            continue;
        };
        let parameters = program.machine_type_parameters(machine);
        let mut type_bounds_satisfied = true;
        for binding in bindings {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == binding.parameter);
            match &binding.value {
                StaticMachineBindingValue::Type(actual) => {
                    let Some(parameter) = parameter else {
                        diagnostics.push(Diagnostic::error(
                            "static type tuple lost its declared binder",
                        ));
                        continue;
                    };
                    type_bounds_satisfied &= crate::unwrapped_type_reference(program, *actual)
                        .is_some_and(|actual| {
                            crate::declared_property_requirements(&parameter.bounds)
                                .iter()
                                .all(|property| {
                                    crate::type_satisfies_declared_property(
                                        program,
                                        &symbols,
                                        &[],
                                        actual,
                                        *property,
                                    )
                                })
                        });
                }
                StaticMachineBindingValue::Const(actual) => {
                    let Some(parameter) = parameter else {
                        diagnostics.push(Diagnostic::error(
                            "const specialization lost its declared binder",
                        ));
                        continue;
                    };
                    validate_const_binding(
                        program,
                        application,
                        machine,
                        parameter,
                        *actual,
                        &mut diagnostics,
                    );
                }
                StaticMachineBindingValue::RuntimeValue(_)
                | StaticMachineBindingValue::Machine(_)
                | StaticMachineBindingValue::Conformance(_) => {}
            }
        }
        if !type_bounds_satisfied {
            let open_owner = program
                .machine_by_symbol(application.lexical_owner)
                .is_some_and(|owner| !program.machine_type_parameters(owner).is_empty());
            let tuple = if open_owner {
                "call tuple"
            } else {
                "concrete specialization tuple"
            };
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` has a {tuple} that does not satisfy its authored type bounds",
                machine.name,
            )));
        }
        let complete = parameters.iter().all(|parameter| match parameter.kind {
            TypeParameterKind::Implementation | TypeParameterKind::Proposition { .. } => true,
            _ => bindings.iter().any(|binding| binding.parameter == parameter.symbol),
        }) && machine.conformance_bounds.iter().all(|bound| bindings.iter().any(|binding| {
            binding.parameter == bound.binder
                && matches!(&binding.value, StaticMachineBindingValue::Conformance(argument)
                    if !argument.symbol.is_valid()
                        || program.symbols.get(argument.symbol).kind != symbols::SymbolKind::ConformanceParameter)
        })) && bindings.iter().all(|binding| match &binding.value {
            StaticMachineBindingValue::Machine(argument) => !argument.symbol.is_valid()
                || program.symbols.get(argument.symbol).kind != symbols::SymbolKind::MachineParameter,
            StaticMachineBindingValue::Const(actual) => !matches!(
                program.type_reference_table.type_reference(*actual),
                TypeReferenceNode::Named { symbol, .. } if symbol.is_valid()
            ),
            _ => true,
        });
        if complete {
            validate_conformance_bindings(program, machine, bindings, &mut diagnostics);
        }
    }
    if diagnostics.iter().any(Diagnostic::is_error) {
        Err(diagnostics)
    } else {
        Ok(())
    }
}

fn validate_const_binding(
    program: &dyn crate::CheckingInput,
    application: &StaticMachineApplication,
    machine: &Machine,
    parameter: &TypeParameter,
    actual: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(actual)
        && symbol.is_valid()
    {
        // An open result tuple may forward its caller's own const binder. It
        // is not a closed value; prove the declared carrier equality instead.
        let caller = program.machine_by_symbol(application.lexical_owner);
        let forwarded = caller.and_then(|caller| {
            program
                .machine_type_parameters(caller)
                .iter()
                .find(|forwarded| forwarded.symbol == *symbol)
        });
        if let Some(forwarded) = forwarded
            && let TypeParameterKind::Const {
                type_reference: expected,
            }
            | TypeParameterKind::Value {
                type_reference: expected,
            } = parameter.kind
            && let TypeParameterKind::Const {
                type_reference: actual,
            }
            | TypeParameterKind::Value {
                type_reference: actual,
            } = forwarded.kind
            && program.package_qualified_type_identity(expected)
                == program.package_qualified_type_identity(actual)
        {
            return;
        }
    }
    if let Err(mut errors) =
        crate::validate_closed_const_argument(program, machine.name.as_str(), parameter, actual)
    {
        diagnostics.append(&mut errors);
    }
}

fn validate_conformance_bindings(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    bindings: &[StaticMachineBinding],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for bound in &machine.conformance_bounds {
        let Some(StaticMachineBinding {
            value: StaticMachineBindingValue::Conformance(argument),
            ..
        }) = bindings
            .iter()
            .find(|binding| binding.parameter == bound.binder)
        else {
            continue;
        };
        let Some(selected) = program
            .conformances()
            .iter()
            .find(|selected| selected.symbol == argument.symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` binds `{}` to a symbol that is not a named conformance",
                machine.name, bound.binder_name,
            )));
            continue;
        };
        let application =
            match crate::conformance_applications::close_conformance_application(program, argument)
            {
                Ok(application) => application,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
        let carrier = crate::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
        if application.trait_definition != carrier.base
            || !conformance_arguments_match(program, machine, bindings, bound, &application)
        {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` cannot bind `{}` to conformance `{}`: expected a complete `{}` implementation with the instantiated trait arguments",
                machine.name, bound.binder_name,
                selected.alias.as_ref().map_or("<unnamed>", |name| name.as_str()), bound.trait_name,
            )));
            continue;
        }
        if let Some(refinement) = carrier.refinement {
            diagnostics.extend(crate::refinement_fit::refinement_fit_diagnostics(
                program,
                refinement,
                selected,
                machine.name.as_str(),
                bound.binder_name.as_str(),
            ));
        }
    }
}

fn conformance_arguments_match(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    bindings: &[StaticMachineBinding],
    bound: &GenericConformanceBound,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
) -> bool {
    let substitutions = bindings
        .iter()
        .filter_map(|binding| match binding.value {
            StaticMachineBindingValue::Type(actual) => Some((
                binding.parameter,
                program.normalized_type_identity(actual).into_string(),
            )),
            StaticMachineBindingValue::Const(actual)
            | StaticMachineBindingValue::RuntimeValue(actual) => {
                Some((binding.parameter, program.display_type_reference(actual)))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let lifetimes = machine
        .lifetime_parameters
        .iter()
        .map(|parameter| {
            (
                parameter.as_str().to_owned(),
                "__ordinary_call_region".to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let identity = |handle, substitutions: &[(SymbolHandle, String)]| {
        crate::conformance_applications::substituted_type_identity_with_lifetimes(
            program,
            handle,
            substitutions,
            &lifetimes,
        )
    };
    // A refinement head may reorder or partially apply its base parameters.
    // Compare the instantiated base, not the surface refinement's arguments.
    let carrier = crate::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
    let required = match carrier
        .refinement
        .and_then(|refinement| refinement.refines.as_ref().map(|base| (refinement, base)))
    {
        Some((refinement, base)) => {
            let head = program
                .data_type_parameters
                .span_or_empty(refinement.type_parameters)
                .iter()
                .map(|parameter| parameter.symbol)
                .zip(
                    bound
                        .arguments
                        .iter()
                        .map(|argument| identity(*argument, &substitutions)),
                )
                .collect::<Vec<_>>();
            program
                .type_reference_table
                .type_reference_handles(base.arguments)
                .iter()
                .map(|argument| identity(*argument, &head))
                .collect::<Vec<_>>()
        }
        None => bound
            .arguments
            .iter()
            .map(|argument| identity(*argument, &substitutions))
            .collect::<Vec<_>>(),
    };
    application.trait_arguments.len() == required.len()
        && required
            .iter()
            .zip(&application.trait_arguments)
            .all(|(required, actual)| {
                let actual = application.lifetime_arguments.iter().fold(
                    actual.clone(),
                    |identity, lifetime| {
                        identity.replace(&format!("'{lifetime}"), "'__ordinary_call_region")
                    },
                );
                *required == actual
            })
}
