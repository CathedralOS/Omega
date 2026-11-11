/*
Static machine parameter, static argument, and static binding admission and
type-argument inference (`machine_calls::{machine_parameters, static_arguments,
static_bindings}`).
*/
pub mod machine_parameters {
    pub(crate) mod callable_shape {
        use super::contract_facts::validate_contract_facts;
        use super::type_refinement::{
            BinderBinding, TypeBinding, corresponding_types_match, required_type_matches,
        };
        use super::{
            MachineBlockingRow, MachineStateIndex, MachineSuspensionRow, machine_and_state,
            machine_parameter_contract_definition, machine_parameter_signature,
        };
        use crate::borrow_contracts::CallableBorrowContract;
        use crate::borrow_contracts::PreparedCallableBorrowRefinement;
        use crate::frozen_program::expression_table;
        use crate::machine_calls::effect_inference::full_parameter_invocation;
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::data::{TypeParameter, TypeParameterKind};
        use typed_trees::machine::Machine;
        use typed_trees::signature::StateSignature;
        use typed_trees::signature::{SignatureContract, StateParameter};
        use typed_trees::state::State;
        use typed_trees::typed_trees::AppliedMachineCallable;
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) mod binder_scope {
            use super::BinderBinding;
            use crate::borrow_contracts::CallableLifetimeFrame;

            #[derive(Clone, Default)]
            pub(crate) struct CallableBinderScope {
                pub(crate) static_bindings: Vec<BinderBinding>,
                pub(crate) lifetime_frames: Vec<CallableLifetimeFrame>,
            }

            impl CallableBinderScope {
                pub(crate) fn reversed(&self) -> Self {
                    Self {
                        static_bindings: self
                            .static_bindings
                            .iter()
                            .map(|binding| BinderBinding {
                                required: binding.actual,
                                actual: binding.required,
                            })
                            .collect(),
                        lifetime_frames: self
                            .lifetime_frames
                            .iter()
                            .map(|frame| CallableLifetimeFrame {
                                required: frame.actual.clone(),
                                actual: frame.required.clone(),
                            })
                            .collect(),
                    }
                }
            }
        }
        pub(crate) mod nominal_inputs {
            use super::super::type_refinement::{BinderBinding, TypeBinding};
            use super::complete_callable_type_matches;
            use diagnostics::Diagnostic;
            use typed_trees::data::{MachineParameterContract, TypeParameter};

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn validate(
                program: &dyn crate::CheckingInput,
                label: &str,
                promised: &MachineParameterContract,
                demanded: &MachineParameterContract,
                generic_types: &[&TypeParameter],
                bindings: &mut Vec<TypeBinding>,
                binder_bindings: &[BinderBinding],
                diagnostics: &mut Vec<Diagnostic>,
            ) {
                let MachineParameterContract::Nominal {
                    trait_definition: demanded_trait,
                    requirement: demanded_requirement,
                    trait_arguments: demanded_arguments,
                    ..
                } = demanded
                else {
                    return;
                };
                let mut candidate = bindings.clone();
                let admitted = match promised {
                    MachineParameterContract::Nominal {
                        trait_definition,
                        requirement,
                        trait_arguments,
                        ..
                    } => {
                        trait_definition == demanded_trait
                            && requirement == demanded_requirement
                            && trait_arguments.len() == demanded_arguments.len()
                            && trait_arguments.iter().zip(demanded_arguments).all(
                                |(promised, demanded)| {
                                    complete_callable_type_matches(
                                        program,
                                        *demanded,
                                        *promised,
                                        generic_types,
                                        &mut candidate,
                                        binder_bindings,
                                    )
                                },
                            )
                    }
                    _ => false,
                };
                if admitted {
                    *bindings = candidate;
                } else {
                    diagnostics.push(Diagnostic::error(format!(
            "{label} strengthens its nested nominal input authority; the promised callback does not carry the exact demanded requirement application"
                    )));
                }
            }
        }
        use binder_scope::CallableBinderScope;

        pub(crate) struct CallableParts<'a> {
            pub(crate) lexical_owner: SymbolHandle,
            pub(crate) contract_owner: SymbolHandle,
            pub(crate) lifetime_parameters: &'a [typed_trees::name::Identifier],
            pub(crate) type_parameters: &'a [TypeParameter],
            pub(crate) parameters: &'a [StateParameter],
            pub(crate) return_type: TypeReferenceHandle,
            pub(crate) terminates: bool,
            pub(crate) contracts: &'a [SignatureContract],
            pub(crate) proof_site: Option<super::ValidatedNominalMachineUseSite>,
        }

        impl<'a> CallableParts<'a> {
            fn of_signature(
                program: &'a dyn crate::CheckingInput,
                signature: &'a StateSignature,
            ) -> Self {
                Self {
                    lexical_owner: crate::borrow_contracts::lexical_region_owner(
                        program,
                        signature.symbol,
                    ),
                    contract_owner: signature.symbol,
                    lifetime_parameters: &signature.lifetime_parameters,
                    type_parameters: program.state_signature_type_parameters(signature),
                    parameters: program.state_signature_parameters(signature),
                    return_type: signature.return_type,
                    terminates: signature.termination_guarantee.promises_termination(),
                    contracts: program.state_signature_contracts(signature),
                    proof_site: None,
                }
            }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_selected_callable_shape(
            program: &dyn crate::CheckingInput,
            index: &MachineStateIndex,
            generic_call: &str,
            parameter: &TypeParameter,
            requirement: &StateSignature,
            selected_symbol: SymbolHandle,
            application: typed_trees::static_applications::StaticMachineApplicationHandle,
            ordinal: usize,
            selected_name: &str,
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
            proof_site: Option<super::ValidatedNominalMachineUseSite>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if let Some((actual_machine, actual_state)) =
                machine_and_state(program, index, selected_symbol)
            {
                if let Some(applied) = program.applied_machine_callable(
                    application,
                    ordinal,
                    selected_symbol,
                    parameter.symbol,
                    requirement,
                ) {
                    let label = format!(
                        "machine argument `{}` for `{generic_call}`",
                        actual_machine.name
                    );
                    validate_applied_formation(
                        program,
                        &label,
                        parameter,
                        &applied,
                        proof_site,
                        generic_types,
                        bindings,
                        diagnostics,
                    );
                    return;
                }
                if program.has_applied_machine_selection(application, ordinal, selected_symbol) {
                    diagnostics.push(Diagnostic::error(format!(
                "machine argument `{selected_name}` for `{generic_call}` lost its exact applied demand for `{}`",
                parameter.name
            )));
                    return;
                }
                validate_callable_shape(
                    program,
                    generic_call,
                    parameter,
                    requirement,
                    actual_machine,
                    actual_state,
                    proof_site,
                    generic_types,
                    bindings,
                    diagnostics,
                );
                return;
            }

            if let Some((actual_parameter, actual_contract)) =
                machine_parameter_contract_definition(program, selected_symbol)
            {
                let label = format!(
                    "machine parameter `{}` forwarded into `{generic_call}`",
                    actual_parameter.name
                );
                if matches!(
                    actual_contract,
                    typed_trees::data::MachineParameterContract::RequirementIdentity
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                "{label}: declaration identity and callable requirement are different parameter roles"
            )));
                    return;
                }
                let actual_signature = machine_parameter_signature(program, actual_contract);
                let applied = program.applied_machine_callable(
                    application,
                    ordinal,
                    selected_symbol,
                    parameter.symbol,
                    requirement,
                );
                if program.has_applied_machine_selection(application, ordinal, selected_symbol)
                    && applied.is_none()
                {
                    diagnostics.push(Diagnostic::error(
                        "forwarded callable lost its exact applied demand",
                    ));
                    return;
                }
                let (required, selected) = applied
                    .as_ref()
                    .map_or((requirement, actual_signature), |view| {
                        (view.required, view.selected)
                    });
                validate_callable_parts(
                    program,
                    &label,
                    parameter,
                    required,
                    &CallableParts {
                        proof_site,
                        ..CallableParts::of_signature(program, selected)
                    },
                    selected,
                    generic_types,
                    bindings,
                    &mut CallableBinderScope::default(),
                    diagnostics,
                );
                return;
            }

            if let Some(applied) = program.applied_machine_callable(
                application,
                ordinal,
                selected_symbol,
                parameter.symbol,
                requirement,
            ) {
                let label = format!("machine argument `{selected_name}` for `{generic_call}`");
                validate_applied_formation(
                    program,
                    &label,
                    parameter,
                    &applied,
                    proof_site,
                    generic_types,
                    bindings,
                    diagnostics,
                );
                return;
            }

            diagnostics.push(Diagnostic::error(format!(
        "static machine argument `{selected_name}` does not name a callable machine entry or an in-scope machine parameter"
            )));
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_applied_formation(
            program: &dyn crate::CheckingInput,
            label: &str,
            parameter: &TypeParameter,
            applied: &AppliedMachineCallable<'_>,
            proof_site: Option<super::ValidatedNominalMachineUseSite>,
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            validate_callable_formation(
                program,
                label,
                parameter,
                applied.required,
                &CallableParts {
                    lexical_owner: applied.lexical_owner,
                    proof_site,
                    ..CallableParts::of_signature(program, applied.selected)
                },
                Some(applied),
                generic_types,
                bindings,
                &mut CallableBinderScope::default(),
                diagnostics,
            );
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_callable_shape(
            program: &dyn crate::CheckingInput,
            generic_call: &str,
            parameter: &TypeParameter,
            requirement: &StateSignature,
            actual_machine: &Machine,
            actual_state: &State,
            proof_site: Option<super::ValidatedNominalMachineUseSite>,
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let label = format!(
                "machine argument `{}` for `{generic_call}`",
                actual_machine.name
            );
            let actual_parameters = program.requirement_callable_parameters(
                requirement,
                &program.machine_callable_parameters(actual_machine),
            );
            validate_callable_formation(
                program,
                &label,
                parameter,
                requirement,
                &CallableParts {
                    lexical_owner: actual_machine.symbol,
                    contract_owner: actual_machine.symbol,
                    lifetime_parameters: &actual_machine.lifetime_parameters,
                    type_parameters: &actual_parameters,
                    parameters: program.state_parameters(actual_state),
                    return_type: actual_state.return_type,
                    terminates: matches!(
                        &actual_machine.termination_plan.interface,
                        language_semantics::TerminationInterface::Published(
                            language_semantics::TerminationGuarantee::Terminates { .. }
                        )
                    ),
                    contracts: program.machine_contracts(actual_machine),
                    proof_site,
                },
                None,
                generic_types,
                bindings,
                &mut CallableBinderScope::default(),
                diagnostics,
            );
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_callable_parts(
            program: &dyn crate::CheckingInput,
            label: &str,
            parameter: &TypeParameter,
            requirement: &StateSignature,
            actual: &CallableParts<'_>,
            actual_signature: &StateSignature,
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
            binder_bindings: &mut CallableBinderScope,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            validate_callable_formation(
                program,
                label,
                parameter,
                requirement,
                actual,
                None,
                generic_types,
                bindings,
                binder_bindings,
                diagnostics,
            );
            let actual_operational = crate::machine_calls::effect_inference::signature_operational(
                program,
                actual_signature,
            );
            validate_callable_effects(
                program,
                label,
                parameter,
                requirement,
                actual.parameters,
                &crate::machine_calls::effect_inference::signature_service_reach(
                    program,
                    actual_signature,
                ),
                &crate::declared_signature_invocations(program, actual_signature),
                actual_operational.may_suspend,
                actual_operational.may_block,
                diagnostics,
            );
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_callable_formation(
            program: &dyn crate::CheckingInput,
            label: &str,
            parameter: &TypeParameter,
            requirement: &StateSignature,
            actual: &CallableParts<'_>,
            applied: Option<&AppliedMachineCallable<'_>>,
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
            binder_bindings: &mut CallableBinderScope,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let formation_diagnostic_count = diagnostics.len();
            let actual_parameters = actual.parameters;
            let required_types = program.state_signature_type_parameters(requirement);
            if required_types.len() != actual.type_parameters.len() {
                diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: its callable signature expects {} generic parameter(s), got {}",
            parameter.name, required_types.len(), actual.type_parameters.len(),
        )));
                return;
            }
            binder_bindings.static_bindings.extend(
                required_types
                    .iter()
                    .zip(actual.type_parameters)
                    .map(|(required, actual)| BinderBinding {
                        required: required.symbol,
                        actual: actual.symbol,
                    }),
            );
            let required_parameters = program.state_signature_parameters(requirement);
            if required_parameters.len() != actual_parameters.len() {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} does not refine `{}`: expected {} parameter(s), got {}",
                    parameter.name,
                    required_parameters.len(),
                    actual_parameters.len()
                )));
                return;
            }

            let mut formation_binders = binder_bindings.static_bindings.clone();
            formation_binders.extend(required_parameters.iter().zip(actual_parameters).map(
                |(required, actual)| BinderBinding {
                    required: required.symbol,
                    actual: actual.symbol,
                },
            ));

            for (index, (required, actual)) in required_parameters
                .iter()
                .zip(actual_parameters)
                .enumerate()
            {
                if required.is_mutable != actual.is_mutable
                    || required.is_const != actual.is_const
                    || required.relevance != actual.relevance
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} does not refine `{}`: parameter {} has a different calling mode",
                        parameter.name, index
                    )));
                    continue;
                }
                if !complete_callable_type_matches(
                    program,
                    actual.type_reference,
                    required.type_reference,
                    generic_types,
                    bindings,
                    &formation_binders,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} does not refine `{}`: parameter {} expects `{}`, got `{}`",
                        parameter.name,
                        index,
                        program.display_type_reference(required.type_reference),
                        program.display_type_reference(actual.type_reference)
                    )));
                }
            }

            if !complete_callable_type_matches(
                program,
                actual.return_type,
                requirement.return_type,
                generic_types,
                bindings,
                &formation_binders,
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} does not refine `{}`: expected return `{}`, got `{}`",
                    parameter.name,
                    program.display_type_reference(requirement.return_type),
                    program.display_type_reference(actual.return_type)
                )));
            }

            if diagnostics.len() != formation_diagnostic_count {
                return;
            }

            let mut input_binders = binder_bindings.static_bindings.clone();
            input_binders.extend(
                program
                    .state_signature_parameters(requirement)
                    .iter()
                    .zip(actual.parameters)
                    .map(|(required, actual)| BinderBinding {
                        required: required.symbol,
                        actual: actual.symbol,
                    }),
            );
            let input_substitutions = bindings
                .iter()
                .map(|binding| (binding.symbol, binding.actual))
                .collect::<Vec<_>>();
            let (prepared_borrow, frame) = match with_callable_region_contracts(
                program,
                requirement,
                actual,
                applied,
                &input_substitutions,
                &input_binders,
                &binder_bindings.lifetime_frames,
                |required, actual| {
                    let prepared =
                        PreparedCallableBorrowRefinement::prepare(program, &required, &actual)?;
                    let frame = prepared.lifetime_frame(program, &required, &actual)?;
                    Ok((prepared, frame))
                },
            ) {
                Ok(value) => value,
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} does not refine `{}`: {reason}",
                        parameter.name
                    )));
                    return;
                }
            };
            let previous_frames = binder_bindings.lifetime_frames.len();
            binder_bindings.lifetime_frames.push(frame);
            validate_callable_type_parameters(
                program,
                label,
                parameter,
                requirement,
                actual.type_parameters,
                generic_types,
                bindings,
                binder_bindings,
                diagnostics,
            );
            binder_bindings.lifetime_frames.truncate(previous_frames);

            if diagnostics.len() != formation_diagnostic_count {
                return;
            }
            let substitutions = bindings
                .iter()
                .map(|binding| (binding.symbol, binding.actual))
                .collect::<Vec<_>>();
            if let Err(message) = with_callable_region_contracts(
                program,
                requirement,
                actual,
                applied,
                &substitutions,
                &formation_binders,
                &binder_bindings.lifetime_frames,
                |required, actual| prepared_borrow.validate(program, required, actual),
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} does not refine `{}`: {message}",
                    parameter.name
                )));
                return;
            }

            if requirement.termination_guarantee.promises_termination() && !actual.terminates {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} does not refine `{}`: the requirement guarantees termination",
                    parameter.name
                )));
            }

            let actual_types = applied.map_or_else(Vec::new, |view| {
                program
                    .expression_table
                    .static_machine_application_bindings(view.provider)
                    .iter()
                    .filter_map(|binding| match binding.value {
                        typed_trees::static_applications::StaticMachineBindingValue::Type(
                            actual,
                        ) => Some((binding.parameter, actual)),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            });
            validate_contract_facts(
                program,
                label,
                parameter,
                requirement,
                actual.contracts,
                required_parameters,
                actual_parameters,
                actual.contract_owner,
                actual.return_type,
                &binder_bindings
                    .static_bindings
                    .iter()
                    .map(|binding| (binding.required, binding.actual))
                    .collect::<Vec<_>>(),
                &substitutions,
                super::contract_facts::CallableProofScope {
                    site: actual.proof_site,
                    types: &actual_types,
                },
                diagnostics,
            );
        }

        fn with_callable_region_contracts<R>(
            program: &dyn crate::CheckingInput,
            requirement: &StateSignature,
            actual: &CallableParts<'_>,
            applied: Option<&AppliedMachineCallable<'_>>,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            binder_bindings: &[BinderBinding],
            enclosing_lifetimes: &[crate::borrow_contracts::CallableLifetimeFrame],
            judge: impl FnOnce(
                CallableBorrowContract<'_>,
                CallableBorrowContract<'_>,
            ) -> Result<R, String>,
        ) -> Result<R, String> {
            use crate::borrow_contracts::CallableLifetimeApplication;
            let binders = binder_bindings
                .iter()
                .enumerate()
                .flat_map(|(ordinal, binding)| {
                    let name = format!("$formation{ordinal}");
                    [(binding.required, name.clone()), (binding.actual, name)]
                })
                .collect::<Vec<_>>();
            let mut actual_applications = enclosing_lifetimes
                .iter()
                .map(|frame| CallableLifetimeApplication::Enclosing {
                    bindings: &frame.actual,
                })
                .collect::<Vec<_>>();
            let mut required_applications = enclosing_lifetimes
                .iter()
                .map(|frame| CallableLifetimeApplication::Enclosing {
                    bindings: &frame.required,
                })
                .collect::<Vec<_>>();
            let mut required = CallableBorrowContract {
                lexical_owner: crate::borrow_contracts::lexical_region_owner(
                    program,
                    requirement.symbol,
                ),
                lifetime_parameters: &requirement.lifetime_parameters,
                parameters: program.state_signature_parameters(requirement),
                return_type: requirement.return_type,
                substitutions,
                substitution_scope_starts: &[0],
                application_scopes: &[],
                binders: &binders,
                lifetime_applications: &[],
                argument_owner: actual.lexical_owner,
            };
            let mut selected = CallableBorrowContract {
                lexical_owner: actual.lexical_owner,
                lifetime_parameters: actual.lifetime_parameters,
                parameters: actual.parameters,
                return_type: actual.return_type,
                substitutions,
                substitution_scope_starts: &[0],
                application_scopes: &[],
                binders: &binders,
                lifetime_applications: &[],
                argument_owner: actual.lexical_owner,
            };
            let caller_substitutions;
            let caller_lifetimes;
            let selected_lifetimes;
            let provider_substitutions;
            let provider_lifetimes;
            if let Some(applied) = applied {
                let source = expression_table(program).static_machine_application(applied.provider);
                let machine = program
                    .machine_by_symbol(source.binding_declaration)
                    .ok_or("applied callable lost its original machine declaration")?;
                let state = program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == source.registration_operation)
                    .ok_or("applied callable lost its original entry state")?;
                caller_substitutions = applied
                    .caller_operands
                    .iter()
                    .map(|operand| (operand.parameter, operand.actual))
                    .collect::<Vec<_>>();
                provider_substitutions = expression_table(program)
                    .static_machine_application_bindings(applied.provider)
                    .iter()
                    .filter_map(|binding| match binding.value {
                        typed_trees::static_applications::StaticMachineBindingValue::Type(
                            reference,
                        )
                        | typed_trees::static_applications::StaticMachineBindingValue::Const(
                            reference,
                        ) => Some((binding.parameter, reference)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                caller_lifetimes = crate::borrow_contracts::resolve_callable_lifetime_bindings(
                    program,
                    expression_table(program)
                        .static_machine_application(applied.invocation)
                        .lifetime_binding_declaration,
                    expression_table(program)
                        .static_machine_application(applied.invocation)
                        .lexical_owner,
                    applied.caller_lifetimes,
                    &required_applications,
                )
                .map_err(str::to_owned)?;
                required_applications.push(CallableLifetimeApplication::Enclosing {
                    bindings: &caller_lifetimes,
                });
                required = CallableBorrowContract {
                    lexical_owner: crate::borrow_contracts::lexical_region_owner(
                        program,
                        applied.source_required.symbol,
                    ),
                    lifetime_parameters: &applied.source_required.lifetime_parameters,
                    parameters: program.state_signature_parameters(applied.source_required),
                    return_type: applied.source_required.return_type,
                    substitutions: &caller_substitutions,
                    substitution_scope_starts: &[0],
                    application_scopes: &[],
                    binders: &binders,
                    lifetime_applications: &[],
                    argument_owner: expression_table(program)
                        .static_machine_application(applied.invocation)
                        .lexical_owner,
                };
                provider_lifetimes = program
                    .static_application_lifetimes_for_declaration(applied.provider, machine.symbol)
                    .map_err(str::to_owned)?;
                selected_lifetimes = crate::borrow_contracts::resolve_callable_lifetime_bindings(
                    program,
                    machine.symbol,
                    source.lexical_owner,
                    &provider_lifetimes,
                    &actual_applications,
                )
                .map_err(str::to_owned)?;
                actual_applications.push(CallableLifetimeApplication::Invocation {
                    bindings: &selected_lifetimes,
                });
                selected = CallableBorrowContract {
                    lexical_owner: machine.symbol,
                    lifetime_parameters: &machine.lifetime_parameters,
                    parameters: program.state_parameters(state),
                    return_type: state.return_type,
                    substitutions: &provider_substitutions,
                    substitution_scope_starts: &[0],
                    application_scopes: &[],
                    binders: &binders,
                    lifetime_applications: &[],
                    argument_owner: applied.lexical_owner,
                };
            }
            required.lifetime_applications = &required_applications;
            selected.lifetime_applications = &actual_applications;
            judge(required, selected)
        }

        #[allow(clippy::too_many_arguments)]
        fn complete_callable_type_matches(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
            binder_bindings: &[BinderBinding],
        ) -> bool {
            let needs_inference = generic_types.iter().any(|parameter| {
                !bindings
                    .iter()
                    .any(|binding| binding.symbol == parameter.symbol)
            });
            if needs_inference
                && !required_type_matches(
                    program,
                    actual,
                    required,
                    generic_types,
                    bindings,
                    binder_bindings,
                )
            {
                return false;
            }
            if !actual.is_valid() || !required.is_valid() {
                return actual.is_valid() == required.is_valid();
            }
            corresponding_types_match(program, actual, required, bindings, binder_bindings)
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_proposition_parameter_formation(
            program: &dyn crate::CheckingInput,
            label: &str,
            required: &typed_trees::data::PropositionParameterSignature,
            actual: &typed_trees::data::PropositionParameterSignature,
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
            binder_bindings: &[BinderBinding],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let required = program.state_parameters.span_or_empty(required.parameters);
            let actual = program.state_parameters.span_or_empty(actual.parameters);
            if required.len() != actual.len() {
                diagnostics.push(Diagnostic::error(format!(
                    "{label}: proposition parameter signatures have different arity"
                )));
                return;
            }
            let mut formation_binders = binder_bindings.to_vec();
            formation_binders.extend(required.iter().zip(actual).map(|(required, actual)| {
                BinderBinding {
                    required: required.symbol,
                    actual: actual.symbol,
                }
            }));
            for (position, (required, actual)) in required.iter().zip(actual).enumerate() {
                if required.is_mutable != actual.is_mutable
                    || required.is_const != actual.is_const
                    || required.relevance != actual.relevance
                    || !complete_callable_type_matches(
                        program,
                        actual.type_reference,
                        required.type_reference,
                        generic_types,
                        bindings,
                        &formation_binders,
                    )
                {
                    diagnostics.push(Diagnostic::error(format!(
                "{label}: proposition parameter {position} has a different complete calling shape"
            )));
                }
            }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_selected_callable_effects(
            program: &dyn crate::CheckingInput,
            suspensions: &[MachineSuspensionRow],
            blockings: &[MachineBlockingRow],
            service_reaches: &flow_effects::ServiceReachInferencePlan,
            invocations: &flow_effects::InvocationInferencePlan,
            label: &str,
            parameter: &TypeParameter,
            requirement: &StateSignature,
            actual_machine: &Machine,
            actual_state: &State,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let services = service_reaches
                .for_machine(actual_machine.symbol)
                .map(|summary| service_reaches.services(summary.effective))
                .unwrap_or_else(|| {
                    program
                        .service_reach_rows
                        .services(actual_machine.service_reach_row)
                });
            let actual_invocations = invocations
                .for_machine(actual_machine.symbol)
                .map(|summary| summary.effective.as_slice())
                .unwrap_or_default();
            let may_suspend = suspensions
                .iter()
                .find(|row| row.symbol == actual_machine.symbol)
                .map(|row| row.transitive_may_suspend)
                .unwrap_or(actual_machine.suspends);
            let may_block = blockings
                .iter()
                .find(|row| row.symbol == actual_machine.symbol)
                .map(|row| row.transitive_may_block)
                .unwrap_or(actual_machine.blocks);
            validate_callable_effects(
                program,
                label,
                parameter,
                requirement,
                program.state_parameters(actual_state),
                services,
                actual_invocations,
                may_suspend,
                may_block,
                diagnostics,
            );
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_callable_effects(
            program: &dyn crate::CheckingInput,
            label: &str,
            parameter: &TypeParameter,
            requirement: &StateSignature,
            actual_parameters: &[StateParameter],
            services: &[language_semantics::ServiceReachId],
            invocations: &[flow_effects::InvocationTarget],
            may_suspend: bool,
            may_block: bool,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let required_parameters = program.state_signature_parameters(requirement);
            let allowed_services = crate::machine_calls::effect_inference::signature_service_reach(
                program,
                requirement,
            );
            let operational =
                crate::machine_calls::effect_inference::signature_operational(program, requirement);
            for service in services {
                if allowed_services.contains(service) {
                    continue;
                }
                let name = program
                    .service_reaches
                    .definition(*service)
                    .map(|definition| definition.name.as_str())
                    .unwrap_or("<unknown boundary service>");
                diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: service reach `{name}` exceeds its authored ceiling",
            parameter.name
        )));
            }

            let allowed_invocations = crate::declared_signature_invocations(program, requirement)
                .iter()
                .filter_map(|target| full_parameter_invocation(*target, required_parameters))
                .collect::<Vec<_>>();
            for invocation in invocations {
                if full_parameter_invocation(*invocation, actual_parameters)
                    .is_some_and(|target| allowed_invocations.contains(&target))
                {
                    continue;
                }
                let name = match invocation {
                    flow_effects::InvocationTarget::Parameter(index) => actual_parameters
                        .get(*index as usize)
                        .map(|parameter| parameter.name.as_str())
                        .unwrap_or("<unknown binding>"),
                    flow_effects::InvocationTarget::Service(symbol) => program
                        .trait_definition_by_symbol(*symbol)
                        .map(|definition| definition.name.as_str())
                        .unwrap_or("<unknown boundary service>"),
                };
                diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: synchronous invocation `{name}` exceeds its authored `invokes` ceiling",
            parameter.name
        )));
            }

            if may_suspend && !operational.may_suspend {
                diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: it may suspend, but the requirement omits `suspends`",
            parameter.name
        )));
            }
            if may_block && !operational.may_block {
                diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: it may block, but the requirement omits `blocks`",
            parameter.name
        )));
            }
        }

        pub(crate) fn validate_trait_callable_parameter_refinement(
            program: &dyn crate::CheckingInput,
            label: &str,
            enclosing_lifetimes: &crate::borrow_contracts::CallableLifetimeFrame,
            requirement_parameters: &[TypeParameter],
            actual_parameters: &[TypeParameter],
            generic_types: &[&TypeParameter],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let mut bindings = Vec::new();
            let static_bindings = requirement_parameters
                .iter()
                .zip(actual_parameters)
                .map(|(required, provider)| BinderBinding {
                    required: required.symbol,
                    actual: provider.symbol,
                })
                .collect::<Vec<_>>();
            let binder_bindings = CallableBinderScope {
                static_bindings,
                lifetime_frames: vec![enclosing_lifetimes.clone()],
            };

            for (required, actual) in requirement_parameters.iter().zip(actual_parameters) {
                let (
                    TypeParameterKind::Machine {
                        contract: required_contract,
                    },
                    TypeParameterKind::Machine {
                        contract: actual_contract,
                    },
                ) = (&required.kind, &actual.kind)
                else {
                    continue;
                };
                let nested_label = format!("machine parameter `{}` of {label}", actual.name);
                validate_callable_input(
                    program,
                    &nested_label,
                    required,
                    required_contract,
                    actual_contract,
                    generic_types,
                    &mut bindings,
                    &binder_bindings,
                    diagnostics,
                );
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_callable_type_parameters(
            program: &dyn crate::CheckingInput,
            label: &str,
            parameter: &TypeParameter,
            requirement: &StateSignature,
            actual_parameters: &[TypeParameter],
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
            binder_bindings: &mut CallableBinderScope,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let required_parameters = program.state_signature_type_parameters(requirement);
            if required_parameters.len() != actual_parameters.len() {
                diagnostics.push(Diagnostic::error(format!(
            "{label} does not refine `{}`: its callable signature expects {} generic parameter(s), got {}",
            parameter.name,
            required_parameters.len(),
            actual_parameters.len()
        )));
                return;
            }

            for (index, (required, actual)) in required_parameters
                .iter()
                .zip(actual_parameters)
                .enumerate()
            {
                match (&required.kind, &actual.kind) {
                    (TypeParameterKind::Implementation, TypeParameterKind::Implementation) => {}
                    (
                        TypeParameterKind::Proposition {
                            contract: required_contract,
                        },
                        TypeParameterKind::Proposition {
                            contract: actual_contract,
                        },
                    ) => {
                        validate_proposition_parameter_formation(
                            program,
                            label,
                            required_contract,
                            actual_contract,
                            generic_types,
                            bindings,
                            &binder_bindings.static_bindings,
                            diagnostics,
                        );
                    }
                    (TypeParameterKind::Type, TypeParameterKind::Type) => {
                        if typed_trees::data::type_parameter_demands_stronger_properties(
                            required, actual,
                        ) {
                            diagnostics.push(Diagnostic::error(format!(
                        "{label} does not refine `{}`: generic parameter {} demands stronger type properties",
                        parameter.name, index
                    )));
                        }
                    }
                    (
                        TypeParameterKind::Machine {
                            contract: required_contract,
                        },
                        TypeParameterKind::Machine {
                            contract: actual_contract,
                        },
                    ) => {
                        let nested_label =
                            format!("nested machine parameter `{}` of {label}", actual.name);
                        validate_callable_input(
                            program,
                            &nested_label,
                            required,
                            required_contract,
                            actual_contract,
                            generic_types,
                            bindings,
                            binder_bindings,
                            diagnostics,
                        );
                    }
                    _ => {
                        if let Some((required_type, actual_type)) =
                            crate::value_custody::type_shapes::scalar_parameter_type_pair(
                                &required.kind,
                                &actual.kind,
                            )
                        {
                            if !complete_callable_type_matches(
                                program,
                                actual_type,
                                required_type,
                                generic_types,
                                bindings,
                                &binder_bindings.static_bindings,
                            ) {
                                diagnostics.push(Diagnostic::error(format!(
                            "{label} does not refine `{}`: const generic parameter {} has a different type",
                            parameter.name, index
                        )));
                            }
                        } else {
                            diagnostics.push(Diagnostic::error(format!(
                        "{label} does not refine `{}`: generic parameter {} has a different kind",
                        parameter.name, index
                    )));
                        }
                    }
                }
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_callable_input(
            program: &dyn crate::CheckingInput,
            label: &str,
            parameter: &TypeParameter,
            promised: &typed_trees::data::MachineParameterContract,
            demanded: &typed_trees::data::MachineParameterContract,
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
            binder_bindings: &CallableBinderScope,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            match (promised, demanded) {
                (
                    typed_trees::data::MachineParameterContract::RequirementIdentity,
                    typed_trees::data::MachineParameterContract::RequirementIdentity,
                ) => return,
                (typed_trees::data::MachineParameterContract::RequirementIdentity, _)
                | (_, typed_trees::data::MachineParameterContract::RequirementIdentity) => {
                    diagnostics.push(Diagnostic::error(format!(
                "{label}: declaration identity and callable requirement are different parameter roles"
            )));
                    return;
                }
                _ => {}
            }
            nominal_inputs::validate(
                program,
                label,
                promised,
                demanded,
                generic_types,
                bindings,
                &binder_bindings.static_bindings,
                diagnostics,
            );
            let promised = machine_parameter_signature(program, promised);
            let demanded = machine_parameter_signature(program, demanded);
            let mut reversed = binder_bindings.reversed();
            validate_callable_parts(
                program,
                label,
                parameter,
                demanded,
                &CallableParts::of_signature(program, promised),
                promised,
                generic_types,
                bindings,
                &mut reversed,
                diagnostics,
            );
        }
    }
    pub(crate) mod family_demands {
        use super::{
            CallSelection, FormedStaticMachineSelections, MachineStateIndex, TypeBinding,
            form_call_selection, machine_and_state,
        };
        use diagnostics::Diagnostic;

        pub(crate) fn form(
            program: &dyn crate::CheckingInput,
            index: &MachineStateIndex,
            call: CallSelection<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            occurrence: &mut FormedStaticMachineSelections,
        ) {
            let Some((head, _)) = machine_and_state(program, index, call.target_symbol) else {
                return;
            };
            let Some(family) = program
                .target_machine_families
                .iter()
                .find(|family| family.canonical == head.symbol)
            else {
                return;
            };
            let Some(specialization) = occurrence.rows.requirement_call_specializations.first()
            else {
                return;
            };
            let head_parameters = program.machine_type_parameters(head);
            for member in &family.members {
                if member.machine == head.symbol {
                    continue;
                }
                let Some(machine) = program.machine_by_symbol(member.machine) else {
                    diagnostics.push(Diagnostic::error(
                        "target-family demand lost its retained declaration",
                    ));
                    return;
                };
                let member_parameters = program.machine_type_parameters(machine);
                let mut bindings = Vec::with_capacity(specialization.type_bindings.len());
                for binding in &specialization.type_bindings {
                    let corresponding = head_parameters
                        .iter()
                        .position(|parameter| parameter.symbol == binding.parameter)
                        .and_then(|position| member_parameters.get(position));
                    let Some(parameter) = corresponding else {
                        diagnostics.push(Diagnostic::error(
                            "target-family demand lost its generic binder correspondence",
                        ));
                        return;
                    };
                    bindings.push(TypeBinding {
                        symbol: parameter.symbol,
                        actual: binding.actual,
                    });
                }
                let mut admitted = FormedStaticMachineSelections::default();
                form_call_selection(
                    program,
                    index,
                    CallSelection {
                        target_symbol: machine.symbol,
                        ..call
                    },
                    &bindings,
                    diagnostics,
                    &mut admitted,
                );
                occurrence.pending_effects.extend(admitted.pending_effects);
            }
        }
    }
    #[cfg(test)]
    pub(crate) mod parameter_contract_tests {
        use super::*;
        use symbols::{SymbolNameRef, SymbolTableBuilder};
        use typed_trees::{
            TypedTrees, data::DataDefinition, name::Identifier, type_aliases::TypeAliasDefinition,
        };

        fn callable(
            symbol: SymbolHandle,
            nested: arena::HandleSpan<TypeParameter>,
            suspends: bool,
        ) -> TypeParameter {
            TypeParameter {
                symbol,
                name: Identifier::generated("Selected"),
                kind: TypeParameterKind::Machine {
                    contract: MachineParameterContract::Structural(
                        typed_trees::signature::StateSignature {
                            symbol,
                            name: Identifier::generated("Selected"),
                            type_parameters: nested,
                            suspends,
                            ..Default::default()
                        },
                    ),
                },
                ..Default::default()
            }
        }

        #[test]
        fn alias_and_data_contracts_rejoin_full_owned_binders_and_nested_telescopes() {
            for alias in [false, true] {
                let mut builder = SymbolTableBuilder::default();
                let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
                let kind = if alias {
                    SymbolKind::TypeAlias
                } else {
                    SymbolKind::Data
                };
                let owners = builder.insert_children(
                    root,
                    [
                        (kind, SymbolNameRef::Static("Owner")),
                        (kind, SymbolNameRef::Static("Other")),
                    ],
                );
                let owner = owners.handle_at(0);
                let other = owners.handle_at(1);
                let selected = builder
                    .insert_children(
                        owner,
                        [(
                            SymbolKind::MachineParameter,
                            SymbolNameRef::Static("Selected"),
                        )],
                    )
                    .start();
                let nested = builder
                    .insert_children(
                        selected,
                        [(SymbolKind::MachineParameter, SymbolNameRef::Static("Inner"))],
                    )
                    .start();
                let foreign = builder
                    .insert_children(
                        other,
                        [(
                            SymbolKind::MachineParameter,
                            SymbolNameRef::Static("Selected"),
                        )],
                    )
                    .start();
                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..Default::default()
                };
                let inner = program.tables.data_type_parameters.insert_many([callable(
                    nested,
                    arena::HandleSpan::empty(),
                    false,
                )]);
                let own = program
                    .tables
                    .data_type_parameters
                    .insert_many([callable(selected, inner, false)]);
                let orphan = program.tables.data_type_parameters.insert_many([callable(
                    foreign,
                    arena::HandleSpan::empty(),
                    true,
                )]);
                let publish = |program: &mut TypedTrees, symbol, parameters| {
                    if alias {
                        program.push_type_alias_definition(TypeAliasDefinition {
                            symbol,
                            type_parameters: parameters,
                            ..Default::default()
                        });
                    } else {
                        program.push_data_definition(DataDefinition {
                            symbol,
                            type_parameters: parameters,
                            ..Default::default()
                        });
                    }
                };
                publish(&mut program, owner, own);
                for symbol in [selected, nested] {
                    let (parameter, contract) =
                        machine_parameter_contract_definition(&program, symbol)
                            .expect("owned callable");
                    assert_eq!(parameter.symbol, symbol);
                    let MachineParameterContract::Structural(signature) = contract else {
                        panic!("structural contract");
                    };
                    assert_eq!(signature.symbol, symbol);
                    assert!(!signature.suspends);
                }
                assert!(machine_parameter_contract_definition(&program, foreign).is_none());
                assert!(
                    machine_parameter_contract_definition(&program, SymbolHandle::invalid())
                        .is_none()
                );
                publish(&mut program, other, orphan);
                let (parameter, contract) =
                    machine_parameter_contract_definition(&program, foreign)
                        .expect("separately owned same-spelled callable");
                assert_eq!(parameter.symbol, foreign);
                let MachineParameterContract::Structural(signature) = contract else {
                    panic!("structural contract");
                };
                assert!(signature.suspends);
            }
        }

        #[test]
        fn search_steps_past_identity_only_parameters() {
            let mut builder = SymbolTableBuilder::default();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let owner = builder
                .insert_children(root, [(SymbolKind::Data, SymbolNameRef::Static("Owner"))])
                .start();
            let parameters = builder.insert_children(
                owner,
                [
                    (SymbolKind::MachineParameter, SymbolNameRef::Static("Named")),
                    (
                        SymbolKind::MachineParameter,
                        SymbolNameRef::Static("Selected"),
                    ),
                ],
            );
            let named = parameters.handle_at(0);
            let selected = parameters.handle_at(1);
            let nested = builder
                .insert_children(
                    selected,
                    [(SymbolKind::MachineParameter, SymbolNameRef::Static("Inner"))],
                )
                .start();
            let mut program = TypedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            let inner = program.tables.data_type_parameters.insert_many([callable(
                nested,
                arena::HandleSpan::empty(),
                false,
            )]);
            let own = program.tables.data_type_parameters.insert_many([
                TypeParameter {
                    symbol: named,
                    name: Identifier::generated("Named"),
                    kind: TypeParameterKind::Machine {
                        contract: MachineParameterContract::RequirementIdentity,
                    },
                    ..Default::default()
                },
                callable(selected, inner, false),
            ]);
            program.push_data_definition(DataDefinition {
                symbol: owner,
                type_parameters: own,
                ..Default::default()
            });
            for symbol in [selected, nested] {
                let (parameter, _) = machine_parameter_contract_definition(&program, symbol)
                    .expect("callable after it");
                assert_eq!(parameter.symbol, symbol);
            }
            let (parameter, contract) =
                machine_parameter_contract_definition(&program, named).expect("identity parameter");
            assert_eq!(parameter.symbol, named);
            assert!(matches!(
                contract,
                MachineParameterContract::RequirementIdentity
            ));
        }
    }
    pub(crate) mod selected_dependencies {
        use crate::frozen_program::expression_table;
        use symbols::SymbolHandle;
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::references::TypedReferences;
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) fn machine_selection_dependency_symbols(
            program: &dyn crate::CheckingInput,
            arguments: &[StaticMachineArgument],
        ) -> Vec<SymbolHandle> {
            let mut dependencies = MachineSelectionDependencies {
                program,
                symbols: Vec::new(),
                types: Vec::new(),
            };
            for argument in arguments {
                typed_trees::references::visit_static_argument(
                    program,
                    &mut dependencies,
                    argument,
                );
            }
            dependencies.symbols
        }

        pub(crate) fn call_selection_dependency_symbols<'call>(
            program: &'call dyn crate::CheckingInput,
            call: typed_trees::references::StaticArgumentCall<'call>,
        ) -> Vec<SymbolHandle> {
            let mut dependencies = MachineSelectionDependencies {
                program,
                symbols: Vec::new(),
                types: Vec::new(),
            };
            let live = match call {
                typed_trees::references::StaticArgumentCall::Expression(_, call) => {
                    &call.machine_arguments
                }
                typed_trees::references::StaticArgumentCall::Statement(call) => {
                    &call.machine_arguments
                }
            };
            for argument in live.iter().chain(call.arguments(program)) {
                typed_trees::references::visit_static_argument(
                    program,
                    &mut dependencies,
                    argument,
                );
            }
            for application in call.applications() {
                typed_trees::references::visit_static_machine_bindings(
                    program,
                    &mut dependencies,
                    application,
                );
            }
            dependencies.symbols
        }

        pub(crate) fn specialization_selection_dependency_symbols(
            program: &dyn crate::CheckingInput,
            target: SymbolHandle,
        ) -> Option<Vec<SymbolHandle>> {
            let mut dependencies = MachineSelectionDependencies {
                program,
                symbols: Vec::new(),
                types: Vec::new(),
            };
            let owner = program.machine_by_symbol(target)?;
            program.executable_application_template(owner.symbol).ok()?;
            let mut applications = Vec::new();
            if let Some(application) = program.machine_specialization(owner.symbol) {
                applications.push(application.binding_application);
            }
            for (_, captured) in program
                .captured_machine_applications
                .iter()
                .filter(|(_, captured)| captured.instance == owner.symbol)
            {
                applications.push(captured.binding_application);
                applications.push(captured.supplying_application);
                typed_trees::references::visit_closed_conformance(
                    program,
                    &mut dependencies,
                    &captured.conformance_application,
                );
            }
            for application in applications {
                if !application.is_valid() {
                    return None;
                }
                for argument in
                    expression_table(program).static_machine_application_arguments(application)
                {
                    typed_trees::references::visit_static_argument(
                        program,
                        &mut dependencies,
                        argument,
                    );
                }
                typed_trees::references::visit_static_machine_bindings(
                    program,
                    &mut dependencies,
                    application,
                );
            }
            Some(dependencies.symbols)
        }

        pub(crate) struct MachineSelectionDependencies<'program> {
            program: &'program dyn crate::CheckingInput,
            symbols: Vec<SymbolHandle>,
            types: Vec<TypeReferenceHandle>,
        }

        impl Extend<SymbolHandle> for MachineSelectionDependencies<'_> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
                for symbol in symbols {
                    if super::is_machine_selection_symbol(self.program, symbol)
                        && !self.symbols.contains(&symbol)
                    {
                        self.symbols.push(symbol);
                    }
                }
            }
        }

        impl TypedReferences for MachineSelectionDependencies<'_> {
            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                crate::machine_calls::record_reference(&mut self.types, reference)
            }
        }
    }
    pub(crate) use selected_dependencies::{
        call_selection_dependency_symbols, machine_selection_dependency_symbols,
        specialization_selection_dependency_symbols,
    };
    pub(crate) mod closed_types {
        use super::type_refinement::{TypeBinding, required_type_matches_exact};
        use super::{
            ExpressionHandle, ExpressionNode, Machine, State, StaticMachineArgument, SymbolHandle,
            TypeParameterKind, machine_and_state, machine_parameter_signature, machine_state_index,
        };
        use typed_trees::signature::StateSignature;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub fn closed_static_call_type_bindings(
            program: &dyn crate::CheckingInput,
            caller: &Machine,
            state: &State,
            signature: &StateSignature,
            selected: &[StaticMachineArgument],
            arguments: &[ExpressionHandle],
        ) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
            let parameters = program.state_signature_type_parameters(signature);
            if parameters.iter().any(|parameter| {
                !matches!(
                    parameter.kind,
                    TypeParameterKind::Type | TypeParameterKind::Machine { .. }
                )
            }) {
                return None;
            }
            let generic_types = parameters
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect::<Vec<_>>();
            let requirements = parameters
                .iter()
                .filter_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Machine { contract } => {
                        Some(machine_parameter_signature(program, contract))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let machine_arguments =
                super::machine_selection_arguments(program, selected).collect::<Vec<_>>();
            if machine_arguments.len() != requirements.len() {
                return None;
            }
            let index = std::cell::OnceCell::new();
            let selected_types =
                super::type_selections::selected_types(program, selected).collect::<Vec<_>>();
            if selected_types.len() > generic_types.len() {
                return None;
            }
            let mut bindings = generic_types
                .iter()
                .zip(selected_types)
                .map(|(parameter, actual)| TypeBinding {
                    symbol: parameter.symbol,
                    actual,
                })
                .collect::<Vec<_>>();
            for (requirement, selected) in requirements.into_iter().zip(machine_arguments) {
                if selected.application.is_some()
                    || selected.const_literal.is_some()
                    || selected.evidence_projection.is_some()
                    || !program
                        .state_signature_type_parameters(requirement)
                        .is_empty()
                {
                    return None;
                }
                let index = index.get_or_init(|| machine_state_index(program));
                let (machine, entry) = machine_and_state(program, index, selected.symbol)?;
                if !program.machine_type_parameters(machine).is_empty() {
                    return None;
                }
                let required = program.state_signature_parameters(requirement);
                let actual = program.state_parameters(entry);
                if required.len() != actual.len() {
                    return None;
                }
                for (required, actual) in required.iter().zip(actual) {
                    if required.is_mutable != actual.is_mutable
                        || required.is_const != actual.is_const
                        || !required_type_matches_exact(
                            program,
                            actual.type_reference,
                            required.type_reference,
                            &generic_types,
                            &mut bindings,
                        )
                    {
                        return None;
                    }
                }
                if !required_type_matches_exact(
                    program,
                    entry.return_type,
                    requirement.return_type,
                    &generic_types,
                    &mut bindings,
                ) {
                    return None;
                }
            }
            let required = program
                .state_signature_parameters(signature)
                .iter()
                .collect::<Vec<_>>();
            if required.len() != arguments.len() {
                return None;
            }
            for (required, argument) in required.iter().zip(arguments) {
                let actual = match program.expression_table.expression(*argument) {
                    ExpressionNode::Borrow(_) => return None,
                    ExpressionNode::Name(path) => {
                        crate::value_custody::expression_types::named_value_type_reference(
                            program, path,
                        )
                    }
                    _ => crate::value_custody::places::declared_place_type_raw(
                        program,
                        caller,
                        Some(state),
                        *argument,
                    ),
                };
                if let Some(actual) = actual {
                    if !required_type_matches_exact(
                        program,
                        actual,
                        required.type_reference,
                        &generic_types,
                        &mut bindings,
                    ) {
                        return None;
                    }
                } else {
                    let expected = match program
                        .type_reference_table
                        .type_reference(required.type_reference)
                    {
                        TypeReferenceNode::Named { symbol, .. } => bindings
                            .iter()
                            .find(|binding| binding.symbol == *symbol)
                            .map(|binding| binding.actual)
                            .unwrap_or(required.type_reference),
                        _ => required.type_reference,
                    };
                    match program.expression_table.expression(*argument) {
                        ExpressionNode::Boolean(_)
                        | ExpressionNode::Integer(_)
                        | ExpressionNode::Float(_) => {}
                        ExpressionNode::StructLiteral(literal) => {
                            let symbol = &program
                                .type_reference_table
                                .type_reference(expected)
                                .as_named_symbol()?;
                            if !symbol.is_valid()
                                || *symbol != literal.type_symbol
                                || !program.data_definitions().iter().any(|definition| {
                                    definition.symbol == *symbol
                                        && program.data_type_parameters(definition).is_empty()
                                        && definition.lifetime_parameters.is_empty()
                                })
                            {
                                return None;
                            }
                        }
                        _ => return None,
                    }
                    if !crate::value_custody::expression_types::argument_matches_type_reference_handle(
                program, *argument, expected,
            ) {
                return None;
            }
                }
            }
            if generic_types.iter().any(|parameter| {
                !bindings
                    .iter()
                    .any(|binding| binding.symbol == parameter.symbol && binding.actual.is_valid())
            }) {
                return None;
            }
            Some(
                bindings
                    .into_iter()
                    .map(|binding| (binding.symbol, binding.actual))
                    .collect(),
            )
        }
    }
    pub(crate) mod type_selections {
        use symbols::SymbolKind;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) fn selected_types<'arguments>(
            program: &'arguments dyn crate::CheckingInput,
            arguments: &'arguments [StaticMachineArgument],
        ) -> impl Iterator<Item = TypeReferenceHandle> + 'arguments {
            arguments.iter().filter_map(|argument| {
                if argument.type_reference.is_valid() {
                    return Some(argument.type_reference);
                }
                let is_type = argument.symbol.is_valid()
                    && match program.symbols.get(argument.symbol).kind {
                        SymbolKind::BuiltinType | SymbolKind::Data => true,
                        SymbolKind::TypeParameter => program
                            .data_type_parameter_by_symbol(argument.symbol)
                            .is_some_and(|parameter| {
                                matches!(parameter.kind, TypeParameterKind::Type)
                            }),
                        _ => false,
                    };
                if is_type {
                    return program
                        .type_reference_table
                        .find_named_type_reference(argument.symbol);
                }
                None
            })
        }
    }
    pub use closed_types::closed_static_call_type_bindings;
    use typed_trees::static_applications::StaticMachineBindingValue;
    pub(crate) mod contract_facts {
        use crate::frozen_program::expression_table;
        use diagnostics::Diagnostic;
        use symbols::JoinedPath;
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameter;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};

        #[derive(Clone, Copy, Default)]
        pub(crate) struct CallableProofScope<'a> {
            pub(crate) site: Option<super::ValidatedNominalMachineUseSite>,
            pub(crate) types: &'a [(
                symbols::SymbolHandle,
                typed_trees::types::TypeReferenceHandle,
            )],
        }
        impl CallableProofScope<'_> {
            fn proves_type_fact(
                self,
                program: &dyn crate::CheckingInput,
                expression: ExpressionHandle,
            ) -> bool {
                if program.applied_static_type_predicate_value(expression, self.types) == Some(true)
                {
                    return true;
                }
                match self.site {
                    Some(super::ValidatedNominalMachineUseSite::Expression(context)) => {
                        program.static_type_requirement_entailed(expression, context, self.types)
                    }
                    Some(super::ValidatedNominalMachineUseSite::Statement(context)) => program
                        .static_type_requirement_entailed_at_statement(
                            expression, context, self.types,
                        ),
                    None => false,
                }
            }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_contract_facts(
            program: &dyn crate::CheckingInput,
            label: &str,
            parameter: &TypeParameter,
            requirement: &typed_trees::signature::StateSignature,
            actual_contracts: &[SignatureContract],
            required_parameters: &[StateParameter],
            actual_parameters: &[StateParameter],
            actual_owner: symbols::SymbolHandle,
            actual_result: typed_trees::types::TypeReferenceHandle,
            binder_pairs: &[(SymbolHandle, SymbolHandle)],
            substitutions: &[(
                symbols::SymbolHandle,
                typed_trees::types::TypeReferenceHandle,
            )],
            proof_scope: CallableProofScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let required_contracts = program.state_signature_contracts(requirement);
            let prepare_implication = || {
                crate::proof_contracts::contract_entailment::CallableContractRefinement::prepare(
                    program,
                    crate::proof_contracts::contract_entailment::CallableContract {
                        owner: requirement.symbol,
                        contracts: required_contracts,
                        parameters: required_parameters,
                        result: requirement.return_type,
                    },
                    crate::proof_contracts::contract_entailment::CallableContract {
                        owner: actual_owner,
                        contracts: actual_contracts,
                        parameters: actual_parameters,
                        result: actual_result,
                    },
                    binder_pairs,
                )
            };
            validate_callable_contract_refinement(
                program,
                label,
                parameter.name.as_str(),
                required_contracts,
                actual_contracts,
                required_parameters,
                actual_parameters,
                requirement.symbol,
                actual_owner,
                substitutions,
                proof_scope,
                prepare_implication,
                diagnostics,
            );
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_callable_contract_refinement(
            program: &dyn crate::CheckingInput,
            label: &str,
            requirement_identity: &str,
            required_contracts: &[SignatureContract],
            actual_contracts: &[SignatureContract],
            required_parameters: &[StateParameter],
            actual_parameters: &[StateParameter],
            required_owner: symbols::SymbolHandle,
            actual_owner: symbols::SymbolHandle,
            substitutions: &[(
                symbols::SymbolHandle,
                typed_trees::types::TypeReferenceHandle,
            )],
            proof_scope: CallableProofScope<'_>,
            prepare_implication: impl FnOnce() -> Option<
                crate::proof_contracts::contract_entailment::CallableContractRefinement,
            >,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let mut prepare_implication = Some(prepare_implication);
            let mut implication = None;
            for kind in [
                SignatureContractKind::Requires,
                SignatureContractKind::Ensures,
            ] {
                if kind == SignatureContractKind::Requires
                    && super::reflection_coverage::is_open_field_query(program, proof_scope.site)
                {
                    continue;
                }
                let required = normalized_facts(
                    program,
                    required_contracts,
                    &kind,
                    required_parameters,
                    substitutions,
                );
                let actual = normalized_facts_in(
                    program,
                    actual_contracts,
                    &kind,
                    actual_parameters,
                    &[],
                    proof_scope,
                );
                let valid = match &kind {
                    SignatureContractKind::Requires => {
                        actual.iter().all(|fact| required.contains(fact))
                    }
                    SignatureContractKind::Ensures => required.iter().all(|fact| {
                        actual.contains(fact)
                            || single_guarantee_left_prefix(
                                program,
                                required_contracts,
                                actual_contracts,
                                required_parameters,
                                actual_parameters,
                                required_owner,
                                actual_owner,
                            )
                    }),
                    SignatureContractKind::EnsuresForResultCase { .. } => {
                        unreachable!(
                            "outcome-specific guarantees are not machine-parameter contracts"
                        )
                    }
                    SignatureContractKind::Crashes { .. } => {
                        unreachable!("crash routes compare separately")
                    }
                };
                if !valid {
                    if let Some(prepare) = prepare_implication.take() {
                        implication = prepare();
                    }
                    if implication
                        .as_ref()
                        .is_some_and(|judgment| judgment.proves(program, &kind))
                    {
                        continue;
                    }
                    diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: its {} facts are not a conservative refinement",
                requirement_identity,
                contract_kind_name(&kind)
            )));
                }
            }
            validate_crash_contract_refinement(
                program,
                label,
                requirement_identity,
                required_contracts,
                actual_contracts,
                required_parameters,
                actual_parameters,
                substitutions,
                diagnostics,
            );
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_crash_contract_refinement(
            program: &dyn crate::CheckingInput,
            label: &str,
            requirement_identity: &str,
            required_contracts: &[SignatureContract],
            actual_contracts: &[SignatureContract],
            required_parameters: &[StateParameter],
            actual_parameters: &[StateParameter],
            substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let mut checked = Vec::<SignatureContractKind>::new();
            for actual_contract in actual_contracts {
                let SignatureContractKind::Crashes { .. } = &actual_contract.kind else {
                    continue;
                };
                if checked.contains(&actual_contract.kind) {
                    continue;
                }
                checked.push(actual_contract.kind.clone());

                let actual_bucket = normalized_crash_bucket(
                    program,
                    actual_contracts,
                    &actual_contract.kind,
                    actual_parameters,
                    &[],
                )
                .expect("an actual crash contract contributes its own bucket");
                let required_bucket = normalized_crash_bucket(
                    program,
                    required_contracts,
                    &actual_contract.kind,
                    required_parameters,
                    substitutions,
                );
                let valid = required_bucket.is_some_and(|required| {
                    required.unconditional
                        || (!actual_bucket.unconditional
                            && actual_bucket
                                .routes
                                .iter()
                                .all(|route| required.routes.contains(route)))
                });
                if !valid {
                    let SignatureContractKind::Crashes { cause } = &actual_contract.kind else {
                        unreachable!("checked crash bucket kind")
                    };
                    diagnostics.push(Diagnostic::error(format!(
                "{label} does not refine `{}`: its `crashes {cause:?}` routes are not contained by the required crash ceiling",
                requirement_identity,
            )));
                }
            }
        }

        pub(crate) struct NormalizedCrashBucket {
            unconditional: bool,
            routes: Vec<String>,
        }

        fn normalized_crash_bucket(
            program: &dyn crate::CheckingInput,
            contracts: &[SignatureContract],
            kind: &SignatureContractKind,
            parameters: &[StateParameter],
            substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
        ) -> Option<NormalizedCrashBucket> {
            let matching = contracts
                .iter()
                .filter(|contract| &contract.kind == kind)
                .collect::<Vec<_>>();
            if matching.is_empty() {
                return None;
            }
            let unconditional = matching.iter().any(|contract| {
                normalized_facts(
                    program,
                    std::slice::from_ref(*contract),
                    kind,
                    parameters,
                    substitutions,
                )
                .is_empty()
            });
            let mut routes = normalized_facts(program, contracts, kind, parameters, substitutions);
            routes.sort();
            routes.dedup();
            Some(NormalizedCrashBucket {
                unconditional,
                routes,
            })
        }

        fn normalized_facts(
            program: &dyn crate::CheckingInput,
            contracts: &[SignatureContract],
            kind: &SignatureContractKind,
            parameters: &[StateParameter],
            substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
        ) -> Vec<String> {
            normalized_facts_in(
                program,
                contracts,
                kind,
                parameters,
                substitutions,
                CallableProofScope {
                    site: None,
                    types: substitutions,
                },
            )
        }

        fn normalized_facts_in(
            program: &dyn crate::CheckingInput,
            contracts: &[SignatureContract],
            kind: &SignatureContractKind,
            parameters: &[StateParameter],
            substitutions: &[(
                symbols::SymbolHandle,
                typed_trees::types::TypeReferenceHandle,
            )],
            proof_scope: CallableProofScope<'_>,
        ) -> Vec<String> {
            let mut facts = Vec::new();
            for contract in contracts.iter().filter(|contract| &contract.kind == kind) {
                for fact in program.tables.proof_facts.span_or_empty(contract.facts) {
                    if matches!(
                        fact,
                        ProofFact::Expression(expression)
                            if matches!(
                                expression_table(program).expression(*expression),
                                ExpressionNode::Boolean(true)
                            ) || matches!(fact, ProofFact::Expression(expression)
                                if proof_scope.proves_type_fact(program, *expression))
                    ) {
                        continue;
                    }
                    let raw = match fact {
                        ProofFact::Expression(expression) => {
                            expression_table(program).display_name(*expression)
                        }
                        ProofFact::Membership(membership) => format!(
                            "{} in {}{}",
                            expression_table(program).display_name(membership.value),
                            program.domain_path_members(membership.domain).joined_path(),
                            if membership.domain_arguments.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    "<{}>",
                                    program
                                        .type_reference_table
                                        .type_reference_handles(membership.domain_arguments)
                                        .iter()
                                        .map(|argument| program
                                            .normalized_type_identity_text(*argument))
                                        .collect::<Vec<_>>()
                                        .join(",")
                                )
                            }
                        ),
                        ProofFact::Proposition(application) => {
                            let binders = application
                                .binder_arguments
                                .iter()
                                .map(|argument| argument.path.joined_path())
                                .collect::<Vec<_>>();
                            let arguments = expression_table(program)
                                .expression_handles(application.arguments)
                                .iter()
                                .map(|argument| expression_table(program).display_name(*argument))
                                .collect::<Vec<_>>();
                            let binder_suffix = if binders.is_empty() {
                                String::new()
                            } else {
                                format!("[{}]", binders.join(", "))
                            };
                            format!(
                                "{}{binder_suffix}({})",
                                application.name.as_str(),
                                arguments.join(", ")
                            )
                        }
                    };
                    let substituted = substitute_type_actuals(program, &raw, substitutions);
                    facts.push(alpha_normalize(&substituted, parameters));
                }
            }
            facts.sort();
            facts.dedup();
            facts
        }

        fn single_guarantee_left_prefix(
            program: &dyn crate::CheckingInput,
            required_contracts: &[SignatureContract],
            actual_contracts: &[SignatureContract],
            required_parameters: &[StateParameter],
            actual_parameters: &[StateParameter],
            expected_required_owner: SymbolHandle,
            expected_actual_owner: SymbolHandle,
        ) -> bool {
            for parameters in [required_parameters, actual_parameters] {
                for (index, parameter) in parameters.iter().enumerate() {
                    if !parameter.symbol.is_valid()
                        || parameters[..index]
                            .iter()
                            .any(|earlier| earlier.symbol == parameter.symbol)
                    {
                        return false;
                    }
                }
            }
            let required = required_contracts
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Ensures)
                .flat_map(|contract| program.tables.proof_facts.span_or_empty(contract.facts))
                .collect::<Vec<_>>();
            let [ProofFact::Expression(required)] = required.as_slice() else {
                return false;
            };
            let Some((required_owner, required_type)) = expression_result_owner(program, *required)
            else {
                return false;
            };
            if required_owner != expected_required_owner || !expected_actual_owner.is_valid() {
                return false;
            }
            actual_contracts
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Ensures)
                .flat_map(|contract| program.tables.proof_facts.span_or_empty(contract.facts))
                .any(|fact| {
                    let ProofFact::Expression(expression) = fact else {
                        return false;
                    };
                    let mut prefix = *expression;
                    while let ExpressionNode::Binary(binary) =
                        expression_table(program).expression(prefix)
                    {
                        if binary.operator != BinaryOperator::And {
                            break;
                        }
                        prefix = binary.left;
                        let Some((actual_owner, actual_type)) =
                            expression_result_owner(program, prefix)
                        else {
                            continue;
                        };
                        if actual_owner != expected_actual_owner {
                            continue;
                        }
                        if !required_type.is_valid() || !actual_type.is_valid() {
                            continue;
                        }
                        let required_identity = program.normalized_type_identity(required_type);
                        let actual_identity = program.normalized_type_identity(actual_type);
                        if !required_identity.is_valid()
                            || !actual_identity.is_valid()
                            || required_identity != actual_identity
                        {
                            continue;
                        }
                        if crate::has_builtin_parameter_bound_expression_meaning(
                            program,
                            required_owner,
                            required_parameters,
                            *required,
                        ) && crate::has_builtin_parameter_bound_expression_meaning(
                            program,
                            actual_owner,
                            actual_parameters,
                            prefix,
                        ) && same_guarantee_expression(
                            program,
                            *required,
                            prefix,
                            required_owner,
                            actual_owner,
                            required_parameters,
                            actual_parameters,
                        ) {
                            return true;
                        }
                    }
                    false
                })
        }

        fn expression_result_owner(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<(SymbolHandle, typed_trees::types::TypeReferenceHandle)> {
            let mut nodes = Vec::new();
            crate::queries::expression_traversal::collect_expression_tree(
                program, expression, &mut nodes,
            );
            let mut owner = None;
            for node in nodes {
                if let Some(candidate) = crate::reserved_result_owner(program, node) {
                    if owner.is_some_and(|existing| existing != candidate) {
                        return None;
                    }
                    owner = Some(candidate);
                }
            }
            owner
        }

        fn same_guarantee_expression(
            program: &dyn crate::CheckingInput,
            required: ExpressionHandle,
            actual: ExpressionHandle,
            required_owner: SymbolHandle,
            actual_owner: SymbolHandle,
            required_parameters: &[StateParameter],
            actual_parameters: &[StateParameter],
        ) -> bool {
            match (
                expression_table(program).expression(required),
                expression_table(program).expression(actual),
            ) {
                (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
                (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
                (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
                    if crate::reserved_result_owner(program, required)
                        .is_some_and(|(owner, _)| owner == required_owner)
                        && crate::reserved_result_owner(program, actual)
                            .is_some_and(|(owner, _)| owner == actual_owner)
                    {
                        return true;
                    }
                    let table = expression_table(program);
                    let left_members = table.name_path_members(left.members);
                    let right_members = table.name_path_members(right.members);
                    let left_symbols = table.name_path_member_symbols(left.member_symbols);
                    let right_symbols = table.name_path_member_symbols(right.member_symbols);
                    if left_members.len() != 1
                        || right_members.len() != 1
                        || left_symbols.len() != 1
                        || right_symbols.len() != 1
                        || !left.symbol.is_valid()
                        || !right.symbol.is_valid()
                        || left.head_symbol != left.symbol
                        || right.head_symbol != right.symbol
                        || left_symbols[0] != left.symbol
                        || right_symbols[0] != right.symbol
                    {
                        return false;
                    }
                    let left_index = required_parameters
                        .iter()
                        .position(|p| p.symbol == left.symbol);
                    let right_index = actual_parameters
                        .iter()
                        .position(|p| p.symbol == right.symbol);
                    match (left_index, right_index) {
                        (Some(left), Some(right)) => left == right,
                        (None, None) => left.symbol == right.symbol,
                        _ => false,
                    }
                }
                (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
                    left.operator == right.operator
                        && same_guarantee_expression(
                            program,
                            left.left,
                            right.left,
                            required_owner,
                            actual_owner,
                            required_parameters,
                            actual_parameters,
                        )
                        && same_guarantee_expression(
                            program,
                            left.right,
                            right.right,
                            required_owner,
                            actual_owner,
                            required_parameters,
                            actual_parameters,
                        )
                }
                _ => false,
            }
        }

        fn substitute_type_actuals(
            program: &dyn crate::CheckingInput,
            text: &str,
            substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
        ) -> String {
            if substitutions.is_empty() {
                return text.to_owned();
            }
            let mut names = Vec::new();
            for (symbol, actual) in substitutions {
                let Some(parameter) = program.data_type_parameter_by_symbol(*symbol) else {
                    continue;
                };
                if !matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type) {
                    continue;
                }
                let rendered = program.display_type_reference(*actual);
                if rendered.is_empty()
                    || names
                        .iter()
                        .any(|(name, _)| name == parameter.name.as_str())
                {
                    continue;
                }
                names.push((parameter.name.as_str().to_owned(), rendered));
            }
            if names.is_empty() {
                return text.to_owned();
            }
            let characters: Vec<char> = text.chars().collect();
            let mut output = String::with_capacity(text.len());
            let mut cursor = 0;
            while cursor < characters.len() {
                if !characters[cursor].is_ascii_alphanumeric() && characters[cursor] != '_' {
                    output.push(characters[cursor]);
                    cursor += 1;
                    continue;
                }
                let start = cursor;
                while cursor < characters.len()
                    && (characters[cursor].is_ascii_alphanumeric() || characters[cursor] == '_')
                {
                    cursor += 1;
                }
                let word: String = characters[start..cursor].iter().collect();
                let previous = characters[..start]
                    .iter()
                    .rev()
                    .copied()
                    .find(|character| !character.is_whitespace());
                if matches!(previous, Some('<' | ','))
                    && let Some((_, rendered)) = names.iter().find(|(name, _)| name == &word)
                {
                    output.push_str(rendered);
                } else {
                    output.push_str(&word);
                }
            }
            output
        }

        fn alpha_normalize(text: &str, parameters: &[StateParameter]) -> String {
            let characters: Vec<char> = text.chars().collect();
            let mut output = String::with_capacity(text.len());
            let mut cursor = 0;
            while cursor < characters.len() {
                if !characters[cursor].is_ascii_alphanumeric() && characters[cursor] != '_' {
                    output.push(characters[cursor]);
                    cursor += 1;
                    continue;
                }

                let start = cursor;
                while cursor < characters.len()
                    && (characters[cursor].is_ascii_alphanumeric() || characters[cursor] == '_')
                {
                    cursor += 1;
                }
                let word: String = characters[start..cursor].iter().collect();
                let previous = characters[..start]
                    .iter()
                    .rev()
                    .copied()
                    .find(|character| !character.is_whitespace());
                let next = characters[cursor..]
                    .iter()
                    .copied()
                    .find(|character| !character.is_whitespace());
                let is_declaration_name =
                    matches!(previous, Some('.' | ':')) || matches!(next, Some(':' | '(' | '{'));
                if !is_declaration_name
                    && let Some(index) = parameters
                        .iter()
                        .position(|parameter| parameter.name.as_str() == word)
                {
                    output.push('$');
                    output.push_str(&index.to_string());
                } else {
                    output.push_str(&word);
                }
            }
            output
        }

        pub(crate) fn contract_kind_name(kind: &SignatureContractKind) -> &'static str {
            match kind {
                SignatureContractKind::Requires => "requires",
                SignatureContractKind::Ensures => "ensures",
                SignatureContractKind::EnsuresForResultCase { .. } => "outcome-specific ensures",
                SignatureContractKind::Crashes { .. } => "crashes",
            }
        }
    }
    pub(crate) mod nominal_admission {
        use super::type_refinement::{TypeBinding, required_type_matches_exact};
        use super::{MachineStateIndex, machine_and_state, machine_parameter_contract_definition};
        use crate::frozen_program::expression_table;
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::data::{MachineParameterContract, TypeParameter};
        use typed_trees::trait_definition::AppliedTraitScope;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct AdmittedNominalSelection {
            pub(crate) selected_machine: SymbolHandle,
            pub(crate) selected_entry: SymbolHandle,
            pub(crate) receiving_trait: SymbolHandle,
            pub(crate) receiving_application: Result<String, String>,
            pub(crate) satisfaction_trait: SymbolHandle,
            pub(crate) satisfaction_requirement: SymbolHandle,
            pub(crate) canonical_requirement_overload: String,
        }

        fn retain_selected_operands<'a>(
            program: &'a dyn crate::CheckingInput,
            application: typed_trees::static_applications::StaticMachineApplicationHandle,
            operands: &mut Vec<(
                SymbolHandle,
                typed_trees::type_identity::IndexIdentityArgument<'a>,
            )>,
        ) {
            if !application.is_valid() {
                return;
            }
            for binding in
                expression_table(program).static_machine_application_bindings(application)
            {
                if let typed_trees::static_applications::StaticMachineBindingValue::Machine(
                    argument,
                )
                | typed_trees::static_applications::StaticMachineBindingValue::Conformance(
                    argument,
                ) = &binding.value
                    && !operands
                        .iter()
                        .any(|(parameter, _)| *parameter == binding.parameter)
                {
                    operands.push((
                        binding.parameter,
                        typed_trees::type_identity::IndexIdentityArgument::StaticArgument(argument),
                    ));
                }
            }
        }

        pub(crate) fn validate_nominal_machine_selection(
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
                receiving_trait,
                receiving_arguments,
                trait_definition: required_trait,
                requirement: required_requirement,
                trait_arguments: required_arguments,
                ..
            } = required_contract
            else {
                return Ok(None);
            };
            let applied = program.applied_machine_callable(
                application,
                ordinal,
                selected_symbol,
                parameter.symbol,
                super::machine_parameter_signature(program, required_contract),
            );
            let provider_application = machine_and_state(program, index, selected_symbol)
                .and_then(|(machine, _)| program.machine_specialization(machine.symbol))
                .map(|instance| instance.binding_application)
                .filter(|handle| handle.is_valid())
                .or_else(|| applied.as_ref().map(|view| view.provider));
            let provider_operands = provider_application.map(|provider| {
                let mut operands = expression_table(program)
                    .static_machine_application_bindings(provider)
                    .iter()
                    .filter_map(|binding| match binding.value {
                        typed_trees::static_applications::StaticMachineBindingValue::Type(
                            actual,
                        ) => Some((
                            binding.parameter,
                            typed_trees::type_identity::IndexIdentityArgument::TypeArgument(actual),
                        )),
                        typed_trees::static_applications::StaticMachineBindingValue::Const(
                            actual,
                        ) => Some((
                            binding.parameter,
                            typed_trees::type_identity::IndexIdentityArgument::TypeReference(
                                actual,
                            ),
                        )),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                retain_selected_operands(program, provider, &mut operands);
                operands
            });
            let mut operand_mismatch = None::<String>;
            let mut helper_mismatch = None::<String>;
            let mut arguments_match =
                |actual: &[typed_trees::types::TypeReferenceHandle],
                 actual_owner: SymbolHandle,
                 actual_scope: Option<&AppliedTraitScope<'_>>,
                 bindings: &mut Vec<TypeBinding>| {
                    if actual.len() != required_arguments.len() {
                        return false;
                    }
                    let scoped_actuals = if let Some(scope) = actual_scope {
                        let identities = program
                            .trait_type_parameters(scope.declaration)
                            .iter()
                            .map(|parameter| scope.argument_identity(program, parameter.symbol))
                            .collect::<Option<Vec<_>>>();
                        let Some(identities) = identities else {
                            operand_mismatch = Some(
                                "inline requirement application lost an argument identity"
                                    .to_owned(),
                            );
                            return false;
                        };
                        Some(identities)
                    } else {
                        None
                    };
                    if applied.is_none() {
                        let Some(definition) = program.trait_definition_by_symbol(*required_trait)
                        else {
                            return false;
                        };
                        let trait_parameters = program.trait_type_parameters(definition);
                        if trait_parameters.len() != actual.len() {
                            return false;
                        }
                        if provider_application.is_none()
                            && actual_scope.is_none()
                            && !actual
                                .iter()
                                .zip(required_arguments)
                                .zip(trait_parameters)
                                .all(|((actual, required), slot)| {
                                    if !matches!(
                                        slot.kind,
                                        typed_trees::data::TypeParameterKind::Type
                                    ) && !slot.kind.is_const_or_value()
                                    {
                                        return true;
                                    }
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
                            AppliedTypeIdentityScope, IndexIdentityArgument,
                            applied_index_identity, applied_type_identity,
                        };
                        let mut operands = bindings
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
                        retain_selected_operands(program, application, &mut operands);
                        let receiving = program.symbols.get(parameter.symbol).parent;
                        let lifetimes = match program
                            .static_application_lifetimes_for_declaration(application, receiving)
                        {
                            Ok(lifetimes) => lifetimes,
                            Err(reason) => {
                                operand_mismatch = Some(reason.to_owned());
                                return false;
                            }
                        };
                        let lifetimes =
                            crate::machine_calls::static_bindings::lifetime_identity_rows(
                                &lifetimes,
                            );
                        let provider_lifetimes = provider_application
                            .map(|handle| {
                                let provider =
                                    expression_table(program).static_machine_application(handle);
                                program
                        .static_application_lifetimes_for_declaration(
                            handle,
                            provider.binding_declaration,
                        )
                        .map(|bindings| {
                            crate::machine_calls::static_bindings::lifetime_identity_rows(&bindings)
                        })
                            })
                            .transpose();
                        let provider_lifetimes = match provider_lifetimes {
                            Ok(lifetimes) => lifetimes.unwrap_or_default(),
                            Err(reason) => {
                                operand_mismatch = Some(reason.to_owned());
                                return false;
                            }
                        };
                        let provider_scopes = provider_application
                            .map(|handle| {
                                let provider =
                                    expression_table(program).static_machine_application(handle);
                                AppliedTypeIdentityScope {
                                    declaration: provider.binding_declaration,
                                    arguments: provider_operands.as_deref().unwrap_or(&[]),
                                    lifetimes: &provider_lifetimes,
                                }
                            })
                            .into_iter()
                            .collect::<Vec<_>>();
                        let actual_owner = provider_application.map_or(actual_owner, |handle| {
                            expression_table(program)
                                .static_machine_application(handle)
                                .lexical_owner
                        });
                        let caller_owner = if application.is_valid() {
                            expression_table(program)
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
                        let Some(definition) = program.trait_definition_by_symbol(*required_trait)
                        else {
                            return false;
                        };
                        if program.trait_type_parameters(definition).len() != actual.len() {
                            return false;
                        }
                        return actual
                .iter()
                .zip(required_arguments)
                .zip(program.trait_type_parameters(definition))
                .enumerate()
                .all(|(ordinal, ((actual, required), parameter))| {
                    let actual_reference = *actual;
                    let required_reference = *required;
                    let (actual, required) = if parameter.kind.is_const_or_value()
                        || matches!(
                            parameter.kind,
                            typed_trees::data::TypeParameterKind::Machine { .. }
                        ) {
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
                    let actual = scoped_actuals
                        .as_ref()
                        .map_or(actual, |identities| identities.get(ordinal).copied());
                    let matches = actual
                        .zip(required)
                        .is_some_and(|(actual, required)| actual == required);
                    if !matches && operand_mismatch.is_none() {
                        operand_mismatch = Some(if matches!(parameter.kind, typed_trees::data::TypeParameterKind::Machine { .. }) {
                            let message = format!("helper `{}` requires `{}`, but the implementation promises `{}`",
                                parameter.name,
                                selected_operand_description(program, required_reference, &operands),
                                selected_operand_description(program, actual_reference, provider_operands.as_deref().unwrap_or(&[])));
                            helper_mismatch = Some(message.clone());
                            message
                        } else {
                            format!(
                                "actual index/type identity {:?}, required {:?}",
                                actual.and_then(|identity| program.type_identity_text(identity)),
                                required.and_then(|identity| program.type_identity_text(identity))
                            )
                        });
                    }
                    matches
                });
                    }
                    let applied = applied.as_ref().expect("retained applied selection");
                    use typed_trees::type_identity::{
                        AppliedTypeIdentityScope, IndexIdentityArgument, applied_index_identity,
                        applied_type_identity,
                    };
                    let Some(provider_application) = provider_application else {
                        operand_mismatch = Some("applied provider tuple is absent".to_owned());
                        return false;
                    };
                    let Some(provider_operands) = provider_operands.as_ref() else {
                        operand_mismatch = Some("applied provider operands are absent".to_owned());
                        return false;
                    };
                    let provider =
                        expression_table(program).static_machine_application(provider_application);
                    let caller_owner = expression_table(program)
                        .static_machine_application(application)
                        .lexical_owner;
                    let provider_lifetime_rows = match program
                        .static_application_lifetimes_for_declaration(
                            provider_application,
                            provider.binding_declaration,
                        ) {
                        Ok(lifetimes) => lifetimes,
                        Err(reason) => {
                            operand_mismatch = Some(reason.to_owned());
                            return false;
                        }
                    };
                    let provider_lifetimes =
                        crate::machine_calls::static_bindings::lifetime_identity_rows(
                            &provider_lifetime_rows,
                        );
                    let caller_lifetimes = match program
                        .static_application_lifetimes_for_declaration(
                            application,
                            program.symbols.get(parameter.symbol).parent,
                        ) {
                        Ok(lifetimes) => lifetimes,
                        Err(reason) => {
                            operand_mismatch = Some(reason.to_owned());
                            return false;
                        }
                    };
                    let caller_lifetimes =
                        crate::machine_calls::static_bindings::lifetime_identity_rows(
                            &caller_lifetimes,
                        );
                    let provider_scope = AppliedTypeIdentityScope {
                        declaration: provider.binding_declaration,
                        arguments: provider_operands,
                        lifetimes: &provider_lifetimes,
                    };
                    let mut caller_operands = applied
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
                    retain_selected_operands(program, application, &mut caller_operands);
                    let caller_scope = AppliedTypeIdentityScope {
                        declaration: program.symbols.get(parameter.symbol).parent,
                        arguments: &caller_operands,
                        lifetimes: &caller_lifetimes,
                    };
                    let Some(definition) = program.trait_definition_by_symbol(*required_trait)
                    else {
                        return false;
                    };
                    let parameters = program.trait_type_parameters(definition);
                    parameters.len() == actual.len()
            && actual
                .iter()
                .zip(required_arguments)
                .zip(parameters)
                .enumerate()
                .all(|(ordinal, ((actual, required), parameter))| {
                    let actual_reference = *actual;
                    let required_reference = *required;
                    let (actual, required) = if parameter.kind.is_const_or_value()
                        || matches!(
                            parameter.kind,
                            typed_trees::data::TypeParameterKind::Machine { .. }
                        ) {
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
                    let actual = scoped_actuals
                        .as_ref()
                        .map_or(actual, |identities| identities.get(ordinal).copied());
                    let matches = actual
                        .zip(required)
                        .is_some_and(|(actual, required)| actual == required);
                    if !matches && operand_mismatch.is_none() {
                        operand_mismatch = Some(if matches!(parameter.kind, typed_trees::data::TypeParameterKind::Machine { .. }) {
                            let message = format!("helper `{}` requires `{}`, but the implementation promises `{}`",
                                parameter.name,
                                selected_operand_description(program, required_reference, &caller_operands),
                                selected_operand_description(program, actual_reference, provider_operands));
                            helper_mismatch = Some(message.clone());
                            message
                        } else {
                            format!(
                                "actual index/type identity {:?}, required {:?}",
                                actual.and_then(|identity| program.type_identity_text(identity)),
                                required.and_then(|identity| program.type_identity_text(identity))
                            )
                        });
                    }
                    matches
                })
                };

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
                        None,
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
                    if expression_table(program)
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
                    None,
                    &mut scratch,
                ) {
                    matching_rows += 1;
                    admitted_bindings = Some(scratch);
                }
            }
            let map_scope = match program.machine_conformance_requirement_scope(
                selected_machine.symbol,
                *required_trait,
                *required_requirement,
            ) {
                Ok(scope) => scope,
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(reason));
                    return Err(());
                }
            };
            if let Some(scope) = map_scope {
                let actuals = program
                    .trait_type_parameters(scope.declaration)
                    .iter()
                    .map(|parameter| {
                        scope
                            .parameter_application(parameter.symbol)
                            .map(|(reference, _)| reference)
                    })
                    .collect::<Option<Vec<_>>>();
                let Some(actuals) = actuals else {
                    diagnostics.push(Diagnostic::error(
                        "inline requirement application lost its declared argument roster",
                    ));
                    return Err(());
                };
                let mut scratch = bindings.clone();
                if arguments_match(&actuals, scope.argument_owner, Some(&scope), &mut scratch) {
                    if matching_rows == 0 {
                        matching_rows = 1;
                    }
                    admitted_bindings = Some(scratch);
                }
            }
            if matching_rows == 1
                && let Some(admitted) = admitted_bindings
            {
                *bindings = admitted;
            }
            if matching_rows == 0
                && let Some(reason) = helper_mismatch
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{selected_name}` does not satisfy `{}::{}`: {reason}",
                    trait_definition.name, requirement.name
                )));
                return Err(());
            }
            if matching_rows != 1 {
                let provider_tuple = provider_application
                    .map(|handle| {
                        expression_table(program)
                            .static_machine_application_bindings(handle)
                            .iter()
                            .filter_map(|binding| {
                                match binding.value {
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
                    }
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
                receiving_trait: *receiving_trait,
                receiving_application: receiving_application_identity(
                    program,
                    *receiving_trait,
                    receiving_arguments,
                    parameter.symbol,
                    application,
                    bindings,
                ),
                satisfaction_trait: *required_trait,
                satisfaction_requirement: *required_requirement,
                canonical_requirement_overload: program
                    .trait_requirement_callable_identity_text(trait_definition, requirement),
            }))
        }

        fn receiving_application_identity(
            program: &dyn crate::CheckingInput,
            receiving_trait: SymbolHandle,
            receiving_arguments: &[typed_trees::types::TypeReferenceHandle],
            parameter: SymbolHandle,
            application: typed_trees::static_applications::StaticMachineApplicationHandle,
            bindings: &[TypeBinding],
        ) -> Result<String, String> {
            let declaration = program.declaration_scope_parent(parameter);
            let caller = if application.is_valid() {
                expression_table(program)
                    .static_machine_application(application)
                    .lexical_owner
            } else {
                declaration
            };
            let lifetimes =
                program.static_application_lifetimes_for_declaration(application, declaration)?;
            let mut arguments = bindings
                .iter()
                .map(|binding| {
                    let argument = if program
                        .data_type_parameter_by_symbol(binding.symbol)
                        .is_some_and(|parameter| parameter.kind.is_const_or_value())
                    {
                        typed_trees::type_identity::IndexIdentityArgument::TypeReference(
                            binding.actual,
                        )
                    } else {
                        typed_trees::type_identity::IndexIdentityArgument::TypeArgument(
                            binding.actual,
                        )
                    };
                    (binding.symbol, argument)
                })
                .collect::<Vec<_>>();
            retain_selected_operands(program, application, &mut arguments);
            program
                .trait_definition_by_symbol(receiving_trait)
                .and_then(|receiving| {
                    AppliedTraitScope::new(
                        program,
                        receiving,
                        typed_trees::trait_definition::TraitApplicationArguments {
                            lexical_owner: declaration,
                            enclosing_data_applications: &[],
                            arguments: receiving_arguments,
                            lifetimes: &[],
                        },
                    )
                })
                .and_then(|scope| {
                    scope.portable_application_identity_under_invocation(
                        program,
                        declaration,
                        caller,
                        &arguments,
                        &lifetimes,
                    )
                })
                .and_then(|identity| program.type_identity_text(identity))
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    "callback binder lost its complete receiving trait application".to_owned()
                })
        }

        fn selected_operand_description(
            program: &dyn crate::CheckingInput,
            reference: typed_trees::types::TypeReferenceHandle,
            bindings: &[(
                SymbolHandle,
                typed_trees::type_identity::IndexIdentityArgument<'_>,
            )],
        ) -> String {
            if let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
                && let Some((
                    _,
                    typed_trees::type_identity::IndexIdentityArgument::StaticArgument(argument),
                )) = bindings.iter().find(|(parameter, _)| parameter == symbol)
            {
                return argument.display_name();
            }
            program.display_type_reference(reference)
        }
    }
    pub(crate) mod reflection_coverage {
        use super::ValidatedNominalMachineUseSite;
        use typed_trees::expression::{ExpressionNode, StaticMachineArgument};
        use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

        pub(crate) fn is_open_field_query(
            program: &dyn crate::CheckingInput,
            site: Option<ValidatedNominalMachineUseSite>,
        ) -> bool {
            let (target, arguments) = match site {
                Some(ValidatedNominalMachineUseSite::Statement(statement)) => {
                    let Some(call) = program.statement_table.statement(statement).as_call() else {
                        return false;
                    };
                    (call.target_symbol, call.machine_arguments.as_ref())
                }
                Some(ValidatedNominalMachineUseSite::Expression(expression)) => {
                    let Some(call) = program.expression_table.call_node(expression) else {
                        return false;
                    };
                    (call.target_symbol, call.machine_arguments.as_ref())
                }
                None => return false,
            };
            if !target.is_valid() {
                return false;
            }
            let symbol = program.symbols.get(target);
            let declaration = if symbol.kind == symbols::SymbolKind::State {
                symbol.parent
            } else {
                target
            };
            let Some(source) = program
                .symbols
                .symbol_source_span(declaration)
                .and_then(|span| program.symbols.source_file(span))
            else {
                return false;
            };
            if source.origin != source::SourceOrigin::Toolchain
                || source.path.strip_prefix(&source.package_root).ok()
                    != Some(std::path::Path::new("reflect.omg"))
                || !matches!(
                    program.symbols.name(declaration),
                    "visit_runtime_fields"
                        | "visit_runtime_fields_mut"
                        | "inspect_record_fields"
                        | "visit_active_fields"
                        | "visit_active_fields_mut"
                )
            {
                return false;
            }
            let Some(subject) = arguments.first() else {
                return false;
            };
            has_open_operand(program, subject)
        }
        fn has_open_operand(
            program: &dyn crate::CheckingInput,
            subject: &StaticMachineArgument,
        ) -> bool {
            let is_parameter = |symbol: symbols::SymbolHandle| {
                symbol.is_valid()
                    && program.symbols.get(symbol).kind == symbols::SymbolKind::TypeParameter
            };
            let mut operands = vec![subject];
            let mut types = Vec::new();
            let mut expressions = Vec::new();
            while let Some(argument) = operands.pop() {
                if is_parameter(argument.symbol) {
                    return true;
                }
                if argument.type_reference.is_valid() {
                    types.push(argument.type_reference);
                }
                if let Some(application) = &argument.application {
                    operands.extend(application.arguments.iter());
                }
            }
            let mut visited = Vec::new();
            while let Some(reference) = types.pop() {
                if visited.contains(&reference) {
                    continue;
                }
                visited.push(reference);
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Named { symbol, .. } if is_parameter(*symbol) => {
                        return true;
                    }
                    TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    } => {
                        if is_parameter(*base_symbol) {
                            return true;
                        }
                        types.extend_from_slice(
                            program
                                .type_reference_table
                                .type_reference_handles(*arguments),
                        );
                    }
                    TypeReferenceNode::Reference { referee, .. } => types.push(*referee),
                    TypeReferenceNode::Constrained { base_type, .. } => types.push(*base_type),
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } => {
                        types.push(*element_type);
                        match length {
                            FixedArrayLength::ConstParameter { symbol, .. }
                                if is_parameter(*symbol) =>
                            {
                                return true;
                            }
                            FixedArrayLength::Expression(reference) => types.push(*reference),
                            _ => {}
                        }
                    }
                    TypeReferenceNode::Slice { element_type } => types.push(*element_type),
                    TypeReferenceNode::ConstExpression(expression) => expressions.push(*expression),
                    _ => {}
                }
            }
            let mut visited = Vec::new();
            while let Some(expression) = expressions.pop() {
                if visited.contains(&expression) {
                    continue;
                }
                visited.push(expression);
                let node = program.expression_table.expression(expression);
                if matches!(node, ExpressionNode::Name(name) if is_parameter(name.symbol)) {
                    return true;
                }
                crate::queries::expression_traversal::executable_children(program, node, |child| {
                    expressions.push(child)
                });
            }
            false
        }
    }
    pub(crate) mod type_refinement {
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameter;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

        #[derive(Clone, Copy)]
        pub(crate) struct TypeBinding {
            pub(crate) symbol: SymbolHandle,
            pub(crate) actual: TypeReferenceHandle,
        }

        #[derive(Clone, Copy)]
        pub(crate) struct BinderBinding {
            pub(crate) required: SymbolHandle,
            pub(crate) actual: SymbolHandle,
        }

        pub(crate) fn required_type_matches(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
            binder_bindings: &[BinderBinding],
        ) -> bool {
            required_type_matches_inner::<false>(
                program,
                actual,
                required,
                generic_types,
                bindings,
                binder_bindings,
            )
        }

        pub(crate) fn required_type_matches_exact(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
        ) -> bool {
            required_type_matches_inner::<true>(
                program,
                actual,
                required,
                generic_types,
                bindings,
                &[],
            )
        }

        fn required_type_matches_inner<const EXACT: bool>(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            generic_types: &[&TypeParameter],
            bindings: &mut Vec<TypeBinding>,
            binder_bindings: &[BinderBinding],
        ) -> bool {
            if !actual.is_valid() || !required.is_valid() {
                return actual.is_valid() == required.is_valid();
            }
            if let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(required)
            {
                if let Some(binding) = binder_bindings
                    .iter()
                    .find(|binding| binding.required == *symbol)
                {
                    return matches!(
                        program.type_reference_table.type_reference(actual),
                        TypeReferenceNode::Named { symbol, .. } if *symbol == binding.actual
                    );
                }
                if let Some(binding) = bindings.iter().find(|binding| binding.symbol == *symbol) {
                    if program
                        .data_type_parameter_by_symbol(*symbol)
                        .is_some_and(|parameter| parameter.kind.is_const_or_value())
                    {
                        return program
                            .static_const_type_reference_identity(actual)
                            .zip(program.static_const_type_reference_identity(binding.actual))
                            .map_or_else(
                                || match (
                                    program.type_reference_table.type_reference(actual),
                                    program.type_reference_table.type_reference(binding.actual),
                                ) {
                                    (
                                        TypeReferenceNode::Named { symbol: actual, .. },
                                        TypeReferenceNode::Named {
                                            symbol: required, ..
                                        },
                                    ) if actual.is_valid() && required.is_valid() => {
                                        actual == required
                                    }
                                    _ => {
                                        program.normalized_type_identity(actual)
                                            == program.normalized_type_identity(binding.actual)
                                    }
                                },
                                |(actual, required)| actual == required,
                            );
                    }
                    return required_type_matches_inner::<EXACT>(
                        program,
                        actual,
                        binding.actual,
                        &[],
                        &mut Vec::new(),
                        binder_bindings,
                    );
                }
                if let Some(parameter) = generic_types
                    .iter()
                    .find(|parameter| parameter.symbol.is_valid() && parameter.symbol == *symbol)
                {
                    bindings.push(TypeBinding {
                        symbol: parameter.symbol,
                        actual,
                    });
                    return true;
                }
            }

            match (
                program.type_reference_table.type_reference(actual),
                program.type_reference_table.type_reference(required),
            ) {
                (
                    TypeReferenceNode::Reference {
                        referee: actual_inner,
                        access: actual_access,
                        ..
                    },
                    TypeReferenceNode::Reference {
                        referee: required_inner,
                        access: required_access,
                        ..
                    },
                ) => {
                    actual_access == required_access
                        && required_type_matches_inner::<EXACT>(
                            program,
                            *actual_inner,
                            *required_inner,
                            generic_types,
                            bindings,
                            binder_bindings,
                        )
                }
                typed_trees::constrained_type_reference_pair!(actual_base, required_base) => {
                    required_type_matches_inner::<EXACT>(
                        program,
                        *actual_base,
                        *required_base,
                        generic_types,
                        bindings,
                        binder_bindings,
                    )
                }
                (
                    _,
                    TypeReferenceNode::Constrained {
                        base_type: required_base,
                        ..
                    },
                ) if EXACT => required_type_matches_inner::<EXACT>(
                    program,
                    actual,
                    *required_base,
                    generic_types,
                    bindings,
                    binder_bindings,
                ),
                (
                    TypeReferenceNode::Constrained {
                        base_type: actual_base,
                        ..
                    },
                    _,
                ) if EXACT => required_type_matches_inner::<EXACT>(
                    program,
                    *actual_base,
                    required,
                    generic_types,
                    bindings,
                    binder_bindings,
                ),
                (
                    TypeReferenceNode::FixedArray {
                        element_type: actual_element,
                        length: actual_length,
                    },
                    TypeReferenceNode::FixedArray {
                        element_type: required_element,
                        length: required_length,
                    },
                ) => {
                    fixed_array_lengths_match(
                        program,
                        actual_length,
                        required_length,
                        bindings,
                        binder_bindings,
                    ) && required_type_matches_inner::<EXACT>(
                        program,
                        *actual_element,
                        *required_element,
                        generic_types,
                        bindings,
                        binder_bindings,
                    )
                }
                (
                    TypeReferenceNode::Slice {
                        element_type: actual_element,
                    },
                    TypeReferenceNode::Slice {
                        element_type: required_element,
                    },
                ) => required_type_matches_inner::<EXACT>(
                    program,
                    *actual_element,
                    *required_element,
                    generic_types,
                    bindings,
                    binder_bindings,
                ),
                (
                    TypeReferenceNode::Generic {
                        base_symbol: actual_base,
                        base_name: actual_name,
                        arguments: actual_arguments,
                        ..
                    },
                    TypeReferenceNode::Generic {
                        base_symbol: required_base,
                        base_name: required_name,
                        arguments: required_arguments,
                        ..
                    },
                ) => {
                    let same_base = if actual_base.is_valid() && required_base.is_valid() {
                        actual_base == required_base
                    } else {
                        !EXACT && actual_name == required_name
                    };
                    let actual_arguments = program
                        .type_reference_table
                        .type_reference_handles(*actual_arguments);
                    let required_arguments = program
                        .type_reference_table
                        .type_reference_handles(*required_arguments);
                    same_base
                        && actual_arguments.len() == required_arguments.len()
                        && actual_arguments.iter().zip(required_arguments).all(
                            |(actual, required)| {
                                required_type_matches_inner::<EXACT>(
                                    program,
                                    *actual,
                                    *required,
                                    generic_types,
                                    bindings,
                                    binder_bindings,
                                )
                            },
                        )
                }
                (
                    TypeReferenceNode::Named { symbol: actual, .. },
                    TypeReferenceNode::Named {
                        symbol: required, ..
                    },
                ) if EXACT => actual.is_valid() && actual == required,
                (TypeReferenceNode::Unit, TypeReferenceNode::Unit) if EXACT => true,
                _ if EXACT => actual == required,
                _ => {
                    corresponding_types_match(program, actual, required, bindings, binder_bindings)
                }
            }
        }

        pub(crate) fn corresponding_types_match(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            bindings: &[TypeBinding],
            binder_bindings: &[BinderBinding],
        ) -> bool {
            let mut positional = Vec::with_capacity(binder_bindings.len() * 2);
            for (position, binding) in binder_bindings.iter().enumerate() {
                let label = format!("$formation{position}");
                positional.push((binding.required, label.clone()));
                positional.push((binding.actual, label));
            }
            let substitutions = bindings
                .iter()
                .map(|binding| (binding.symbol, binding.actual))
                .collect::<Vec<_>>();
            let identity = |reference| {
                program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
                    binders: &positional,
                    substitutions: &substitutions,
                    ..typed_trees::type_identity::TypeIdentityRequest::ordinary(reference)
                })
            };
            identity(required) == identity(actual)
        }

        fn fixed_array_lengths_match(
            program: &dyn crate::CheckingInput,
            actual: &FixedArrayLength,
            required: &FixedArrayLength,
            bindings: &[TypeBinding],
            binder_bindings: &[BinderBinding],
        ) -> bool {
            let mut binders = Vec::with_capacity(binder_bindings.len() * 2);
            for (ordinal, binding) in binder_bindings.iter().enumerate() {
                let label = format!("$formation{ordinal}");
                binders.push((binding.required, label.clone()));
                binders.push((binding.actual, label));
            }
            let substitutions = bindings
                .iter()
                .map(|binding| (binding.symbol, binding.actual))
                .collect::<Vec<_>>();
            if program
                .fixed_array_length_identity(actual, &binders, &substitutions)
                .zip(program.fixed_array_length_identity(required, &binders, &substitutions))
                .is_some_and(|(actual, required)| actual == required)
            {
                return true;
            }
            match (actual, required) {
                (FixedArrayLength::Literal(actual), FixedArrayLength::Literal(required)) => {
                    actual == required
                }
                (
                    FixedArrayLength::ConstParameter { symbol: actual, .. },
                    FixedArrayLength::ConstParameter {
                        symbol: required, ..
                    },
                ) => binder_bindings
                    .iter()
                    .find(|binding| binding.required == *required)
                    .map_or(actual == required, |binding| binding.actual == *actual),
                (
                    FixedArrayLength::ConstCall {
                        machine: actual, ..
                    },
                    FixedArrayLength::ConstCall {
                        machine: required, ..
                    },
                ) => actual.is_valid() && actual == required,
                _ => false,
            }
        }

        pub fn infer_call_type_arguments(
            program: &dyn crate::CheckingInput,
            parameters: &[TypeParameter],
            caller_parameters: &[TypeParameter],
            mut required: TypeReferenceHandle,
            mut actual: TypeReferenceHandle,
            bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
        ) {
            if let TypeReferenceNode::Reference { referee, .. } =
                program.type_reference_table.type_reference(required)
            {
                required = *referee;
                if let TypeReferenceNode::Reference { referee, .. } =
                    program.type_reference_table.type_reference(actual)
                {
                    actual = *referee;
                }
            }
            if let (
                TypeReferenceNode::FixedArray {
                    element_type: actual_element,
                    ..
                },
                TypeReferenceNode::Slice {
                    element_type: required_element,
                },
            ) = (
                program.type_reference_table.type_reference(actual),
                program.type_reference_table.type_reference(required),
            ) {
                actual = *actual_element;
                required = *required_element;
            }
            let generic_types = parameters
                .iter()
                .filter(|parameter| {
                    matches!(
                        parameter.kind,
                        TypeParameterKind::Type | TypeParameterKind::Const { .. }
                    )
                })
                .collect::<Vec<_>>();
            let mut inferred = bindings
                .iter()
                .map(|(symbol, actual)| TypeBinding {
                    symbol: *symbol,
                    actual: *actual,
                })
                .collect::<Vec<_>>();
            if required_type_matches_exact(program, actual, required, &generic_types, &mut inferred)
                && inferred.iter().all(|binding| {
                    let Some(parameter) = parameters
                        .iter()
                        .find(|parameter| parameter.symbol == binding.symbol)
                    else {
                        return false;
                    };
                    !matches!(parameter.kind, TypeParameterKind::Const { .. })
                        || crate::value_custody::type_references::const_argument_matches_parameter(
                            program,
                            parameter,
                            binding.actual,
                            caller_parameters,
                        )
                })
            {
                *bindings = inferred
                    .into_iter()
                    .map(|binding| (binding.symbol, binding.actual))
                    .collect();
            }
        }
    }
    pub use type_refinement::infer_call_type_arguments;

    use crate::frozen_program::expression_table;
    use callable_shape::validate_selected_callable_effects;
    use callable_shape::validate_selected_callable_shape;
    pub(crate) use callable_shape::validate_trait_callable_parameter_refinement;
    pub(crate) use contract_facts::validate_callable_contract_refinement;
    pub(crate) mod installed_interrupts {
        use super::*;
        use typed_trees::expression::{InstalledInterruptOperationRequest, StaticMachineArgument};
        use typed_trees::static_applications::StaticMachineApplicationHandle;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct PreparedInstalledInterruptRegistration {
            pub site: ValidatedNominalMachineUseSite,
            pub control_service: language_semantics::ServiceReachId,
            pub application: StaticMachineApplicationHandle,
            pub target: SymbolHandle,
            pub registrar_owner: SymbolHandle,
            pub registrar_application_identity: String,
            pub registrar_declaring_application: String,
            pub registrar_arguments: Vec<String>,
            pub registrar_lifetimes: Vec<String>,
            pub timer_slot: StaticMachineArgument,
            pub slot_application: typed_trees::typed_trees::ClosedConformanceApplication,
            pub nominal_uses: Vec<ValidatedNominalMachineUse>,
            pub specializations: Vec<ValidatedRequirementCallSpecialization>,
        }

        pub(crate) fn form(
            program: &dyn crate::CheckingInput,
            index: &MachineStateIndex,
            site: ValidatedNominalMachineUseSite,
            request: &InstalledInterruptOperationRequest,
            diagnostics: &mut Vec<Diagnostic>,
            selections: &mut FormedStaticMachineSelections,
        ) {
            let Some(target) = source_call_target(program, site) else {
                diagnostics.push(Diagnostic::error(
                    "interrupt installation lost its source call",
                ));
                return;
            };
            let Some(machine) = program
                .machine_holding_state(target)
                .or_else(|| program.machine_by_symbol(target))
            else {
                diagnostics.push(Diagnostic::error(
                    "interrupt installation lost its source declaration",
                ));
                return;
            };
            match crate::compiler_operations::static_interrupt_declaration(program, machine) {
                Ok(Some(declaration))
                    if declaration.operation
                        == language_core::static_interrupts::StaticInterruptOperation::Install => {}
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(reason));
                    return;
                }
                _ => {
                    diagnostics.push(Diagnostic::error(
                        "installation metadata does not belong to the core installation operation",
                    ));
                    return;
                }
            }
            let control_service = match machine_control_service(program, machine) {
                Ok(service) => service,
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(reason));
                    return;
                }
            };
            let application = request.registrar_application;
            if !application.is_valid() || !request.registrar_target.is_valid() {
                let owner = match site {
                    ValidatedNominalMachineUseSite::Expression(handle) => program
                        .expression_table
                        .call_node(handle)
                        .map(|call| call.authored_machine_application),
                    ValidatedNominalMachineUseSite::Statement(handle) => program
                        .statement_table
                        .statement(handle)
                        .as_call()
                        .map(|call| call.authored_machine_application),
                }
                .filter(|application| application.is_valid())
                .map(|application| {
                    program
                        .expression_table
                        .static_machine_application(application)
                        .lexical_owner
                });
                if owner
                    .and_then(|owner| program.machine_by_symbol(owner))
                    .is_some_and(|machine| program.callable_body_has_open_parameters(machine))
                {
                    return;
                }
                diagnostics.push(Diagnostic::error(
                    "interrupt installation requires its complete registrar application",
                ));
                return;
            }
            let slot_application =
                match crate::conformance_applications::close_conformance_application(
                    program,
                    &request.timer_slot,
                ) {
                    Ok(application) => application,
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        return;
                    }
                };
            let source = program
                .expression_table
                .static_machine_application(application);
            let identities = match registrar_identities(program, request) {
                Ok(identities) => identities,
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(reason));
                    return;
                }
            };
            let mut registrar = FormedStaticMachineSelections::default();
            form_call_selection(
                program,
                index,
                CallSelection {
                    site,
                    target_symbol: source.registration_operation,
                    selected_target: request.registrar_target,
                    target_name: source.target.as_str(),
                    arguments: program
                        .expression_table
                        .static_machine_application_arguments(application),
                    application,
                },
                &[],
                diagnostics,
                &mut registrar,
            );
            selections.pending_effects.extend(registrar.pending_effects);
            selections.rows.installed_interrupt_registrations.push(
                PreparedInstalledInterruptRegistration {
                    site,
                    control_service,
                    application,
                    target: request.registrar_target,
                    registrar_owner: request.registrar_trait,
                    registrar_application_identity: identities.receiving,
                    registrar_declaring_application: identities.declaring,
                    registrar_arguments: identities.arguments,
                    registrar_lifetimes: identities.lifetimes,
                    timer_slot: request.timer_slot.clone(),
                    slot_application,
                    nominal_uses: registrar.rows.nominal_uses,
                    specializations: registrar.rows.requirement_call_specializations,
                },
            );
        }

        fn machine_control_service(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
        ) -> Result<language_semantics::ServiceReachId, String> {
            let required = language_core::static_interrupts::StaticInterruptOperation::Install
                .required_service_identity()
                .ok_or("interrupt installation has no catalog service requirement")?;
            let mut selected = None;
            for service in program
                .service_reach_rows
                .services(machine.service_reach_row)
            {
                let definition = program
                    .service_reaches
                    .definition(*service)
                    .ok_or("interrupt installer reaches an unknown service")?;
                if program.symbol_toolchain_relative_path(definition.symbol)
                    != Some(std::path::Path::new("assembly.omg"))
                {
                    continue;
                }
                let identity = program
                    .normalized_hermetic_symbol_identity(definition.symbol)
                    .map_err(|reason| format!("interrupt installer service identity: {reason}"))?;
                if identity != required {
                    continue;
                }
                if selected.replace(*service).is_some() {
                    return Err(
                        "interrupt installer repeats the exact core MachineControl service".into(),
                    );
                }
            }
            selected.ok_or_else(|| {
                "interrupt installer has no exact core MachineControl service".into()
            })
        }

        pub(crate) struct RegistrarIdentities {
            receiving: String,
            declaring: String,
            arguments: Vec<String>,
            lifetimes: Vec<String>,
        }

        fn registrar_identities(
            program: &dyn crate::CheckingInput,
            request: &InstalledInterruptOperationRequest,
        ) -> Result<RegistrarIdentities, &'static str> {
            let receiving = request
                .registrar_receiving_scope
                .scope(program)
                .and_then(|scope| scope.portable_application_identity(program))
                .and_then(|identity| program.type_identity_text(identity))
                .ok_or("interrupt registrar lost its selected owner application")?;
            let closed = program
                .closed_requirement_application(request.registrar_target)
                .ok_or("interrupt registrar lost its closed requirement application")?;
            let declaring = program
                .type_identity_text(closed.portable_trait_application_identity)
                .ok_or("interrupt registrar lost its declaring owner application")?;
            if closed.local_arguments.len() != closed.portable_local_arguments.len() {
                return Err("interrupt registrar lost part of its member argument list");
            }
            let arguments = closed
                .local_arguments
                .iter()
                .zip(closed.portable_local_arguments)
                .map(|(argument, identity)| {
                    program
                        .requirement_argument_identity_text(*argument, *identity)
                        .ok_or("interrupt registrar lost a member argument identity")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let source = program
                .expression_table
                .static_machine_application(request.registrar_application);
            let bindings = program
                .expression_table
                .static_machine_application_lifetimes(request.registrar_application);
            if source.lifetime_binding_declaration != closed.template {
                return Err("interrupt registrar lifetime bindings name a different member");
            }
            let parameters = typed_trees::type_identity::declaration_lifetime_parameters(
                program.typed_program(),
                closed.template,
            )
            .ok_or("interrupt registrar lost its declared lifetime parameters")?;
            if parameters.len() != bindings.len()
                || parameters
                    .iter()
                    .zip(bindings)
                    .any(|(parameter, binding)| parameter != &binding.parameter)
            {
                return Err(
                    "interrupt registrar lifetime arguments differ from its declaration order",
                );
            }
            let lifetimes = bindings
                .iter()
                .map(|binding| {
                    typed_trees::type_identity::portable_applied_lifetime_identity(
                        program.typed_program(),
                        binding.argument.as_str(),
                        source.lexical_owner,
                        &[],
                    )
                    .and_then(|identity| program.type_identity_text(identity))
                    .ok_or("interrupt registrar lost an applied lifetime identity")
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(RegistrarIdentities {
                receiving,
                declaring,
                arguments,
                lifetimes,
            })
        }
    }
    pub use installed_interrupts::PreparedInstalledInterruptRegistration;

    use nominal_admission::validate_nominal_machine_selection;
    use type_refinement::TypeBinding;

    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::BuildOperation;
    use symbols::{JoinedPath, SymbolHandle, SymbolKind};
    use typed_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::StatementHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Clone, Copy)]
    pub(crate) struct MachineSuspensionRow {
        symbol: SymbolHandle,
        transitive_may_suspend: bool,
    }

    #[derive(Clone, Copy)]
    pub(crate) struct MachineBlockingRow {
        symbol: SymbolHandle,
        transitive_may_block: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ValidatedNominalMachineUseSite {
        Statement(StatementHandle),
        Expression(ExpressionHandle),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedNominalMachineUse {
        pub site: ValidatedNominalMachineUseSite,
        pub registration_operation: SymbolHandle,
        pub registration_requirement_trait: Option<SymbolHandle>,
        pub static_machine_ordinal: u32,
        pub selected_machine: SymbolHandle,
        pub selected_entry: SymbolHandle,
        pub receiving_trait: SymbolHandle,
        pub receiving_application: Result<String, String>,
        pub satisfaction_trait: SymbolHandle,
        pub satisfaction_requirement: SymbolHandle,
        pub canonical_requirement_overload: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ValidatedRequirementCallTypeBinding {
        pub parameter: SymbolHandle,
        pub actual: TypeReferenceHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ValidatedRequirementCallMachineSelection {
        pub static_machine_ordinal: u32,
        pub parameter: SymbolHandle,
        pub selected_machine: SymbolHandle,
        pub selected: SymbolHandle,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedRequirementCallSpecialization {
        pub site: ValidatedNominalMachineUseSite,
        pub registration_operation: SymbolHandle,
        pub type_bindings: Vec<ValidatedRequirementCallTypeBinding>,
        pub machine_selections: Vec<ValidatedRequirementCallMachineSelection>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct PreparedStaticMachineSelections {
        pub installed_interrupt_registrations: Vec<PreparedInstalledInterruptRegistration>,
        pub crash_refinements: Vec<StaticMachineContractObligation>,
        pub nominal_uses: Vec<ValidatedNominalMachineUse>,
        pub requirement_call_specializations: Vec<ValidatedRequirementCallSpecialization>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub(crate) struct FormedStaticMachineSelections {
        rows: PreparedStaticMachineSelections,
        pending_effects: Vec<StaticMachineContractObligation>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StaticMachineContractObligation {
        parameter: arena::Handle<TypeParameter>,
        applied: typed_trees::typed_trees::AppliedMachineCallableId,
        selected: SymbolHandle,
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
        ordinal: usize,
        original_selected: SymbolHandle,
        label: String,
    }

    impl StaticMachineContractObligation {
        pub(crate) fn from_applied_callable(
            program: &dyn crate::CheckingInput,
            applied: &typed_trees::typed_trees::AppliedMachineCallable<'_>,
        ) -> Option<Self> {
            let handle = program.data_type_parameter_handle_by_symbol(applied.demand)?;
            let parameter = program.data_type_parameters.get(handle);
            let TypeParameterKind::Machine { contract } = &parameter.kind else {
                return None;
            };
            if program
                .machine_parameter_contract_view(contract)?
                .signature()
                != applied.source_required
            {
                return None;
            }
            let obligation = Self {
                parameter: handle,
                applied: applied.identity,
                selected: crate::call_target_state(program, applied.selected_entry)?.symbol,
                application: applied.invocation,
                ordinal: applied.ordinal,
                original_selected: applied.selected_entry,
                label: "selected callable input".to_owned(),
            };
            (std::ptr::eq(
                obligation.required_signature(program).ok()?,
                applied.required,
            ))
            .then_some(obligation)
        }

        pub(crate) fn supplies(
            &self,
            program: &dyn crate::CheckingInput,
            application: typed_trees::static_applications::StaticMachineApplicationHandle,
            ordinal: usize,
            demand: SymbolHandle,
            selected: SymbolHandle,
        ) -> bool {
            program.applied_machine_callable_origin(application) == Some(self.application)
                && self.ordinal == ordinal
                && self.original_selected == selected
                && program.data_type_parameters.get(self.parameter).symbol == demand
        }

        pub fn bounds_static_call(
            &self,
            program: &dyn crate::CheckingInput,
            site: crate::CallSite<'_>,
        ) -> bool {
            site.static_machine_parameter()
                == program.data_type_parameters.get(self.parameter).symbol
                && crate::call_target_entry_symbol(program, site.target_symbol(program))
                    == self.selected
        }

        pub fn input_identity(
            &self,
        ) -> (
            typed_trees::static_applications::StaticMachineApplicationHandle,
            usize,
        ) {
            (self.application, self.ordinal)
        }

        pub fn selected_entry(&self) -> SymbolHandle {
            self.selected
        }
        pub fn label(&self) -> &str {
            &self.label
        }

        pub fn required_signature<'p>(
            &self,
            program: &'p dyn crate::CheckingInput,
        ) -> Result<&'p typed_trees::signature::StateSignature, &'static str> {
            let parameter = program.data_type_parameters.get(self.parameter);
            let TypeParameterKind::Machine { contract } = &parameter.kind else {
                return Err("the selected callable lost its machine binder");
            };
            let required = machine_parameter_signature(program, contract);
            if self.applied.is_valid() {
                let applied = program
                    .applied_machine_callable_at(self.applied)
                    .ok_or("the selected callable lost its retained applied input row")?;
                if applied.invocation != self.application
                    || applied.ordinal != self.ordinal
                    || applied.selected_entry != self.original_selected
                    || applied.demand != parameter.symbol
                    || applied.source_required != required
                {
                    return Err(
                        "the selected callable input row belongs to another application or binder",
                    );
                }
                return Ok(applied.required);
            }
            if let Some(applied) = program.applied_machine_callable(
                self.application,
                self.ordinal,
                self.original_selected,
                parameter.symbol,
                required,
            ) {
                return Ok(applied.required);
            }
            if program.has_applied_machine_selection(
                self.application,
                self.ordinal,
                self.original_selected,
            ) {
                return Err("the selected callable lost its exact applied requirement");
            }
            Ok(required)
        }
    }

    fn project_operational_rows(
        operational: &flow_effects::OperationalPlan,
    ) -> (Vec<MachineSuspensionRow>, Vec<MachineBlockingRow>) {
        let suspensions = operational
            .machines()
            .iter()
            .map(|summary| MachineSuspensionRow {
                symbol: summary.symbol,
                transitive_may_suspend: summary.transitive_may_suspend,
            })
            .collect();
        let blockings = operational
            .machines()
            .iter()
            .map(|summary| MachineBlockingRow {
                symbol: summary.symbol,
                transitive_may_block: summary.transitive_may_block,
            })
            .collect();
        (suspensions, blockings)
    }

    pub(crate) fn form_static_machine_selections(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> FormedStaticMachineSelections {
        let mut selections = FormedStaticMachineSelections::default();
        validate_conformance_machine_selections(program, diagnostics);
        let mathematical_expressions = crate::mathematical_applications::expression_nodes(program);
        let index = machine_state_index(program);
        for (handle, expression) in expression_table(program).iter_expressions() {
            if let ExpressionNode::Call(call) = expression {
                if mathematical_expressions.contains(&handle) {
                    continue;
                }
                if !typed_trees::operator::named_expression_call_candidates(program, call)
                    .is_empty()
                {
                    continue;
                }
                let (target_symbol, target, arguments) =
                    if call.authored_machine_application.is_valid() {
                        let application = expression_table(program)
                            .static_machine_application(call.authored_machine_application);
                        (
                            application.registration_operation,
                            application.target.as_str(),
                            expression_table(program).static_machine_application_arguments(
                                call.authored_machine_application,
                            ),
                        )
                    } else {
                        (
                            call.target_symbol,
                            call.target.as_str(),
                            call.machine_arguments.as_ref(),
                        )
                    };
                validate_call_selection(
                    program,
                    &index,
                    ValidatedNominalMachineUseSite::Expression(handle),
                    target_symbol,
                    target,
                    arguments,
                    diagnostics,
                    &mut selections,
                );
                if let Some(request) = &call.installed_interrupt_operation {
                    installed_interrupts::form(
                        program,
                        &index,
                        ValidatedNominalMachineUseSite::Expression(handle),
                        request,
                        diagnostics,
                        &mut selections,
                    );
                }
            }
        }

        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (statement_handle, statement) in program
                    .statement_table
                    .iter_statements(state.statement_nodes)
                {
                    if let Some(call) = statement.as_call() {
                        if !typed_trees::operator::named_statement_call_candidates(program, call)
                            .is_empty()
                        {
                            continue;
                        }
                        let (target_symbol, target, arguments) =
                            if call.authored_machine_application.is_valid() {
                                let application = expression_table(program)
                                    .static_machine_application(call.authored_machine_application);
                                (
                                    application.registration_operation,
                                    application.target.as_str(),
                                    expression_table(program).static_machine_application_arguments(
                                        call.authored_machine_application,
                                    ),
                                )
                            } else {
                                (
                                    call.target_symbol,
                                    call.target.as_str(),
                                    call.machine_arguments.as_ref(),
                                )
                            };
                        validate_call_selection(
                            program,
                            &index,
                            ValidatedNominalMachineUseSite::Statement(statement_handle),
                            target_symbol,
                            target,
                            arguments,
                            diagnostics,
                            &mut selections,
                        );
                        if let Some(request) = &call.installed_interrupt_operation {
                            installed_interrupts::form(
                                program,
                                &index,
                                ValidatedNominalMachineUseSite::Statement(statement_handle),
                                request,
                                diagnostics,
                                &mut selections,
                            );
                        }
                    }
                }
            }
        }
        selections
    }

    pub(crate) fn admit_static_machine_effects(
        program: &dyn crate::CheckingInput,
        operational: &flow_effects::OperationalPlan,
        formed: FormedStaticMachineSelections,
    ) -> Result<PreparedStaticMachineSelections, Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let service_reaches = crate::infer_service_reaches(program, operational);
        let invocations = crate::infer_synchronous_invocations(program);
        let (suspensions, blockings) = project_operational_rows(operational);
        let index = machine_state_index(program);
        for obligation in &formed.pending_effects {
            let parameter = program.data_type_parameters.get(obligation.parameter);
            let TypeParameterKind::Machine { contract } = &parameter.kind else {
                unreachable!("formed machine selection has a machine binder")
            };
            let requirement = machine_parameter_signature(program, contract);
            let (machine, state) = machine_and_state(program, &index, obligation.selected)
                .expect("formed concrete selection retains its callable entry");
            validate_selected_callable_effects(
                program,
                &suspensions,
                &blockings,
                &service_reaches,
                &invocations,
                &obligation.label,
                parameter,
                requirement,
                machine,
                state,
                &mut diagnostics,
            );
        }
        if diagnostics.is_empty() {
            let mut prepared = formed.rows;
            prepared.crash_refinements = formed.pending_effects;
            Ok(prepared)
        } else {
            Err(diagnostics)
        }
    }

    pub fn validate_static_machine_selections(
        program: &dyn crate::CheckingInput,
    ) -> Result<PreparedStaticMachineSelections, Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let selections = form_static_machine_selections(program, &mut diagnostics);
        if diagnostics.is_empty() {
            let operational = crate::infer_operational_may(program);
            admit_static_machine_effects(program, &operational, selections)
        } else {
            Err(diagnostics)
        }
    }

    pub(crate) fn machine_selection_arguments<'call>(
        program: &dyn crate::CheckingInput,
        arguments: &'call [StaticMachineArgument],
    ) -> impl Iterator<Item = &'call StaticMachineArgument> {
        arguments
            .iter()
            .filter(|argument| is_machine_selection_symbol(program, argument.symbol))
    }

    fn is_machine_selection_symbol(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> bool {
        symbol.is_valid()
            && matches!(
                program.symbols.get(symbol).kind,
                SymbolKind::State | SymbolKind::MachineParameter
            )
    }

    fn source_call_target(
        program: &dyn crate::CheckingInput,
        site: ValidatedNominalMachineUseSite,
    ) -> Option<SymbolHandle> {
        match site {
            ValidatedNominalMachineUseSite::Expression(handle) => expression_table(program)
                .call_node(handle)
                .map(|call| call.target_symbol),
            ValidatedNominalMachineUseSite::Statement(handle) => program
                .statement_table
                .statement(handle)
                .as_call()
                .map(|call| call.target_symbol),
        }
    }

    fn validate_call_selection(
        program: &dyn crate::CheckingInput,
        index: &MachineStateIndex,
        site: ValidatedNominalMachineUseSite,
        target_symbol: SymbolHandle,
        target_name: &str,
        arguments: &[StaticMachineArgument],
        diagnostics: &mut Vec<Diagnostic>,
        selections: &mut FormedStaticMachineSelections,
    ) {
        let call = CallSelection {
            site,
            target_symbol,
            target_name,
            arguments,
            selected_target: source_call_target(program, site).unwrap_or(target_symbol),
            application: match site {
                ValidatedNominalMachineUseSite::Expression(handle) => {
                    program.expression_table.call_node(handle).map_or(
                        typed_trees::static_applications::StaticMachineApplicationHandle::invalid(),
                        |call| call.authored_machine_application,
                    )
                }
                ValidatedNominalMachineUseSite::Statement(handle) => {
                    program.statement_table.statement(handle).as_call().map_or(
                        typed_trees::static_applications::StaticMachineApplicationHandle::invalid(),
                        |call| call.authored_machine_application,
                    )
                }
            },
        };
        let mut occurrence = FormedStaticMachineSelections::default();
        form_call_selection(program, index, call, &[], diagnostics, &mut occurrence);
        family_demands::form(program, index, call, diagnostics, &mut occurrence);
        selections
            .rows
            .nominal_uses
            .extend(occurrence.rows.nominal_uses);
        selections
            .rows
            .requirement_call_specializations
            .extend(occurrence.rows.requirement_call_specializations);
        selections
            .pending_effects
            .extend(occurrence.pending_effects);
    }

    #[derive(Clone, Copy)]
    pub(crate) struct CallSelection<'call> {
        site: ValidatedNominalMachineUseSite,
        target_symbol: SymbolHandle,
        selected_target: SymbolHandle,
        target_name: &'call str,
        arguments: &'call [StaticMachineArgument],
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
    }

    fn form_call_selection(
        program: &dyn crate::CheckingInput,
        index: &MachineStateIndex,
        call: CallSelection<'_>,
        initial_bindings: &[TypeBinding],
        diagnostics: &mut Vec<Diagnostic>,
        selections: &mut FormedStaticMachineSelections,
    ) {
        let CallSelection {
            site,
            target_symbol,
            selected_target,
            target_name,
            arguments,
            application,
            ..
        } = call;
        let closed_requirement = program
            .closed_requirement_application(selected_target)
            .filter(|closed| closed.template == target_symbol);
        let registration_requirement_trait = closed_requirement
            .map(|closed| closed.declaring_trait)
            .or_else(|| {
                crate::selected_requirement_signature(program, target_symbol)
                    .map(|(declaration, _)| declaration.symbol)
            });
        fn contract_and_type_parameters(
            parameters: &[TypeParameter],
        ) -> (
            Vec<(&TypeParameter, &MachineParameterContract)>,
            Vec<&TypeParameter>,
        ) {
            (
                parameters
                    .iter()
                    .filter_map(|parameter| match &parameter.kind {
                        TypeParameterKind::Machine { contract } => Some((parameter, contract)),
                        _ => None,
                    })
                    .collect(),
                parameters
                    .iter()
                    .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                    .collect(),
            )
        }

        // Constant and layout preparation can arrive before mathematical terms
        // are checked. The declaration determines the call kind; source
        // formation and runtime checks still apply separately.
        if crate::mathematical_applications::is_mathematical_declaration(program, target_symbol) {
            return;
        }

        if !target_symbol.is_valid()
            && matches!(
                BuildOperation::from_call_target(target_name),
                Some(
                    BuildOperation::ProviderSelection
                        | BuildOperation::BoundaryAcceptance
                        | BuildOperation::WireCompatibilityRequest
                        | BuildOperation::RepresentationSelection
                        | BuildOperation::ServiceExclusion
                        | BuildOperation::ServiceAudit
                        | BuildOperation::AssumedBindingContract
                        | BuildOperation::TestGroupRegistration
                )
            )
        {
            return;
        }
        let (requirements, generic_types, invocation_type_count): (Vec<_>, Vec<_>, usize) =
            if let Some((callee, _)) = machine_and_state(program, index, target_symbol) {
                let (requirements, generic_types) =
                    contract_and_type_parameters(program.machine_type_parameters(callee));
                let invocation_type_count = generic_types.len();
                (requirements, generic_types, invocation_type_count)
            } else if let Some((declaring_machine, signature)) =
                program.machine_parameter_signature(target_symbol)
            {
                let (requirements, mut generic_types) = contract_and_type_parameters(
                    program.state_signature_type_parameters(signature),
                );
                let invocation_type_count = generic_types.len();
                generic_types.extend(
                    contract_and_type_parameters(
                        program.machine_type_parameters(declaring_machine),
                    )
                    .1,
                );
                (requirements, generic_types, invocation_type_count)
            } else if let Some(signature) = closed_requirement
                .map(|closed| closed.signature)
                .or_else(|| {
                    crate::selected_requirement_signature(program, target_symbol)
                        .map(|(_, signature)| signature)
                })
            {
                let (requirements, _) = contract_and_type_parameters(
                    program.requirement_invocation_contract_parameters(signature),
                );
                let (_, generic_types) = contract_and_type_parameters(
                    program.state_signature_type_parameters(signature),
                );
                let invocation_type_count = generic_types.len();
                (requirements, generic_types, invocation_type_count)
            } else {
                if !arguments.is_empty() {
                    let source_span = match site {
                        ValidatedNominalMachineUseSite::Expression(handle) => {
                            expression_table(program).source_span(handle)
                        }
                        ValidatedNominalMachineUseSite::Statement(handle) => {
                            program
                                .statement_table
                                .statement(handle)
                                .as_call()
                                .expect("validated call site")
                                .source_span
                        }
                    };
                    diagnostics.push(Diagnostic::error(format!(
                    "call `{target_name}` supplies static machine arguments, but its generic callee did not resolve"
                )).with_source_span(source_span));
                }
                return;
            };

        let declared_parameters = declared_type_parameters(program, index, target_symbol);
        if let Some(projection) = arguments
            .iter()
            .enumerate()
            .find_map(|(ordinal, argument)| {
                let projection = argument.evidence_projection.as_ref()?;
                let value_slot = declared_parameters.get(ordinal).is_some_and(|parameter| {
                    matches!(parameter.kind, TypeParameterKind::Value { .. })
                });
                (!value_slot).then_some(projection)
            })
        {
            diagnostics.push(Diagnostic::error(format!(
            "proof-static evidence projection `{}.{}` cannot select an executable machine parameter; erased evidence cannot eliminate into runtime computation",
            projection.term, projection.member
        )));
            return;
        }

        let selected_machines = if application.is_valid() {
            expression_table(program).static_machine_application_selected_machines(application)
        } else {
            &[]
        };
        let machine_arguments = if selected_machines.is_empty() {
            machine_selection_arguments(program, arguments).collect::<Vec<_>>()
        } else {
            selected_machines.iter().collect::<Vec<_>>()
        };
        if machine_arguments.len() != requirements.len() {
            diagnostics.push(Diagnostic::error(format!(
                "generic call `{target_name}` requires {} static machine argument(s), got {}",
                requirements.len(),
                machine_arguments.len()
            )));
            return;
        }

        let mut bindings = initial_bindings.to_vec();
        if application.is_valid() {
            for binding in
                expression_table(program).static_machine_application_bindings(application)
            {
                let actual = match binding.value {
                    StaticMachineBindingValue::Type(actual)
                    | StaticMachineBindingValue::Const(actual) => actual,
                    _ => continue,
                };
                if !bindings
                    .iter()
                    .any(|known| known.symbol == binding.parameter)
                {
                    bindings.push(TypeBinding {
                        symbol: binding.parameter,
                        actual,
                    });
                }
            }
        }
        for (parameter, selected) in generic_types
            .iter()
            .take(invocation_type_count)
            .zip(type_selections::selected_types(program, arguments))
        {
            if !bindings
                .iter()
                .any(|binding| binding.symbol == parameter.symbol)
            {
                bindings.push(TypeBinding {
                    symbol: parameter.symbol,
                    actual: selected,
                });
            }
        }
        let mut machine_selections = Vec::new();

        for (static_machine_ordinal, ((parameter, requirement), selected)) in
            requirements.into_iter().zip(machine_arguments).enumerate()
        {
            let rendered = selected.path.joined_path();
            let requirement_signature = machine_parameter_signature(program, requirement);
            if selected.application.is_some()
                && program
                    .applied_machine_callable(
                        application,
                        static_machine_ordinal,
                        selected.symbol,
                        parameter.symbol,
                        requirement_signature,
                    )
                    .is_none()
            {
                diagnostics.push(Diagnostic::error(format!(
                "static machine argument `{rendered}` for `{}` is a nested machine application without a completely admitted callable tuple",
                parameter.name
            )));
                continue;
            }
            if selected.symbol == parameter.symbol {
                continue;
            }
            if !selected.symbol.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                "static machine argument `{rendered}` for `{}` does not resolve to a concrete machine",
                parameter.name
            )));
                continue;
            }
            let Ok(nominal_selection) = validate_nominal_machine_selection(
                program,
                index,
                target_name,
                parameter,
                requirement,
                selected.symbol,
                &rendered,
                application,
                static_machine_ordinal,
                &generic_types,
                &mut bindings,
                diagnostics,
            ) else {
                continue;
            };
            let requirement = requirement_signature;
            validate_selected_callable_shape(
                program,
                index,
                target_name,
                parameter,
                requirement,
                selected.symbol,
                application,
                static_machine_ordinal,
                &rendered,
                &generic_types,
                &mut bindings,
                Some(site),
                diagnostics,
            );
            let (selected_machine, selection_symbol) =
                machine_and_state(program, index, selected.symbol)
                    .map(|(machine, state)| (machine.symbol, state.symbol))
                    .unwrap_or((SymbolHandle::invalid(), selected.symbol));
            if let Some((machine, _)) = machine_and_state(program, index, selected.symbol) {
                selections
                    .pending_effects
                    .push(StaticMachineContractObligation {
                        parameter: program
                            .data_type_parameters
                            .iter()
                            .find_map(|(handle, candidate)| {
                                std::ptr::eq(candidate, parameter).then_some(handle)
                            })
                            .expect("formed contract binder belongs to the typed parameter arena"),
                        applied: program
                            .applied_machine_callable(
                                application,
                                static_machine_ordinal,
                                selected.symbol,
                                parameter.symbol,
                                requirement,
                            )
                            .map(|view| view.identity)
                            .unwrap_or_default(),
                        selected: selection_symbol,
                        application,
                        ordinal: static_machine_ordinal,
                        original_selected: selected.symbol,
                        label: format!("machine argument `{}` for `{target_name}`", machine.name),
                    });
            }
            machine_selections.push(ValidatedRequirementCallMachineSelection {
                static_machine_ordinal: u32::try_from(static_machine_ordinal)
                    .expect("static machine argument ordinal overflow"),
                parameter: parameter.symbol,
                selected_machine,
                selected: selection_symbol,
            });
            if let Some(selection) = nominal_selection {
                selections
                    .rows
                    .nominal_uses
                    .push(ValidatedNominalMachineUse {
                        site,
                        registration_operation: target_symbol,
                        registration_requirement_trait,
                        static_machine_ordinal: u32::try_from(static_machine_ordinal)
                            .expect("static machine argument ordinal overflow"),
                        selected_machine: selection.selected_machine,
                        selected_entry: selection.selected_entry,
                        receiving_trait: selection.receiving_trait,
                        receiving_application: selection.receiving_application,
                        satisfaction_trait: selection.satisfaction_trait,
                        satisfaction_requirement: selection.satisfaction_requirement,
                        canonical_requirement_overload: selection.canonical_requirement_overload,
                    });
            }
        }

        if !machine_selections.is_empty() {
            selections.rows.requirement_call_specializations.push(
                ValidatedRequirementCallSpecialization {
                    site,
                    registration_operation: target_symbol,
                    type_bindings: bindings
                        .iter()
                        .map(|binding| ValidatedRequirementCallTypeBinding {
                            parameter: binding.symbol,
                            actual: binding.actual,
                        })
                        .collect(),
                    machine_selections,
                },
            );
        }
    }

    pub(crate) fn validate_data_machine_selection(
        program: &dyn crate::CheckingInput,
        family_name: &str,
        parameter: &TypeParameter,
        requirement: &MachineParameterContract,
        selected_symbol: SymbolHandle,
        selected_name: &str,
        generic_types: &[&TypeParameter],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        validate_declaration_machine_selection(
            program,
            family_name,
            parameter,
            requirement,
            selected_symbol,
            selected_name,
            generic_types,
            typed_trees::static_applications::StaticMachineApplicationHandle::invalid(),
            0,
            substitutions,
            diagnostics,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_declaration_machine_selection(
        program: &dyn crate::CheckingInput,
        family_name: &str,
        parameter: &TypeParameter,
        requirement: &MachineParameterContract,
        selected_symbol: SymbolHandle,
        selected_name: &str,
        generic_types: &[&TypeParameter],
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
        ordinal: usize,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let index = machine_state_index(program);
        let mut bindings = substitutions
            .iter()
            .map(|(symbol, actual)| TypeBinding {
                symbol: *symbol,
                actual: *actual,
            })
            .collect::<Vec<_>>();
        if validate_nominal_machine_selection(
            program,
            &index,
            family_name,
            parameter,
            requirement,
            selected_symbol,
            selected_name,
            application,
            ordinal,
            generic_types,
            &mut bindings,
            diagnostics,
        )
        .is_err()
        {
            return;
        }
        let requirement = machine_parameter_signature(program, requirement);
        let (service_reaches, suspensions, blockings) = {
            let operational = crate::infer_operational_may(program);
            let service_reaches = crate::infer_service_reaches(program, &operational);
            let (suspensions, blockings) = project_operational_rows(&operational);
            (service_reaches, suspensions, blockings)
        };
        let invocations = crate::infer_synchronous_invocations(program);
        validate_selected_callable_shape(
            program,
            &index,
            family_name,
            parameter,
            requirement,
            selected_symbol,
            application,
            ordinal,
            selected_name,
            generic_types,
            &mut bindings,
            None,
            diagnostics,
        );
        let applied_requirement = program
            .applied_machine_callable(
                application,
                ordinal,
                selected_symbol,
                parameter.symbol,
                requirement,
            )
            .map_or(requirement, |applied| applied.required);
        if let Some((machine, state)) = machine_and_state(program, &index, selected_symbol) {
            let label = format!("machine argument `{}` for `{family_name}`", machine.name);
            validate_selected_callable_effects(
                program,
                &suspensions,
                &blockings,
                &service_reaches,
                &invocations,
                &label,
                parameter,
                applied_requirement,
                machine,
                state,
                diagnostics,
            );
        }
    }

    fn machine_parameter_contract_definition(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<(&TypeParameter, &MachineParameterContract)> {
        for machine in program.machines() {
            if let Some(found) = machine_parameter_contract_definition_in(
                program,
                program.machine_type_parameters(machine),
                symbol,
            ) {
                return Some(found);
            }
        }
        for declaration in program.traits() {
            if let Some(found) = machine_parameter_contract_definition_in(
                program,
                program.trait_type_parameters(declaration),
                symbol,
            ) {
                return Some(found);
            }
        }
        for conformance in program.conformances() {
            if let Some(found) = machine_parameter_contract_definition_in(
                program,
                program.conformance_type_parameters(conformance),
                symbol,
            ) {
                return Some(found);
            }
        }
        for data in program.data_definitions() {
            if let Some(found) = machine_parameter_contract_definition_in(
                program,
                program.data_type_parameters(data),
                symbol,
            ) {
                return Some(found);
            }
        }
        for alias in program.type_aliases() {
            if let Some(found) = machine_parameter_contract_definition_in(
                program,
                program
                    .tables
                    .data_type_parameters
                    .span_or_empty(alias.type_parameters),
                symbol,
            ) {
                return Some(found);
            }
        }
        None
    }

    fn machine_parameter_contract_definition_in<'program>(
        program: &'program dyn crate::CheckingInput,
        parameters: &'program [TypeParameter],
        symbol: SymbolHandle,
    ) -> Option<(&'program TypeParameter, &'program MachineParameterContract)> {
        for parameter in parameters {
            let TypeParameterKind::Machine { contract } = &parameter.kind else {
                continue;
            };
            if parameter.symbol == symbol {
                return Some((parameter, contract));
            }
            if matches!(contract, MachineParameterContract::RequirementIdentity) {
                continue;
            }
            let signature = machine_parameter_signature(program, contract);
            if let Some(found) = machine_parameter_contract_definition_in(
                program,
                program.state_signature_type_parameters(signature),
                symbol,
            ) {
                return Some(found);
            }
        }
        None
    }

    fn machine_parameter_signature<'program>(
        program: &'program dyn crate::CheckingInput,
        contract: &'program MachineParameterContract,
    ) -> &'program typed_trees::signature::StateSignature {
        program
            .machine_parameter_contract_view(contract)
            .expect("typed machine-parameter contract must retain a valid requirement identity")
            .signature()
    }

    pub(crate) type MachineStateIndex = symbols::SymbolKeyMap<SymbolHandle, (u32, u32)>;

    fn machine_state_index(program: &dyn crate::CheckingInput) -> MachineStateIndex {
        let mut index = MachineStateIndex::default();
        for (machine_position, machine) in program.machines().iter().enumerate() {
            let states = program.machine_states(machine);
            for (state_position, state) in states.iter().enumerate() {
                index.insert(
                    state.symbol,
                    (
                        u32::try_from(machine_position).expect("machine position overflow"),
                        u32::try_from(state_position).expect("state position overflow"),
                    ),
                );
            }
            if !states.is_empty() {
                index.entry(machine.symbol).or_insert((
                    u32::try_from(machine_position).expect("machine position overflow"),
                    0,
                ));
            }
        }
        index
    }

    pub(crate) fn static_machine_input_parameters(
        program: &dyn crate::CheckingInput,
        target_symbol: SymbolHandle,
    ) -> &[TypeParameter] {
        if let Some(machine) = program.machine_by_own_or_state_symbol(target_symbol) {
            return program.machine_type_parameters(machine);
        }
        if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
            return program.state_signature_type_parameters(signature);
        }
        crate::selected_requirement_signature(program, target_symbol)
            .map(|(_, signature)| program.requirement_invocation_contract_parameters(signature))
            .unwrap_or(&[])
    }

    fn declared_type_parameters<'a>(
        program: &'a dyn crate::CheckingInput,
        index: &MachineStateIndex,
        target_symbol: SymbolHandle,
    ) -> &'a [TypeParameter] {
        if let Some((machine, _)) = machine_and_state(program, index, target_symbol) {
            return program.machine_type_parameters(machine);
        }
        if let Some(machine) = program
            .machine_by_symbol(target_symbol)
            .or_else(|| program.machine_holding_state(target_symbol))
        {
            return program.machine_type_parameters(machine);
        }
        program
            .trait_machine_signature(target_symbol)
            .map(|(_, signature)| program.requirement_invocation_contract_parameters(signature))
            .or_else(|| {
                program
                    .machine_parameter_signature(target_symbol)
                    .map(|(_, signature)| {
                        program.requirement_invocation_contract_parameters(signature)
                    })
            })
            .unwrap_or(&[])
    }

    fn machine_and_state<'a>(
        program: &'a dyn crate::CheckingInput,
        index: &MachineStateIndex,
        selected_symbol: SymbolHandle,
    ) -> Option<(&'a Machine, &'a State)> {
        if !selected_symbol.is_valid() {
            return None;
        }
        let (machine_position, state_position) = index.get(&selected_symbol)?;
        let machine = program
            .machines()
            .get(*machine_position as usize)
            .expect("machine-state index position out of range");
        let state = program
            .machine_states(machine)
            .get(*state_position as usize)
            .expect("machine-state index state out of range");
        Some((machine, state))
    }

    fn validate_conformance_machine_selections(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (_, _, supplying, application) in program
            .typed_program()
            .closed_conformance_application_sites()
        {
            let Some(declaration) = program.conformance_by_symbol(application.declaration) else {
                diagnostics.push(Diagnostic::error(
                    "selected map Machine demand lost its declaration",
                ));
                continue;
            };
            let parameters = program.conformance_type_parameters(declaration);
            if !parameters
                .iter()
                .any(|p| matches!(p.kind, TypeParameterKind::Machine { .. }))
            {
                continue;
            }
            let invocation = application.callable_application;
            if !invocation.is_valid() {
                diagnostics.push(Diagnostic::error(
                    "closed map Machine argument lost its occurrence-owned callable demand",
                ));
                continue;
            }
            if let Err(reason) = crate::machine_calls::calls::conformance_binding_is_exact(
                program,
                supplying,
                invocation,
                application,
            ) {
                diagnostics.push(Diagnostic::error(reason));
                continue;
            }
            let selected =
                expression_table(program).static_machine_application_selected_machines(invocation);
            let substitutions = expression_table(program)
                .static_machine_application_bindings(invocation)
                .iter()
                .filter_map(|binding| match binding.value {
                    StaticMachineBindingValue::Type(actual)
                    | StaticMachineBindingValue::Const(actual) => Some((binding.parameter, actual)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let generic_types = parameters
                .iter()
                .filter(|p| {
                    matches!(
                        p.kind,
                        TypeParameterKind::Type
                            | TypeParameterKind::Const { .. }
                            | TypeParameterKind::Value { .. }
                    )
                })
                .collect::<Vec<_>>();
            let mut ordinal = 0;
            for parameter in parameters {
                let TypeParameterKind::Machine { contract } = &parameter.kind else {
                    continue;
                };
                let Some(argument) = selected.get(ordinal) else {
                    diagnostics.push(Diagnostic::error(
                        "closed map lost a captured Machine selection",
                    ));
                    break;
                };
                let required = machine_parameter_signature(program, contract);
                let original =
                    application
                        .arguments
                        .iter()
                        .zip(parameters)
                        .find_map(|(argument, binder)| {
                            (binder.symbol == parameter.symbol).then_some(argument)
                        });
                let view = program.applied_machine_callable(
                    invocation,
                    ordinal,
                    argument.symbol,
                    parameter.symbol,
                    required,
                );
                let exact = original.zip(view).is_some_and(|(original, view)| {
                    let provider =
                        expression_table(program).static_machine_application(view.provider);
                    provider_tuple_matches_source(program, original, view.provider)
                        && program.applied_machine_callable_matches(
                            invocation,
                            ordinal,
                            argument.symbol,
                            parameter.symbol,
                            program
                                .expression_table
                                .static_machine_application_bindings(view.provider),
                            program
                                .expression_table
                                .static_machine_application_lifetimes(view.provider),
                            required,
                            &substitutions,
                            program
                                .expression_table
                                .static_machine_application_lifetimes(invocation),
                        )
                        && provider.registration_operation
                            == program
                                .machine_by_symbol(original.symbol)
                                .and_then(|machine| program.machine_states(machine).first())
                                .map_or(original.symbol, |state| state.symbol)
                        && provider.lexical_owner
                            == expression_table(program)
                                .static_machine_application(invocation)
                                .lexical_owner
                        && view.caller_operands.len() == substitutions.len()
                        && view.caller_operands.iter().zip(&substitutions).all(
                            |(binding, (parameter, actual))| {
                                binding.parameter == *parameter && binding.actual == *actual
                            },
                        )
                        && view.caller_lifetimes
                            == expression_table(program)
                                .static_machine_application_lifetimes(invocation)
                });
                if !exact {
                    diagnostics.push(Diagnostic::error("captured Machine selection differs from its exact source operand and applied demand"));
                    ordinal += 1;
                    continue;
                }
                validate_declaration_machine_selection(
                    program,
                    declaration
                        .alias
                        .as_ref()
                        .map_or("<conformance>", |n| n.as_str()),
                    parameter,
                    contract,
                    argument.symbol,
                    &argument.display_name(),
                    &generic_types,
                    invocation,
                    ordinal,
                    &substitutions,
                    diagnostics,
                );
                ordinal += 1;
            }
            if ordinal != selected.len() {
                diagnostics.push(Diagnostic::error(
                    "closed map has surplus captured Machine selections",
                ));
            }
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum StaticOperandCategory {
        Type,
        Const,
        Machine,
        Implementation,
    }

    fn provider_tuple_matches_source(
        program: &dyn crate::CheckingInput,
        original: &StaticMachineArgument,
        handle: typed_trees::static_applications::StaticMachineApplicationHandle,
    ) -> bool {
        let descriptor = expression_table(program).static_machine_application(handle);
        let Some(application) = &original.application else {
            return true;
        };
        let Some(machine) = program.machine_by_symbol(descriptor.binding_declaration) else {
            return application.arguments.is_empty() && application.lifetime_arguments.is_empty();
        };
        let parameters = program.machine_type_parameters(machine);
        let bindings = expression_table(program).static_machine_application_bindings(handle);
        let mut type_ordinal = 0;
        let mut const_ordinal = 0;
        let mut machine_ordinal = 0;
        let mut evidence_ordinal = 0;
        for argument in &application.arguments {
            let kind = if argument.type_reference.is_valid()
                || (argument.symbol.is_valid()
                    && matches!(
                        program.symbols.get(argument.symbol).kind,
                        SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
                    )) {
                StaticOperandCategory::Type
            } else if program.static_const_argument_identity(argument).is_some() {
                StaticOperandCategory::Const
            } else if argument.symbol.is_valid()
                && matches!(
                    program.symbols.get(argument.symbol).kind,
                    SymbolKind::State | SymbolKind::MachineParameter
                )
            {
                StaticOperandCategory::Machine
            } else {
                StaticOperandCategory::Implementation
            };
            if kind == StaticOperandCategory::Implementation {
                let Some(bound) = machine.conformance_bounds.get(evidence_ordinal) else {
                    return false;
                };
                evidence_ordinal += 1;
                let selected = bindings
                    .iter()
                    .filter(|binding| binding.parameter == bound.binder)
                    .collect::<Vec<_>>();
                let [selected] = selected.as_slice() else {
                    return false;
                };
                if !matches!(&selected.value, StaticMachineBindingValue::Conformance(actual) if actual == argument)
                {
                    return false;
                }
                continue;
            }
            let ordinal = match kind {
                StaticOperandCategory::Type => &mut type_ordinal,
                StaticOperandCategory::Const => &mut const_ordinal,
                StaticOperandCategory::Machine => &mut machine_ordinal,
                StaticOperandCategory::Implementation => {
                    unreachable!("evidence operands were handled above")
                }
            };
            let parameter = parameters
                .iter()
                .filter(|parameter| {
                    matches!(
                        (kind, &parameter.kind),
                        (StaticOperandCategory::Type, TypeParameterKind::Type)
                            | (
                                StaticOperandCategory::Const,
                                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                            )
                            | (
                                StaticOperandCategory::Machine,
                                TypeParameterKind::Machine { .. }
                            )
                    )
                })
                .nth(*ordinal);
            *ordinal += 1;
            let Some(parameter) = parameter else {
                return false;
            };
            let selected = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol)
                .collect::<Vec<_>>();
            let [selected] = selected.as_slice() else {
                return false;
            };
            if !program.static_binding_matches_argument(
                &parameter.kind,
                &selected.value,
                argument,
                descriptor.lexical_owner,
                descriptor.lexical_owner,
            ) {
                return false;
            }
        }
        if !application.lifetime_arguments.is_empty() {
            let lifetimes = expression_table(program).static_machine_application_lifetimes(handle);
            if lifetimes.len() != application.lifetime_arguments.len()
                || lifetimes.len() != machine.lifetime_parameters.len()
                || lifetimes
                    .iter()
                    .zip(
                        machine
                            .lifetime_parameters
                            .iter()
                            .zip(application.lifetime_arguments.iter()),
                    )
                    .any(|(binding, (parameter, argument))| {
                        binding.parameter != *parameter || binding.argument != *argument
                    })
            {
                return false;
            }
        }
        true
    }
}

pub mod static_arguments {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use numerics::literals::LandedIntegerType;
    use symbols::SymbolKind;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
    use typed_trees::references::StaticArgumentCall;
    use typed_trees::state::State;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) mod invocation_parameters {
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameter;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;
        use typed_trees::references::{TypedReferences, visit_type_reference};
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) fn invocation_parameters(
            program: &dyn crate::CheckingInput,
            realization: &Machine,
            family: &Machine,
            parameters: &[TypeParameter],
            trait_symbol: SymbolHandle,
            requirement_symbol: SymbolHandle,
        ) -> Vec<TypeParameter> {
            let mut captured = ApplicationReferences::default();
            for edge in program
                .machine_trait_conformances(family)
                .iter()
                .filter(|edge| {
                    edge.symbol == trait_symbol && edge.requirement_symbol == requirement_symbol
                })
            {
                for argument in program
                    .type_reference_table
                    .type_reference_handles(edge.arguments)
                {
                    visit_type_reference(program, &mut captured, *argument);
                }
            }
            captured.symbols.extend(
                program
                    .conformances()
                    .iter()
                    .filter(|conformance| {
                        conformance.rows.iter().any(|row| {
                            (row.realization_machine == realization.symbol
                                || row.realization_machine == family.symbol)
                                && row.source.owns_realization()
                        })
                    })
                    .flat_map(|conformance| program.conformance_type_parameters(conformance))
                    .map(|parameter| parameter.symbol),
            );
            parameters
                .iter()
                .filter(|parameter| !captured.symbols.contains(&parameter.symbol))
                .cloned()
                .collect()
        }

        #[derive(Default)]
        pub(crate) struct ApplicationReferences {
            symbols: Vec<SymbolHandle>,
            types: Vec<TypeReferenceHandle>,
            expressions: Vec<ExpressionHandle>,
        }

        impl Extend<SymbolHandle> for ApplicationReferences {
            fn extend<References: IntoIterator<Item = SymbolHandle>>(
                &mut self,
                references: References,
            ) {
                self.symbols.extend(references);
            }
        }

        impl TypedReferences for ApplicationReferences {
            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                crate::machine_calls::record_reference(&mut self.types, reference)
            }

            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                crate::machine_calls::record_reference(&mut self.expressions, expression)
            }
        }
    }
    pub(crate) mod product_operands {
        use crate::frozen_program::expression_table;
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
            if !matches!(
                application.role?,
                ProductOperandRole::BoundarySubject | ProductOperandRole::Provider
            ) {
                return None;
            }
            Some((
                program.symbols.name(application.declaration),
                program
                    .data_type_parameters
                    .span(application.declaration_parameters)?,
            ))
        }

        fn occurrence<'a>(
            program: &dyn crate::CheckingInput,
            application: &ProductOperandApplication,
            occurrences: &'a [ProductSelectionOccurrence],
        ) -> Option<&'a ProductSelectionOccurrence> {
            let mut matching = occurrences.iter().filter(|occurrence| {
                occurrence.machine == application.owner
                    && occurrence.request.operands.iter().any(|operand| {
                        program
                            .typed_program()
                            .same_product_operand_source(operand, &application.operand)
                    })
            });
            let first = matching.next()?;
            if matching
                .any(|other| other.state != first.state || other.expression != first.expression)
            {
                return None;
            }
            Some(first)
        }

        pub(crate) fn validate_parameter_correspondence(
            program: &dyn crate::CheckingInput,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let typed = program.typed_program();
            let occurrences =
                typed_trees::product_operand::collect_product_selection_occurrences(typed);
            for handle in typed.live_product_operand_applications() {
                let application = typed.product_operand_applications.get(handle);
                let Some((name, parameters)) = parameters(program, application) else {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "Product nominal application lost its declared telescope: `{}` ({:?})",
                            typed.symbols.name(application.declaration),
                            application.role,
                        ))
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
                if let Err(error) = super::validate_arguments(
                    program,
                    name,
                    parameters,
                    &arguments.arguments,
                    super::ArgumentScope::Caller(state),
                ) {
                    diagnostics.push(error.with_source_span(application.operand.source_span));
                }
                let source = expression_table(typed)
                    .static_machine_application(application.supplying_application);
                if let Err(error) =
                    crate::conformance_applications::validate_conformance_lifetime_arguments(
                        program,
                        &application.argument,
                        source.lexical_owner,
                    )
                {
                    diagnostics.push(error.with_source_span(application.operand.source_span));
                }
            }
        }

        pub(crate) fn validate_source_recipes(
            program: &dyn crate::CheckingInput,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            let typed = program.typed_program();
            let occurrences =
                typed_trees::product_operand::collect_product_selection_occurrences(typed);
            for handle in typed.live_product_operand_applications() {
                let application = typed.product_operand_applications.get(handle);
                let Some((_, parameters)) = parameters(program, application) else {
                    continue;
                };
                let Some(arguments) = application.argument.application.as_ref() else {
                    continue;
                };
                if application.environment.is_none()
                    && let Some(context) = occurrence(program, application, &occurrences)
                {
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
                    let source = expression_table(typed)
                        .static_machine_application(application.supplying_application);
                    for (parameter, argument) in parameters.iter().zip(arguments.arguments.iter()) {
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
                            "Product const recipe requires its exact original source state or an independently closed expression",
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
    pub(crate) mod requirement_predicates {
        use crate::frozen_program::expression_table;
        use crate::proof_contracts::contract_entailment::static_applications::generic_tuple_requirements;
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, StaticMachineArgument};
        use typed_trees::references::StaticArgumentCall;
        use typed_trees::{machine::Machine, state::State, trait_definition::AppliedTraitScope};

        pub(crate) struct CallScopes<'program> {
            entries: Vec<(ExpressionHandle, &'program Machine, &'program State)>,
        }

        impl<'program> CallScopes<'program> {
            pub(crate) fn build(program: &'program dyn crate::CheckingInput) -> Self {
                if !program.traits().iter().any(|owner| {
            program
                .trait_machine_signatures(owner)
                .iter()
                .any(|signature| !generic_tuple_requirements(program, signature).is_empty())
        }) && !expression_table(program).iter_expressions().any(|(_, expression)| {
                matches!(
                    expression,
                    typed_trees::expression::ExpressionNode::Call(call)
                        if call.requirement_application.is_valid()
                            || matches!(expression_table(program).build_selection_request(call.build_selection),
                                Some(typed_trees::expression::BuildSelectionRequest::ProviderReadiness(_)))
                )
            })
        {
            return Self {
                entries: Vec::new(),
            };
        }
                let mut entries = Vec::new();
                let mut expressions = Vec::new();
                for machine in program.machines() {
                    for state in program.machine_states(machine) {
                        expressions.clear();
                        for statement in program.statement_table.statements(state.statement_nodes) {
                            for (_, root) in program.tables.expression_roots(statement) {
                                crate::queries::expression_traversal::collect_expression_tree(
                                    program,
                                    root,
                                    &mut expressions,
                                );
                            }
                        }
                        entries.extend(expressions.iter().filter_map(|expression| {
                            expression_table(program).call_node(*expression)?;
                            Some((*expression, machine, state))
                        }));
                    }
                }
                entries.sort_by_key(|(expression, _, _)| {
                    (expression.arena_index(), expression.generation())
                });
                Self { entries }
            }

            pub(crate) fn context(
                &self,
                expression: ExpressionHandle,
            ) -> Option<(&'program Machine, &'program State)> {
                let identity = (expression.arena_index(), expression.generation());
                let first = self.entries.partition_point(|(candidate, _, _)| {
                    (candidate.arena_index(), candidate.generation()) < identity
                });
                let (_, machine, state) = self
                    .entries
                    .get(first)
                    .filter(|entry| entry.0 == expression)?;
                if self
                    .entries
                    .get(first + 1)
                    .is_some_and(|entry| entry.0 == expression)
                {
                    return None;
                }
                Some((*machine, *state))
            }
        }

        pub(crate) fn validate(
            program: &dyn crate::CheckingInput,
            call: StaticArgumentCall<'_>,
            context: Option<(&Machine, &State)>,
            arguments: &[StaticMachineArgument],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let (owner, source, declared_scope) = match super::source_requirement(program, call) {
                Ok(Some(source)) => source,
                Ok(None) => return,
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(reason));
                    return;
                }
            };
            let target = source.symbol;
            if generic_tuple_requirements(program, source).is_empty() {
                return;
            }
            let executable = match call {
                StaticArgumentCall::Expression(_, call) => call.target_symbol,
                StaticArgumentCall::Statement(call) => call.target_symbol,
            };
            let scope = if let Some(scope) = declared_scope {
                Some((scope, false))
            } else if let Some(application) = program.closed_requirement_application(executable) {
                AppliedTraitScope::new(
                    program,
                    owner,
                    typed_trees::trait_definition::TraitApplicationArguments {
                        lexical_owner: call.lexical_owner(program),
                        enclosing_data_applications: &[],
                        arguments: application.trait_arguments,
                        lifetimes: &[],
                    },
                )
                .map(|scope| (scope, false))
            } else {
                context
                    .and_then(|(machine, state)| {
                        let (receiver, operands) = match call {
                    StaticArgumentCall::Expression(_, call) => (
                        crate::machine_calls::calls::dynamic_requirement::expression_trait_receiver(
                            program,
                            call.receiver,
                        ),
                        expression_table(program).expression_handles(call.arguments),
                    ),
                    StaticArgumentCall::Statement(call) => (
                        call.receiver_symbol,
                        program.statement_table.expression_handles(call.arguments),
                    ),
                };
                        crate::machine_calls::calls::dynamic_requirement::dynamic_selector_call(
                            program,
                            machine,
                            Some(state),
                            receiver,
                            executable,
                            operands,
                        )
                        .ok()
                        .flatten()
                        .filter(|dynamic| dynamic.signature.symbol == target)
                        .map(|dynamic| (dynamic.application, true))
                    })
                    .or_else(|| {
                        program
                            .trait_type_parameters(owner)
                            .is_empty()
                            .then(|| (AppliedTraitScope::abstract_scope(owner), false))
                    })
            };
            let (scope, resolved_dynamically) = match scope {
                Some((scope, dynamic)) => (Some(scope), dynamic),
                None => (None, false),
            };
            if resolved_dynamically
                && let typed_trees::finite_family::FamilyProbe::Finite { arity, tuples } =
                    program.finite_signature_family(source)
            {
                let applied = arguments
                    .iter()
                    .map(|argument| program.static_const_argument_identity(argument))
                    .collect::<Option<Vec<_>>>();
                let closed_non_member = applied.is_some_and(|identities| {
                    identities.len() == arity
                        && !tuples
                            .iter()
                            .any(|tuple| tuple.identities.as_ref() == identities)
                });
                if closed_non_member {
                    return;
                }
            }
            let signature = scope
                .as_ref()
                .and_then(|scope| program.applied_callable_signature(scope, target));
            let Some(signature) = signature else {
                return;
            };
            validate_tuple_requirements(
                program,
                owner,
                signature,
                arguments,
                call.lexical_owner(program),
                context.map(|(_, state)| state),
                true,
                diagnostics,
            );
        }

        pub(crate) fn validate_tuple_requirements(
            program: &dyn crate::CheckingInput,
            owner: &typed_trees::trait_definition::TraitDefinition,
            source: &typed_trees::signature::StateSignature,
            arguments: &[StaticMachineArgument],
            lexical_owner: SymbolHandle,
            caller_state: Option<&State>,
            entry_proof_follows: bool,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            use crate::StrictArithmeticImplicationJudgment;
            use crate::proof_contracts::contract_entailment::static_applications::static_application_requirement_judgment;
            let judgment = static_application_requirement_judgment(
                program,
                source,
                arguments,
                lexical_owner,
                caller_state,
                &[],
            );
            match judgment {
        StrictArithmeticImplicationJudgment::Proven => {}
        StrictArithmeticImplicationJudgment::Refuted => diagnostics.push(Diagnostic::error(format!(
            "static application of requirement `{}::{}` violates its declared requires facts: the complete tuple is structurally false", owner.name, source.name,
        ))),
        StrictArithmeticImplicationJudgment::Unknown if entry_proof_follows => {}
        StrictArithmeticImplicationJudgment::Unknown => diagnostics.push(Diagnostic::error(format!(
            "cannot prove declared requires facts for static application of requirement `{}::{}` under its complete applied telescope", owner.name, source.name,
        ))),
            }
        }
    }
    pub(crate) use invocation_parameters::invocation_parameters;

    pub fn validate_source_const_recipes(
        program: &dyn CheckingInput,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let scopes = requirement_predicates::CallScopes::build(program);
        let expression_states = expression_call_states(program);
        let mut validate = |call, context| {
            let Some(state) = call_state(call, context, &expression_states) else {
                return;
            };
            let Some(caller) = program.machine_holding_state(state.symbol) else {
                return;
            };
            let parameters = match source_telescope(program, call) {
                Ok(Some((_, parameters))) => parameters,
                Ok(None) => return,
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(reason));
                    return;
                }
            };
            let arguments = call.arguments(program);
            crate::value_custody::const_expressions::validate_argument_recipes(
                program,
                caller,
                state,
                &parameters,
                arguments,
                &mut diagnostics,
                formation_obligations,
            );
        };
        for (handle, expression) in expression_table(program).iter_expressions() {
            if let ExpressionNode::Call(call) = expression {
                validate(
                    StaticArgumentCall::Expression(handle, call),
                    scopes.context(handle),
                );
            }
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(call) = statement.as_call() {
                        validate(StaticArgumentCall::Statement(call), Some((machine, state)));
                    }
                }
            }
        }
        product_operands::validate_source_recipes(program, &mut diagnostics, formation_obligations);
        crate::program_validation::finish_strict_diagnostics(diagnostics)
    }

    pub fn validate_authored_static_arguments(
        program: &dyn CheckingInput,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let symbols = crate::TopLevelSymbols::build(program, &mut diagnostics);
        for argument in program.typed_program().designated_static_type_arguments() {
            let Some(owner) = program.machine(argument.owner) else {
                diagnostics.push(
                    Diagnostic::error("designated static argument lost its exact source owner")
                        .with_source_span(argument.source_span),
                );
                continue;
            };
            crate::validate_static_type_argument(
                program,
                crate::StaticTypeArgumentOwner::Machine(owner),
                argument.reference,
                &symbols,
                &mut diagnostics,
                formation_obligations,
            );
        }
        let scopes = requirement_predicates::CallScopes::build(program);
        let expression_states = expression_call_states(program);
        for (handle, expression) in expression_table(program).iter_expressions() {
            if let ExpressionNode::Call(call) = expression {
                validate_call(
                    program,
                    StaticArgumentCall::Expression(handle, call),
                    scopes.context(handle),
                    &expression_states,
                    &symbols,
                    &mut diagnostics,
                    formation_obligations,
                );
            }
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(call) = statement.as_call() {
                        validate_call(
                            program,
                            StaticArgumentCall::Statement(call),
                            Some((machine, state)),
                            &expression_states,
                            &symbols,
                            &mut diagnostics,
                            formation_obligations,
                        );
                    }
                }
            }
        }
        product_operands::validate_parameter_correspondence(program, &mut diagnostics);
        for application in program.closed_requirement_applications() {
            let source =
                expression_table(program).static_machine_application(application.application);
            let arguments = expression_table(program)
                .static_machine_application_arguments(application.application);
            let Some((owner, requirement)) = program.trait_machine_signature(application.template)
            else {
                diagnostics.push(Diagnostic::error(
                    "closed requirement lost its authored declaration",
                ));
                continue;
            };
            let Some(scope) = application.trait_scope.scope(program) else {
                diagnostics.push(Diagnostic::error(
                    "closed requirement lost its retained source scope",
                ));
                continue;
            };
            if scope.declaration.symbol != owner.symbol {
                diagnostics.push(Diagnostic::error(
                    "closed requirement source scope selects a different declaring trait",
                ));
                continue;
            }
            if scope.application_identity(program) != Some(application.trait_application_identity)
                || scope.portable_application_identity(program)
                    != Some(application.portable_trait_application_identity)
            {
                diagnostics.push(Diagnostic::error("closed requirement source scope differs from its complete retained application identity"));
                continue;
            }
            if let Err(reason) = program.require_receiving_trait_operands(&scope) {
                diagnostics.push(Diagnostic::error(format!(
                    "closed requirement source operands are not closed: {reason}"
                )));
                continue;
            }
            let Some(signature) =
                program.closed_requirement_admission_signature(application.signature.symbol)
            else {
                diagnostics.push(Diagnostic::error(
                    "closed requirement lost its produced admission signature",
                ));
                continue;
            };
            if program.state_signature_type_parameters(&signature).len() != arguments.len()
                || application.local_arguments.len() != arguments.len()
                || source.binding_declaration != application.template
            {
                diagnostics.push(Diagnostic::error(
                    "closed requirement admission tuple differs from its exact source declaration",
                ));
                continue;
            }
            if let Err(error) = validate_arguments(
                program,
                requirement.name.as_str(),
                program.state_signature_type_parameters(&signature),
                arguments,
                ArgumentScope::ClosedRequirement(application.local_arguments),
            ) {
                diagnostics.push(error);
            }
            requirement_predicates::validate_tuple_requirements(
                program,
                owner,
                &signature,
                arguments,
                source.lexical_owner,
                None,
                false,
                &mut diagnostics,
            );
        }
        crate::program_validation::finish_diagnostics(diagnostics)
    }

    fn source_telescope<'program>(
        program: &'program dyn CheckingInput,
        call: StaticArgumentCall<'_>,
    ) -> Result<
        Option<(
            &'program str,
            std::borrow::Cow<'program, [typed_trees::data::TypeParameter]>,
        )>,
        String,
    > {
        let target = call.registration_operation(program);
        if let Some(machine) = program
            .machine_by_symbol(target)
            .or_else(|| program.machine_holding_state(target))
        {
            return Ok(Some((
                machine.name.as_str(),
                std::borrow::Cow::Owned(program.machine_callable_parameters(machine)),
            )));
        }
        let Some((_, source, scope)) = source_requirement(program, call)? else {
            return Ok(None);
        };
        let requirement = match scope {
        Some(scope) => program
            .applied_callable_signature(&scope, source.symbol)
            .or_else(|| {
                (scope.substitutions.is_empty() && scope.declaration.lifetime_parameters.is_empty())
                    .then_some(source)
            })
            .ok_or_else(|| {
                format!(
                    "source application of requirement `{}` lost its exact outer trait telescope",
                    source.name,
                )
            })?,
        None => source,
    };
        Ok(Some((
            requirement.name.as_str(),
            std::borrow::Cow::Borrowed(
                program.requirement_invocation_contract_parameters(requirement),
            ),
        )))
    }

    fn source_requirement<'program>(
        program: &'program dyn CheckingInput,
        call: StaticArgumentCall<'_>,
    ) -> Result<
        Option<(
            &'program typed_trees::trait_definition::TraitDefinition,
            &'program typed_trees::signature::StateSignature,
            Option<typed_trees::trait_definition::AppliedTraitScope<'program>>,
        )>,
        String,
    > {
        let target = call.registration_operation(program);
        if let Some((owner, signature)) = program.trait_machine_signature(target) {
            return Ok(Some((owner, signature, None)));
        }
        let Some(machine) = program.machine_by_symbol(call.lexical_owner(program)) else {
            return Ok(None);
        };
        let Some(requirement) = crate::machine_calls::calls::named_conformance_target_requirement(
            program, machine, target,
        )?
        else {
            return Ok(None);
        };
        Ok(Some((
            requirement.application.declaration,
            requirement.signature,
            Some(requirement.application),
        )))
    }

    fn expression_call_states(
        program: &dyn CheckingInput,
    ) -> Vec<(typed_trees::expression::ExpressionHandle, &State)> {
        let mut states = Vec::new();
        let mut expressions = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    expressions.clear();
                    for (_, root) in program.tables.expression_roots(statement) {
                        crate::queries::expression_traversal::collect_expression_tree(
                            program,
                            root,
                            &mut expressions,
                        );
                    }
                    states.extend(expressions.iter().filter_map(|expression| {
                        expression_table(program).call_node(*expression)?;
                        Some((*expression, state))
                    }));
                }
            }
        }
        states.sort_by_key(|(expression, _)| (expression.arena_index(), expression.generation()));
        states
    }

    fn expression_state<'program>(
        states: &'program [(typed_trees::expression::ExpressionHandle, &'program State)],
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<&'program State> {
        let identity = (expression.arena_index(), expression.generation());
        let first = states.partition_point(|(candidate, _)| {
            (candidate.arena_index(), candidate.generation()) < identity
        });
        states.get(first).and_then(|(candidate, state)| {
            ((candidate.arena_index(), candidate.generation()) == identity).then_some(*state)
        })
    }

    fn call_state<'program>(
        call: StaticArgumentCall<'_>,
        context: Option<(&'program typed_trees::machine::Machine, &'program State)>,
        expression_states: &'program [(
            typed_trees::expression::ExpressionHandle,
            &'program State,
        )],
    ) -> Option<&'program State> {
        if let Some((_, state)) = context {
            return Some(state);
        }
        let StaticArgumentCall::Expression(handle, _) = call else {
            return None;
        };
        expression_state(expression_states, handle)
    }

    fn validate_call(
        program: &dyn CheckingInput,
        call: StaticArgumentCall<'_>,
        context: Option<(&typed_trees::machine::Machine, &typed_trees::state::State)>,
        expression_states: &[(typed_trees::expression::ExpressionHandle, &State)],
        symbols: &crate::TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        if let Err(diagnostic) =
            crate::compiler_operations::validate_declaration_operands(program, call)
        {
            diagnostics.push(diagnostic);
            return;
        }
        let arguments = call.arguments(program);
        if let StaticArgumentCall::Expression(_, expression) = call
            && let Some(typed_trees::expression::BuildSelectionRequest::ProviderReadiness(request)) =
                expression_table(program).build_selection_request(expression.build_selection)
        {
            match context {
                Some((machine, _)) => {
                    crate::validate_static_type_argument(
                        program,
                        crate::StaticTypeArgumentOwner::Machine(machine),
                        request.requirement,
                        symbols,
                        diagnostics,
                        formation_obligations,
                    );
                    if let Err(reason) = program
                        .build_provider_requirement_scope(request.requirement, machine.symbol)
                    {
                        diagnostics
                            .push(Diagnostic::error(reason).with_source_span(request.source_span));
                    }
                }
                None => diagnostics.push(Diagnostic::error(
                    "provider readiness has no retained caller machine",
                )),
            }
        }
        requirement_predicates::validate(program, call, context, arguments, diagnostics);
        if let StaticArgumentCall::Expression(_, applied) = call
            && applied.requirement_application.is_valid()
        {
            match context {
                Some((machine, _)) => crate::validate_static_type_argument(
                    program,
                    crate::StaticTypeArgumentOwner::Machine(machine),
                    applied.requirement_application,
                    symbols,
                    diagnostics,
                    formation_obligations,
                ),
                None => diagnostics.push(Diagnostic::error(
                    "applied requirement call has no retained caller machine",
                )),
            }
        }
        if arguments.is_empty() {
            return;
        }
        let telescope = match source_telescope(program, call) {
            Ok(telescope) => telescope,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(reason));
                return;
            }
        };
        if let Some((name, parameters)) = telescope
            && !parameters.is_empty()
        {
            let state = call_state(call, context, expression_states);
            if let Err(error) = validate_arguments(
                program,
                name,
                &parameters,
                arguments,
                ArgumentScope::Caller(state),
            ) {
                diagnostics.push(error);
            }
            if let Some(state) = state
                && let Some(machine) = program.machine_holding_state(state.symbol)
            {
                crate::value_custody::const_expressions::validate_argument_recipes(
                    program,
                    machine,
                    state,
                    &parameters,
                    arguments,
                    diagnostics,
                    formation_obligations,
                );
            }
        }
        let owner = call.lexical_owner(program);
        let caller = crate::StaticTypeArgumentOwner::from_lexical_owner(program, owner);
        validate_types(
            program,
            caller,
            call,
            arguments,
            symbols,
            diagnostics,
            formation_obligations,
        );
    }

    fn validate_types(
        program: &dyn CheckingInput,
        caller: Option<crate::StaticTypeArgumentOwner<'_>>,
        source_call: StaticArgumentCall<'_>,
        arguments: &[StaticMachineArgument],
        symbols: &crate::TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        for argument in arguments {
            if argument.names_implementation(&program.symbols) {
                if let Err(error) =
                    crate::conformance_applications::validate_conformance_lifetime_arguments(
                        program,
                        argument,
                        source_call.lexical_owner(program),
                    )
                {
                    diagnostics.push(error);
                }
            } else if argument.type_reference.is_valid()
                && const_expression_argument(program, argument).is_none()
                && !is_runtime_value_expression(program, argument)
            {
                if let Some(caller) = caller {
                    crate::validate_static_type_argument(
                        program,
                        caller,
                        argument.type_reference,
                        symbols,
                        diagnostics,
                        formation_obligations,
                    );
                } else {
                    let owner = source_call.lexical_owner(program);
                    let owner_label = if owner.is_valid() {
                        format!(
                            "{:?} `{}`",
                            program.symbols.get(owner).kind,
                            program.symbols.name(owner)
                        )
                    } else {
                        "<missing>".to_owned()
                    };
                    let target = source_call.registration_operation(program);
                    let target = if target.is_valid() {
                        program.symbols.name(target)
                    } else {
                        "<unresolved>"
                    };
                    let mut diagnostic = Diagnostic::error(format!(
                        "structural static type argument has no retained caller type/lifetime context for `{target}`; lexical owner {owner_label}",
                    ));
                    if let StaticArgumentCall::Expression(handle, _) = source_call {
                        diagnostic = diagnostic
                            .with_source_span(expression_table(program).source_span(handle));
                    }
                    diagnostics.push(diagnostic);
                }
            }
            if let Some(application) = &argument.application {
                validate_types(
                    program,
                    caller,
                    source_call,
                    &application.arguments,
                    symbols,
                    diagnostics,
                    formation_obligations,
                );
            }
        }
    }

    pub fn spelling(
        program: &dyn CheckingInput,
        argument: &StaticMachineArgument,
    ) -> Option<String> {
        typed_trees::typed_trees::static_const_argument_spelling(program.typed_program(), argument)
    }

    pub fn forwarded_type(
        program: &dyn CheckingInput,
        argument: &StaticMachineArgument,
    ) -> TypeReferenceHandle {
        if !argument.symbol.is_valid() {
            return TypeReferenceHandle::invalid();
        }
        match program
            .data_type_parameter_by_symbol(argument.symbol)
            .map(|parameter| &parameter.kind)
        {
            Some(
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference },
            ) => *type_reference,
            _ => TypeReferenceHandle::invalid(),
        }
    }

    pub fn is_runtime_value_expression(
        program: &dyn CheckingInput,
        argument: &StaticMachineArgument,
    ) -> bool {
        let TypeReferenceNode::ConstExpression(expression) = program
            .type_reference_table
            .type_reference(argument.type_reference)
        else {
            return false;
        };
        expression_table(program).expression_is_valid(*expression)
    }

    pub fn is_runtime_value_subject(
        program: &dyn CheckingInput,
        argument: &StaticMachineArgument,
    ) -> bool {
        !argument.type_reference.is_valid()
            && argument.application.is_none()
            && argument.evidence_projection.is_none()
            && argument.const_literal.is_none()
            && argument.path.len() == 1
            && (!argument.symbol.is_valid()
                || matches!(
                    program.symbols.get(argument.symbol).kind,
                    SymbolKind::Local | SymbolKind::Parameter
                ))
    }

    pub(crate) fn validate_structural_type_operand(
        program: &dyn CheckingInput,
        argument: &StaticMachineArgument,
    ) -> Result<(), &'static str> {
        if !argument.type_reference.is_valid()
            || !program
                .type_reference_table
                .contains_type_reference(argument.type_reference)
            || !argument.path.is_empty()
            || argument.symbol.is_valid()
            || argument.const_literal.is_some()
            || argument.application.is_some()
            || argument.evidence_projection.is_some()
        {
            return Err("structural static type argument has invalid or mixed argument custody");
        }
        Ok(())
    }

    #[derive(Clone, Copy)]
    pub(crate) enum ArgumentScope<'scope> {
        Caller(Option<&'scope State>),
        ClosedRequirement(&'scope [typed_trees::typed_trees::RequirementLocalArgument]),
    }

    fn validate_arguments(
        program: &dyn CheckingInput,
        name: &str,
        parameters: &[typed_trees::data::TypeParameter],
        arguments: &[StaticMachineArgument],
        scope: ArgumentScope<'_>,
    ) -> Result<(), Diagnostic> {
        let caller = match scope {
            ArgumentScope::Caller(Some(state)) => program.machine_holding_state(state.symbol),
            _ => None,
        };
        let has_types = parameters
            .iter()
            .any(|parameter| matches!(parameter.kind, TypeParameterKind::Type));
        let has_values = parameters
            .iter()
            .any(|parameter| parameter.kind.is_const_or_value());
        for (ordinal, argument) in arguments.iter().enumerate() {
            let parameter = parameters.get(ordinal);
            if let ArgumentScope::ClosedRequirement(coordinates) = &scope
                && let Some(typed_trees::typed_trees::RequirementLocalArgument::RuntimeValue {
                    carrier,
                }) = coordinates.get(ordinal)
            {
                let Some(typed_trees::data::TypeParameter {
                    kind: TypeParameterKind::Value { type_reference },
                    ..
                }) = parameter
                else {
                    return Err(Diagnostic::error(
                        "closed requirement has a runtime carrier outside a Value slot",
                    ));
                };
                if !carrier.is_valid()
                    || !program
                        .type_reference_table
                        .contains_type_reference(*carrier)
                    || !type_reference.is_valid()
                    || !program
                        .type_reference_table
                        .contains_type_reference(*type_reference)
                {
                    return Err(Diagnostic::error(
                        "closed requirement runtime carrier differs from its declared Value carrier",
                    ));
                }
                let actual = program.normalized_type_identity(*carrier);
                let required = program.normalized_type_identity(*type_reference);
                if !actual.is_valid() || !required.is_valid() || actual != required {
                    return Err(Diagnostic::error(
                        "closed requirement runtime carrier differs from its declared Value carrier",
                    ));
                }
                continue;
            }
            // A retained source type describes an implementation operand; it
            // does not change that operand into a stored data type.
            if parameter
                .is_some_and(|parameter| parameter.kind == TypeParameterKind::Implementation)
            {
                if !argument.names_implementation(&program.symbols) {
                    return Err(Diagnostic::error(format!(
                        "machine `{name}` static argument {} requires a named implementation",
                        ordinal + 1,
                    )));
                }
                continue;
            }
            if argument.names_implementation(&program.symbols) {
                return Err(Diagnostic::error(format!(
                    "machine `{name}` static argument {} requires its declared binder kind, not an implementation",
                    ordinal + 1,
                )));
            }
            if let Some(parameter) = parameter
                && matches!(parameter.kind, TypeParameterKind::Const { .. })
                && program
                    .data_type_parameter_by_symbol(argument.symbol)
                    .is_some_and(|source| matches!(source.kind, TypeParameterKind::Value { .. }))
                && !caller.is_some_and(|caller| {
                    program
                        .typed_program()
                        .finite_value_binder_in_scope(caller.symbol, argument.symbol)
                })
            {
                return Err(Diagnostic::error(format!(
                    "const parameter `{}` of `{name}` cannot accept runtime-capable Value binder `{}`",
                    parameter.name,
                    argument.display_name(),
                )));
            }
            let named_type = argument.symbol.is_valid()
                && (matches!(
                    program.symbols.get(argument.symbol).kind,
                    SymbolKind::BuiltinType | SymbolKind::Data
                ) || program
                    .data_type_parameter_by_symbol(argument.symbol)
                    .is_some_and(|parameter| matches!(parameter.kind, TypeParameterKind::Type)));
            if const_expression_argument(program, argument).is_some()
                && !parameter.is_some_and(|parameter| parameter.kind.is_const_or_value())
            {
                return Err(Diagnostic::error(format!(
                    "machine `{}` static argument {} requires its declared binder kind, not a const value",
                    name,
                    ordinal + 1
                )));
            }
            if parameter.is_some_and(|parameter| {
                parameter.kind.is_const_or_value()
                    && const_expression_argument(program, argument).is_some_and(|expression| {
                        crate::value_custody::const_expressions::has_carrier(
                            program,
                            expression,
                            &parameter.kind,
                            program
                                .type_reference_table
                                .operand_lifetime_owner(argument.type_reference),
                        )
                    })
            }) {
                continue;
            }
            let value_parameter = parameter
                .is_some_and(|parameter| matches!(parameter.kind, TypeParameterKind::Value { .. }));
            let runtime_expression = is_runtime_value_expression(program, argument);
            let state = match scope {
                ArgumentScope::Caller(state) => state,
                ArgumentScope::ClosedRequirement(_) => None,
            };
            let runtime_field = state.is_some_and(|state| {
                argument
                    .evidence_projection
                    .as_ref()
                    .is_some_and(|projection| {
                        crate::runtime_field_subject(
                            program,
                            state,
                            &projection.term,
                            &projection.member,
                        )
                        .is_some()
                    })
            });
            let admitted_runtime = value_parameter && (runtime_expression || runtime_field);
            if ((argument.type_reference.is_valid() && !admitted_runtime)
                || (named_type && has_types))
                && !parameters
                    .get(ordinal)
                    .is_some_and(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
            {
                return Err(Diagnostic::error(format!(
                    "machine `{}` static argument {} requires its declared binder kind, not a type argument",
                    name,
                    ordinal + 1
                )));
            }
            if admitted_runtime {
                continue;
            }
            if value_parameter && let Some(projection) = &argument.evidence_projection {
                return Err(Diagnostic::error(format!(
                    "value parameter of machine `{name}` received `{}.{}`, which is not a field of a local or parameter in this state",
                    projection.term, projection.member
                )));
            }
            if argument.type_reference.is_valid() {
                validate_structural_type_operand(program, argument).map_err(Diagnostic::error)?;
                continue;
            }
            if named_type
                && argument.application.is_none()
                && program.symbols.get(argument.symbol).kind == SymbolKind::Data
                && let Some(definition) = program.data_definition_by_symbol(argument.symbol)
                && (!definition.type_parameters.is_empty()
                    || !definition.lifetime_parameters.is_empty())
            {
                return Err(Diagnostic::error(format!(
                    "static type argument `{}` selects generic data without its complete type/lifetime arguments",
                    definition.name,
                )));
            }
            let declaration = program.const_declarations().iter().find(|declaration| {
                argument.symbol.is_valid() && declaration.symbol == argument.symbol
            });
            let forwarded = forwarded_type(program, argument);
            let forwarded_type = forwarded.is_valid().then_some(forwarded);
            if declaration.is_none()
                && forwarded_type.is_none()
                && spelling(program, argument).is_none()
            {
                let type_shaped = argument.type_reference.is_valid()
                    || argument.symbol.is_valid()
                        && matches!(
                            program.symbols.get(argument.symbol).kind,
                            SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
                        );
                if has_values && !has_types && type_shaped {
                    return Err(Diagnostic::error(format!(
                        "machine `{}` requires a const value, not type `{}`",
                        name,
                        argument.display_name()
                    )));
                }
                if let Some(parameter) = parameter
                    && matches!(parameter.kind, TypeParameterKind::Value { .. })
                    && argument.application.is_none()
                    && argument.evidence_projection.is_none()
                    && !type_shaped
                {
                    if is_runtime_value_subject(program, argument) {
                        continue;
                    }
                    let parameter_name = parameter.name.as_str();
                    return Err(Diagnostic::error(format!(
                        "value parameter `{parameter_name}` of machine `{}` received `{}`, which \
                     is neither a static const value nor an ordinary runtime value subject",
                        name,
                        argument.display_name()
                    )));
                }
                continue;
            }
            let Some(parameter) = parameter else {
                return Err(Diagnostic::error(format!(
                    "machine `{}` has no const parameter for extra argument `{}`",
                    name,
                    argument.display_name()
                )));
            };
            let parameter_name = &parameter.name;
            let required = match &parameter.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => type_reference,
                _ => {
                    return Err(Diagnostic::error(format!(
                        "machine `{}` static argument {} requires its declared binder kind, not a const value",
                        name,
                        ordinal + 1
                    )));
                }
            };
            if argument.type_reference.is_valid()
                || argument.application.is_some()
                || argument.evidence_projection.is_some()
            {
                return Err(Diagnostic::error(format!(
                    "const argument `{}` must select a closed value or a const binder",
                    argument.display_name()
                )));
            }
            if let Some(actual) = declaration
                .map(|declaration| declaration.declared_type)
                .or(forwarded_type)
                && program.normalized_type_identity(actual)
                    != program.normalized_type_identity(*required)
            {
                return Err(Diagnostic::error(format!(
                    "const parameter `{parameter_name}` of `{}` requires `{}`, but `{}` declares `{}`",
                    name,
                    program.display_type_reference(*required),
                    argument.display_name(),
                    program.display_type_reference(actual)
                )));
            }
            if let Some(landing) = argument
                .const_literal
                .as_ref()
                .and_then(|literal| literal.landing())
            {
                let actual = match landing.landed_type {
                    LandedIntegerType::I8 => PrimitiveType::I8,
                    LandedIntegerType::I16 => PrimitiveType::I16,
                    LandedIntegerType::I32 => PrimitiveType::I32,
                    LandedIntegerType::I64 => PrimitiveType::I64,
                    LandedIntegerType::U8 => PrimitiveType::U8,
                    LandedIntegerType::U16 => PrimitiveType::U16,
                    LandedIntegerType::U32 => PrimitiveType::U32,
                    LandedIntegerType::U64 => PrimitiveType::U64,
                    LandedIntegerType::Addr => PrimitiveType::Addr,
                };
                if program.type_reference_table.primitive_type(*required) != Some(actual) {
                    return Err(Diagnostic::error(format!(
                        "const parameter `{parameter_name}` of `{}` requires `{}`, but its literal already landed as `{}`",
                        name,
                        program.display_type_reference(*required),
                        landing.landed_type.name(),
                    )));
                }
            }
            if declaration.is_some() && spelling(program, argument).is_none() {
                return Err(Diagnostic::error(format!(
                    "const argument `{}` has no eligible canonical value",
                    argument.display_name()
                )));
            }
        }
        Ok(())
    }

    fn const_expression_argument(
        program: &dyn CheckingInput,
        argument: &StaticMachineArgument,
    ) -> Option<ExpressionHandle> {
        if !argument.type_reference.is_valid() {
            return None;
        }
        match program
            .type_reference_table
            .type_reference(argument.type_reference)
        {
            TypeReferenceNode::ConstExpression(expression) => Some(*expression),
            _ => None,
        }
    }
}

pub mod static_bindings {
    pub(crate) mod accessor_primitives {
        use diagnostics::Diagnostic;
        use typed_trees::static_applications::{
            StaticMachineApplication, StaticMachineBinding, StaticMachineBindingValue,
        };
        use typed_trees::typed_trees::AccessorPrimitive;

        pub(crate) fn validate(
            program: &dyn crate::CheckingInput,
            application: &StaticMachineApplication,
            bindings: &[StaticMachineBinding],
            selected_targets: impl Iterator<Item = symbols::SymbolHandle>,
        ) -> Result<(), Diagnostic> {
            let Some(operation) = AccessorPrimitive::from_declaration(
                program.typed_program(),
                application.binding_declaration,
            )?
            else {
                return Ok(());
            };
            let machine = program
                .machine(application.binding_declaration)
                .ok_or_else(|| {
                    Diagnostic::error("placed primitive application lost its declaration")
                })?;
            let parameters = program.machine_type_parameters(machine);
            let selected_type = |parameter| {
                bindings.iter().find_map(|binding| match &binding.value {
                    StaticMachineBindingValue::Type(reference)
                        if binding.parameter == parameter =>
                    {
                        Some(*reference)
                    }
                    _ => None,
                })
            };
            let (Some(accessor), Some(value)) = (
                selected_type(parameters[0].symbol),
                selected_type(parameters[1].symbol),
            ) else {
                return Err(Diagnostic::error(format!(
                    "accessor {} application requires an exact accessor/value tuple; generic callers must declare an explicit operation requirement",
                    operation.operation(),
                )));
            };
            let target = operation.select_target(program.typed_program(), accessor, value)?;
            for selected in selected_targets {
                if selected != target.state_symbol {
                    return Err(Diagnostic::error(
                        "placed primitive application does not select its exact plan-issued field operation",
                    ));
                }
            }
            let Some((view, field)) = program.access_view_plans.iter().find_map(|view| {
                view.fields
                    .iter()
                    .find(|field| {
                        field
                            .accessor_targets
                            .iter()
                            .any(|candidate| candidate.state_symbol == target.state_symbol)
                    })
                    .map(|field| (view, field))
            }) else {
                return Err(Diagnostic::error(
                    "placed primitive selection lost its field plan",
                ));
            };
            if matches!(
                &field.access,
                access_plans::FieldAccess::Stable {
                    exposure: access_plans::AccessExposure::BindingPrivate,
                    ..
                } | access_plans::FieldAccess::Region {
                    exposure: access_plans::AccessExposure::BindingPrivate,
                    ..
                } | access_plans::FieldAccess::External {
                    exposure: access_plans::AccessExposure::BindingPrivate,
                    ..
                }
            ) && !program
                .symbols
                .same_symbol_source_package(application.lexical_owner, view.policy_symbol)
            {
                return Err(Diagnostic::error(format!(
                    "binding-private accessor `{}` belongs to access policy `{}`'s package",
                    field.field_name, view.policy_name,
                )));
            }
            Ok(())
        }
    }
    pub(crate) mod forwarded_evidence {
        use diagnostics::Diagnostic;
        use typed_trees::{
            static_applications::{
                StaticMachineApplication, StaticMachineBinding, StaticMachineBindingValue,
                StaticMachineLifetimeBinding,
            },
            type_identity::AppliedTypeIdentityScope,
        };

        pub(crate) fn validate(
            program: &dyn crate::CheckingInput,
            machine: &super::StaticDeclaration<'_>,
            application: &StaticMachineApplication,
            bindings: &[StaticMachineBinding],
            lifetimes: &[StaticMachineLifetimeBinding],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let operands = super::index_identity_arguments(bindings);
            let regions = super::lifetime_identity_rows(lifetimes);
            let scope = AppliedTypeIdentityScope {
                declaration: machine.symbol,
                arguments: &operands,
                lifetimes: &regions,
            };
            for required in machine.conformance_bounds {
                let Some(selected) = bindings.iter().find_map(|binding| {
                    if binding.parameter != required.binder {
                        return None;
                    }
                    let StaticMachineBindingValue::Conformance(argument) = &binding.value else {
                        return None;
                    };
                    (argument.symbol.is_valid()
                        && program.symbols.get(argument.symbol).kind
                            == symbols::SymbolKind::ConformanceParameter)
                        .then_some(argument)
                }) else {
                    continue;
                };
                let required_arguments =
                    crate::conformance_applications::applied_bound_argument_identities(
                        program,
                        required,
                        machine.symbol,
                        application.lexical_owner,
                        &[scope],
                    );
                if let Err(reason) = crate::conformance_applications::validate_forwarded_argument(
                    program,
                    required,
                    selected,
                    |actual| {
                        required_arguments
                            .as_ref()
                            .zip(
                                crate::conformance_applications::applied_bound_argument_identities(
                                    program,
                                    actual,
                                    program.symbols.get(actual.binder).parent,
                                    application.lexical_owner,
                                    &[],
                                )
                                .as_ref(),
                            )
                            .is_some_and(|(required, actual)| required == actual)
                    },
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "generic machine `{}` cannot bind implementation `{}`: {reason}",
                        machine.name, required.binder_name,
                    )));
                }
            }
        }
    }

    use crate::frozen_program::expression_table;
    use crate::value_custody::type_references::AppliedValueCarrier;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::data::{TypeParameter, TypeParameterKind};
    use typed_trees::expression::ExpressionNode;
    use typed_trees::machine::GenericConformanceBound;
    use typed_trees::name::Identifier;
    use typed_trees::statement::StatementNode;
    use typed_trees::static_applications::{
        StaticMachineApplication, StaticMachineApplicationHandle, StaticMachineBinding,
        StaticMachineBindingValue, StaticMachineLifetimeBinding,
    };
    use typed_trees::type_identity::IndexIdentityArgument;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) struct StaticDeclaration<'program> {
        symbol: SymbolHandle,
        name: &'program Identifier,
        parameters: &'program [TypeParameter],
        lifetime_parameters: &'program [Identifier],
        conformance_bounds: &'program [GenericConformanceBound],
    }

    pub(crate) struct RetainedCallTarget<'program> {
        application: StaticMachineApplicationHandle,
        executable: SymbolHandle,
        contract: SymbolHandle,
        requirement_application: TypeReferenceHandle,
        dispatch: Option<&'program typed_trees::typed_trees::StaticRequirementDispatch>,
    }

    impl<'program> StaticDeclaration<'program> {
        fn find(program: &'program dyn crate::CheckingInput, symbol: SymbolHandle) -> Option<Self> {
            if let Some(application) = program.closed_requirement_application(symbol) {
                let (_, source) = program.trait_machine_signature(application.template)?;
                return Some(Self {
                    symbol: application.signature.symbol,
                    name: &source.name,
                    parameters: program
                        .requirement_invocation_contract_parameters(application.signature),
                    lifetime_parameters: &source.lifetime_parameters,
                    conformance_bounds: &application.signature.conformance_bounds,
                });
            }
            if let Some(machine) = program.machine_by_symbol(symbol) {
                return Some(Self {
                    symbol: machine.symbol,
                    name: &machine.name,
                    parameters: program.machine_type_parameters(machine),
                    lifetime_parameters: &machine.lifetime_parameters,
                    conformance_bounds: &machine.conformance_bounds,
                });
            }
            if let Some(data) = program.unique_data_definition_by_symbol(symbol) {
                return Some(Self {
                    symbol,
                    name: &data.name,
                    parameters: program.data_type_parameters(data),
                    lifetime_parameters: &data.lifetime_parameters,
                    conformance_bounds: &[],
                });
            }
            if let Some(domain) = program.domain_definition(symbol) {
                return Some(Self {
                    symbol,
                    name: &domain.name,
                    parameters: program.domain_type_parameters(domain),
                    lifetime_parameters: &[],
                    conformance_bounds: &[],
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
                conformance_bounds: &signature.conformance_bounds,
            })
        }
    }

    fn is_declared_source_tuple(
        program: &dyn crate::CheckingInput,
        application: &StaticMachineApplication,
        bindings: &[StaticMachineBinding],
        call: &RetainedCallTarget<'_>,
        application_handle: StaticMachineApplicationHandle,
    ) -> bool {
        let target = call.contract;
        let Some((declaring_trait, signature)) =
            program.trait_machine_signature(application.binding_declaration)
        else {
            return false;
        };
        let bound = program
            .machine_by_symbol(application.lexical_owner)
            .and_then(|machine| {
                crate::machine_calls::calls::named_conformance_target_requirement(
                    program, machine, target,
                )
                .ok()
                .flatten()
            });
        if target != application.binding_declaration
            && bound
                .as_ref()
                .is_none_or(|bound| bound.signature.symbol != application.binding_declaration)
        {
            return false;
        }
        let mut open = bound.is_some()
            || program
                .machine_by_symbol(application.lexical_owner)
                .is_some_and(|owner| {
                    program
                        .typed_program()
                        .callable_body_has_open_parameters(owner)
                });
        let scope = if call.requirement_application.is_valid() {
            program
                .applied_requirement_scope(application.lexical_owner, call.requirement_application)
                .and_then(|scope| scope.declaring_scope(program, declaring_trait.symbol))
        } else if let Some(dispatch) = call.dispatch {
            let selected = program.selected_conformance_applications(
                application.lexical_owner,
                dispatch.selection_application,
                dispatch.evidence_parameter,
                dispatch.requirement,
                dispatch.application_report_fingerprint,
                dispatch.application_commitment,
            );
            let [selected] = selected.as_slice() else {
                return false;
            };
            selected
                .trait_scope
                .scope(program)
                .and_then(|scope| scope.declaring_scope(program, declaring_trait.symbol))
        } else if let Some(bound) = &bound {
            bound
                .application
                .declaring_scope(program, declaring_trait.symbol)
        } else if program.trait_type_parameters(declaring_trait).is_empty()
            && declaring_trait.lifetime_parameters.is_empty()
        {
            Some(typed_trees::trait_definition::AppliedTraitScope::abstract_scope(declaring_trait))
        } else {
            dynamic_selector_scope(program, application, application_handle).inspect(|_| {
                open = true;
            })
        };
        let Some(scope) = scope else {
            return false;
        };
        if let Some(bound) = &bound
            && bound
                .application
                .declaring_scope(program, declaring_trait.symbol)
                .is_none_or(|declared| !declared.same_application(program, &scope))
        {
            return false;
        }
        if program.require_declared_trait_operands(&scope).is_err() {
            return false;
        }
        open |= program.require_receiving_trait_operands(&scope).is_err();
        let operands = bindings
            .iter()
            .filter_map(|binding| match binding.value {
                StaticMachineBindingValue::Type(actual)
                | StaticMachineBindingValue::Const(actual) => Some(actual),
                _ => None,
            })
            .collect::<Vec<_>>();
        let owner = application.lexical_owner;
        if program
            .require_declared_application_operands(&operands, owner)
            .is_err()
        {
            return true;
        }
        open |= program
            .require_owned_application_operands(&operands, owner)
            .is_err();
        let Some(declaration) = StaticDeclaration::find(program, owner) else {
            return false;
        };
        for binding in bindings {
            match &binding.value {
                StaticMachineBindingValue::RuntimeValue {
                    subject,
                    expression,
                    ..
                } => {
                    if (!subject.is_valid() && !expression.is_valid())
                        || !program
                            .state_signature_type_parameters(signature)
                            .iter()
                            .any(|parameter| {
                                parameter.symbol == binding.parameter
                                    && matches!(parameter.kind, TypeParameterKind::Value { .. })
                            })
                    {
                        return false;
                    }
                    open = true;
                }
                StaticMachineBindingValue::Machine(argument)
                    if argument.symbol.is_valid()
                        && program.symbols.get(argument.symbol).kind
                            == symbols::SymbolKind::MachineParameter =>
                {
                    if !declaration.parameters.iter().any(|parameter| {
                        parameter.symbol == argument.symbol
                            && matches!(parameter.kind, TypeParameterKind::Machine { .. })
                    }) {
                        return false;
                    }
                    open = true;
                }
                StaticMachineBindingValue::Conformance(argument)
                    if argument.symbol.is_valid()
                        && program.symbols.get(argument.symbol).kind
                            == symbols::SymbolKind::ConformanceParameter =>
                {
                    if !declaration
                        .conformance_bounds
                        .iter()
                        .any(|bound| bound.binder == argument.symbol)
                    {
                        return false;
                    }
                    open = true;
                }
                _ => {}
            }
        }
        if !open && authored_tuple_is_refuted(program, signature, application_handle, owner) {
            return true;
        }
        open
    }

    fn dynamic_selector_scope<'program>(
        program: &'program dyn crate::CheckingInput,
        application: &StaticMachineApplication,
        application_handle: StaticMachineApplicationHandle,
    ) -> Option<typed_trees::trait_definition::AppliedTraitScope<'program>> {
        let machine = program.machine_by_symbol(application.lexical_owner)?;
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Call(call) = statement
                    && call.authored_machine_application == application_handle
                    && let Some(scope) = selector_scope(
                        program,
                        machine,
                        Some(state),
                        call.receiver_symbol,
                        call.target_symbol,
                        program.statement_table.expression_handles(call.arguments),
                        application.binding_declaration,
                    )
                {
                    return Some(scope);
                }
                for (_, root) in program.tables.expression_roots(statement) {
                    let mut expressions = Vec::new();
                    crate::queries::expression_traversal::collect_expression_tree(
                        program,
                        root,
                        &mut expressions,
                    );
                    for expression in expressions {
                        let ExpressionNode::Call(call) =
                            expression_table(program).expression(expression)
                        else {
                            continue;
                        };
                        if call.authored_machine_application != application_handle {
                            continue;
                        }
                        let arguments =
                            expression_table(program).expression_handles(call.arguments);
                        if let Some(scope) = selector_scope(
                        program,
                        machine,
                        Some(state),
                        crate::machine_calls::calls::dynamic_requirement::expression_trait_receiver(
                            program,
                            call.receiver,
                        ),
                        call.target_symbol,
                        arguments,
                        application.binding_declaration,
                    ) {
                        return Some(scope);
                    }
                    }
                }
            }
        }
        None
    }

    fn selector_scope<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        state: Option<&'program typed_trees::state::State>,
        receiver: SymbolHandle,
        target: SymbolHandle,
        arguments: &[typed_trees::expression::ExpressionHandle],
        binding_declaration: SymbolHandle,
    ) -> Option<typed_trees::trait_definition::AppliedTraitScope<'program>> {
        let dynamic = crate::machine_calls::calls::dynamic_requirement::dynamic_selector_call(
            program, machine, state, receiver, target, arguments,
        )
        .ok()
        .flatten()?;
        (dynamic.signature.symbol == binding_declaration).then_some(dynamic.application)
    }

    fn authored_tuple_is_refuted(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        application_handle: StaticMachineApplicationHandle,
        lexical_owner: SymbolHandle,
    ) -> bool {
        let arguments =
            expression_table(program).static_machine_application_arguments(application_handle);
        crate::proof_contracts::contract_entailment::static_applications::static_application_requirement_judgment(
        program,
        signature,
        arguments,
        lexical_owner,
        None,
        &[],
    ) == crate::StrictArithmeticImplicationJudgment::Refuted
    }

    pub fn validate_static_machine_bindings(
        program: &dyn crate::CheckingInput,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let mut judged: Vec<(SymbolHandle, SymbolHandle, StaticMachineApplicationHandle)> =
            Vec::new();
        let references =
            typed_trees::references::collect_program_references(program.typed_program());
        let mut call_targets = Vec::new();
        let mut applications = references
            .expressions
            .iter()
            .filter_map(
                |expression| match expression_table(program).expression(*expression) {
                    ExpressionNode::Call(call) => {
                        call_targets.push(RetainedCallTarget {
                            application: call.authored_machine_application,
                            executable: call.target_symbol,
                            contract: call
                                .static_requirement_dispatch
                                .as_ref()
                                .map_or(call.target_symbol, |dispatch| dispatch.contract_symbol()),
                            requirement_application: call.requirement_application,
                            dispatch: call.static_requirement_dispatch.as_ref(),
                        });
                        Some(call.authored_machine_application)
                    }
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
                            StatementNode::Call(call) => {
                                call_targets.push(RetainedCallTarget {
                                    application: call.authored_machine_application,
                                    executable: call.target_symbol,
                                    contract: call
                                        .static_requirement_dispatch
                                        .as_ref()
                                        .map_or(call.target_symbol, |dispatch| {
                                            dispatch.contract_symbol()
                                        }),
                                    requirement_application: TypeReferenceHandle::invalid(),
                                    dispatch: call.static_requirement_dispatch.as_ref(),
                                });
                                Some(call.authored_machine_application)
                            }
                            _ => None,
                        }),
                );
            }
        }
        applications.extend(
            program
                .machine_specializations
                .iter()
                .filter(|specialization| {
                    program.machine_by_symbol(specialization.instance).is_some()
                })
                .map(|specialization| specialization.binding_application),
        );
        applications.extend(
            program
                .cleanup_selections()
                .map(|selection| selection.application),
        );
        applications.retain(|handle| handle.is_valid());
        applications.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
        applications.dedup();
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
                    .map(|view| (view.provider, view.selected_entry))
            })
            .collect::<Vec<_>>();
        call_targets.extend(
            providers
                .iter()
                .map(|(application, target)| RetainedCallTarget {
                    application: *application,
                    executable: *target,
                    contract: *target,
                    requirement_application: TypeReferenceHandle::invalid(),
                    dispatch: None,
                }),
        );
        applications.extend(providers.into_iter().map(|(provider, _)| provider));
        for handle in applications {
            let application = expression_table(program).static_machine_application(handle);
            let bindings = expression_table(program).static_machine_application_bindings(handle);
            let lifetimes = expression_table(program).static_machine_application_lifetimes(handle);
            if let Err(diagnostic) = accessor_primitives::validate(
                program,
                application,
                bindings,
                call_targets
                    .iter()
                    .filter_map(|call| (call.application == handle).then_some(call.executable)),
            ) {
                diagnostics.push(diagnostic);
            }
            if bindings.is_empty() && lifetimes.is_empty() {
                continue;
            }
            let declaration_symbol = if program
                .machine_by_symbol(application.binding_declaration)
                .is_none()
                && program
                    .trait_machine_signature(application.binding_declaration)
                    .is_some()
            {
                let mut selected = None;
                let mut valid = true;
                for call in call_targets
                    .iter()
                    .filter(|call| call.application == handle)
                {
                    let target = call.contract;
                    if let Some(slot) = program.dynamic_slot_by_symbol(target) {
                        let requirement = program
                            .closed_requirement_application(slot.requirement)
                            .map_or(slot.requirement, |closed| closed.template);
                        if requirement != application.binding_declaration
                            || selected.is_some_and(|previous| previous != slot.requirement)
                        {
                            valid = false;
                            break;
                        }
                        selected = Some(slot.requirement);
                        continue;
                    }
                    let Some(closed) = program.closed_requirement_application(target) else {
                        if !selected
                            .is_some_and(|previous| previous != application.binding_declaration)
                            && is_declared_source_tuple(
                                program,
                                application,
                                bindings,
                                call,
                                handle,
                            )
                        {
                            selected = Some(application.binding_declaration);
                            continue;
                        }
                        valid = false;
                        break;
                    };
                    if closed.template != application.binding_declaration
                        || selected.is_some_and(|previous| previous != target)
                    {
                        valid = false;
                        break;
                    }
                    selected = Some(target);
                }
                let Some(selected) = selected.filter(|_| valid) else {
                    diagnostics.push(Diagnostic::error(
                    format!("retained requirement tuple lost its exact source call application: call `{}` in `{}` binds declaration {:?}, but its retained targets are {:?}", application.target,
                        program.symbols.name(application.lexical_owner), application.binding_declaration,
                        call_targets.iter().filter(|call| call.application == handle).map(|call| (call.executable, call.contract, call.requirement_application)).collect::<Vec<_>>()),
                ));
                    continue;
                };
                selected
            } else {
                application.binding_declaration
            };
            let lifetimes = match program
                .static_application_lifetimes_for_declaration(handle, declaration_symbol)
            {
                Ok(lifetimes) => lifetimes,
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(format!(
                    "static lifetime tuple lost its declaration-owned region correspondence: {reason}; receiving `{}` from `{}`",
                    program.symbols.name(declaration_symbol), program.symbols.name(application.lexical_owner),
                )));
                    continue;
                }
            };
            if !lifetimes.is_empty() {
                let declared = StaticDeclaration::find(program, declaration_symbol);
                let owner = StaticDeclaration::find(program, application.lexical_owner);
                if declared.zip(owner).is_none_or(|(declared, owner)| {
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
            let Some(machine) = StaticDeclaration::find(program, declaration_symbol) else {
                diagnostics.push(Diagnostic::error(format!(
                    "static tuple for `{}` lost its original binding declaration",
                    application.target,
                )));
                continue;
            };
            let scope = application.lexical_owner;
            if judged.iter().any(|(declaration, owner, previous)| {
                *declaration == machine.symbol
                    && *owner == scope
                    && expression_table(program).static_machine_application_bindings(*previous)
                        == bindings
                    && expression_table(program)
                        .static_machine_application(*previous)
                        .lifetime_binding_declaration
                        == application.lifetime_binding_declaration
                    && expression_table(program).static_machine_application_lifetimes(*previous)
                        == expression_table(program).static_machine_application_lifetimes(handle)
            }) {
                continue;
            }
            judged.push((machine.symbol, scope, handle));
            let parameters = machine.parameters;
            let applied_arguments = parameters
                .iter()
                .map(|parameter| {
                    let mut selected = bindings
                        .iter()
                        .filter(|binding| binding.parameter == parameter.symbol);
                    let Some(binding) = selected.next() else {
                        return TypeReferenceHandle::invalid();
                    };
                    if selected.next().is_some() {
                        return TypeReferenceHandle::invalid();
                    }
                    match (&parameter.kind, &binding.value) {
                        (TypeParameterKind::Type, StaticMachineBindingValue::Type(actual))
                        | (
                            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                            StaticMachineBindingValue::Const(actual),
                        ) => *actual,
                        _ => TypeReferenceHandle::invalid(),
                    }
                })
                .collect::<Vec<_>>();
            let scope_parameters: &[TypeParameter] =
                StaticDeclaration::find(program, application.lexical_owner)
                    .map_or(&[], |owner| owner.parameters);
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
                            &applied_arguments,
                            &mut diagnostics,
                            formation_obligations,
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
            _ => true,
        });
            let open = bindings.iter().any(|binding| {
                matches!(&binding.value, StaticMachineBindingValue::Const(actual)
                if !typed_trees::domain::index_argument_is_closed(program, *actual))
            });
            forwarded_evidence::validate(
                program,
                &machine,
                application,
                bindings,
                &lifetimes,
                &mut diagnostics,
            );
            if complete {
                validate_conformance_bindings(
                    program,
                    &machine,
                    application,
                    bindings,
                    &lifetimes,
                    open,
                    &mut diagnostics,
                );
            }
        }
        crate::program_validation::finish_diagnostics(diagnostics)
    }

    fn validate_const_binding(
        program: &dyn crate::CheckingInput,
        application: &StaticMachineApplication,
        machine: &StaticDeclaration<'_>,
        parameter: &TypeParameter,
        actual: TypeReferenceHandle,
        applied_arguments: &[TypeReferenceHandle],
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        if let TypeReferenceNode::ConstExpression(expression) =
            program.type_reference_table.type_reference(actual)
            && let TypeParameterKind::Const {
                type_reference: expected,
            }
            | TypeParameterKind::Value {
                type_reference: expected,
            } = parameter.kind
            && crate::value_custody::const_expressions::has_carrier(
                program,
                *expression,
                &parameter.kind,
                application.lexical_owner,
            )
            && let Some(supplier) = StaticDeclaration::find(program, application.lexical_owner)
        {
            if let Some(caller) = program.machine_by_symbol(application.lexical_owner)
                && let Some(entry) = program.machine_states(caller).first()
            {
                crate::value_custody::const_expressions::validate_formation(
                    program,
                    caller,
                    entry,
                    *expression,
                    expected,
                    &format!(
                        "machine `{}` const argument `{}`",
                        caller.name, parameter.name
                    ),
                    diagnostics,
                    formation_obligations,
                );
            } else {
                let prior = program
                    .data_definition_by_symbol(supplier.symbol)
                    .map(|data| data.requires_facts)
                    .or_else(|| {
                        program
                            .domain_definition(supplier.symbol)
                            .map(|domain| domain.facts)
                    })
                    .into_iter()
                    .flat_map(|facts| program.proof_facts.span_or_empty(facts))
                    .filter_map(typed_trees::domain::ProofFact::as_expression)
                    .copied()
                    .collect::<Vec<_>>();
                crate::value_custody::type_references::validate_const_argument_expression_formation(
                    program,
                    machine.name.as_str(),
                    parameter,
                    AppliedValueCarrier::applied(expected, machine.parameters, applied_arguments),
                    actual,
                    crate::value_custody::type_references::TypeParameterScope::authored(
                        supplier.symbol,
                        supplier.parameters,
                        supplier.lifetime_parameters,
                    ),
                    &prior,
                    None,
                    diagnostics,
                    formation_obligations,
                );
            }
            return;
        }
        if let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(actual)
            && symbol.is_valid()
        {
            let caller = StaticDeclaration::find(program, application.lexical_owner);
            let forwarded = caller.and_then(|caller| {
                caller
                    .parameters
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
                && AppliedValueCarrier::applied(expected, machine.parameters, applied_arguments)
                    .matches(program, actual)
            {
                return;
            }
        }
        if let Err(mut errors) = crate::validate_closed_const_argument(
            program,
            machine.name.as_str(),
            parameter,
            actual,
            machine.parameters,
            applied_arguments,
        ) {
            diagnostics.append(&mut errors);
        }
    }

    fn validate_conformance_bindings(
        program: &dyn crate::CheckingInput,
        machine: &StaticDeclaration<'_>,
        call: &StaticMachineApplication,
        bindings: &[StaticMachineBinding],
        lifetimes: &[typed_trees::static_applications::StaticMachineLifetimeBinding],
        open: bool,
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
            let application = match crate::conformance_applications::close_conformance_application(
                program, argument,
            ) {
                Ok(application) => application,
                Err(_) if open => continue,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let carrier = crate::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
            let matched = conformance_arguments_match(
                program,
                machine,
                call.lexical_owner,
                bindings,
                lifetimes,
                bound,
                &application,
            );
            if open && matched.is_none() {
                continue;
            }
            if matched != Some(true) {
                diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` cannot bind `{}` to conformance `{}`: expected a complete `{}` implementation with the instantiated trait arguments",
                machine.name, bound.binder_name,
                selected.alias.as_ref().map_or("<unnamed>", |name| name.as_str()), bound.trait_name,
            )));
                continue;
            }
            if let Some(refinement) = carrier.refinement {
                let arguments = bindings
                    .iter()
                    .map(|binding| {
                        let reference = match &binding.value {
                            StaticMachineBindingValue::Type(reference)
                            | StaticMachineBindingValue::Const(reference)
                            | StaticMachineBindingValue::RuntimeValue {
                                type_reference: reference,
                                ..
                            } => *reference,
                            StaticMachineBindingValue::Machine(argument)
                            | StaticMachineBindingValue::Conformance(argument) => {
                                argument.type_reference
                            }
                        };
                        reference
                            .is_valid()
                            .then_some((binding.parameter, reference))
                    })
                    .collect::<Option<Vec<_>>>();
                let refinement_application = arguments.and_then(|arguments| {
                    typed_trees::trait_definition::AppliedTraitScope::new(
                        program.typed_program(),
                        refinement,
                        typed_trees::trait_definition::TraitApplicationArguments {
                            lexical_owner: machine.symbol,
                            enclosing_data_applications: &[],
                            arguments: &bound.arguments,
                            lifetimes: &bound.lifetime_arguments,
                        },
                    )?
                    .under_invocation(
                        program.typed_program(),
                        machine.symbol,
                        call.lexical_owner,
                        &arguments,
                        lifetimes,
                    )
                });
                diagnostics.extend(crate::refinement_fit::refinement_fit_diagnostics(
                    program,
                    refinement,
                    refinement_application.as_ref(),
                    selected,
                    machine.name.as_str(),
                    bound.binder_name.as_str(),
                ));
            }
        }
    }

    pub(crate) fn index_identity_arguments<'application>(
        bindings: &'application [StaticMachineBinding],
    ) -> Vec<(SymbolHandle, IndexIdentityArgument<'application>)> {
        bindings
            .iter()
            .map(|binding| (binding.parameter, index_identity_argument(&binding.value)))
            .collect()
    }

    pub(crate) fn lifetime_identity_rows(
        lifetimes: &[StaticMachineLifetimeBinding],
    ) -> Vec<(String, String)> {
        lifetimes
            .iter()
            .map(|binding| {
                (
                    binding.parameter.as_str().to_owned(),
                    binding.argument.as_str().to_owned(),
                )
            })
            .collect()
    }

    fn index_identity_argument(value: &StaticMachineBindingValue) -> IndexIdentityArgument<'_> {
        match value {
            StaticMachineBindingValue::Type(actual) => IndexIdentityArgument::TypeArgument(*actual),
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
            StaticMachineBindingValue::Machine(actual)
            | StaticMachineBindingValue::Conformance(actual) => {
                IndexIdentityArgument::StaticArgument(actual)
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
    ) -> Option<bool> {
        use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};

        let enclosing = program.enclosing_invocation_frames(lexical_owner).ok()?;
        let argument_owner = enclosing
            .last()
            .map_or(lexical_owner, |frame| frame.argument_owner);
        let enclosing_arguments = enclosing
            .iter()
            .map(|frame| index_identity_arguments(frame.bindings))
            .collect::<Vec<_>>();
        let enclosing_lifetimes = enclosing
            .iter()
            .map(|frame| lifetime_identity_rows(&frame.lifetimes))
            .collect::<Vec<_>>();
        let mut scopes = enclosing
            .iter()
            .zip(&enclosing_arguments)
            .zip(&enclosing_lifetimes)
            .rev()
            .map(|((frame, arguments), lifetimes)| AppliedTypeIdentityScope {
                declaration: frame.declaration,
                arguments,
                lifetimes,
            })
            .collect::<Vec<_>>();
        let substitutions = index_identity_arguments(bindings);
        let lifetimes = lifetime_identity_rows(lifetimes);
        let machine_scope = AppliedTypeIdentityScope {
            declaration: machine.symbol,
            arguments: &substitutions,
            lifetimes: &lifetimes,
        };

        let Some(conformance) = program.conformance_by_symbol(application.declaration) else {
            return Some(false);
        };
        let parameters = program.conformance_type_parameters(conformance);
        if parameters.len() != application.arguments.len() {
            return Some(false);
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
        scopes.push(machine_scope);
        let required = crate::conformance_applications::applied_bound_argument_identities(
            program,
            bound,
            machine.symbol,
            argument_owner,
            &scopes,
        );
        scopes.pop();
        scopes.push(actual_scope);
        let actual = (|| {
            let carrier = crate::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
            let scope = typed_trees::trait_definition::AppliedTraitScope::from_conformance(
                program.typed_program(),
                conformance,
            )?
            .declaring_scope(program.typed_program(), carrier.base)?;
            crate::conformance_applications::applied_scope_argument_identities(
                program,
                &scope,
                argument_owner,
                &scopes,
            )
        })();
        required
            .zip(actual)
            .map(|(required, actual)| required == actual)
    }
}
