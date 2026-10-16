//! Admit derived static tuples against their original declaration once, after
//! typed closure. Exact arena operands survive executable retargeting; clone
//! names and display fingerprints are not evidence of a satisfied bound.
//! Open calls owe judgments for their known type and const operands. Evidence
//! forwarding uses the caller's exact declared promise; named implementation
//! closure still requires a closed tuple. Runtime Value carriers keep ordinary
//! value checking.

mod forwarded_evidence;

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::GenericConformanceBound;
use typed_trees::name::Identifier;
use typed_trees::statement::StatementNode;
use typed_trees::static_applications::{
    StaticMachineApplication, StaticMachineApplicationHandle, StaticMachineBinding,
    StaticMachineBindingValue,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The authored telescope belongs to a callable declaration, including a trait
/// requirement reached through a machine parameter. Executable machine rosters
/// do not own those requirement binders and cannot substitute for their scope.
struct StaticDeclaration<'program> {
    symbol: SymbolHandle,
    name: &'program Identifier,
    parameters: &'program [TypeParameter],
    lifetime_parameters: &'program [Identifier],
    conformance_bounds: &'program [GenericConformanceBound],
}

impl<'program> StaticDeclaration<'program> {
    fn find(program: &'program dyn crate::CheckingInput, symbol: SymbolHandle) -> Option<Self> {
        if let Some(machine) = program.machine_by_symbol(symbol) {
            return Some(Self {
                symbol: machine.symbol,
                name: &machine.name,
                parameters: program.machine_type_parameters(machine),
                lifetime_parameters: &machine.lifetime_parameters,
                conformance_bounds: &machine.conformance_bounds,
            });
        }
        let signature = program
            .trait_machine_signature(symbol)
            .map(|(_, signature)| signature)
            .or_else(|| {
                program
                    .retained_static_machine_contract(symbol)
                    .map(|view| view.signature())
            })?;
        Some(Self {
            symbol: signature.symbol,
            name: &signature.name,
            parameters: program.state_signature_type_parameters(signature),
            lifetime_parameters: &signature.lifetime_parameters,
            conformance_bounds: &[],
        })
    }
}

pub fn validate_static_machine_bindings(
    program: &dyn crate::CheckingInput,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
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
    // A symbolic selected provider has no executable specialization yet. Its
    // occurrence-owned tuple still owes every declared bound, including unused
    // binders. Only views reachable from live calls become obligations.
    let map_applications = program
        .machine_specializations
        .iter()
        .flat_map(|receipt| receipt.conformance_applications.iter())
        .map(|application| application.callable_application)
        .filter(|handle| handle.is_valid())
        .collect::<Vec<_>>();
    let providers = applications
        .iter()
        .chain(map_applications.iter())
        .flat_map(|application| {
            program
                .applied_machine_callables(*application)
                .map(|view| view.provider)
        })
        .collect::<Vec<_>>();
    applications.extend(providers);
    for handle in applications {
        let application = program.expression_table.static_machine_application(handle);
        let bindings = program
            .expression_table
            .static_machine_application_bindings(handle);
        let lifetimes = program
            .expression_table
            .static_machine_application_lifetimes(handle);
        if !lifetimes.is_empty() {
            let declared = StaticDeclaration::find(program, application.binding_declaration);
            let owner = StaticDeclaration::find(program, application.lexical_owner);
            if declared.zip(owner).is_none_or(|(declared, owner)| {
                // Ordinary calls may infer only the used part of a lifetime
                // telescope. Applied provider views separately require a
                // complete explicitly selected tuple, including unused slots.
                lifetimes.iter().enumerate().any(|(ordinal, binding)| {
                    !declared
                        .lifetime_parameters
                        .iter()
                        .any(|parameter| parameter == &binding.parameter)
                        || lifetimes[..ordinal]
                            .iter()
                            .any(|previous| previous.parameter == binding.parameter)
                        || typed_trees::type_identity::lifetime_parameter_identity(
                            program,
                            owner.symbol,
                            binding.argument.as_str(),
                        )
                        .is_none()
                })
            }) {
                diagnostics.push(Diagnostic::error("static lifetime tuple does not retain its complete declaration and lexical region custody"));
                continue;
            }
        }
        if bindings.is_empty() {
            continue;
        }
        let Some(machine) = StaticDeclaration::find(program, application.binding_declaration)
        else {
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
        let scope = application.lexical_owner;
        if judged.iter().any(|(declaration, owner, previous)| {
            *declaration == machine.symbol
                && *owner == scope
                && program
                    .expression_table
                    .static_machine_application_bindings(*previous)
                    == bindings
                && program
                    .expression_table
                    .static_machine_application_lifetimes(*previous)
                    == program
                        .expression_table
                        .static_machine_application_lifetimes(handle)
        }) {
            continue;
        }
        judged.push((machine.symbol, scope, handle));
        let parameters = machine.parameters;
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
                        &machine,
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
        forwarded_evidence::validate(
            program,
            &machine,
            application,
            bindings,
            program
                .expression_table
                .static_machine_application_lifetimes(handle),
            &mut diagnostics,
        );
        if complete {
            validate_conformance_bindings(
                program,
                &machine,
                application,
                bindings,
                program
                    .expression_table
                    .static_machine_application_lifetimes(handle),
                &mut diagnostics,
            );
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
    machine: &StaticDeclaration<'_>,
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
    machine: &StaticDeclaration<'_>,
    call: &StaticMachineApplication,
    bindings: &[StaticMachineBinding],
    lifetimes: &[typed_trees::static_applications::StaticMachineLifetimeBinding],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for bound in machine.conformance_bounds {
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
            || !conformance_arguments_match(
                program,
                machine,
                call.lexical_owner,
                bindings,
                lifetimes,
                bound,
                &application,
            )
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
    machine: &StaticDeclaration<'_>,
    lexical_owner: SymbolHandle,
    bindings: &[StaticMachineBinding],
    lifetimes: &[typed_trees::static_applications::StaticMachineLifetimeBinding],
    bound: &GenericConformanceBound,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
) -> bool {
    use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};

    let substitutions = bindings
        .iter()
        .map(|binding| {
            (
                binding.parameter,
                match &binding.value {
                    StaticMachineBindingValue::Type(actual) => {
                        IndexIdentityArgument::TypeArgument(*actual)
                    }
                    StaticMachineBindingValue::Const(actual) => {
                        IndexIdentityArgument::TypeReference(*actual)
                    }
                    StaticMachineBindingValue::RuntimeValue {
                        subject,
                        expression,
                        ..
                    } => {
                        if expression.is_valid() {
                            IndexIdentityArgument::Expression(*expression)
                        } else {
                            IndexIdentityArgument::Subject(*subject)
                        }
                    }
                    StaticMachineBindingValue::Machine(argument)
                    | StaticMachineBindingValue::Conformance(argument) => {
                        IndexIdentityArgument::StaticArgument(argument)
                    }
                },
            )
        })
        .collect::<Vec<_>>();
    let lifetimes = lifetimes
        .iter()
        .map(|binding| {
            (
                binding.parameter.as_str().to_owned(),
                binding.argument.as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let machine_scope = AppliedTypeIdentityScope {
        declaration: machine.symbol,
        arguments: &substitutions,
        lifetimes: &lifetimes,
    };

    let Some(conformance) = program.conformance_by_symbol(application.declaration) else {
        return false;
    };
    let parameters = program.conformance_type_parameters(conformance);
    if parameters.len() != application.arguments.len() {
        return false;
    }
    let actual_arguments = parameters
        .iter()
        .zip(application.arguments.iter())
        .map(|(parameter, argument)| {
            (
                parameter.symbol,
                IndexIdentityArgument::StaticArgument(argument),
            )
        })
        .collect::<Vec<_>>();
    let actual_lifetimes = conformance
        .lifetime_parameters
        .iter()
        .zip(&application.lifetime_arguments)
        .map(|(parameter, argument)| (parameter.as_str().to_owned(), argument.clone()))
        .collect::<Vec<_>>();
    let actual_scope = AppliedTypeIdentityScope {
        declaration: conformance.symbol,
        arguments: &actual_arguments,
        lifetimes: &actual_lifetimes,
    };
    let required = crate::conformance_applications::applied_bound_argument_identities(
        program,
        bound,
        machine.symbol,
        lexical_owner,
        &[machine_scope],
    );
    let actual = crate::conformance_applications::applied_trait_argument_identities(
        program,
        application.trait_definition,
        program
            .type_reference_table
            .type_reference_handles(conformance.arguments),
        conformance.symbol,
        lexical_owner,
        &[actual_scope],
    );
    required
        .zip(actual)
        .is_some_and(|(required, actual)| required == actual)
}
