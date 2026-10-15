use super::type_refinement::{TypeBinding, required_type_matches_exact};
use super::{MachineStateIndex, machine_and_state, machine_parameter_contract_definition};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::data::{MachineParameterContract, TypeParameter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AdmittedNominalSelection {
    pub(super) selected_machine: SymbolHandle,
    pub(super) selected_entry: SymbolHandle,
    pub(super) satisfaction_trait: SymbolHandle,
    pub(super) satisfaction_requirement: SymbolHandle,
    pub(super) canonical_requirement_overload: String,
}

pub(super) fn validate_nominal_machine_selection(
    program: &dyn crate::CheckingInput,
    index: &MachineStateIndex,
    generic_owner: &str,
    parameter: &TypeParameter,
    required_contract: &MachineParameterContract,
    selected_symbol: SymbolHandle,
    selected_name: &str,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    ordinal: usize,
    generic_types: &[&TypeParameter],
    bindings: &mut Vec<TypeBinding>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Option<AdmittedNominalSelection>, ()> {
    let MachineParameterContract::Nominal {
        trait_definition: required_trait,
        requirement: required_requirement,
        trait_arguments: required_arguments,
        ..
    } = required_contract
    else {
        return Ok(None);
    };
    // A satisfaction edge remains universally authored in the provider's
    // telescope. Apply its retained provider tuple separately from the caller's
    // requirement tuple; rewriting that edge would change its authority.
    let applied = program.applied_machine_callable(
        application,
        ordinal,
        selected_symbol,
        parameter.symbol,
        super::machine_parameter_signature(program, required_contract),
    );
    // An executable selection can retain its ordinary specialization tuple
    // even when no contextual demand view was needed. The authored edge still
    // belongs to that template; its fixed operands are never inferred again.
    let provider_application = machine_and_state(program, index, selected_symbol)
        .and_then(|(machine, _)| program.machine_specialization(machine.symbol))
        .map(|instance| instance.binding_application)
        .filter(|handle| handle.is_valid())
        .or_else(|| applied.as_ref().map(|view| view.provider));
    let provider_operands = provider_application.map(|provider| {
        program
            .expression_table
            .static_machine_application_bindings(provider)
            .iter()
            .filter_map(|binding| match binding.value {
                typed_trees::static_applications::StaticMachineBindingValue::Type(actual) => {
                    Some((
                        binding.parameter,
                        typed_trees::type_identity::IndexIdentityArgument::TypeArgument(actual),
                    ))
                }
                typed_trees::static_applications::StaticMachineBindingValue::Const(actual) => {
                    Some((
                        binding.parameter,
                        typed_trees::type_identity::IndexIdentityArgument::TypeReference(actual),
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    let mut operand_mismatch = None::<String>;
    let mut arguments_match = |actual: &[typed_trees::types::TypeReferenceHandle],
                               actual_owner: SymbolHandle,
                               bindings: &mut Vec<TypeBinding>| {
        if actual.len() != required_arguments.len() {
            return false;
        }
        if applied.is_none() {
            // Inference may fill only missing caller Type slots. It is followed
            // by exact region-aware equality; carrier equality grants no
            // nominal authority over a differently declared region.
            if provider_application.is_none()
                && !actual
                    .iter()
                    .zip(required_arguments)
                    .all(|(actual, required)| {
                        required_type_matches_exact(
                            program,
                            *actual,
                            *required,
                            generic_types,
                            bindings,
                        )
                    })
            {
                return false;
            }
            use typed_trees::type_identity::{
                AppliedTypeIdentityScope, IndexIdentityArgument, applied_index_identity,
                applied_type_identity,
            };
            let operands = bindings
                .iter()
                .map(|binding| {
                    let value = if program
                        .data_type_parameter_by_symbol(binding.symbol)
                        .is_some_and(|parameter| parameter.kind.is_const_or_value())
                    {
                        IndexIdentityArgument::TypeReference(binding.actual)
                    } else {
                        IndexIdentityArgument::TypeArgument(binding.actual)
                    };
                    (binding.symbol, value)
                })
                .collect::<Vec<_>>();
            let lifetimes = if application.is_valid() {
                program
                    .expression_table
                    .static_machine_application_lifetimes(application)
            } else {
                &[]
            };
            let lifetimes = lifetimes
                .iter()
                .map(|binding| {
                    (
                        binding.parameter.as_str().to_owned(),
                        binding.argument.as_str().to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            let provider_lifetimes = provider_application
                .map(|handle| {
                    program
                        .expression_table
                        .static_machine_application_lifetimes(handle)
                        .iter()
                        .map(|binding| {
                            (
                                binding.parameter.as_str().to_owned(),
                                binding.argument.as_str().to_owned(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let provider_scopes = provider_application
                .map(|handle| {
                    let provider = program.expression_table.static_machine_application(handle);
                    AppliedTypeIdentityScope {
                        declaration: provider.binding_declaration,
                        arguments: provider_operands.as_deref().unwrap_or(&[]),
                        lifetimes: &provider_lifetimes,
                    }
                })
                .into_iter()
                .collect::<Vec<_>>();
            let actual_owner = provider_application.map_or(actual_owner, |handle| {
                program
                    .expression_table
                    .static_machine_application(handle)
                    .lexical_owner
            });
            let caller_owner = if application.is_valid() {
                program
                    .expression_table
                    .static_machine_application(application)
                    .lexical_owner
            } else {
                actual_owner
            };
            let scope = AppliedTypeIdentityScope {
                declaration: program.symbols.get(parameter.symbol).parent,
                arguments: &operands,
                lifetimes: &lifetimes,
            };
            let Some(definition) = program.trait_definition_by_symbol(*required_trait) else {
                return false;
            };
            if program.trait_type_parameters(definition).len() != actual.len() {
                return false;
            }
            return actual
                .iter()
                .zip(required_arguments)
                .zip(program.trait_type_parameters(definition))
                .all(|((actual, required), parameter)| {
                    let (actual, required) = if parameter.kind.is_const_or_value() {
                        (
                            applied_index_identity(
                                program,
                                IndexIdentityArgument::TypeReference(*actual),
                                actual_owner,
                                &provider_scopes,
                            ),
                            applied_index_identity(
                                program,
                                IndexIdentityArgument::TypeReference(*required),
                                caller_owner,
                                &[scope],
                            ),
                        )
                    } else {
                        (
                            applied_type_identity(program, *actual, actual_owner, &provider_scopes),
                            applied_type_identity(program, *required, caller_owner, &[scope]),
                        )
                    };
                    let matches = actual
                        .zip(required)
                        .is_some_and(|(actual, required)| actual == required);
                    if !matches && operand_mismatch.is_none() {
                        operand_mismatch = Some(format!(
                            "actual index/type identity {:?}, required {:?}",
                            actual.and_then(|identity| program.type_identity_text(identity)),
                            required.and_then(|identity| program.type_identity_text(identity))
                        ));
                    }
                    matches
                });
        }
        let applied = applied.as_ref().expect("retained applied selection");
        use typed_trees::type_identity::{
            AppliedTypeIdentityScope, IndexIdentityArgument, applied_index_identity,
            applied_type_identity,
        };
        let provider = program
            .expression_table
            .static_machine_application(provider_application.expect("applied provider tuple"));
        let caller_owner = program
            .expression_table
            .static_machine_application(application)
            .lexical_owner;
        let provider_lifetimes = program
            .expression_table
            .static_machine_application_lifetimes(
                provider_application.expect("applied provider tuple"),
            )
            .iter()
            .map(|binding| {
                (
                    binding.parameter.as_str().to_owned(),
                    binding.argument.as_str().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        let caller_lifetimes = applied
            .caller_lifetimes
            .iter()
            .map(|binding| {
                (
                    binding.parameter.as_str().to_owned(),
                    binding.argument.as_str().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        let provider_scope = AppliedTypeIdentityScope {
            declaration: provider.binding_declaration,
            arguments: provider_operands
                .as_ref()
                .expect("applied provider operands"),
            lifetimes: &provider_lifetimes,
        };
        let caller_operands = applied
            .caller_operands
            .iter()
            .map(|operand| {
                let argument = if program
                    .data_type_parameter_by_symbol(operand.parameter)
                    .is_some_and(|parameter| parameter.kind.is_const_or_value())
                {
                    IndexIdentityArgument::TypeReference(operand.actual)
                } else {
                    IndexIdentityArgument::TypeArgument(operand.actual)
                };
                (operand.parameter, argument)
            })
            .collect::<Vec<_>>();
        let caller_scope = AppliedTypeIdentityScope {
            declaration: program.symbols.get(parameter.symbol).parent,
            arguments: &caller_operands,
            lifetimes: &caller_lifetimes,
        };
        let Some(definition) = program.trait_definition_by_symbol(*required_trait) else {
            return false;
        };
        let parameters = program.trait_type_parameters(definition);
        parameters.len() == actual.len()
            && actual.iter().zip(required_arguments).zip(parameters).all(
                |((actual, required), parameter)| {
                    let (actual, required) = if parameter.kind.is_const_or_value() {
                        (
                            applied_index_identity(
                                program,
                                IndexIdentityArgument::TypeReference(*actual),
                                provider.lexical_owner,
                                &[provider_scope],
                            ),
                            applied_index_identity(
                                program,
                                IndexIdentityArgument::TypeReference(*required),
                                caller_owner,
                                &[caller_scope],
                            ),
                        )
                    } else {
                        (
                            applied_type_identity(
                                program,
                                *actual,
                                provider.lexical_owner,
                                &[provider_scope],
                            ),
                            applied_type_identity(
                                program,
                                *required,
                                caller_owner,
                                &[caller_scope],
                            ),
                        )
                    };
                    let matches = actual
                        .zip(required)
                        .is_some_and(|(actual, required)| actual == required);
                    if !matches && operand_mismatch.is_none() {
                        operand_mismatch = Some(format!(
                            "actual index/type identity {:?}, required {:?}",
                            actual.and_then(|identity| program.type_identity_text(identity)),
                            required.and_then(|identity| program.type_identity_text(identity))
                        ));
                    }
                    matches
                },
            )
    };

    // Forwarding another static binder is sound only when that binder carries
    // the same exact nominal authority. Structural coincidence does not
    // establish a named satisfaction row.
    if let Some((selected_parameter, selected_contract)) =
        machine_parameter_contract_definition(program, selected_symbol)
    {
        if let MachineParameterContract::Nominal {
            trait_definition,
            requirement,
            trait_arguments,
            ..
        } = selected_contract
            && trait_definition == required_trait
            && requirement == required_requirement
        {
            let mut scratch = bindings.clone();
            if arguments_match(
                trait_arguments,
                program.lifetime_declaration_owner(
                    program.declaration_scope_parent(selected_parameter.symbol),
                ),
                &mut scratch,
            ) {
                *bindings = scratch;
                return Ok(None);
            }
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine parameter `{}` forwarded into `{generic_owner}` does not carry the exact nominal requirement of `{}`; matching callable structure establishes no satisfaction row",
            selected_parameter.name, parameter.name
        )));
        return Err(());
    }

    let Some((selected_machine, selected_state)) =
        machine_and_state(program, index, selected_symbol)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "static machine argument `{selected_name}` for nominal parameter `{}` does not resolve to a concrete machine entry",
            parameter.name
        )));
        return Err(());
    };
    let Some(entry_state) = program.machine_states(selected_machine).first() else {
        diagnostics.push(Diagnostic::error(format!(
            "static machine argument `{selected_name}` for nominal parameter `{}` has no callable entry",
            parameter.name
        )));
        return Err(());
    };
    if entry_state.symbol != selected_state.symbol {
        diagnostics.push(Diagnostic::error(format!(
            "static machine argument `{selected_name}` selects a non-entry state; nominal parameter `{}` requires the machine entry that owns its satisfaction row",
            parameter.name
        )));
        return Err(());
    }

    let view = program
        .machine_parameter_contract_view(required_contract)
        .expect("typed nominal contract must retain a valid exact requirement identity");
    let typed_trees::data::MachineParameterContractView::Nominal {
        trait_definition,
        requirement,
    } = view
    else {
        unreachable!("nominal contract projected as structural")
    };
    let satisfaction_owner =
        if let Some(instance) = program.machine_specialization(selected_machine.symbol) {
            let Some(template) = program.machine_by_symbol(instance.template) else {
                diagnostics.push(Diagnostic::error(
                    "nominal provider specialization lost its authored satisfaction owner",
                ));
                return Err(());
            };
            if program
                .expression_table
                .static_machine_application(instance.binding_application)
                .binding_declaration
                != template.symbol
            {
                diagnostics.push(Diagnostic::error(
                    "nominal provider tuple does not rejoin its authored satisfaction owner",
                ));
                return Err(());
            }
            template
        } else {
            selected_machine
        };
    let mut admitted_bindings = None;
    let mut matching_rows = 0usize;
    for conformance in program.machine_trait_conformances(satisfaction_owner) {
        if conformance.symbol != *required_trait
            || !conformance
                .requirement
                .as_ref()
                .is_some_and(|name| name.as_str() == requirement.name.as_str())
        {
            continue;
        }
        let mut scratch = bindings.clone();
        if arguments_match(
            program
                .type_reference_table
                .type_reference_handles(conformance.arguments),
            selected_machine.symbol,
            &mut scratch,
        ) {
            matching_rows += 1;
            admitted_bindings = Some(scratch);
        }
    }
    if matching_rows == 1
        && let Some(admitted) = admitted_bindings
    {
        *bindings = admitted;
    }
    if matching_rows != 1 {
        let provider_tuple = provider_application
            .map(|handle| {
                program
                    .expression_table
                    .static_machine_application_bindings(handle)
                    .iter()
                    .filter_map(|binding| match binding.value {
                        typed_trees::static_applications::StaticMachineBindingValue::Type(
                            actual,
                        )
                        | typed_trees::static_applications::StaticMachineBindingValue::Const(
                            actual,
                        ) => Some(format!(
                            "{}={}",
                            program.symbols.display_path(binding.parameter, "::"),
                            program.display_type_reference(actual)
                        )),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "no retained provider application".to_owned());
        let caller_tuple = bindings
            .iter()
            .map(|binding| {
                format!(
                    "{}={}",
                    program.symbols.display_path(binding.symbol, "::"),
                    program.display_type_reference(binding.actual)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let required_arguments = required_arguments
            .iter()
            .map(|argument| program.display_type_reference(*argument))
            .collect::<Vec<_>>()
            .join(", ");
        diagnostics.push(Diagnostic::error(format!(
            "static machine argument `{selected_name}` for nominal parameter `{}` retains {matching_rows} authored satisfaction row(s) for exact requirement `{}::{}`; exactly one is required and structural coincidence establishes none; required operands [{required_arguments}], provider [{provider_tuple}], caller [{caller_tuple}], contextual demand {}, operand mismatch {}",
            parameter.name, trait_definition.name, requirement.name, applied.is_some(),
            operand_mismatch.unwrap_or_else(|| "unresolved source correspondence".to_owned())
        )));
        return Err(());
    }

    Ok(Some(AdmittedNominalSelection {
        selected_machine: selected_machine.symbol,
        selected_entry: entry_state.symbol,
        satisfaction_trait: *required_trait,
        satisfaction_requirement: *required_requirement,
        canonical_requirement_overload: program
            .trait_requirement_callable_identity_text(trait_definition, requirement),
    }))
}
