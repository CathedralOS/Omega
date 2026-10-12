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
                ExpressionNode::Call(call) => program
                    .expression_table
                    .source_call(call)
                    .map(|original| original.authored_machine_application),
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
    // Actual call occurrences owe supplied lifetime correspondence. A
    // declaration specialization separately describes a quantified family;
    // retaining that receipt never turns it into an additional source call.
    let source_applications = applications.clone();
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
    let providers = applications
        .iter()
        .flat_map(|application| {
            program
                .applied_machine_callables(*application)
                .map(|view| view.provider)
        })
        .collect::<Vec<_>>();
    applications.extend(providers);
    for handle in applications {
        for argument_index in 0..2 {
            if let Some(retained) = program.product_declaration_application(handle, argument_index)
            {
                validate_product_application(program, retained, &mut diagnostics);
            }
        }
        let application = program.expression_table.static_machine_application(handle);
        let bindings = program
            .expression_table
            .static_machine_application_bindings(handle);
        let lifetimes = program
            .expression_table
            .static_machine_application_lifetimes(handle);
        if source_applications.contains(&handle) {
            let source_lifetimes =
                typed_trees::static_applications::StaticApplicationDeclaration::for_application(
                    program.typed_program(),
                    application,
                )
                .and_then(|declaration| {
                    declaration.bind_lifetimes(program.typed_program(), handle)
                });
            if let Err(reason) = source_lifetimes {
                let describe = |symbol: SymbolHandle| {
                    if symbol.is_valid() {
                        format!(
                            "{} ({:?})",
                            program.symbols.display_path(symbol, "::"),
                            program.symbols.get(symbol).kind
                        )
                    } else {
                        "<absent>".to_owned()
                    }
                };
                diagnostics.push(Diagnostic::error(format!(
                    "{reason}: original callable `{}`, registration {}, binding declaration {}, selected interface {}, lexical owner {}",
                    application.target.as_str(), describe(application.registration_operation),
                    describe(application.binding_declaration), describe(application.selected_interface),
                    describe(application.lexical_owner),
                )));
                continue;
            }
        }
        if !lifetimes.is_empty() {
            let declared = program.machine_by_symbol(application.binding_declaration);
            let owner = program.machine_by_symbol(application.lexical_owner);
            if declared.zip(owner).is_none_or(|(declared, owner)| {
                // Complete source correspondence was judged on the actual
                // occurrence above. Every retained named operand still owes
                // its own declaration and lexical scope, including families.
                lifetimes.iter().enumerate().any(|(ordinal, binding)| {
                    !declared
                        .lifetime_parameters
                        .iter()
                        .any(|parameter| parameter == &binding.parameter)
                        || lifetimes[..ordinal]
                            .iter()
                            .any(|previous| previous.parameter == binding.parameter)
                        || binding.argument.named().is_some_and(|argument| {
                            typed_trees::type_identity::lifetime_parameter_identity(
                                program,
                                owner.symbol,
                                argument.as_str(),
                            )
                            .is_none()
                        })
                })
            }) {
                diagnostics.push(Diagnostic::error("static lifetime tuple does not retain its complete declaration and lexical region custody"));
                continue;
            }
        }
        if bindings.is_empty() {
            continue;
        }
        let machine = program.machine_by_symbol(application.binding_declaration);
        let requirement = program
            .trait_machine_signature(application.binding_declaration)
            .filter(|(owner, _)| owner.is_boundary);
        let Some(name) = machine
            .map(|machine| &machine.name)
            .or_else(|| requirement.map(|(_, signature)| &signature.name))
        else {
            diagnostics.push(Diagnostic::error(format!(
                "static tuple for `{}` lost its original binding declaration",
                application.target,
            )));
            continue;
        };
        let declaration =
            match typed_trees::static_applications::StaticApplicationDeclaration::for_application(
                program.typed_program(),
                application,
            ) {
                Ok(declaration) => declaration,
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(reason));
                    continue;
                }
            };
        // Calls and instances can retain the same exact tuple. Judge it once,
        // without hashing display strings or aliasing their independently
        // owned application spans. Type operands can mention caller bounds,
        // including inside compounds; forwarded consts also read that scope.
        let scope = application.lexical_owner;
        if judged.iter().any(|(declaration, owner, previous)| {
            *declaration == application.binding_declaration
                && *owner == scope
                && program
                    .expression_table
                    .static_machine_application(*previous)
                    .captured_lexical_scope
                    == application.captured_lexical_scope
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
        judged.push((application.binding_declaration, scope, handle));
        let parameters = declaration.parameters();
        let Some((_, scope_parameters)) = lexical_parameters(program, application) else {
            diagnostics.push(Diagnostic::error(
                "static application lost its original lexical capture",
            ));
            continue;
        };
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
                        name.as_str(),
                        parameter,
                        *actual,
                        &mut diagnostics,
                    );
                }
                StaticMachineBindingValue::Machine(actual) => {
                    let Some(parameter) = parameter else {
                        diagnostics.push(Diagnostic::error(
                            "static callable tuple lost its declared binder",
                        ));
                        continue;
                    };
                    let TypeParameterKind::Machine { contract } = &parameter.kind else {
                        diagnostics.push(Diagnostic::error(
                            "static callable tuple changed its declared parameter kind",
                        ));
                        continue;
                    };
                    let generic_types = parameters
                        .iter()
                        .copied()
                        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                        .collect::<Vec<_>>();
                    let machine_ordinal = parameters
                        .iter()
                        .take_while(|previous| previous.symbol != parameter.symbol)
                        .filter(|previous| {
                            matches!(previous.kind, TypeParameterKind::Machine { .. })
                        })
                        .count();
                    crate::machine_calls::machine_parameters::validate_static_machine_operand(
                        program,
                        name.as_str(),
                        parameter,
                        contract,
                        actual,
                        handle,
                        machine_ordinal,
                        &actual.display_name(),
                        &generic_types,
                        bindings,
                        &mut diagnostics,
                    );
                }
                StaticMachineBindingValue::RuntimeValue { .. }
                | StaticMachineBindingValue::Conformance(_) => {}
            }
        }
        if !type_bounds_satisfied {
            let open_owner = !scope_parameters.is_empty();
            let tuple = if open_owner {
                "call tuple"
            } else {
                "concrete specialization tuple"
            };
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` has a {tuple} that does not satisfy its authored type bounds",
                name,
            )));
        }
        let complete = parameters.iter().all(|parameter| match parameter.kind {
            TypeParameterKind::Implementation | TypeParameterKind::Proposition { .. } => true,
            _ => bindings.iter().any(|binding| binding.parameter == parameter.symbol),
        }) && declaration.conformance_bounds().iter().all(|bound| bindings.iter().any(|binding| {
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
            application.binding_declaration,
            name.as_str(),
            declaration.conformance_bounds(),
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
                application.binding_declaration,
                name.as_str(),
                declaration.conformance_bounds(),
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

fn lexical_parameters<'program>(
    program: &'program dyn crate::CheckingInput,
    application: &StaticMachineApplication,
) -> Option<(&'program typed_trees::TypedTrees, &'program [TypeParameter])> {
    use typed_trees::type_identity::AppliedTypeIdentityOwner;
    match application.type_identity_owner(program.typed_program())? {
        AppliedTypeIdentityOwner::Captured(scope) => Some((scope.owner(), scope.type_parameters())),
        AppliedTypeIdentityOwner::Declaration(symbol) => Some((
            program.typed_program(),
            program
                .machine_by_symbol(symbol)
                .map_or(&[], |owner| program.machine_type_parameters(owner)),
        )),
    }
}

fn validate_const_binding(
    program: &dyn crate::CheckingInput,
    application: &StaticMachineApplication,
    name: &str,
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
        let Some((_, scope_parameters)) = lexical_parameters(program, application) else {
            diagnostics.push(Diagnostic::error(
                "static application lost its original lexical capture",
            ));
            return;
        };
        let forwarded = scope_parameters
            .iter()
            .find(|forwarded| forwarded.symbol == *symbol);
        if let Some(forwarded) = forwarded
            && let TypeParameterKind::Const {
                type_reference: expected,
            }
            | TypeParameterKind::Value {
                type_reference: expected,
            } = parameter.kind
            && let Some(lexical_owner) = application.type_identity_owner(program.typed_program())
            && let Some(carrier) =
                crate::value_custody::type_references::declared_parameter_value_carrier(
                    program.typed_program(),
                    lexical_owner,
                    forwarded,
                )
            && crate::value_custody::type_references::declared_value_carriers_match(
                carrier,
                crate::value_custody::type_references::DeclaredValueCarrier {
                    program: program.typed_program(),
                    type_reference: expected,
                    lexical_owner:
                        typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(
                            parameter.symbol,
                        ),
                },
                typed_trees::type_identity::TypeIdentityQualification::PackageQualified,
            )
        {
            return;
        }
    }
    if let Err(mut errors) = crate::validate_closed_const_argument(program, name, parameter, actual)
    {
        diagnostics.append(&mut errors);
    }
}

fn validate_conformance_bindings(
    program: &dyn crate::CheckingInput,
    declaration: SymbolHandle,
    name: &str,
    conformance_bounds: &[GenericConformanceBound],
    call: &StaticMachineApplication,
    bindings: &[StaticMachineBinding],
    lifetimes: &[typed_trees::static_applications::StaticMachineLifetimeBinding],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for bound in conformance_bounds {
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
                name, bound.binder_name,
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
                declaration,
                call,
                bindings,
                lifetimes,
                bound,
                &application,
            )
        {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` cannot bind `{}` to conformance `{}`: expected a complete `{}` implementation with the instantiated trait arguments",
                name, bound.binder_name,
                selected.alias.as_ref().map_or("<unnamed>", |name| name.as_str()), bound.trait_name,
            )));
            continue;
        }
        if let Some(refinement) = carrier.refinement {
            diagnostics.extend(crate::refinement_fit::refinement_fit_diagnostics(
                program,
                refinement,
                selected,
                name,
                bound.binder_name.as_str(),
            ));
        }
    }
}

fn conformance_arguments_match(
    program: &dyn crate::CheckingInput,
    declaration: SymbolHandle,
    call: &StaticMachineApplication,
    bindings: &[StaticMachineBinding],
    lifetimes: &[typed_trees::static_applications::StaticMachineLifetimeBinding],
    bound: &GenericConformanceBound,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
) -> bool {
    use typed_trees::type_identity::{
        AppliedTypeIdentityScope, IndexIdentityArgument, applied_type_identity,
    };

    let Some(lexical_owner) = call.type_identity_owner(program.typed_program()) else {
        return false;
    };
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
                    StaticMachineBindingValue::RuntimeValue { subject, .. } => {
                        IndexIdentityArgument::Subject(*subject)
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
                binding.argument.scope_argument(),
            )
        })
        .collect::<Vec<_>>();
    let machine_scope = AppliedTypeIdentityScope {
        region_role: typed_trees::type_identity::AppliedTypeIdentityRegionRole::Supplied,
        declaration,
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
        .map(|(parameter, argument)| (parameter.as_str().to_owned(), Some(argument.clone())))
        .collect::<Vec<_>>();
    let actual_scope = AppliedTypeIdentityScope {
        region_role: typed_trees::type_identity::AppliedTypeIdentityRegionRole::Supplied,
        declaration: conformance.symbol,
        arguments: &actual_arguments,
        lifetimes: &actual_lifetimes,
    };
    let required = crate::conformance_applications::applied_bound_argument_identities(
        program,
        bound,
        lexical_owner,
        &[machine_scope],
        typed_trees::type_identity::TypeIdentityQualification::Ordinary,
    );
    let actual = program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
        .iter()
        .map(|reference| {
            applied_type_identity(
                program.typed_program(),
                *reference,
                lexical_owner,
                &[actual_scope],
                typed_trees::type_identity::TypeIdentityQualification::Ordinary,
            )
        })
        .collect::<Option<Vec<_>>>();
    required
        .zip(actual)
        .is_some_and(|(required, actual)| required == actual)
}

/// Admit one relocated descriptor tuple at the same static admission boundary
/// as executable tuples. The source edge retains exact operand/lifetime arity;
/// typed bindings alone cannot hide a missing or extra authored operand.
fn validate_product_application(
    source: &dyn crate::CheckingInput,
    retained: &typed_trees::typed_trees::product_applications::ProductDeclarationOperand,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let program = retained.owner.as_ref();
    let application = program
        .expression_table
        .static_machine_application(retained.application);
    let Some(schema) = crate::compiler_operations::product_application_declaration(
        program,
        application,
        retained.kind,
    ) else {
        diagnostics.push(Diagnostic::error(
            "Product description lost its exact declaration telescope",
        ));
        return;
    };
    let arguments = program
        .expression_table
        .static_machine_application_arguments(retained.application);
    let bindings = program
        .expression_table
        .static_machine_application_bindings(retained.application);
    let lifetimes = program
        .expression_table
        .static_machine_application_lifetimes(retained.application);
    let parameters = schema
        .parameters
        .iter()
        .map(|parameter| (*parameter).clone())
        .collect::<Vec<_>>();
    let source_argument = source
        .expression_table
        .static_machine_application_arguments(retained.source_application)
        .get(retained.argument_index);
    let Some(source_argument) = source_argument else {
        diagnostics.push(Diagnostic::error(
            "Product application lost its authored operand",
        ));
        return;
    };
    let supplied = source_argument
        .application
        .as_ref()
        .map_or(&[][..], |application| application.arguments.as_ref());
    let lifetime_arguments = source_argument
        .application
        .as_ref()
        .map_or(&[][..], |application| {
            application.lifetime_arguments.as_ref()
        });
    let captured_count = schema.captured_lifetimes.len();
    let explicit_lifetime_count = captured_count + schema.invocation_lifetimes.len();
    if supplied.len() != arguments.len()
        || !(lifetime_arguments.len() == captured_count
            || lifetime_arguments.len() == explicit_lifetime_count)
        || lifetimes.len() != lifetime_arguments.len()
    {
        diagnostics.push(Diagnostic::error(
            "Product requirement application requires its complete static and lifetime telescope",
        ));
        return;
    }
    let Some(derived) = crate::static_arguments::derive_explicit_static_bindings(
        program,
        &parameters,
        schema.conformance_bounds,
        arguments,
    ) else {
        diagnostics.push(Diagnostic::error(format!(
            "Product requirement `{}` requires a complete static tuple in declared binder order",
            schema.name
        )));
        return;
    };
    if application.registration_operation != retained.declaration
        || application.binding_declaration != retained.declaration
        || derived != bindings
    {
        diagnostics.push(Diagnostic::error(
            "Product requirement tuple lost its exact declaration/binder correspondence",
        ));
        return;
    }
    if let Err(error) = crate::static_arguments::validate_argument_telescope(
        program,
        schema.name.as_str(),
        &parameters,
        arguments,
    ) {
        diagnostics.push(error);
    }
    let symbols = crate::TopLevelSymbols::build(program, diagnostics);
    let caller = match application.type_identity_owner(program) {
        Some(typed_trees::type_identity::AppliedTypeIdentityOwner::Captured(scope)) => {
            Some(crate::StaticTypeArgumentOwner::Captured(scope))
        }
        Some(typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(symbol)) => program
            .machine(symbol)
            .map(crate::StaticTypeArgumentOwner::Machine),
        None => {
            diagnostics.push(Diagnostic::error(
                "Product application lost its original lexical capture",
            ));
            return;
        }
    };
    crate::static_arguments::validate_types(program, caller, arguments, &symbols, diagnostics);
    validate_product_argument_visibility(program, arguments, retained.source_span, diagnostics);
    for binding in bindings {
        match &binding.value {
            StaticMachineBindingValue::Type(actual) => {
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == binding.parameter);
                if !parameter.is_some_and(|parameter| {
                    crate::declared_property_requirements(&parameter.bounds)
                        .iter()
                        .all(|property| {
                            crate::type_satisfies_declared_property(
                                program,
                                &symbols,
                                &[],
                                *actual,
                                *property,
                            )
                        })
                }) {
                    diagnostics.push(Diagnostic::error(
                        "Product requirement type tuple does not satisfy its declaration bounds",
                    ));
                }
            }
            StaticMachineBindingValue::Const(actual) => {
                if let Some(parameter) = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == binding.parameter)
                    && let Err(mut errors) = crate::validate_closed_const_argument(
                        program,
                        schema.name.as_str(),
                        parameter,
                        *actual,
                    )
                {
                    diagnostics.append(&mut errors);
                }
            }
            StaticMachineBindingValue::Machine(actual) => {
                if let Some(parameter) = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == binding.parameter)
                    && let TypeParameterKind::Machine { contract } = &parameter.kind
                {
                    let generic_types = parameters
                        .iter()
                        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                        .collect::<Vec<_>>();
                    let machine_ordinal = parameters
                        .iter()
                        .take_while(|previous| previous.symbol != parameter.symbol)
                        .filter(|previous| {
                            matches!(previous.kind, TypeParameterKind::Machine { .. })
                        })
                        .count();
                    crate::machine_calls::machine_parameters::validate_static_machine_operand(
                        program,
                        schema.name.as_str(),
                        parameter,
                        contract,
                        actual,
                        retained.application,
                        machine_ordinal,
                        &actual.display_name(),
                        &generic_types,
                        bindings,
                        diagnostics,
                    );
                }
            }
            StaticMachineBindingValue::Conformance(actual) => {
                if schema
                    .conformance_bounds
                    .iter()
                    .any(|bound| bound.binder == binding.parameter)
                {
                    continue;
                }
                if let Err(error) =
                    crate::conformance_applications::close_conformance_application(program, actual)
                {
                    diagnostics.push(error);
                }
            }
            StaticMachineBindingValue::RuntimeValue { .. } => diagnostics.push(Diagnostic::error(
                "non-executing Product description cannot capture a Build runtime subject",
            )),
        }
    }
    let declared_regions = match caller {
        Some(crate::StaticTypeArgumentOwner::Captured(scope)) => scope.lifetime_parameters(),
        Some(crate::StaticTypeArgumentOwner::Machine(machine)) => {
            machine.lifetime_parameters.as_slice()
        }
        _ => &[],
    };
    for lifetime in lifetimes {
        // A non-executing Product reference has no invocation input from
        // which to infer an anonymous region. Its actual stays named in the
        // original lexical telescope; ordinary calls infer their own inputs.
        if !lifetime
            .argument
            .named()
            .is_some_and(|argument| declared_regions.contains(argument))
        {
            diagnostics.push(Diagnostic::error(
                "Product application lifetime argument is outside its exact lexical owner",
            ));
        }
    }
    validate_conformance_bindings(
        program,
        schema.symbol,
        schema.name.as_str(),
        schema.conformance_bounds,
        application,
        bindings,
        lifetimes,
        diagnostics,
    );
}

fn validate_product_argument_visibility(
    program: &dyn crate::CheckingInput,
    arguments: &[typed_trees::expression::StaticMachineArgument],
    occurrence: source::SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for argument in arguments {
        if argument.type_reference.is_valid() {
            validate_product_type_visibility(
                program,
                argument.type_reference,
                occurrence,
                diagnostics,
            );
        }
        if let Some(application) = &argument.application {
            validate_product_argument_visibility(
                program,
                &application.arguments,
                occurrence,
                diagnostics,
            );
        }
    }
}

fn validate_product_type_visibility(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    occurrence: source::SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    struct ProductTypes {
        types: Vec<TypeReferenceHandle>,
    }
    impl Extend<SymbolHandle> for ProductTypes {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
    }
    impl typed_trees::references::TypedReferences for ProductTypes {
        fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
            if self.types.contains(&reference) {
                return false;
            }
            self.types.push(reference);
            true
        }
    }
    let mut selected = ProductTypes { types: Vec::new() };
    typed_trees::references::visit_type_reference(program, &mut selected, actual);
    for reference in selected.types {
        let symbol = program.type_reference_table.type_symbol(reference);
        if !symbol.is_valid() {
            continue;
        }
        if program.symbols.get(symbol).kind == symbols::SymbolKind::BuiltinType {
            continue;
        }
        let visible = program
            .data_definition_by_symbol(symbol)
            .is_some_and(|definition| {
                program.symbols.product_declaration_visible_from_source(
                    symbol,
                    definition.is_public,
                    occurrence,
                )
            });
        if !visible {
            diagnostics.push(Diagnostic::error(
                "Product description type operand selects a declaration outside its Product scope",
            ));
        }
    }
}
