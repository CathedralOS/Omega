//! Admit derived static tuples against their original declaration once, after
//! typed closure. Exact arena operands survive executable retargeting; clone
//! names and display fingerprints are not evidence of a satisfied bound.
//! Open calls owe judgments for their known type and const operands. Evidence
//! fitting waits for a closed tuple, rather than judging a forwarded binder as
//! a named implementation. Runtime Value carriers keep ordinary value checking.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::{GenericConformanceBound, Machine};
use typed_trees::statement::StatementNode;
use typed_trees::static_applications::{
    StaticMachineApplication, StaticMachineApplicationHandle, StaticMachineBinding,
    StaticMachineBindingValue,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub fn validate_static_machine_bindings(
    program: &dyn crate::CheckingInput,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = crate::TopLevelSymbols::build(program, &mut diagnostics);
    let mut judged: Vec<(SymbolHandle, SymbolHandle, StaticMachineApplicationHandle)> = Vec::new();
    // A private execution graph shares arenas with excluded declarations.
    // Only live roots own judgments; arena capacity is not an obligation roster.
    let references = typed_trees::references::collect_program_references(program.typed_program());
    let mut applications = references
        .expressions
        .iter()
        .filter_map(
            |expression| match program.expression_table.expression(*expression) {
                ExpressionNode::Call(call) => Some(call.authored_machine_application),
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            applications.extend(
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .filter_map(|statement| match statement {
                        StatementNode::Call(call) => Some(call.authored_machine_application),
                        _ => None,
                    }),
            );
        }
    }
    applications.extend(
        program
            .machine_specializations
            .iter()
            .filter(|specialization| program.machine_by_symbol(specialization.instance).is_some())
            .map(|specialization| specialization.binding_application),
    );
    applications.retain(|handle| handle.is_valid());
    applications.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    applications.dedup();
    for handle in applications {
        let application = program.expression_table.static_machine_application(handle);
        let bindings = program
            .expression_table
            .static_machine_application_bindings(handle);
        if bindings.is_empty() {
            continue;
        }
        let Some(machine) = program.machine_by_symbol(application.binding_declaration) else {
            diagnostics.push(Diagnostic::error(format!(
                "static tuple for `{}` lost its original binding declaration",
                application.target,
            )));
            continue;
        };
        // Calls and instances can retain the same exact tuple. Judge it once,
        // without hashing display strings or aliasing their independently
        // owned application spans. Type operands can mention caller bounds,
        // including inside compounds; forwarded consts also read that scope.
        let scope = if bindings.iter().any(|binding| {
            matches!(binding.value, StaticMachineBindingValue::Type(_))
                || matches!(binding.value, StaticMachineBindingValue::Const(actual)
                if matches!(program.type_reference_table.type_reference(actual),
                    TypeReferenceNode::Named { symbol, .. } if symbol.is_valid()))
        }) {
            application.lexical_owner
        } else {
            SymbolHandle::invalid()
        };
        if judged.iter().any(|(declaration, owner, previous)| {
            *declaration == machine.symbol
                && *owner == scope
                && program
                    .expression_table
                    .static_machine_application_bindings(*previous)
                    == bindings
        }) {
            continue;
        }
        judged.push((machine.symbol, scope, handle));
        let parameters = program.machine_type_parameters(machine);
        let scope_parameters: &[TypeParameter] = program
            .machine_by_symbol(application.lexical_owner)
            .map_or(&[], |owner| program.machine_type_parameters(owner));
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
                                        scope_parameters,
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
                StaticMachineBindingValue::RuntimeValue { .. }
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
        let Some(selected) = program.conformance_by_symbol(argument.symbol) else {
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
            | StaticMachineBindingValue::RuntimeValue {
                type_reference: actual,
                ..
            } => Some((binding.parameter, program.display_type_reference(actual))),
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
