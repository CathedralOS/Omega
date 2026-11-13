/*
Trait and conformance validation (`declarations::traits`): conformance
applications, refinement fit, provider region correspondence, dynamic slot
demands, and named implementation revalidation.
*/
use symbols::SymbolHandle;
use typed_trees::trait_definition::TraitDefinition;

pub(crate) mod conformance {
    fn disallowed_reached_service_names<'program>(
        program: &'program dyn crate::CheckingInput,
        service_reaches: &'program flow_effects::ServiceReachInferencePlan,
        machine: &'program typed_trees::machine::Machine,
        allowed_services: &[language_semantics::ServiceReachId],
        diagnostics: &mut Vec<diagnostics::Diagnostic>,
    ) -> Option<Vec<&'program str>> {
        let Some(summary) = service_reaches.for_machine(machine.symbol) else {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "machine `{}` has no inferred service-reach summary for requirement checking",
                machine.name,
            )));
            return None;
        };
        Some(
            service_reaches
                .services(summary.effective)
                .iter()
                .filter(|service| !allowed_services.contains(*service))
                .map(|service| {
                    program
                        .service_reaches
                        .definition(*service)
                        .map(|definition| definition.name.as_str())
                        .unwrap_or("<unknown canonical service>")
                })
                .collect(),
        )
    }

    pub(crate) mod bounds {
        use crate::declarations::traits::conformance::trait_applications::validate_trait_application_obligations;
        use crate::declarations::traits::trait_definition_by_symbol;
        use crate::value_custody::type_references::TypeParameterScope;
        use diagnostics::Diagnostic;
        use typed_trees::machine::Machine;
        use typed_trees::trait_definition::TraitDefinition;
        use typed_trees::types::TypeReferenceHandle;

        pub use typed_trees::trait_definition::GenericBoundRequirement;

        pub(crate) fn named_conformance_argument_matches(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            requirement: &GenericBoundRequirement<'_>,
        ) -> bool {
            program.normalized_type_identity(actual)
                == requirement.application.type_identity(program, required)
        }

        pub(crate) fn validate_generic_conformance_bounds(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            validate_conformance_bounds(
                program,
                "machine",
                machine.name.as_str(),
                &machine.conformance_bounds,
                symbols,
                TypeParameterScope::authored(
                    machine.symbol,
                    program.machine_type_parameters(machine),
                    &machine.lifetime_parameters,
                ),
                diagnostics,
                formation_obligations,
            );
        }

        pub(crate) fn validate_trait_conformance_bounds(
            program: &dyn crate::CheckingInput,
            trait_definition: &TraitDefinition,
            symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            validate_conformance_bounds(
                program,
                "trait",
                trait_definition.name.as_str(),
                &trait_definition.conformance_bounds,
                symbols,
                TypeParameterScope::authored(
                    trait_definition.symbol,
                    program.trait_type_parameters(trait_definition),
                    &trait_definition.lifetime_parameters,
                ),
                diagnostics,
                formation_obligations,
            );
        }

        pub(crate) fn validate_conformance_bounds(
            program: &dyn crate::CheckingInput,
            owner_kind: &str,
            owner_name: &str,
            bounds: &[typed_trees::machine::GenericConformanceBound],
            symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
            caller_scope: crate::value_custody::type_references::TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            for bound in bounds {
                if !bound.binder.is_valid() {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner_kind} `{owner_name}` evidence binder `{}` was not declared",
                        bound.binder_name
                    )));
                    continue;
                }

                let Some(trait_definition) =
                    trait_definition_by_symbol(program, bound.trait_symbol)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner_kind} `{owner_name}` evidence binder `{}` names unknown trait `{}`",
                        bound.binder_name, bound.trait_name
                    )));
                    continue;
                };
                let expected = program.trait_type_parameters(trait_definition).len();
                if bound.arguments.len() != expected {
                    diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` evidence binder `{}` for trait `{}` expects {expected} generic argument(s), got {}",
                bound.binder_name,
                bound.trait_name,
                bound.arguments.len(),
            )));
                    continue;
                }

                validate_trait_application_obligations(
                    program,
                    trait_definition,
                    &bound.arguments,
                    &bound.lifetime_arguments,
                    &format!(
                        "{owner_kind} `{owner_name}` evidence binder `{} satisfies {}`",
                        bound.binder_name, bound.trait_name
                    ),
                    symbols,
                    caller_scope,
                    diagnostics,
                    formation_obligations,
                );
            }
        }
    }
    pub(crate) mod external_shapes {
        use crate::declarations::standard_declarations::boundary_has_explicit_calling_policy;
        use crate::declarations::standard_declarations::is_core_vector;
        use crate::declarations::traits::trait_definition_by_symbol;
        use crate::frozen_program::expression_table;
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionNode;
        use typed_trees::machine::Machine;
        use typed_trees::signature::StateSignature;
        use typed_trees::trait_definition::TraitDefinition;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn validate_external_leaf_native_shapes(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for conformance in
                program
                    .machine_trait_conformances(machine)
                    .iter()
                    .filter(|conformance| {
                        conformance.external_binding.is_some()
                            || conformance.via_expression.is_valid()
                    })
            {
                if matches!(
                    machine.supply_mode,
                    language_semantics::MachineSupplyMode::ExternalRealization {
                        mechanism: Some(
                            language_semantics::ExternalBindingMechanism::CompilerIntrinsic,
                        ),
                        ..
                    }
                ) {
                    continue;
                }
                let Some(trait_definition) =
                    trait_definition_by_symbol(program, conformance.symbol)
                else {
                    continue;
                };
                if boundary_has_explicit_calling_policy(program, trait_definition) {
                    continue;
                }
                let Some(requirement) = program.trait_machine_signature_by_symbol(
                    trait_definition,
                    conformance.requirement_symbol,
                ) else {
                    continue;
                };

                for parameter in program.state_signature_parameters(requirement).iter() {
                    if let Some(carrier) =
                        private_native_carrier(program, parameter.type_reference, &mut Vec::new())
                    {
                        diagnostics.push(native_carrier_diagnostic(
                            program,
                            machine,
                            trait_definition,
                            requirement,
                            parameter.type_reference,
                            carrier,
                            "parameter",
                        ));
                    }
                }
                if requirement.return_type.is_valid()
                    && let Some(carrier) =
                        private_native_carrier(program, requirement.return_type, &mut Vec::new())
                {
                    diagnostics.push(native_carrier_diagnostic(
                        program,
                        machine,
                        trait_definition,
                        requirement,
                        requirement.return_type,
                        carrier,
                        "result",
                    ));
                }
            }
        }

        pub(crate) fn validate_external_via_expression(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            conformance: &typed_trees::machine::TraitConformance,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let expression = conformance.via_expression;
            if !expression.is_valid() {
                return;
            }
            let source_span = expression_table(program).source_span(expression);
            let ExpressionNode::Call(call) = expression_table(program).expression(expression)
            else {
                diagnostics.push(
            Diagnostic::error(format!(
                "external realization `{}` must use one direct zero-argument machine call after `via`",
                machine.name
            ))
            .with_source_span(source_span),
        );
                return;
            };
            let has_value_arguments = !expression_table(program)
                .expression_handles(call.arguments)
                .is_empty();
            let invalid_call_shape = call.receiver.is_valid()
                || has_value_arguments
                || !call.carries_only_positional_arguments()
                || !call.selects_only_nominal_route()
                || !call.target_symbol.is_valid();
            if invalid_call_shape {
                diagnostics.push(
            Diagnostic::error(format!(
                "external realization `{}` must use one exact receiverless zero-argument machine call after `via`",
                machine.name
            ))
            .with_source_span(source_span),
        );
                return;
            }

            let producers = program
                .machines()
                .iter()
                .filter_map(|producer| {
                    program
                        .state_in_machine(producer, call.target_symbol)
                        .map(|state| (producer, state))
                })
                .collect::<Vec<_>>();
            let [(producer, state)] = producers.as_slice() else {
                diagnostics.push(
            Diagnostic::error(format!(
                "external realization `{}` does not select one exact `via` producer machine",
                machine.name
            ))
            .with_source_span(source_span),
        );
                return;
            };
            let bodies = if producer.target_family_interface {
                crate::machine_calls::calls::target_family_members(program, producer)
                    .map(|members| {
                        members
                            .iter()
                            .filter_map(|member| program.machine(member.machine))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            } else {
                vec![*producer]
            };
            let rung_holds = !bodies.is_empty()
                && bodies.iter().all(|body| {
                    let entry = program.machine_states(body).first();
                    let entry_state = if std::ptr::eq(*body, *producer) {
                        Some(*state)
                    } else {
                        entry
                    };
                    body.body_is_present
                        && body.lifetime_parameters.is_empty()
                        && program.machine_type_parameters(body).is_empty()
                        && entry_state.is_some_and(|entry_state| {
                            program.state_parameters(entry_state).is_empty()
                                && producer_entry_state(program, body).is_some_and(
                                    |producer_entry| producer_entry.symbol == entry_state.symbol,
                                )
                        })
                });
            if !rung_holds {
                diagnostics.push(
            Diagnostic::error(format!(
                "external realization `{}` selects `via` producer `{}`, but the first evaluated-binding rung requires its body-bearing, non-generic, zero-parameter entry state",
                machine.name, producer.name
            ))
            .with_source_span(source_span),
        );
            }
        }

        fn producer_entry_state<'program>(
            program: &'program dyn crate::CheckingInput,
            producer: &Machine,
        ) -> Option<&'program typed_trees::state::State> {
            let leaf = producer
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or_default();
            program
                .machine_states(producer)
                .iter()
                .find(|candidate| candidate.name.as_str() == leaf)
                .or_else(|| program.machine_states(producer).first())
        }

        #[derive(Debug, Clone, Copy)]
        pub(crate) enum PrivateNativeCarrier {
            Slice,
            Vector,
        }

        impl PrivateNativeCarrier {
            const fn label(self) -> &'static str {
                match self {
                    Self::Slice => "safe slice",
                    Self::Vector => "vector carrier",
                }
            }
        }

        fn private_native_carrier(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
            visiting: &mut Vec<SymbolHandle>,
        ) -> Option<PrivateNativeCarrier> {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Callable { .. } => None,
                TypeReferenceNode::Constrained { base_type, .. } => {
                    private_native_carrier(program, *base_type, visiting)
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    private_native_carrier(program, *referee, visiting)
                }
                TypeReferenceNode::Slice { .. } => Some(PrivateNativeCarrier::Slice),
                TypeReferenceNode::FixedArray { element_type, .. } => {
                    private_native_carrier(program, *element_type, visiting)
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } => {
                    if is_core_vector(program, *base_symbol) {
                        return Some(PrivateNativeCarrier::Vector);
                    }
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .find_map(|argument| private_native_carrier(program, *argument, visiting))
                        .or_else(|| private_record_carrier(program, *base_symbol, visiting))
                }
                TypeReferenceNode::Named { symbol, .. } => {
                    if is_core_vector(program, *symbol) {
                        return Some(PrivateNativeCarrier::Vector);
                    }
                    private_record_carrier(program, *symbol, visiting)
                }
                TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Unit => None,
            }
        }

        fn private_record_carrier(
            program: &dyn crate::CheckingInput,
            symbol: SymbolHandle,
            visiting: &mut Vec<SymbolHandle>,
        ) -> Option<PrivateNativeCarrier> {
            if !symbol.is_valid() || visiting.contains(&symbol) {
                return None;
            }
            let definition =
                crate::declarations::data_queries::data_definition_by_symbol(program, symbol)?;
            visiting.push(symbol);
            let carrier = program.data_members(definition).iter().find_map(|member| {
                let field = member.as_field()?;
                private_native_carrier(program, field.type_reference, visiting)
            });
            visiting.pop();
            carrier
        }

        fn native_carrier_diagnostic(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            trait_definition: &TraitDefinition,
            requirement: &StateSignature,
            type_reference: TypeReferenceHandle,
            carrier: PrivateNativeCarrier,
            position: &str,
        ) -> Diagnostic {
            Diagnostic::error(format!(
                "external leaf `{}` cannot use {} `{}` as a default-native {} for `{}::{}`; declare the foreign pointer/length/terminator or record shape explicitly in a checked adapter, or select a custom `Calling<C>` policy that publishes a canonical representation",
                machine.name,
                carrier.label(),
                program.display_type_reference(type_reference),
                position,
                trait_definition.name,
                requirement.name,
            ))
        }
    }
    pub(crate) mod generic_operator {
        use crate::declarations::traits::conformance::signature_matching::trait_type_bindings;
        use crate::declarations::traits::conformance::signature_matching::type_references_match_for_trait_bindings;
        use crate::declarations::traits::trait_definition_by_symbol;
        use language_core::OperatorSpelling;
        use typed_trees::machine::Machine;
        use typed_trees::signature::StateSignature;
        use typed_trees::types::TypeReferenceHandle;

        pub fn generic_bound_operator_requirement<'program>(
            program: &'program dyn crate::CheckingInput,
            machine: &'program Machine,
            spelling: OperatorSpelling,
            operands: &[Option<TypeReferenceHandle>],
        ) -> Result<Option<&'program StateSignature>, String> {
            if operands.iter().any(Option::is_none) {
                return Ok(None);
            }
            let mut selected = None;
            for bound in &machine.conformance_bounds {
                let Some(trait_definition) =
                    trait_definition_by_symbol(program, bound.trait_symbol)
                else {
                    continue;
                };
                for requirement in program.trait_machine_signatures(trait_definition) {
                    if requirement.spelling != Some(spelling) {
                        continue;
                    }
                    let parameters = program.state_signature_parameters(requirement);
                    if parameters.len() != operands.len() {
                        continue;
                    }
                    let mut bindings = trait_type_bindings(
                        program.trait_type_parameters(trait_definition).iter(),
                        bound.arguments.iter(),
                    );
                    if !operands.iter().zip(parameters).all(|(actual, parameter)| {
                        actual.is_some_and(|actual| {
                            type_references_match_for_trait_bindings(
                                program,
                                actual,
                                parameter.type_reference,
                                program.trait_type_parameters(trait_definition),
                                &mut bindings,
                            )
                        })
                    }) {
                        continue;
                    }
                    if selected.is_some() {
                        return Err(format!(
                            "machine `{}` has multiple declared conformance bounds selecting operator {:?}",
                            machine.name, spelling,
                        ));
                    }
                    selected = Some(requirement);
                }
            }
            Ok(selected)
        }
    }
    pub(crate) mod machine_conformance {
        pub(crate) mod closed_requirement {
            use diagnostics::Diagnostic;
            use language_semantics::MachineSupplyMode;
            use language_semantics::const_value::DecodedCanonicalConstValue;
            use typed_trees::machine::Machine;
            use typed_trees::operator::{
                ClosedOperatorApplicationArgument, ClosedOperatorRealizationApplication,
                closed_operator_realization_application,
            };
            use typed_trees::typed_trees::MachineSpecialization;

            pub fn resolve<'program>(
                program: &'program dyn crate::CheckingInput,
                provider: &Machine,
                requirement: &'program Machine,
            ) -> Result<&'program Machine, Diagnostic> {
                if program.machine_type_parameters(requirement).is_empty()
                    || !program.machine_type_parameters(provider).is_empty()
                {
                    return Ok(requirement);
                }
                let failure = || {
                    Diagnostic::error(format!(
                        "provider machine `{}` has no unique checked closed application of top-level boundary requirement `{}`",
                        provider.name, requirement.name,
                    ))
                };
                if provider.supply_mode != MachineSupplyMode::CheckedBody
                    || !provider.body_is_present
                {
                    return Err(failure());
                }
                let operator = program
                    .machine_token_binding_by_symbol(requirement.symbol)
                    .ok_or_else(failure)?;
                let mut receipts = program
                    .machine_specializations
                    .iter()
                    .filter(|receipt| receipt.instance == provider.symbol);
                let receipt = receipts.next().ok_or_else(failure)?;
                if receipts.next().is_some() {
                    return Err(failure());
                }
                let mut applications = receipt
                    .operator_realizations
                    .iter()
                    .filter(|application| application.requirement_symbol == requirement.symbol);
                let application = applications.next().ok_or_else(failure)?;
                if applications.next().is_some()
                    || closed_operator_realization_application(program, provider, operator).as_ref()
                        != Some(application)
                {
                    return Err(failure());
                }
                crate::declarations::operators::validate_closed_operator_application(
                    program,
                    operator,
                    &application.arguments,
                )?;
                let mut requirements = program
                    .machine_specializations
                    .iter()
                    .filter(|receipt| receipt.template == requirement.symbol)
                    .filter_map(|receipt| {
                        program
                            .machine(receipt.instance)
                            .map(|machine| (receipt, machine))
                    })
                    .filter(|(_, candidate)| {
                        candidate.supply_mode == MachineSupplyMode::TopLevelRequirement
                            && program.machine_type_parameters(candidate).is_empty()
                            && closed_operator_realization_application(program, candidate, operator)
                                .is_some_and(|reconstructed| {
                                    same_application(program, application, &reconstructed)
                                })
                    });
                let (closed_receipt, closed) = requirements.next().ok_or_else(failure)?;
                if requirements.next().is_some() {
                    return Err(failure());
                }
                if program
                    .machine_specializations
                    .iter()
                    .filter(|receipt| receipt.instance == closed.symbol)
                    .count()
                    != 1
                    || closed_receipt.template_parameters != requirement.type_parameters
                    || !retains_application_tuple(program, closed_receipt, application)
                {
                    return Err(failure());
                }
                Ok(closed)
            }

            fn retains_application_tuple(
                program: &dyn crate::CheckingInput,
                receipt: &MachineSpecialization,
                application: &ClosedOperatorRealizationApplication,
            ) -> bool {
                let mut types = receipt.type_argument_identities.iter();
                let mut constants = receipt.const_argument_identities.iter();
                for argument in &application.arguments {
                    let matches = match argument {
                        ClosedOperatorApplicationArgument::Type { type_reference, .. } => {
                            types.next().is_some_and(|identity| {
                                *identity == program.normalized_type_identity_text(*type_reference)
                            })
                        }
                        ClosedOperatorApplicationArgument::Const { value, .. } => {
                            let Some(DecodedCanonicalConstValue::Integer { value, .. }) =
                                value.decode_encoding()
                            else {
                                return false;
                            };
                            let value = value.to_string();
                            let reference = program
                                .type_reference_table
                                .named_references()
                                .find(|(_, symbol, name)| !symbol.is_valid() && *name == value);
                            reference.is_some_and(|(reference, _, _)| {
                                constants.next().is_some_and(|identity| {
                                    *identity == program.normalized_type_identity_text(reference)
                                })
                            })
                        }
                    };
                    if !matches {
                        return false;
                    }
                }
                types.next().is_none() && constants.next().is_none()
            }

            fn same_application(
                program: &dyn crate::CheckingInput,
                left: &ClosedOperatorRealizationApplication,
                right: &ClosedOperatorRealizationApplication,
            ) -> bool {
                left.requirement_symbol == right.requirement_symbol
                    && left.arguments.len() == right.arguments.len()
                    && left
                        .arguments
                        .iter()
                        .zip(&right.arguments)
                        .all(|(left, right)| match (left, right) {
                            (
                                ClosedOperatorApplicationArgument::Type {
                                    binder_symbol: left_binder,
                                    type_reference: left_type,
                                },
                                ClosedOperatorApplicationArgument::Type {
                                    binder_symbol: right_binder,
                                    type_reference: right_type,
                                },
                            ) => {
                                left_binder == right_binder
                                    && program.package_qualified_type_identity(*left_type)
                                        == program.package_qualified_type_identity(*right_type)
                            }
                            (
                                ClosedOperatorApplicationArgument::Const {
                                    binder_symbol: left_binder,
                                    declared_carrier: left_carrier,
                                    value: left_value,
                                },
                                ClosedOperatorApplicationArgument::Const {
                                    binder_symbol: right_binder,
                                    declared_carrier: right_carrier,
                                    value: right_value,
                                },
                            ) => {
                                left_binder == right_binder
                                    && left_value == right_value
                                    && program.package_qualified_type_identity(*left_carrier)
                                        == program.package_qualified_type_identity(*right_carrier)
                            }
                            _ => false,
                        })
            }
        }

        pub use closed_requirement::resolve as resolve_closed_requirement_application;

        use crate::declarations::traits::conformance::signature_matching::{
            TraitTypeBinding, TraitTypeBindingTarget, authored_conformance_machine,
            authored_satisfier_parameters, parameter_shape_label,
            satisfier_parameter_correspondence,
            type_reference_lifetimes_match_requirement_application,
            type_references_match_for_trait_bindings, validate_applied_trait_signature,
        };
        use crate::declarations::traits::conformance::trait_applications::validate_trait_application_obligations;
        use crate::declarations::traits::trait_definition_by_symbol;
        use crate::frozen_program::expression_table;
        use crate::value_custody::type_references::TypeParameterScope;
        use crate::value_custody::type_references::type_reference_label;
        use diagnostics::Diagnostic;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::domain::ProofFact;
        use typed_trees::machine::Machine;
        use typed_trees::proposition::{ProofSubstitutions, PropositionLabels};
        use typed_trees::signature::{SignatureContractKind, StateSignature};
        use typed_trees::state::State;
        use typed_trees::trait_definition::TraitDefinition;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn validate_machine_trait_conformances(
            program: &dyn crate::CheckingInput,
            service_reaches: &flow_effects::ServiceReachInferencePlan,
            machine: &Machine,
            symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            for conformance in program.machine_trait_conformances(machine) {
                if let Some(requirement) =
                    program
                        .machines_with_symbol(conformance.symbol)
                        .find(|requirement| {
                            requirement.supply_mode
                                == language_semantics::MachineSupplyMode::TopLevelRequirement
                        })
                {
                    validate_machine_top_level_requirement_conformance(
                        program,
                        service_reaches,
                        machine,
                        requirement,
                        conformance,
                        diagnostics,
                    );
                    continue;
                }
                let Some(trait_definition) =
                    trait_definition_by_symbol(program, conformance.symbol)
                else {
                    if validate_machine_operator_conformance(
                        program,
                        machine,
                        conformance,
                        diagnostics,
                    ) {
                        continue;
                    }
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` satisfies unknown trait `{}`",
                        machine.name, conformance.name
                    )));
                    continue;
                };
                let authored_machine = match authored_conformance_machine(program, machine) {
                    Ok(owner) => owner,
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        continue;
                    }
                };
                let expected_lifetime_arguments = trait_definition.lifetime_parameters.len();
                if conformance.trait_lifetime_arguments.len() != expected_lifetime_arguments {
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{}` conformance to trait `{}` expects {expected_lifetime_arguments} target-trait lifetime argument(s), got {}",
                machine.name,
                trait_definition.name,
                conformance.trait_lifetime_arguments.len(),
            )));
                    continue;
                }
                if conformance.trait_lifetime_arguments.iter().any(|ordinal| {
                    usize::try_from(*ordinal).map_or(true, |ordinal| {
                        ordinal >= authored_machine.lifetime_parameters.len()
                    })
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{}` conformance to trait `{}` retains a target-trait lifetime outside its machine telescope",
                machine.name, trait_definition.name,
            )));
                    continue;
                }
                let explicit_type_arguments = program
                    .type_reference_table
                    .type_reference_handles(conformance.arguments);
                let expected_type_arguments = program.trait_type_parameters(trait_definition).len();
                if explicit_type_arguments.len() != expected_type_arguments {
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{}` conformance to trait `{}` expects {expected_type_arguments} generic argument(s), got {}",
                machine.name,
                trait_definition.name,
                explicit_type_arguments.len()
            )));
                    continue;
                }

                let applied_lifetimes = conformance
                    .trait_lifetime_arguments
                    .iter()
                    .map(|ordinal| authored_machine.lifetime_parameters[*ordinal as usize].clone())
                    .collect::<Vec<_>>();
                validate_trait_application_obligations(
                    program,
                    trait_definition,
                    explicit_type_arguments,
                    &applied_lifetimes,
                    &format!(
                        "machine `{}` conformance to trait `{}`",
                        machine.name, trait_definition.name
                    ),
                    symbols,
                    TypeParameterScope::authored(
                        authored_machine.symbol,
                        program.machine_type_parameters(authored_machine),
                        &authored_machine.lifetime_parameters,
                    ),
                    diagnostics,
                    formation_obligations,
                );

                let Some(requirement_name) = conformance.requirement.as_ref() else {
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has a malformed conformance to `{}` without an exact requirement",
                machine.name, trait_definition.name
            )));
                    continue;
                };
                validate_machine_single_requirement(
                    program,
                    service_reaches,
                    machine,
                    trait_definition,
                    requirement_name,
                    conformance.requirement_symbol,
                    conformance.alias.as_ref().map(|alias| alias.as_str()),
                    &conformance.trait_lifetime_arguments,
                    explicit_type_arguments,
                    diagnostics,
                );
            }
        }

        fn validate_machine_top_level_requirement_conformance(
            program: &dyn crate::CheckingInput,
            service_reaches: &flow_effects::ServiceReachInferencePlan,
            machine: &Machine,
            requirement: &Machine,
            conformance: &typed_trees::machine::TraitConformance,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let requirement_identity = requirement.name.as_str();
            let label = format!(
                "provider machine `{}` for top-level boundary requirement `{requirement_identity}`",
                machine.name
            );
            if !conformance.trait_lifetime_arguments.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} supplies target-trait lifetime arguments to a non-trait requirement"
                )));
                return;
            }
            if conformance.requirement_symbol != requirement.symbol {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} does not retain the exact selected requirement symbol"
                )));
                return;
            }
            if !program
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .is_empty()
            {
                diagnostics.push(Diagnostic::error(format!(
            "{label} supplies trait-style arguments; a top-level requirement owns its own static telescope"
        )));
                return;
            }
            let requirement = match closed_requirement::resolve(program, machine, requirement) {
                Ok(requirement) => requirement,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    return;
                }
            };
            let Some(requirement_entry) = program.machine_states(requirement).first() else {
                diagnostics.push(Diagnostic::error(format!(
                    "top-level boundary requirement `{requirement_identity}` has no entry signature"
                )));
                return;
            };
            let Some(provider_entry) = program.machine_states(machine).first() else {
                diagnostics.push(Diagnostic::error(format!("{label} has no entry signature")));
                return;
            };

            if requirement.lifetime_parameters.len() != machine.lifetime_parameters.len() {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} has {} lifetime parameter(s); the requirement declares {}",
                    machine.lifetime_parameters.len(),
                    requirement.lifetime_parameters.len(),
                )));
            }

            let required_type_parameters = program.machine_type_parameters(requirement);
            let actual_type_parameters = program.machine_type_parameters(machine);
            if required_type_parameters.len() != actual_type_parameters.len() {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} has {} static parameter(s); the requirement declares {}",
                    actual_type_parameters.len(),
                    required_type_parameters.len(),
                )));
                return;
            }
            for (index, (required, actual)) in required_type_parameters
                .iter()
                .zip(actual_type_parameters)
                .enumerate()
            {
                if !super::type_parameter_kinds_identical(&required.kind, &actual.kind) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} static parameter {index} has a different kind"
                    )));
                }
            }
            let generic_types = required_type_parameters.iter().collect::<Vec<_>>();
            for (index, (required, actual)) in required_type_parameters
                .iter()
                .zip(actual_type_parameters)
                .enumerate()
            {
                if matches!(
                    (&required.kind, &actual.kind),
                    (TypeParameterKind::Type, TypeParameterKind::Type)
                ) && typed_trees::data::type_parameter_demands_stronger_properties(
                    required, actual,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} static parameter {index} demands stronger type properties"
                    )));
                }
            }
            let mut type_bindings = super::type_parameter_binding_pairs(
                required_type_parameters,
                actual_type_parameters,
            )
            .map(|(required, actual)| TraitTypeBinding {
                parameter_symbol: required.symbol,
                parameter_name: required.name.as_str().to_owned(),
                target: TraitTypeBindingTarget::Parameter(actual.symbol),
                scope_start: 0,
            })
            .collect::<Vec<_>>();
            for (index, (required, actual)) in required_type_parameters
                .iter()
                .zip(actual_type_parameters)
                .enumerate()
            {
                let Some((required_type, actual_type)) =
                    crate::value_custody::type_shapes::scalar_parameter_type_pair(
                        &required.kind,
                        &actual.kind,
                    )
                else {
                    continue;
                };
                if !type_references_match_for_trait_bindings(
                    program,
                    actual_type,
                    required_type,
                    required_type_parameters,
                    &mut type_bindings,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} const static parameter {index} has a different type"
                    )));
                }
            }

            let required_parameters = program.state_parameters(requirement_entry);
            let actual_parameters = program.state_parameters(provider_entry);
            if let Err(diagnostic) = satisfier_parameter_correspondence(
                &format!("machine `{}`", machine.name),
                &format!("top-level boundary requirement `{requirement_identity}`"),
                actual_parameters.len(),
                required_parameters.len(),
            ) {
                diagnostics.push(diagnostic);
                return;
            }
            for (index, (required, actual)) in required_parameters
                .iter()
                .zip(actual_parameters)
                .enumerate()
            {
                if required.is_const != actual.is_const || required.is_mutable != actual.is_mutable
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} parameter {} has shape `{}`, but the requirement declares `{}`",
                        index,
                        parameter_shape_label(program, actual),
                        parameter_shape_label(program, required),
                    )));
                    continue;
                }

                if !type_references_match_for_trait_bindings(
                    program,
                    actual.type_reference,
                    required.type_reference,
                    required_type_parameters,
                    &mut type_bindings,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} parameter {} expects `{}`, got `{}`",
                        index,
                        parameter_shape_label(program, required),
                        parameter_shape_label(program, actual),
                    )));
                }
            }

            let return_matches = type_references_match_for_trait_bindings(
                program,
                provider_entry.return_type,
                requirement_entry.return_type,
                required_type_parameters,
                &mut type_bindings,
            );
            let dispatch_matches = program
                .normalized_result_dispatch_set(provider_entry.return_type)
                == program.normalized_result_dispatch_set(requirement_entry.return_type);
            if !return_matches || !dispatch_matches {
                diagnostics.push(Diagnostic::error(format!(
            "{label} returns `{}`, but the requirement returns `{}` with its exact result dispatch",
            type_reference_label(program, provider_entry.return_type),
            type_reference_label(program, requirement_entry.return_type),
        )));
            }

            if requirement.lifetime_parameters.len() == machine.lifetime_parameters.len() {
                let identity_application =
                    (0..requirement.lifetime_parameters.len() as u32).collect::<Vec<_>>();
                for (index, (required, actual)) in required_parameters
                    .iter()
                    .zip(actual_parameters)
                    .enumerate()
                {
                    if !type_reference_lifetimes_match_requirement_application(
                        program,
                        actual.type_reference,
                        required.type_reference,
                        &requirement.lifetime_parameters,
                        machine,
                        &identity_application,
                        &type_bindings,
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                    "{label} parameter {index} does not realize the requirement's declared lifetime telescope"
                )));
                    }
                }
                if !type_reference_lifetimes_match_requirement_application(
                    program,
                    provider_entry.return_type,
                    requirement_entry.return_type,
                    &requirement.lifetime_parameters,
                    machine,
                    &identity_application,
                    &type_bindings,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                "{label} return type does not realize the requirement's declared lifetime telescope"
            )));
                }
            }

            match top_level_lifetime_frame(program, requirement, machine) {
                Ok(frame) => {
                    crate::machine_calls::machine_parameters::validate_trait_callable_parameter_refinement(
                program,
                &label,
                &frame,
                required_type_parameters,
                actual_type_parameters,
                &generic_types,
                diagnostics,
            );
                }
                Err(reason) => diagnostics.push(Diagnostic::error(format!("{label}: {reason}"))),
            }

            validate_top_level_requirement_effect_ceiling(
                program,
                service_reaches,
                machine,
                requirement,
                &label,
                diagnostics,
            );
            if conformance.external_binding.is_none() && !conformance.via_expression.is_valid() {
                let prepare_implication = || {
                    crate::proof_contracts::contract_entailment::CallableContractRefinement::prepare(
                        program,
                        crate::proof_contracts::contract_entailment::CallableContract {
                            owner: requirement.symbol,
                            contracts: program.machine_contracts(requirement),
                            parameters: required_parameters,
                            result: requirement_entry.return_type,
                        },
                        crate::proof_contracts::contract_entailment::CallableContract {
                            owner: machine.symbol,
                            contracts: program.machine_contracts(machine),
                            parameters: actual_parameters,
                            result: provider_entry.return_type,
                        },
                        &required_type_parameters
                            .iter()
                            .zip(actual_type_parameters)
                            .map(|(required, actual)| (required.symbol, actual.symbol))
                            .collect::<Vec<_>>(),
                    )
                };
                crate::machine_calls::machine_parameters::validate_callable_contract_refinement(
                    program,
                    &label,
                    requirement_identity,
                    program.machine_contracts(requirement),
                    program.machine_contracts(machine),
                    required_parameters,
                    actual_parameters,
                    requirement.symbol,
                    machine.symbol,
                    &[],
                    Default::default(),
                    prepare_implication,
                    diagnostics,
                );
            }
        }

        fn top_level_lifetime_frame(
            program: &dyn crate::CheckingInput,
            required: &Machine,
            actual: &Machine,
        ) -> Result<crate::borrow_contracts::CallableLifetimeFrame, &'static str> {
            let required = program.lifetime_declaration_owner(required.symbol);
            let actual = program.lifetime_declaration_owner(actual.symbol);
            let parameters = |owner| {
                typed_trees::type_identity::declaration_lifetime_parameters(
                    program.typed_program(),
                    owner,
                )
                .ok_or("top-level lifetime application lost its declaration-owned telescope")
            };
            let required_parameters = parameters(required)?;
            let actual_parameters = parameters(actual)?;
            if required_parameters.len() != actual_parameters.len() {
                return Err("enclosing callable lifetime telescopes have different arity");
            }
            Ok(crate::borrow_contracts::CallableLifetimeFrame {
                required: Vec::new(),
                actual: required_parameters
                    .iter()
                    .enumerate()
                    .map(
                        |(ordinal, _)| crate::borrow_contracts::CallableLifetimeBinding {
                            parameter: typed_trees::type_identity::LifetimeParameterIdentity {
                                declaration: actual,
                                ordinal,
                            },
                            argument: typed_trees::type_identity::LifetimeParameterIdentity {
                                declaration: required,
                                ordinal,
                            },
                        },
                    )
                    .collect(),
            })
        }

        pub fn revalidate_top_level_requirement_realization(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            requirement: &Machine,
            conformance: &typed_trees::machine::TraitConformance,
        ) -> Result<(), Vec<Diagnostic>> {
            let mut diagnostics = Vec::new();
            let operational = crate::infer_operational_may(program);
            let service_reaches = crate::infer_service_reaches(program, &operational);
            if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
                || conformance.symbol != requirement.symbol
            {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not retain one exact top-level boundary requirement realization",
            machine.name
        )));
            } else {
                validate_machine_top_level_requirement_conformance(
                    program,
                    &service_reaches,
                    machine,
                    requirement,
                    conformance,
                    &mut diagnostics,
                );
            }
            crate::program_validation::finish_strict_diagnostics(diagnostics)
        }

        pub fn revalidate_machine_trait_conformances(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
        ) -> Result<(), Vec<Diagnostic>> {
            let mut diagnostics = Vec::new();
            let mut formation_obligations = Vec::new();
            let symbols =
                crate::declarations::symbols::TopLevelSymbols::build(program, &mut diagnostics);
            let operational = crate::infer_operational_may(program);
            let service_reaches = crate::infer_service_reaches(program, &operational);
            validate_machine_trait_conformances(
                program,
                &service_reaches,
                machine,
                &symbols,
                &mut diagnostics,
                &mut formation_obligations,
            );
            crate::program_validation::finish_strict_diagnostics(diagnostics)
        }

        fn validate_top_level_requirement_effect_ceiling(
            program: &dyn crate::CheckingInput,
            service_reaches: &flow_effects::ServiceReachInferencePlan,
            machine: &Machine,
            requirement: &Machine,
            label: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let Some(required_reach) = service_reaches.for_machine(requirement.symbol) else {
                diagnostics.push(Diagnostic::error(format!(
                    "top-level boundary requirement `{}` has no inferred service-reach summary",
                    requirement.name,
                )));
                return;
            };
            let required_operational =
                crate::machine_calls::effect_inference::operational::machine_operational(
                    program,
                    requirement,
                );
            super::require_operational_ceiling(
                program,
                service_reaches,
                machine,
                service_reaches.services(required_reach.published),
                required_operational.may_suspend,
                required_operational.may_block,
                |breach| match breach {
                    super::OperationalCeilingBreach::ReachedService(service_name) => {
                        format!(
                            "{label} reaches service `{service_name}` outside the requirement ceiling"
                        )
                    }
                    super::OperationalCeilingBreach::Suspends => {
                        format!("{label} declares `suspends` beyond the requirement ceiling")
                    }
                    super::OperationalCeilingBreach::Blocks => {
                        format!("{label} declares `blocks` beyond the requirement ceiling")
                    }
                },
                diagnostics,
            );

            let allowed_invocations = program.machine_invokes(requirement);
            for invocation in program.machine_invokes(machine) {
                if !allowed_invocations
                    .iter()
                    .any(|allowed| allowed.target == invocation.target)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{label} invokes `{}` outside the requirement ceiling",
                        invocation.name,
                    )));
                }
            }

            let required_termination = requirement
                .termination_plan
                .interface
                .published()
                .unwrap_or(&requirement.termination_plan.checked_summary);
            let actual_termination = machine
                .termination_plan
                .interface
                .published()
                .unwrap_or(&machine.termination_plan.checked_summary);
            if required_termination.promises_termination()
                && !actual_termination.promises_termination()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} does not provide the requirement's termination guarantee"
                )));
            }
        }

        fn validate_machine_operator_conformance(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            conformance: &typed_trees::machine::TraitConformance,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let Some(requirement) = conformance.requirement.as_ref() else {
                return false;
            };
            let namespace = conformance.name.as_str();
            let requirement_name = requirement.as_str();
            let same_path_declarations = program
                .operators()
                .iter()
                .chain(program.machine_token_bindings())
                .filter(|operator| {
                    let path = program.operator_path_members(operator.name);
                    matches!(path, [owner, member]
                if owner.as_str() == namespace
                    && member.as_str() == requirement_name)
                })
                .collect::<Vec<_>>();
            if same_path_declarations.is_empty() {
                return false;
            }

            if !conformance.trait_lifetime_arguments.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` supplies target-trait lifetime arguments to operator requirement `{}::{}`",
            machine.name, namespace, requirement_name,
        )));
                return true;
            }

            if !program
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .is_empty()
            {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` supplies type arguments to operator requirement `{}::{}`; the exact overloaded operator is selected from the machine signature",
            machine.name, namespace, requirement_name,
        )));
                return true;
            }

            let operator = if let Some(operator) =
                typed_trees::operator::resolve_satisfied_checked_operator_for_conformance(
                    program,
                    machine,
                    conformance,
                ) {
                operator
            } else {
                let retained =
                    typed_trees::operator::resolve_specialized_checked_operator_application(
                        program,
                        machine,
                        namespace,
                        requirement_name,
                    );
                let Some((operator, retained)) = retained else {
                    if let [declaration] = same_path_declarations.as_slice()
                        && let Some(entry) = program.machine_states(machine).first()
                        && let Err(diagnostic) = satisfier_parameter_correspondence(
                            &format!("machine `{}`", machine.name),
                            &format!("requirement `{namespace}::{requirement_name}`"),
                            authored_satisfier_parameters(program, machine, entry).len(),
                            program.operator_parameters(declaration).len(),
                        )
                    {
                        diagnostics.push(diagnostic);
                        return true;
                    }
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{}` does not match one exact overload of operator requirement `{}::{}`; its entry parameter and result types must equal one declared requirement signature",
                machine.name, namespace, requirement_name,
            )));
                    return true;
                };
                if let Err(diagnostic) =
                    crate::declarations::operators::validate_closed_operator_application(
                        program,
                        operator,
                        &retained.arguments,
                    )
                {
                    diagnostics.push(diagnostic);
                    return true;
                }
                operator
            };
            if conformance.requirement_symbol != operator.symbol {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` operator realization `{}::{}` does not retain the exact selected overload",
            machine.name, namespace, requirement_name,
        )));
                return true;
            }

            if conformance.external_binding.is_none() && !conformance.via_expression.is_valid() {
                crate::proof_contracts::contract_entailment::check_operator_contract_conformance(
                    program,
                    machine,
                    operator,
                    diagnostics,
                );
            }
            true
        }

        fn validate_machine_single_requirement(
            program: &dyn crate::CheckingInput,
            service_reaches: &flow_effects::ServiceReachInferencePlan,
            machine: &Machine,
            trait_definition: &TraitDefinition,
            requirement_name: &typed_trees::name::Identifier,
            settled_requirement_symbol: symbols::SymbolHandle,
            conformance_alias: Option<&str>,
            trait_lifetime_arguments: &[u32],
            explicit_type_arguments: &[TypeReferenceHandle],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let Some(entry_state) = program.machine_states(machine).first() else {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` satisfies `{}::{}` but has no states",
                    machine.name, trait_definition.name, requirement_name
                )));
                return;
            };
            let implementation_dispatch =
                program.normalized_result_dispatch_set(entry_state.return_type);
            let named_requirements = program
                .trait_machine_signatures(trait_definition)
                .iter()
                .filter(|requirement| requirement.name == *requirement_name)
                .collect::<Vec<_>>();
            if named_requirements.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` satisfies `{}::{}`, but trait `{}` has no requirement named `{}`",
                    machine.name,
                    trait_definition.name,
                    requirement_name,
                    trait_definition.name,
                    requirement_name
                )));
                return;
            }
            let matching_requirements = if named_requirements.len() == 1 {
                named_requirements
            } else {
                named_requirements
                    .into_iter()
                    .filter(|requirement| {
                        program.normalized_result_dispatch_set(requirement.return_type)
                            == implementation_dispatch
                    })
                    .collect::<Vec<_>>()
            };
            let [requirement] = matching_requirements.as_slice() else {
                let dispatch = if implementation_dispatch.is_empty() {
                    "<empty>".to_owned()
                } else {
                    implementation_dispatch.identity()
                };
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}`, but its entry result dispatch set `{dispatch}` selects {} matching requirement overload(s); exactly one is required",
            machine.name,
            trait_definition.name,
            requirement_name,
            matching_requirements.len(),
        )));
                return;
            };
            if requirement.symbol != settled_requirement_symbol {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}` but its retained exact requirement does not match overload resolution",
            machine.name, trait_definition.name, requirement_name,
        )));
                return;
            }

            let lifetimes = trait_lifetime_arguments
                .iter()
                .map(|ordinal| {
                    machine
                        .lifetime_parameters
                        .get(usize::try_from(*ordinal).ok()?)
                        .cloned()
                })
                .collect::<Option<Vec<_>>>();
            let Some(lifetimes) = lifetimes else {
                return;
            };
            let Some(application) = typed_trees::trait_definition::AppliedTraitScope::new(
                program.typed_program(),
                trait_definition,
                typed_trees::trait_definition::TraitApplicationArguments {
                    lexical_owner: machine.symbol,
                    enclosing_data_applications: &[],
                    arguments: explicit_type_arguments,
                    lifetimes: &lifetimes,
                },
            ) else {
                return;
            };
            validate_applied_trait_signature(
                program,
                service_reaches,
                machine,
                entry_state,
                &application,
                requirement,
                None,
                diagnostics,
            );

            validate_concrete_named_evidence_contract_conformance(
                program,
                machine,
                entry_state,
                trait_definition,
                requirement,
                explicit_type_arguments,
                diagnostics,
            );

            crate::proof_contracts::contract_entailment::check_law_conformance(
                program,
                machine,
                conformance_alias,
                &application,
                requirement,
                diagnostics,
                &mut Vec::new(),
            );
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_concrete_named_evidence_contract_conformance(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            entry_state: &State,
            trait_definition: &TraitDefinition,
            requirement: &StateSignature,
            explicit_type_arguments: &[TypeReferenceHandle],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let required_requires = named_contracts(
                program.state_signature_contracts(requirement),
                SignatureContractKind::Requires,
            );
            let required_ensures = named_contracts(
                program.state_signature_contracts(requirement),
                SignatureContractKind::Ensures,
            );
            let actual_requires = named_contracts(
                program.machine_contracts(machine),
                SignatureContractKind::Requires,
            );
            let actual_ensures = named_contracts(
                program.machine_contracts(machine),
                SignatureContractKind::Ensures,
            );
            if required_requires.is_empty()
                && required_ensures.is_empty()
                && actual_requires.is_empty()
                && actual_ensures.is_empty()
            {
                return;
            }

            if !explicit_type_arguments.is_empty()
                || !trait_definition.lifetime_parameters.is_empty()
                || !program.trait_type_parameters(trait_definition).is_empty()
                || !requirement.lifetime_parameters.is_empty()
                || !program
                    .state_signature_type_parameters(requirement)
                    .is_empty()
                || !machine.lifetime_parameters.is_empty()
                || !program.machine_type_parameters(machine).is_empty()
            {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}` with named evidence, but named-witness conformance currently requires a concrete non-generic trait, requirement, and satisfier",
            machine.name, trait_definition.name, requirement.name,
        )));
                return;
            }

            validate_named_contract_lane(
                program,
                machine,
                entry_state,
                trait_definition,
                requirement,
                SignatureContractKind::Requires,
                &required_requires,
                &actual_requires,
                false,
                diagnostics,
            );
            validate_named_contract_lane(
                program,
                machine,
                entry_state,
                trait_definition,
                requirement,
                SignatureContractKind::Ensures,
                &required_ensures,
                &actual_ensures,
                true,
                diagnostics,
            );
        }

        fn named_contracts(
            contracts: &[typed_trees::signature::SignatureContract],
            kind: SignatureContractKind,
        ) -> Vec<&typed_trees::signature::SignatureContract> {
            contracts
                .iter()
                .filter(|contract| contract.kind == kind && contract.binding.is_some())
                .collect()
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_named_contract_lane(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            entry_state: &State,
            trait_definition: &TraitDefinition,
            requirement: &StateSignature,
            kind: SignatureContractKind,
            required: &[&typed_trees::signature::SignatureContract],
            actual: &[&typed_trees::signature::SignatureContract],
            permits_strengthening: bool,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let lane = match kind {
                SignatureContractKind::Requires => "requires",
                SignatureContractKind::Ensures => "ensures",
                _ => unreachable!("named conformance lane is requires or ensures"),
            };
            let cardinality_matches = if permits_strengthening {
                actual.len() >= required.len()
            } else {
                actual.len() == required.len()
            };
            if !cardinality_matches {
                let expectation = if permits_strengthening {
                    format!("at least {}", required.len())
                } else {
                    required.len().to_string()
                };
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}` but its named {lane} lane has {} row(s); the requirement owns {expectation}",
            machine.name,
            trait_definition.name,
            requirement.name,
            actual.len(),
        )));
                return;
            }

            let parameter_substitutions =
                crate::declarations::state_signatures::signature_entry_substitutions(
                    program,
                    requirement,
                    entry_state,
                );

            for (lane_position, (required, actual)) in required.iter().zip(actual).enumerate() {
                let required_name = required
                    .binding
                    .as_ref()
                    .expect("named requirement row has a binding");
                let actual_name = actual
                    .binding
                    .as_ref()
                    .expect("named satisfier row has a binding");
                if kind == SignatureContractKind::Ensures && required_name != actual_name {
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but named ensures lane {} renames public selector `{required_name}` to `{actual_name}`",
                machine.name,
                trait_definition.name,
                requirement.name,
                lane_position,
            )));
                    continue;
                }

                let required_facts = program.proof_facts.span_or_empty(required.facts);
                let actual_facts = program.proof_facts.span_or_empty(actual.facts);
                let (
                    [ProofFact::Proposition(required_application)],
                    [ProofFact::Proposition(actual_application)],
                ) = (required_facts, actual_facts)
                else {
                    continue;
                };
                if !required_application.binder_arguments.is_empty()
                    || !actual_application.binder_arguments.is_empty()
                {
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but named {lane} lane {} uses a generic proposition telescope; this first conformance rung is concrete only",
                machine.name,
                trait_definition.name,
                requirement.name,
                lane_position,
            )));
                    continue;
                }
                let argument_labels = expression_table(program)
                    .expression_handles(required_application.arguments)
                    .iter()
                    .map(|argument| {
                        program.render_proof_expression(
                            *argument,
                            ProofSubstitutions::ByParameter(&parameter_substitutions),
                        )
                    })
                    .collect::<Vec<_>>();
                let expected = program.normalize_nominal_proposition_application(
                    required_application,
                    Some(PropositionLabels {
                        binder_labels: &[],
                        argument_labels: &argument_labels,
                    }),
                );
                let observed =
                    program.normalize_nominal_proposition_application(actual_application, None);
                if expected != observed {
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but named {lane} lane {} does not retain the requirement's exact proposition and evidence interface",
                machine.name,
                trait_definition.name,
                requirement.name,
                lane_position,
            )));
                }
            }
        }

        pub fn compose_forwarded_trait_arguments(
            program: &dyn crate::CheckingInput,
            source_trait: &TraitDefinition,
            source_arguments: &[TypeReferenceHandle],
            parent_arguments: &[TypeReferenceHandle],
        ) -> Vec<TypeReferenceHandle> {
            let source_parameters = program.trait_type_parameters(source_trait);
            parent_arguments
                .iter()
                .map(|argument| {
                    let TypeReferenceNode::Named { symbol, name } =
                        program.type_reference_table.type_reference(*argument)
                    else {
                        return *argument;
                    };
                    source_parameters
                        .iter()
                        .zip(source_arguments.iter())
                        .find(|(parameter, _)| {
                            (parameter.symbol.is_valid() && parameter.symbol == *symbol)
                                || parameter.name.as_str() == name.as_str()
                        })
                        .map(|(_, concrete)| *concrete)
                        .unwrap_or(*argument)
                })
                .collect()
        }
    }
    pub(crate) mod signature_matching {
        pub(crate) mod borrow_refinement {
            use crate::borrow_contracts::CallableBorrowContract;
            use typed_trees::data::TypeParameter;
            use typed_trees::machine::Machine;
            use typed_trees::signature::{StateParameter, StateSignature};
            use typed_trees::state::State;
            use typed_trees::trait_definition::AppliedTraitScope;

            pub(crate) fn with_contracts<R>(
                program: &dyn crate::CheckingInput,
                application: &AppliedTraitScope<'_>,
                requirement: &StateSignature,
                machine: &Machine,
                state: &State,
                actual_parameters: &[StateParameter],
                actual_type_parameters: &[TypeParameter],
                judge: impl FnOnce(
                    CallableBorrowContract<'_>,
                    CallableBorrowContract<'_>,
                ) -> Result<R, String>,
            ) -> Result<R, String> {
                let binders = program
                    .state_signature_type_parameters(requirement)
                    .iter()
                    .zip(actual_type_parameters)
                    .enumerate()
                    .flat_map(|(ordinal, (required, actual))| {
                        let name = format!("$conformance{ordinal}");
                        [(required.symbol, name.clone()), (actual.symbol, name)]
                    })
                    .collect::<Vec<_>>();
                application.with_identity_scopes(program.typed_program(), |scopes| {
                    judge(
                        CallableBorrowContract {
                            lexical_owner: requirement.symbol,
                            lifetime_parameters: &requirement.lifetime_parameters,
                            parameters: program.state_signature_parameters(requirement),
                            return_type: requirement.return_type,
                            substitutions: &application.substitutions,
                            substitution_scope_starts: &application.substitution_scope_starts,
                            application_scopes: scopes,
                            binders: &binders,
                            lifetime_applications: &[],
                            argument_owner: application.argument_owner,
                        },
                        CallableBorrowContract {
                            lexical_owner: machine.symbol,
                            lifetime_parameters: &machine.lifetime_parameters,
                            parameters: actual_parameters,
                            return_type: state.return_type,
                            substitutions: &[],
                            substitution_scope_starts: &[],
                            application_scopes: &[],
                            binders: &binders,
                            lifetime_applications: &[],
                            argument_owner: machine.symbol,
                        },
                    )
                })
            }
        }
        pub(crate) mod evidence {
            use crate::conformance_applications::{
                applied_bound_argument_identities_in_region_scope, validate_forwarded_argument,
            };
            use diagnostics::Diagnostic;
            use typed_trees::{
                data::{TypeParameter, TypeParameterKind},
                expression::StaticMachineArgument,
                machine::Machine,
                signature::StateSignature,
                trait_definition::AppliedTraitScope,
                type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument},
            };

            pub(crate) fn validate(
                program: &dyn crate::CheckingInput,
                application: &AppliedTraitScope<'_>,
                requirement: &StateSignature,
                provider: &Machine,
                actual_parameters: &[TypeParameter],
                frame: &crate::borrow_contracts::CallableLifetimeFrame,
                diagnostics: &mut Vec<Diagnostic>,
            ) {
                let required_parameters = program.state_signature_type_parameters(requirement);
                if !required_parameters
                    .iter()
                    .any(|parameter| parameter.kind == TypeParameterKind::Implementation)
                {
                    return;
                }
                let actuals = actual_parameters
                    .iter()
                    .map(|parameter| StaticMachineArgument {
                        symbol: parameter.symbol,
                        path: vec![parameter.name.clone()].into_boxed_slice(),
                        ..Default::default()
                    })
                    .collect::<Vec<_>>();
                let operands = required_parameters
                    .iter()
                    .zip(&actuals)
                    .map(|(parameter, actual)| {
                        (
                            parameter.symbol,
                            IndexIdentityArgument::StaticArgument(actual),
                        )
                    })
                    .collect::<Vec<_>>();
                let required_regions = frame
                    .required
                    .iter()
                    .map(|binding| (binding.parameter, binding.argument))
                    .collect::<Vec<_>>();
                let actual_regions = frame
                    .actual
                    .iter()
                    .map(|binding| (binding.parameter, binding.argument))
                    .collect::<Vec<_>>();
                application.with_identity_scopes(program.typed_program(), |outer| {
        let mut scopes = outer.to_vec();
        scopes.push(AppliedTypeIdentityScope {
            declaration: requirement.symbol,
            arguments: &operands,
            lifetimes: &[],
        });
        for (required, actual) in required_parameters.iter().zip(actual_parameters) {
            if required.kind != TypeParameterKind::Implementation { continue; }
            let Some(demanded) = program.callable_conformance_bound(provider.symbol, actual.symbol) else {
                diagnostics.push(Diagnostic::error(format!(
                    "provider `{}` evidence parameter `{}` has no exact declared bound",
                    provider.name, actual.name,
                )));
                continue;
            };
            let selected = StaticMachineArgument {
                symbol: required.symbol,
                path: vec![required.name.clone()].into_boxed_slice(),
                ..Default::default()
            };
            let expected = applied_bound_argument_identities_in_region_scope(
                program, demanded, provider.symbol, provider.symbol, &[], &actual_regions,
            );
            if let Err(reason) = validate_forwarded_argument(
                program, demanded, &selected, |promised| {
                    expected.as_ref().zip(
                        applied_bound_argument_identities_in_region_scope(
                            program, promised, requirement.symbol, provider.symbol, &scopes, &required_regions,
                        ).as_ref(),
                    ).is_some_and(|(demanded, promised)| demanded == promised)
                },
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "provider `{}` does not refine requirement `{}` evidence parameter `{}`: {reason}",
                    provider.name, requirement.name, required.name,
                )));
            }
        }
    });
            }
        }
        pub(crate) mod region_application {
            use super::{TraitTypeBinding, TraitTypeBindingTarget};
            use crate::queries::type_regions::{ReferenceStep, TypeRegionScope};
            use typed_trees::type_identity::{NormalizedTypeIdentity, TypeIdentityRequest};
            use typed_trees::types::TypeReferenceHandle;

            #[derive(Clone, Copy)]
            pub(crate) struct RegionApplicationScope<'scope> {
                bindings: &'scope [TraitTypeBinding],
                provider: bool,
            }

            impl<'scope> RegionApplicationScope<'scope> {
                pub(crate) fn requirement(bindings: &'scope [TraitTypeBinding]) -> Self {
                    Self {
                        bindings,
                        provider: false,
                    }
                }

                pub(crate) fn provider(bindings: &'scope [TraitTypeBinding]) -> Self {
                    Self {
                        bindings,
                        provider: true,
                    }
                }

                pub(crate) fn is_provider(self) -> bool {
                    self.provider
                }
            }

            impl TypeRegionScope for RegionApplicationScope<'_> {
                fn erased_identity(
                    self,
                    program: &dyn crate::CheckingInput,
                    reference: TypeReferenceHandle,
                ) -> NormalizedTypeIdentity {
                    let mut substitutions = Vec::new();
                    let mut starts = Vec::new();
                    let mut binders = Vec::new();
                    for (ordinal, binding) in self.bindings.iter().enumerate() {
                        match binding.target {
                            TraitTypeBindingTarget::Type(argument) if !self.provider => {
                                substitutions.push((binding.parameter_symbol, argument));
                                let start = self.bindings[..binding.scope_start]
                                    .iter()
                                    .filter(|binding| {
                                        matches!(binding.target, TraitTypeBindingTarget::Type(_))
                                    })
                                    .count();
                                if starts.last() != Some(&start) {
                                    starts.push(start);
                                }
                            }
                            TraitTypeBindingTarget::Parameter(actual) => {
                                let label = format!("$conformance{ordinal}");
                                binders.push((binding.parameter_symbol, label.clone()));
                                binders.push((actual, label));
                            }
                            _ => {}
                        }
                    }
                    program.scoped_type_identity(
                        TypeIdentityRequest {
                            substitutions: &substitutions,
                            binders: &binders,
                            ..TypeIdentityRequest::ordinary(reference)
                        },
                        &starts,
                    )
                }

                fn named_alias(
                    mut self,
                    _program: &dyn crate::CheckingInput,
                    symbol: symbols::SymbolHandle,
                    _reference: TypeReferenceHandle,
                ) -> Option<ReferenceStep<Self>> {
                    if self.provider {
                        return None;
                    }
                    let bindings = self.bindings;
                    let binding = bindings
                        .iter()
                        .rev()
                        .find(|binding| binding.parameter_symbol == symbol)?;
                    let TraitTypeBindingTarget::Type(argument) = binding.target else {
                        return None;
                    };
                    let bindings = &bindings[..binding.scope_start];
                    self.bindings = bindings;
                    self.provider = bindings.is_empty();
                    Some(ReferenceStep::Chase(self, argument))
                }
            }
        }

        use crate::value_custody::type_references::{type_reference_label, type_references_match};
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::data::{TypeParameter, TypeParameterKind};
        use typed_trees::domain::ProofFact;
        use typed_trees::machine::Machine;
        use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
        use typed_trees::state::State;
        use typed_trees::trait_definition::{AppliedTraitScope, TraitDefinition};
        use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

        fn specialized_instance_realized_tail(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
        ) -> usize {
            let Some(application) = program.machine_specialization(machine.symbol) else {
                return 0;
            };
            let Some(template) = program.machine(application.template) else {
                return 0;
            };
            let Some(state_ordinal) =
                typed_trees::state::state_position(program.machine_states(machine), state.symbol)
            else {
                return 0;
            };
            let Some(template_state) = program.machine_states(template).get(state_ordinal) else {
                return 0;
            };
            let realized = program
                .state_parameters(state)
                .len()
                .saturating_sub(program.state_parameters(template_state).len());
            if realized == 0 {
                return 0;
            }
            let carriers = program
                .data_type_parameters
                .span_or_empty(application.template_parameters)
                .iter()
                .filter_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Value { type_reference } => Some(*type_reference),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let parameters = program.state_parameters(state);
            let trailing = &parameters[parameters.len() - realized..];
            let mut cursor = 0usize;
            let mut matched = 0usize;
            for parameter in trailing {
                while cursor < carriers.len() {
                    let carrier = carriers[cursor];
                    cursor += 1;
                    if type_references_match(program, parameter.type_reference, carrier) {
                        matched += 1;
                        break;
                    }
                }
            }
            if matched == realized { realized } else { 0 }
        }

        pub(crate) fn satisfier_parameter_correspondence(
            satisfier: &str,
            requirement: &str,
            declared: usize,
            required: usize,
        ) -> Result<(), Diagnostic> {
            if declared > required {
                return Err(Diagnostic::error(format!(
                    "{satisfier} declares {declared} parameter(s), but {requirement} declares {required}: a satisfier declares no parameter its requirement lacks; a provider holds no state, so state that outlives a call passes through the requirement's declared parameters"
                )));
            }
            if declared < required {
                return Err(Diagnostic::error(format!(
                    "{satisfier} declares {declared} parameter(s), but {requirement} declares {required}: a satisfier declares every parameter its requirement declares"
                )));
            }
            Ok(())
        }

        pub(crate) fn authored_satisfier_parameters<'program>(
            program: &'program dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
        ) -> &'program [StateParameter] {
            let parameters = program.state_parameters(state);
            let realized_tail = specialized_instance_realized_tail(program, machine, state);
            &parameters[..parameters.len() - realized_tail]
        }

        pub(crate) fn authored_conformance_machine<'program>(
            program: &'program dyn crate::CheckingInput,
            machine: &'program Machine,
        ) -> Result<&'program Machine, Diagnostic> {
            if !program.machine_type_parameters(machine).is_empty()
                || program.machine_trait_conformances(machine).is_empty()
            {
                return Ok(machine);
            }
            let mut applications = program
                .machine_specializations
                .iter()
                .filter(|application| application.instance == machine.symbol);
            let Some(application) = applications.next() else {
                return Ok(machine);
            };
            let template = program
                .machines_with_symbol(application.template)
                .find(|template| {
                    template.symbol != machine.symbol
                        && template.type_parameters == application.template_parameters
                        && !program.machine_type_parameters(template).is_empty()
                        && program.machine_trait_conformances(template)
                            == program.machine_trait_conformances(machine)
                });
            template
        .filter(|_| applications.next().is_none())
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "machine `{}` generic satisfaction lost its exact original family and application",
                machine.name,
            ))
        })
        }

        /// A requirement that wrote `crashes none` admits only crash-free
        /// members (generics.md, "`crashes none` admitting only crash-free
        /// members"): callers through it, and a generic body checked against
        /// it, assume the call never crashes. The member must therefore
        /// publish an empty crash ceiling. A crash route it states breaks the
        /// requirement. An inline member that states none is held to the empty
        /// ceiling by `machine_publishes_crash_ceiling`, so its body is
        /// checked like any `crashes none` body. A member declared elsewhere
        /// that leaves its routes to inference makes no such promise and is
        /// refused. An omitted `crashes` on the requirement defers to the
        /// member's routes instead, and is not judged here.
        fn validate_crash_free_member(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            trait_definition: &TraitDefinition,
            requirement: &StateSignature,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if !requirement.declares_no_crash_route {
                return;
            }
            let member = || {
                format!(
                    "machine `{}` satisfies `{}::{}`, which declares `crashes none`",
                    machine.name, trait_definition.name, requirement.name
                )
            };
            if program.machine_declares_crash(machine) {
                diagnostics.push(Diagnostic::error(format!(
                    "{}, but it publishes a crash route; a member of a `crashes none` requirement must be crash-free",
                    member()
                )));
            } else if !program.machine_publishes_crash_ceiling(machine) {
                diagnostics.push(Diagnostic::error(format!(
                    "{}, but it leaves its crash routes to inference; write `crashes none` on it so its body is checked crash-free",
                    member()
                )));
            }
        }

        pub(crate) fn validate_applied_trait_signature(
            program: &dyn crate::CheckingInput,
            service_reaches: &flow_effects::ServiceReachInferencePlan,
            machine: &Machine,
            state: &State,
            application: &AppliedTraitScope<'_>,
            requirement: &StateSignature,
            selecting: Option<&typed_trees::trait_definition::Conformance>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let trait_definition = application.declaration;
            let explicit_type_arguments = program
                .trait_type_parameters(trait_definition)
                .iter()
                .map(|parameter| {
                    application.parameter_argument(program.typed_program(), parameter.symbol)
                })
                .collect::<Vec<_>>();
            let callable_machine = match authored_conformance_machine(program, machine) {
                Ok(owner) => owner,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    return;
                }
            };
            validate_crash_free_member(
                program,
                callable_machine,
                trait_definition,
                requirement,
                diagnostics,
            );
            let callable_state = if callable_machine.symbol == machine.symbol {
                state
            } else {
                let Some(source_state) = typed_trees::state::state_position(
                    program.machine_states(machine),
                    state.symbol,
                )
                .and_then(|ordinal| program.machine_states(callable_machine).get(ordinal)) else {
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{}` generic satisfaction lost its exact original family and application",
                machine.name,
            )));
                    return;
                };
                source_state
            };
            let actual_parameters =
                authored_satisfier_parameters(program, callable_machine, callable_state);
            let required_parameters = program.state_signature_parameters(requirement);
            if let Err(diagnostic) = satisfier_parameter_correspondence(
                &format!("machine `{}` state `{}`", machine.name, state.name),
                &format!(
                    "trait `{}` machine `{}`",
                    trait_definition.name, requirement.name
                ),
                actual_parameters.len(),
                required_parameters.len(),
            ) {
                diagnostics.push(diagnostic);
                return;
            }

            let trait_type_parameters = program.trait_type_parameters(trait_definition);
            let requirement_type_parameters = program.state_signature_type_parameters(requirement);
            let applied_type_parameters = if requirement.type_parameters.is_empty() {
                program.state_signature_type_parameters(requirement)
            } else {
                let Some(prepared) = program
                    .typed_program()
                    .applied_callable_signature(application, requirement.symbol)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                "trait requirement `{}::{}` has no prepared callable telescope for its exact application",
                trait_definition.name, requirement.name
            )));
                    return;
                };
                program.state_signature_type_parameters(prepared)
            };
            let callable_parameters = program.machine_callable_parameters(callable_machine);
            let invocation_parameters =
                crate::machine_calls::static_arguments::invocation_parameters(
                    program,
                    machine,
                    callable_machine,
                    &callable_parameters,
                    trait_definition.symbol,
                    requirement.symbol,
                );
            let actual_type_parameters =
                program.requirement_callable_parameters(requirement, &invocation_parameters);
            let indexed_law_telescope_groups = indexed_law_callable_telescope_groups(
                program,
                trait_definition,
                requirement,
                &explicit_type_arguments,
            );
            let expected_callable_parameter_count = requirement_type_parameters.len()
                + indexed_law_telescope_groups
                    .iter()
                    .map(|group| group.len())
                    .sum::<usize>();
            if expected_callable_parameter_count != actual_type_parameters.len() {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not satisfy trait `{}` machine `{}`: expected {} callable generic parameter(s), got {}",
            machine.name,
            trait_definition.name,
            requirement.name,
            expected_callable_parameter_count,
            actual_type_parameters.len(),
        )));
                return;
            }
            let effective_requirement_type_parameters = requirement_type_parameters
                .iter()
                .chain(
                    indexed_law_telescope_groups
                        .iter()
                        .flat_map(|group| group.iter()),
                )
                .collect::<Vec<_>>();
            if let Some((index, (required, actual))) = effective_requirement_type_parameters
                .iter()
                .zip(actual_type_parameters.iter())
                .enumerate()
                .find(|(_, (required, actual))| {
                    !super::type_parameter_kinds_callable(&required.kind, &actual.kind)
                })
            {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not satisfy trait `{}` machine `{}`: callable generic parameter {} has incompatible kinds (`{}` versus `{}`)",
            machine.name,
            trait_definition.name,
            requirement.name,
            index,
            required.name,
            actual.name,
        )));
                return;
            }
            let mut required_generic_types = trait_type_parameters.to_vec();
            required_generic_types.extend(
                requirement_type_parameters
                    .iter()
                    .filter(|parameter| {
                        matches!(
                            parameter.kind,
                            TypeParameterKind::Type
                                | TypeParameterKind::Const { .. }
                                | TypeParameterKind::Value { .. }
                        )
                    })
                    .cloned(),
            );
            let applied_generic_types = applied_type_parameters
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect::<Vec<_>>();
            let mut type_bindings = application
                .substitutions
                .iter()
                .enumerate()
                .map(|(ordinal, (parameter, actual))| TraitTypeBinding {
                    parameter_symbol: *parameter,
                    parameter_name: program.symbols.name(*parameter).to_owned(),
                    target: TraitTypeBindingTarget::Type(*actual),
                    scope_start: application
                        .substitution_scope_starts
                        .iter()
                        .rev()
                        .find(|start| **start <= ordinal)
                        .copied()
                        .unwrap_or(0),
                })
                .collect::<Vec<_>>();
            type_bindings.extend(
                super::type_parameter_binding_pairs(
                    requirement_type_parameters,
                    &actual_type_parameters,
                )
                .map(|(required, actual)| TraitTypeBinding {
                    parameter_symbol: required.symbol,
                    parameter_name: required.name.as_str().to_owned(),
                    target: TraitTypeBindingTarget::Parameter(actual.symbol),
                    scope_start: 0,
                }),
            );
            for (index, (required, actual)) in requirement_type_parameters
                .iter()
                .zip(actual_type_parameters.iter())
                .enumerate()
            {
                match (&required.kind, &actual.kind) {
                    (TypeParameterKind::Type, TypeParameterKind::Type) => {
                        if typed_trees::data::type_parameter_demands_stronger_properties(
                            required, actual,
                        ) {
                            diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` does not satisfy trait `{}` machine `{}`: callable generic parameter {} demands stronger type properties",
                        machine.name, trait_definition.name, requirement.name, index
                    )));
                        }
                    }
                    _ => {
                        let Some((required_type, actual_type)) =
                            crate::value_custody::type_shapes::scalar_parameter_type_pair(
                                &required.kind,
                                &actual.kind,
                            )
                        else {
                            continue;
                        };
                        if !type_references_match_for_trait_bindings(
                            program,
                            actual_type,
                            required_type,
                            &required_generic_types,
                            &mut type_bindings,
                        ) {
                            diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` does not satisfy trait `{}` machine `{}`: callable const generic parameter {} has a different type",
                        machine.name, trait_definition.name, requirement.name, index
                    )));
                        }
                    }
                }
            }

            type_bindings.extend(
                required_parameters
                    .iter()
                    .zip(actual_parameters.iter())
                    .map(|(required, actual)| TraitTypeBinding {
                        parameter_symbol: required.symbol,
                        parameter_name: required.name.to_string(),
                        target: TraitTypeBindingTarget::Parameter(actual.symbol),
                        scope_start: 0,
                    }),
            );

            for (index, (actual, required)) in actual_parameters
                .iter()
                .zip(required_parameters.iter())
                .enumerate()
            {
                validate_trait_parameter_match(
                    program,
                    machine,
                    state,
                    trait_definition.name.as_str(),
                    requirement,
                    &required_generic_types,
                    &mut type_bindings,
                    index,
                    actual,
                    required,
                    diagnostics,
                );
            }

            if !type_references_match_for_trait_bindings(
                program,
                callable_state.return_type,
                requirement.return_type,
                &required_generic_types,
                &mut type_bindings,
            ) {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: expected return `{}`, got `{}`",
            machine.name,
            state.name,
            trait_definition.name,
            requirement.name,
            type_reference_label(program, requirement.return_type),
            type_reference_label(program, callable_state.return_type)
        )));
            }
            if let Err(message) = borrow_refinement::with_contracts(
                program,
                application,
                requirement,
                callable_machine,
                callable_state,
                actual_parameters,
                &actual_type_parameters,
                |required, actual| {
                    let prepared =
                        crate::borrow_contracts::PreparedCallableBorrowRefinement::prepare(
                            program, &required, &actual,
                        )?;
                    let frame = prepared.lifetime_frame(program, &required, &actual)?;
                    evidence::validate(
                        program,
                        application,
                        requirement,
                        callable_machine,
                        &actual_type_parameters,
                        &frame,
                        diagnostics,
                    );
                    crate::machine_calls::machine_parameters::validate_trait_callable_parameter_refinement(
                program,
                &format!(
                    "provider machine `{}` for trait requirement `{}::{}`",
                    machine.name, trait_definition.name, requirement.name
                ),
                &frame,
                applied_type_parameters,
                &actual_type_parameters[..requirement_type_parameters.len()],
                &applied_generic_types,
                diagnostics,
            );
                    let mut actual_offset = requirement_type_parameters.len();
                    for (representative, carrier_telescope) in
                        indexed_law_telescope_groups.iter().enumerate()
                    {
                        let actual_end = actual_offset + carrier_telescope.len();
                        let actual_group = &actual_type_parameters[actual_offset..actual_end];
                        let carrier_generic_parameters =
                            carrier_telescope.iter().collect::<Vec<_>>();
                        crate::machine_calls::machine_parameters::validate_trait_callable_parameter_refinement(
                    program,
                    &format!(
                        "provider machine `{}` indexed representative {} for proposition-law requirement `{}::{}`",
                        machine.name,
                        representative + 1,
                        trait_definition.name,
                        requirement.name
                    ),
                    &frame,
                    carrier_telescope,
                    actual_group,
                    &carrier_generic_parameters,
                    diagnostics,
                );
                        actual_offset = actual_end;
                    }
                    prepared.validate(program, required, actual)
                },
            ) {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: conformance borrow contract: {message}",
            machine.name, state.name, trait_definition.name, requirement.name,
        )));
            }

            crate::proof_contracts::contract_entailment::check_contract_refinement(
                program,
                callable_machine,
                callable_state,
                actual_parameters,
                &actual_type_parameters,
                application,
                requirement,
                selecting,
                diagnostics,
            );
            validate_trait_effect_ceiling(
                program,
                service_reaches,
                machine,
                state,
                trait_definition.name.as_str(),
                requirement,
                diagnostics,
            );
        }

        fn validate_trait_effect_ceiling(
            program: &dyn crate::CheckingInput,
            service_reaches: &flow_effects::ServiceReachInferencePlan,
            machine: &Machine,
            state: &State,
            trait_name: &str,
            requirement: &StateSignature,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let allowed_services = crate::machine_calls::effect_inference::signature_service_reach(
                program,
                requirement,
            );
            let operational =
                crate::machine_calls::effect_inference::signature_operational(program, requirement);
            super::require_operational_ceiling(
                program,
                service_reaches,
                machine,
                &allowed_services,
                operational.may_suspend,
                operational.may_block,
                |breach| match breach {
                    super::OperationalCeilingBreach::ReachedService(service_name) => format!(
                        "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: service `{service_name}` is not allowed by the trait requirement",
                        machine.name, state.name, trait_name, requirement.name,
                    ),
                    super::OperationalCeilingBreach::Suspends => format!(
                        "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: `suspends` exceeds the trait requirement's operational ceiling",
                        machine.name, state.name, trait_name, requirement.name,
                    ),
                    super::OperationalCeilingBreach::Blocks => format!(
                        "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: `blocks` exceeds the trait requirement's operational ceiling",
                        machine.name, state.name, trait_name, requirement.name,
                    ),
                },
                diagnostics,
            );
        }

        fn validate_trait_parameter_match(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            trait_name: &str,
            requirement: &StateSignature,
            trait_type_parameters: &[TypeParameter],
            type_bindings: &mut Vec<TraitTypeBinding>,
            index: usize,
            actual: &StateParameter,
            required: &StateParameter,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if actual.is_mutable != required.is_mutable {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter {}: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            index,
            parameter_shape_label(program, required),
            parameter_shape_label(program, actual)
        )));
                return;
            }

            if !type_references_match_for_trait_bindings(
                program,
                actual.type_reference,
                required.type_reference,
                trait_type_parameters,
                type_bindings,
            ) {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter `{}`: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            required.name,
            bound_type_reference_label(program, required.type_reference, type_bindings),
            type_reference_label(program, actual.type_reference)
        )));
            }
        }

        fn bound_type_reference_label(
            program: &dyn crate::CheckingInput,
            handle: TypeReferenceHandle,
            bindings: &[TraitTypeBinding],
        ) -> String {
            let mut label = type_reference_label(program, handle);
            for binding in bindings {
                let TraitTypeBindingTarget::Type(argument) = binding.target else {
                    continue;
                };
                if binding.parameter_name.is_empty() {
                    continue;
                }
                let replacement = type_reference_label(program, argument);
                let mut rebuilt = String::with_capacity(label.len());
                let mut rest = label.as_str();
                while let Some(index) = rest.find(binding.parameter_name.as_str()) {
                    let end = index + binding.parameter_name.len();
                    let word = |character: Option<char>| {
                        character.is_some_and(|character| {
                            character.is_alphanumeric() || character == '_'
                        })
                    };
                    let bounded = !word(rest[..index].chars().next_back())
                        && !word(rest[end..].chars().next());
                    rebuilt.push_str(&rest[..index]);
                    rebuilt.push_str(if bounded {
                        &replacement
                    } else {
                        &binding.parameter_name
                    });
                    rest = &rest[end..];
                }
                rebuilt.push_str(rest);
                label = rebuilt;
            }
            label
        }

        #[derive(Debug, Clone)]
        pub(crate) struct TraitTypeBinding {
            pub(crate) parameter_symbol: SymbolHandle,
            pub(crate) parameter_name: String,
            pub(crate) target: TraitTypeBindingTarget,
            pub(crate) scope_start: usize,
        }

        #[derive(Debug, Clone)]
        pub(crate) enum TraitTypeBindingTarget {
            Type(TypeReferenceHandle),
            Parameter(SymbolHandle),
        }

        pub(crate) fn trait_type_bindings<'a>(
            parameters: impl Iterator<Item = &'a TypeParameter>,
            arguments: impl Iterator<Item = &'a TypeReferenceHandle>,
        ) -> Vec<TraitTypeBinding> {
            parameters
                .zip(arguments)
                .map(|(parameter, argument)| TraitTypeBinding {
                    parameter_symbol: parameter.symbol,
                    parameter_name: parameter.name.to_string(),
                    target: TraitTypeBindingTarget::Type(*argument),
                    scope_start: 0,
                })
                .collect()
        }

        pub(crate) fn type_references_match_for_trait_bindings(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            trait_type_parameters: &[TypeParameter],
            bindings: &mut Vec<TraitTypeBinding>,
        ) -> bool {
            if !actual.is_valid() || !required.is_valid() {
                return actual.is_valid() == required.is_valid();
            }

            if let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(required)
                && symbol.is_valid()
                && let Some(binding) = bindings
                    .iter()
                    .find(|binding| binding.parameter_symbol == *symbol)
                && binding_matches_actual(program, binding, actual, bindings)
            {
                return true;
            }
            if let Some(parameter) =
                required_trait_type_parameter(program, required, trait_type_parameters)
            {
                if let Some(binding) = bindings.iter().find(|binding| {
                    binding.parameter_symbol == parameter.symbol
                        && binding.parameter_name == parameter.name.as_str()
                }) {
                    return binding_matches_actual(program, binding, actual, bindings);
                }

                bindings.push(TraitTypeBinding {
                    parameter_symbol: parameter.symbol,
                    parameter_name: parameter.name.to_string(),
                    target: TraitTypeBindingTarget::Type(actual),
                    scope_start: 0,
                });
                return true;
            }

            macro_rules! match_types {
                ($actual:expr, $required:expr $(,)?) => {
                    type_references_match_for_trait_bindings(
                        program,
                        $actual,
                        $required,
                        trait_type_parameters,
                        bindings,
                    )
                };
            }

            match (
                program.type_reference_table.type_reference(actual),
                program.type_reference_table.type_reference(required),
            ) {
                (
                    TypeReferenceNode::Reference {
                        referee: actual_referee,
                        access: actual_access,
                        lifetime: _,
                    },
                    TypeReferenceNode::Reference {
                        referee: required_referee,
                        access: required_access,
                        lifetime: _,
                    },
                ) => {
                    actual_access == required_access
                        && match_types!(*actual_referee, *required_referee)
                }
                typed_trees::constrained_type_reference_pair!(actual_base, required_base) => {
                    match_types!(*actual_base, *required_base)
                }
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
                    fixed_array_lengths_match(program, actual_length, required_length, bindings)
                        && match_types!(*actual_element, *required_element)
                }
                (
                    TypeReferenceNode::Slice {
                        element_type: actual_element,
                    },
                    TypeReferenceNode::Slice {
                        element_type: required_element,
                    },
                ) => match_types!(*actual_element, *required_element),
                (
                    TypeReferenceNode::Generic {
                        base_symbol: actual_base_symbol,
                        arguments: actual_arguments,
                        ..
                    },
                    TypeReferenceNode::Generic {
                        base_symbol: required_base_symbol,
                        arguments: required_arguments,
                        ..
                    },
                )
                | (
                    TypeReferenceNode::DynamicTrait {
                        symbol: actual_base_symbol,
                        arguments: actual_arguments,
                        ..
                    },
                    TypeReferenceNode::DynamicTrait {
                        symbol: required_base_symbol,
                        arguments: required_arguments,
                        ..
                    },
                ) => {
                    actual_base_symbol.is_valid()
                        && actual_base_symbol == required_base_symbol
                        && actual_arguments.count() == required_arguments.count()
                        && program
                            .type_reference_table
                            .type_reference_handles(*actual_arguments)
                            .iter()
                            .zip(
                                program
                                    .type_reference_table
                                    .type_reference_handles(*required_arguments)
                                    .iter(),
                            )
                            .all(|(actual_argument, required_argument)| {
                                match_types!(*actual_argument, *required_argument)
                            })
                }
                (
                    TypeReferenceNode::Named {
                        symbol: actual_symbol,
                        ..
                    },
                    TypeReferenceNode::Generic { .. },
                ) if actual_symbol.is_valid() => {
                    crate::declarations::data_queries::data_definition_by_symbol(
                        program,
                        *actual_symbol,
                    )
                    .and_then(|definition| definition.generic_instance)
                    .is_some_and(|origin| match_types!(origin, required))
                }
                (TypeReferenceNode::ConstExpression(_), _)
                | (_, TypeReferenceNode::ConstExpression(_)) => {
                    let pairs = bindings
                        .iter()
                        .filter_map(|binding| match binding.target {
                            TraitTypeBindingTarget::Parameter(actual) => {
                                Some((binding.parameter_symbol, actual))
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let required_binders = pairs
                        .iter()
                        .enumerate()
                        .map(|(ordinal, (required, _))| {
                            (*required, format!("callable-value:{ordinal}"))
                        })
                        .collect::<Vec<_>>();
                    let actual_binders = pairs
                        .iter()
                        .enumerate()
                        .map(|(ordinal, (_, actual))| {
                            (*actual, format!("callable-value:{ordinal}"))
                        })
                        .collect::<Vec<_>>();
                    let identity = |reference, binders: &[(symbols::SymbolHandle, String)]| {
                        typed_trees::type_identity::try_normalized_index_identity(
                            program,
                            typed_trees::type_identity::IndexIdentityArgument::TypeReference(
                                reference,
                            ),
                            binders,
                            &[],
                        )
                    };
                    identity(actual, &actual_binders)
                        .zip(identity(required, &required_binders))
                        .is_some_and(|(actual, required)| actual == required)
                }
                _ => type_references_match(program, actual, required),
            }
        }

        pub(crate) fn type_reference_lifetimes_match_requirement_application(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            target_lifetimes: &[typed_trees::name::Identifier],
            machine: &Machine,
            raw_application: &[u32],
            bindings: &[TraitTypeBinding],
        ) -> bool {
            let required_scope = region_application::RegionApplicationScope::requirement(bindings);
            let actual_scope = region_application::RegionApplicationScope::provider(bindings);
            crate::queries::type_regions::type_reference_regions_match(
                program,
                actual,
                required,
                actual_scope,
                required_scope,
                &mut (),
                &mut |_, _, required_scope, _, actual, required| {
                    if required_scope.is_provider() {
                        required.is_none_or(|required| actual == Some(required))
                    } else {
                        requirement_lifetime_matches(
                            actual,
                            required,
                            target_lifetimes,
                            machine,
                            raw_application,
                        )
                    }
                },
            )
        }

        fn requirement_lifetime_matches(
            actual: Option<&typed_trees::name::Identifier>,
            required: Option<&typed_trees::name::Identifier>,
            target_lifetimes: &[typed_trees::name::Identifier],
            machine: &Machine,
            raw_application: &[u32],
        ) -> bool {
            let Some(required) = required else {
                return true;
            };
            let Some(trait_ordinal) = target_lifetimes
                .iter()
                .position(|parameter| parameter.as_str() == required.as_str())
            else {
                return true;
            };
            let Some(machine_ordinal) = raw_application
                .get(trait_ordinal)
                .and_then(|ordinal| usize::try_from(*ordinal).ok())
            else {
                return false;
            };
            let Some(expected) = machine.lifetime_parameters.get(machine_ordinal) else {
                return false;
            };
            actual.is_some_and(|actual| actual.as_str() == expected.as_str())
        }

        fn fixed_array_lengths_match(
            program: &dyn crate::CheckingInput,
            actual: &FixedArrayLength,
            required: &FixedArrayLength,
            bindings: &[TraitTypeBinding],
        ) -> bool {
            let FixedArrayLength::ConstParameter {
                symbol: required_symbol,
                name: required_name,
            } = required
            else {
                return actual == required;
            };
            let Some(binding) = bindings.iter().find(|binding| {
                binding.parameter_symbol == *required_symbol
                    && binding.parameter_name == required_name.as_str()
            }) else {
                return actual == required;
            };
            match binding.target {
                TraitTypeBindingTarget::Parameter(expected_symbol) => matches!(
                    actual,
                    FixedArrayLength::ConstParameter { symbol, .. }
                        if expected_symbol.is_valid() && *symbol == expected_symbol
                ),
                TraitTypeBindingTarget::Type(expected) => {
                    let Some(outer) = bindings.get(..binding.scope_start) else {
                        return false;
                    };
                    let mut substitutions = Vec::with_capacity(outer.len() + 1);
                    let mut scope_starts = Vec::new();
                    for (ordinal, outer) in outer.iter().enumerate() {
                        let TraitTypeBindingTarget::Type(argument) = outer.target else {
                            return false;
                        };
                        if outer.scope_start > ordinal
                            || scope_starts
                                .last()
                                .is_some_and(|start| *start > outer.scope_start)
                        {
                            return false;
                        }
                        if scope_starts.last() != Some(&outer.scope_start) {
                            scope_starts.push(outer.scope_start);
                        }
                        substitutions.push((outer.parameter_symbol, argument));
                    }
                    scope_starts.push(substitutions.len());
                    substitutions.push((binding.parameter_symbol, expected));
                    program
                        .scoped_fixed_array_length_identity(actual, &[], &[])
                        .zip(program.scoped_fixed_array_length_identity(
                            required,
                            &substitutions,
                            &scope_starts,
                        ))
                        .is_some_and(|(actual, required)| actual == required)
                }
            }
        }

        fn binding_matches_actual(
            program: &dyn crate::CheckingInput,
            binding: &TraitTypeBinding,
            actual: TypeReferenceHandle,
            bindings: &[TraitTypeBinding],
        ) -> bool {
            if let TraitTypeBindingTarget::Type(expected) = binding.target {
                let outer = &bindings[..binding.scope_start];
                let substitutions = outer
                    .iter()
                    .filter_map(|binding| match binding.target {
                        TraitTypeBindingTarget::Type(reference) => {
                            Some((binding.parameter_symbol, reference))
                        }
                        TraitTypeBindingTarget::Parameter(_) => None,
                    })
                    .collect::<Vec<_>>();
                let mut scope_starts = outer
                    .iter()
                    .map(|binding| binding.scope_start)
                    .collect::<Vec<_>>();
                scope_starts.dedup();
                if substitutions.len() == outer.len()
                    && program.normalized_type_identity(actual)
                        == program.scoped_type_identity(
                            typed_trees::type_identity::TypeIdentityRequest {
                                substitutions: &substitutions,
                                ..typed_trees::type_identity::TypeIdentityRequest::ordinary(
                                    expected,
                                )
                            },
                            &scope_starts,
                        )
                {
                    return true;
                }
            }
            match &binding.target {
                TraitTypeBindingTarget::Type(expected) => match (
                    program.type_reference_table.type_reference(actual),
                    program.type_reference_table.type_reference(*expected),
                ) {
                    (
                        TypeReferenceNode::Named {
                            symbol: actual_symbol,
                            ..
                        },
                        TypeReferenceNode::Named {
                            symbol: expected_symbol,
                            ..
                        },
                    ) if actual_symbol.is_valid() && expected_symbol.is_valid() => {
                        actual_symbol == expected_symbol
                            || type_reference_is_instance_of_family(
                                program,
                                actual,
                                outer_binding_argument(
                                    program,
                                    *expected,
                                    bindings,
                                    binding.scope_start,
                                ),
                            )
                    }
                    _ => {
                        type_references_match(program, actual, *expected)
                            || type_reference_is_instance_of_family(
                                program,
                                actual,
                                outer_binding_argument(
                                    program,
                                    *expected,
                                    bindings,
                                    binding.scope_start,
                                ),
                            )
                    }
                },
                TraitTypeBindingTarget::Parameter(symbol) => matches!(
                    program.type_reference_table.type_reference(actual),
                    TypeReferenceNode::Named {
                        symbol: actual_symbol,
                        ..
                    } if (*symbol).is_valid() && *actual_symbol == *symbol
                ),
            }
        }

        fn outer_binding_argument(
            program: &dyn crate::CheckingInput,
            mut argument: TypeReferenceHandle,
            bindings: &[TraitTypeBinding],
            mut scope_start: usize,
        ) -> TypeReferenceHandle {
            while let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(argument)
                && let Some(outer) = bindings[..scope_start]
                    .iter()
                    .rev()
                    .find(|binding| binding.parameter_symbol == *symbol)
                && let TraitTypeBindingTarget::Type(supplied) = outer.target
            {
                argument = supplied;
                scope_start = outer.scope_start;
            }
            argument
        }

        fn type_reference_is_instance_of_family(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            family: TypeReferenceHandle,
        ) -> bool {
            let TypeReferenceNode::Named {
                symbol: family_symbol,
                ..
            } = program.type_reference_table.type_reference(family)
            else {
                return false;
            };
            let Some(family_definition) =
                crate::declarations::data_queries::data_definition_by_symbol(
                    program,
                    *family_symbol,
                )
            else {
                return false;
            };
            if program.data_type_parameters(family_definition).is_empty() {
                return false;
            }
            matches!(
                program.type_reference_table.type_reference(actual),
                TypeReferenceNode::Generic { base_symbol, .. } if base_symbol == family_symbol
            )
        }

        fn indexed_law_callable_telescope_groups<'program>(
            program: &'program dyn crate::CheckingInput,
            trait_definition: &TraitDefinition,
            requirement: &StateSignature,
            explicit_type_arguments: &[TypeReferenceHandle],
        ) -> Vec<&'program [TypeParameter]> {
            let has_proposition_law = program
                .state_signature_contracts(requirement)
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Ensures)
                .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                .any(|fact| matches!(fact, ProofFact::Proposition(_)));
            if !has_proposition_law {
                return Vec::new();
            }
            let trait_parameters = program.trait_type_parameters(trait_definition);
            program
                .state_signature_parameters(requirement)
                .iter()
                .filter_map(|representative| {
                    let carrier_parameter = required_trait_type_parameter(
                        program,
                        representative.type_reference,
                        trait_parameters,
                    )?;
                    let carrier = carrier_data_definition_for_argument(
                        program,
                        trait_parameters,
                        explicit_type_arguments,
                        carrier_parameter,
                    )?;
                    let telescope = program.data_type_parameters(carrier);
                    (!telescope.is_empty()).then_some(telescope)
                })
                .collect()
        }

        pub(crate) fn required_trait_type_parameter<'program>(
            program: &'program dyn crate::CheckingInput,
            required: TypeReferenceHandle,
            trait_type_parameters: &'program [TypeParameter],
        ) -> Option<&'program TypeParameter> {
            let symbol = &program
                .type_reference_table
                .type_reference(required)
                .as_named_symbol()?;

            symbol.is_valid().then_some(())?;
            trait_type_parameters
                .iter()
                .find(|parameter| parameter.symbol.is_valid() && parameter.symbol == *symbol)
        }

        pub(crate) fn carrier_data_definition_for_argument<'program>(
            program: &'program dyn crate::CheckingInput,
            trait_parameters: &[TypeParameter],
            applied_arguments: &[TypeReferenceHandle],
            carrier_parameter: &TypeParameter,
        ) -> Option<&'program typed_trees::data::DataDefinition> {
            let carrier_index = typed_trees::data::type_parameter_position(
                trait_parameters,
                carrier_parameter.symbol,
            )?;
            let carrier_argument = *applied_arguments.get(carrier_index)?;
            let TypeReferenceNode::Named {
                symbol: carrier_symbol,
                ..
            } = program
                .type_reference_table
                .type_reference(carrier_argument)
            else {
                return None;
            };
            crate::declarations::data_queries::data_definition_by_symbol(program, *carrier_symbol)
        }

        pub(crate) fn parameter_shape_label(
            program: &dyn crate::CheckingInput,
            parameter: &StateParameter,
        ) -> String {
            format!(
                "{}: {}",
                parameter.name,
                type_reference_label(program, parameter.type_reference)
            )
        }
    }
    pub(crate) mod trait_applications {
        use crate::declarations::traits::conformance::signature_matching::{
            carrier_data_definition_for_argument, required_trait_type_parameter,
            trait_type_bindings, type_references_match_for_trait_bindings,
        };
        use crate::value_custody::type_references::type_references_match;
        use diagnostics::Diagnostic;
        use typed_trees::data::{TypeParameter, TypeParameterKind};
        use typed_trees::trait_definition::TraitDefinition;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn validate_trait_application_obligations(
            program: &dyn crate::CheckingInput,
            applied_trait: &TraitDefinition,
            applied_arguments: &[TypeReferenceHandle],
            applied_lifetime_arguments: &[typed_trees::name::Identifier],
            application_label: &str,
            symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
            caller_scope: crate::value_custody::type_references::TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            let parameters = program.trait_type_parameters(applied_trait);
            if applied_arguments.len() != parameters.len() {
                diagnostics.push(Diagnostic::error(format!(
                    "{application_label} expects {} generic arguments, got {}",
                    parameters.len(),
                    applied_arguments.len()
                )));
                return;
            }

            use crate::value_custody::type_references::{
                AppliedValueCarrier, TypeReferenceOwner, validate_const_application_argument,
                validate_machine_application_argument, validate_parameter_bounds,
                validate_type_reference_handle,
            };
            validate_parameter_bounds(
                program,
                applied_trait.name.as_str(),
                parameters,
                applied_arguments,
                caller_scope,
                diagnostics,
            );
            for (parameter, argument) in parameters.iter().zip(applied_arguments) {
                match &parameter.kind {
                    TypeParameterKind::Type => validate_type_reference_handle(
                        program,
                        *argument,
                        symbols,
                        diagnostics,
                        TypeReferenceOwner::GenericBinder {
                            owner: application_label,
                            parameter: parameter.name.as_str(),
                            generic_depth: 0,
                        },
                        caller_scope,
                        formation_obligations,
                    ),
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => {
                        validate_const_application_argument(
                            program,
                            applied_trait.name.as_str(),
                            parameter,
                            AppliedValueCarrier::applied(
                                *type_reference,
                                parameters,
                                applied_arguments,
                            ),
                            *argument,
                            caller_scope,
                            &[],
                            None,
                            diagnostics,
                            formation_obligations,
                        )
                    }
                    TypeParameterKind::Machine { contract }
                        if !matches!(
                            contract,
                            typed_trees::data::MachineParameterContract::RequirementIdentity
                        ) =>
                    {
                        validate_machine_application_argument(
                            program,
                            applied_trait.symbol,
                            applied_trait.name.as_str(),
                            parameter,
                            contract,
                            *argument,
                            applied_arguments,
                            caller_scope,
                            diagnostics,
                        )
                    }
                    TypeParameterKind::Implementation => validate_implementation_type_arguments(
                        program,
                        *argument,
                        application_label,
                        symbols,
                        caller_scope,
                        diagnostics,
                        formation_obligations,
                    ),
                    TypeParameterKind::Proposition { .. } | TypeParameterKind::Machine { .. } => {}
                }
            }
            super::super::trait_implementation_arguments::validate_trait_implementation_application(
                program,
                applied_trait,
                applied_arguments,
                applied_lifetime_arguments,
                if caller_scope.operand_lifetime_owner.is_valid() {
                    caller_scope.operand_lifetime_owner
                } else {
                    caller_scope.lexical_owner
                },
                application_label,
                diagnostics,
            );

            validate_machine_declaration_identity_arguments(
                program,
                applied_trait,
                applied_arguments,
                application_label,
                diagnostics,
            );
            validate_proposition_family_arguments(
                program,
                applied_trait,
                applied_arguments,
                application_label,
                diagnostics,
            );
        }

        fn validate_machine_declaration_identity_arguments(
            program: &dyn crate::CheckingInput,
            applied_trait: &TraitDefinition,
            applied_arguments: &[TypeReferenceHandle],
            application_label: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for (parameter, argument) in program
                .trait_type_parameters(applied_trait)
                .iter()
                .zip(applied_arguments)
            {
                let TypeParameterKind::Machine {
                    contract: typed_trees::data::MachineParameterContract::RequirementIdentity,
                } = &parameter.kind
                else {
                    continue;
                };
                let TypeReferenceNode::Named { symbol, name } =
                    program.type_reference_table.type_reference(*argument)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                "{application_label} declaration-identity argument `{}` must be one exact machine or trait-requirement name",
                parameter.name,
            )));
                    continue;
                };

                if symbol.is_valid()
                    && program.symbols.get(*symbol).kind == symbols::SymbolKind::MachineParameter
                    && program.data_type_parameters.iter().any(|(_, candidate)| {
                        candidate.symbol == *symbol
                            && matches!(
                        candidate.kind,
                        TypeParameterKind::Machine {
                            contract:
                                typed_trees::data::MachineParameterContract::RequirementIdentity
                        }
                    )
                    })
                {
                    continue;
                }

                if !symbol.is_valid()
                    || program.symbols.get(*symbol).kind != symbols::SymbolKind::State
                {
                    diagnostics.push(Diagnostic::error(format!(
                "{application_label} argument `{name}` for declaration-identity parameter `{}` is not an exact machine or trait requirement",
                parameter.name,
            )));
                    continue;
                }

                if let Some((declaring_trait, selected_requirement)) =
                    program.trait_machine_signature(*symbol)
                {
                    let same_name_count = program
                        .trait_machine_signatures(declaring_trait)
                        .iter()
                        .filter(|requirement| requirement.name == selected_requirement.name)
                        .count();
                    if same_name_count != 1 {
                        diagnostics.push(Diagnostic::error(format!(
                    "{application_label} declaration-identity argument `{name}` does not resolve to one exact trait requirement; signature-free references reject overloads",
                )));
                    }
                }
            }
        }

        fn validate_proposition_family_arguments(
            program: &dyn crate::CheckingInput,
            applied_trait: &TraitDefinition,
            applied_arguments: &[TypeReferenceHandle],
            application_label: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let parameters = program.trait_type_parameters(applied_trait);
            let mut bindings = trait_type_bindings(parameters.iter(), applied_arguments.iter());

            for (parameter, argument) in parameters.iter().zip(applied_arguments) {
                let TypeParameterKind::Proposition { contract: expected } = &parameter.kind else {
                    continue;
                };
                let TypeReferenceNode::Named { symbol, name } =
                    program.type_reference_table.type_reference(*argument)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                "{application_label} proposition-family argument `{}` must be a direct proposition name",
                parameter.name,
            )));
                    continue;
                };
                let actual_declaration = if symbol.is_valid()
                    && program.symbols.get(*symbol).kind == symbols::SymbolKind::Proposition
                {
                    program.proposition_definition_by_symbol(*symbol)
                } else {
                    None
                };
                let actual_parameters = if let Some(declaration) = actual_declaration {
                    Some(program.proposition_parameters(declaration))
                } else if symbol.is_valid()
                    && program.symbols.get(*symbol).kind
                        == symbols::SymbolKind::PropositionParameter
                {
                    program
                        .data_type_parameters
                        .iter()
                        .map(|(_, parameter)| parameter)
                        .find_map(|parameter| match &parameter.kind {
                            TypeParameterKind::Proposition { contract }
                                if parameter.symbol == *symbol =>
                            {
                                Some(program.state_parameters.span_or_empty(contract.parameters))
                            }
                            _ => None,
                        })
                } else {
                    None
                };
                let Some(actual_parameters) = actual_parameters else {
                    diagnostics.push(Diagnostic::error(format!(
                "{application_label} argument `{}` for proposition parameter `{}` is not a proposition family",
                name, parameter.name,
            )));
                    continue;
                };
                let expected_parameters =
                    program.state_parameters.span_or_empty(expected.parameters);
                if actual_parameters.len() != expected_parameters.len() {
                    diagnostics.push(Diagnostic::error(format!(
                "{application_label} proposition family `{name}` has {} value parameter(s), but `{}` requires {}",
                actual_parameters.len(),
                parameter.name,
                expected_parameters.len(),
            )));
                    continue;
                }
                let actual_binders = actual_declaration
                    .map(|declaration| program.proposition_binders(declaration))
                    .unwrap_or(&[]);
                let mut indexed_binder_offset = 0usize;
                let mut used_indexed_carrier = false;
                for (index, (actual, expected)) in actual_parameters
                    .iter()
                    .zip(expected_parameters)
                    .enumerate()
                {
                    let indexed_match = if actual_binders.is_empty() {
                        None
                    } else {
                        indexed_carrier_parameter_matches(
                            program,
                            expected.type_reference,
                            actual.type_reference,
                            parameters,
                            applied_arguments,
                            actual_binders,
                            &mut indexed_binder_offset,
                        )
                    };
                    used_indexed_carrier |= indexed_match.is_some();
                    let matches = indexed_match.unwrap_or_else(|| {
                        type_references_match_for_trait_bindings(
                            program,
                            actual.type_reference,
                            expected.type_reference,
                            parameters,
                            &mut bindings,
                        )
                    });
                    if !matches {
                        let expected_type = required_trait_type_parameter(
                            program,
                            expected.type_reference,
                            parameters,
                        )
                        .and_then(|expected_parameter| {
                            parameters
                                .iter()
                                .position(|candidate| candidate.symbol == expected_parameter.symbol)
                        })
                        .and_then(|index| applied_arguments.get(index).copied())
                        .map(|argument| program.display_type_reference(argument))
                        .unwrap_or_else(|| program.display_type_reference(expected.type_reference));
                        diagnostics.push(Diagnostic::error(format!(
                    "{application_label} proposition family `{name}` value parameter {} has type `{}`, but `{}` requires `{}` after substitution",
                    index + 1,
                    program.display_type_reference(actual.type_reference),
                    parameter.name,
                    expected_type,
                )));
                    }
                }
                if used_indexed_carrier && indexed_binder_offset != actual_binders.len() {
                    diagnostics.push(Diagnostic::error(format!(
                "{application_label} proposition family `{name}` has {} proof-static binder(s), but the substituted carrier telescopes consume {}",
                actual_binders.len(),
                indexed_binder_offset,
            )));
                }
            }
        }

        fn indexed_carrier_parameter_matches(
            program: &dyn crate::CheckingInput,
            expected: TypeReferenceHandle,
            actual: TypeReferenceHandle,
            trait_parameters: &[TypeParameter],
            applied_arguments: &[TypeReferenceHandle],
            proposition_binders: &[typed_trees::proposition::PropositionBinder],
            binder_offset: &mut usize,
        ) -> Option<bool> {
            let carrier_parameter =
                required_trait_type_parameter(program, expected, trait_parameters)?;
            let carrier = carrier_data_definition_for_argument(
                program,
                trait_parameters,
                applied_arguments,
                carrier_parameter,
            )?;
            let carrier_telescope = program.data_type_parameters(carrier);
            if carrier_telescope.is_empty() {
                return None;
            }

            let start = *binder_offset;
            let end = start.saturating_add(carrier_telescope.len());
            *binder_offset = end;
            let Some(binder_group) = proposition_binders.get(start..end) else {
                return Some(false);
            };
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = program.type_reference_table.type_reference(actual)
            else {
                return Some(false);
            };
            if *base_symbol != carrier.symbol {
                return Some(false);
            }
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            if arguments.len() != binder_group.len() {
                return Some(false);
            }

            Some(
        carrier_telescope
            .iter()
            .zip(binder_group)
            .zip(arguments)
            .all(|((carrier_parameter, proposition_binder), argument)| {
                let kind_matches = match (&carrier_parameter.kind, &proposition_binder.kind) {
                    (
                        TypeParameterKind::Type,
                        typed_trees::proposition::PropositionBinderKind::Type,
                    )
                    | (
                        TypeParameterKind::Machine { .. },
                        typed_trees::proposition::PropositionBinderKind::Machine,
                    ) => true,
                    (
                        TypeParameterKind::Const {
                            type_reference: carrier_type,
                        }
                        | TypeParameterKind::Value {
                            type_reference: carrier_type,
                        },
                        typed_trees::proposition::PropositionBinderKind::Const {
                            type_reference: binder_type,
                        },
                    ) => type_references_match(program, *binder_type, *carrier_type),
                    _ => false,
                };
                kind_matches
                    && matches!(
                        program.type_reference_table.type_reference(*argument),
                        TypeReferenceNode::Named { symbol, name }
                            if ((*symbol).is_valid() && *symbol == proposition_binder.symbol)
                                || name == &proposition_binder.name
                    )
            }),
    )
        }

        fn validate_implementation_type_arguments(
            program: &dyn crate::CheckingInput,
            argument: TypeReferenceHandle,
            label: &str,
            symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
            caller_scope: crate::value_custody::type_references::TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            use crate::value_custody::type_references::{
                TypeReferenceOwner, validate_type_reference_handle,
            };
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = program.type_reference_table.type_reference(argument)
            else {
                return;
            };
            let Some(declaration) = program.conformance_by_symbol(*base_symbol) else {
                return;
            };
            let parameters = program.conformance_type_parameters(declaration);
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            if parameters.len() != arguments.len() {
                return;
            }
            crate::value_custody::type_references::validate_parameter_bounds(
                program,
                declaration
                    .alias
                    .as_ref()
                    .map_or(label, |name| name.as_str()),
                parameters,
                arguments,
                caller_scope,
                diagnostics,
            );
            for (parameter, argument) in parameters.iter().zip(arguments) {
                if matches!(parameter.kind, TypeParameterKind::Type) {
                    validate_type_reference_handle(
                        program,
                        *argument,
                        symbols,
                        diagnostics,
                        TypeReferenceOwner::GenericBinder {
                            owner: label,
                            parameter: parameter.name.as_str(),
                            generic_depth: 0,
                        },
                        caller_scope,
                        formation_obligations,
                    );
                }
            }
        }
    }

    pub use bounds::GenericBoundRequirement;
    pub(crate) use bounds::{
        named_conformance_argument_matches, validate_conformance_bounds,
        validate_generic_conformance_bounds, validate_trait_conformance_bounds,
    };
    pub(crate) use external_shapes::{
        validate_external_leaf_native_shapes, validate_external_via_expression,
    };
    pub use generic_operator::generic_bound_operator_requirement;
    pub(crate) use machine_conformance::validate_machine_trait_conformances;
    pub use machine_conformance::{
        compose_forwarded_trait_arguments, resolve_closed_requirement_application,
        revalidate_machine_trait_conformances, revalidate_top_level_requirement_realization,
    };
    pub(crate) use signature_matching::{
        authored_conformance_machine, validate_applied_trait_signature,
    };
    pub(crate) use trait_applications::validate_trait_application_obligations;
    use typed_trees::data::TypeParameter;
    use typed_trees::data::TypeParameterKind;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum TypeParameterKindFamily {
        Type,
        Implementation,
        Const,
        Value,
        Machine,
        Proposition,
    }

    fn type_parameter_kind_family(kind: &TypeParameterKind) -> TypeParameterKindFamily {
        match kind {
            TypeParameterKind::Implementation => TypeParameterKindFamily::Implementation,
            TypeParameterKind::Type => TypeParameterKindFamily::Type,
            TypeParameterKind::Const { .. } => TypeParameterKindFamily::Const,
            TypeParameterKind::Value { .. } => TypeParameterKindFamily::Value,
            TypeParameterKind::Machine { .. } => TypeParameterKindFamily::Machine,
            TypeParameterKind::Proposition { .. } => TypeParameterKindFamily::Proposition,
        }
    }

    pub(crate) fn type_parameter_kinds_identical(
        required: &TypeParameterKind,
        actual: &TypeParameterKind,
    ) -> bool {
        type_parameter_kind_family(required) == type_parameter_kind_family(actual)
    }

    pub(crate) fn type_parameter_kinds_callable(
        required: &TypeParameterKind,
        actual: &TypeParameterKind,
    ) -> bool {
        let family = type_parameter_kind_family(required);
        family == type_parameter_kind_family(actual)
            && family != TypeParameterKindFamily::Proposition
    }

    pub(crate) fn type_parameter_kinds_bindable(
        required: &TypeParameterKind,
        actual: &TypeParameterKind,
    ) -> bool {
        let family = type_parameter_kind_family(required);
        family == type_parameter_kind_family(actual)
            && matches!(
                family,
                TypeParameterKindFamily::Type
                    | TypeParameterKindFamily::Const
                    | TypeParameterKindFamily::Value
            )
    }

    pub(crate) fn type_parameter_binding_pairs<'p>(
        required: &'p [TypeParameter],
        actual: &'p [TypeParameter],
    ) -> impl Iterator<Item = (&'p TypeParameter, &'p TypeParameter)> {
        required.iter().zip(actual).filter(|&(required, actual)| {
            type_parameter_kinds_bindable(&required.kind, &actual.kind)
        })
    }

    pub(crate) enum OperationalCeilingBreach<'a> {
        ReachedService(&'a str),
        Suspends,
        Blocks,
    }

    pub(crate) fn require_operational_ceiling(
        program: &dyn crate::CheckingInput,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        machine: &typed_trees::machine::Machine,
        allowed_services: &[language_semantics::ServiceReachId],
        requirement_suspends: bool,
        requirement_blocks: bool,
        describe: impl Fn(OperationalCeilingBreach<'_>) -> String,
        diagnostics: &mut Vec<diagnostics::Diagnostic>,
    ) {
        let Some(disallowed) = disallowed_reached_service_names(
            program,
            service_reaches,
            machine,
            allowed_services,
            diagnostics,
        ) else {
            return;
        };
        for service_name in disallowed {
            diagnostics.push(diagnostics::Diagnostic::error(describe(
                OperationalCeilingBreach::ReachedService(service_name),
            )));
        }
        if machine.suspends && !requirement_suspends {
            diagnostics.push(diagnostics::Diagnostic::error(describe(
                OperationalCeilingBreach::Suspends,
            )));
        }
        if machine.blocks && !requirement_blocks {
            diagnostics.push(diagnostics::Diagnostic::error(describe(
                OperationalCeilingBreach::Blocks,
            )));
        }
    }
}
pub mod conformance_applications {
    use diagnostics::Diagnostic;
    use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::data::TypeParameterKind;
    use typed_trees::expression::{ExpressionNode, StaticMachineArgument};
    use typed_trees::references::{StaticArgumentCall, StaticArgumentRole};
    use typed_trees::typed_trees::{
        ClosedConformanceApplication, ClosedConformanceConstArgument, ClosedConformanceRowIdentity,
    };
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) mod forwarding {
        use symbols::SymbolKind;
        use typed_trees::{
            data::TypeParameterKind,
            type_identity::{
                AppliedTypeIdentityScope, IndexIdentityArgument, LifetimeParameterIdentity,
                NormalizedTypeIdentity, applied_index_identity_in_region_scope,
                applied_type_identity_in_region_scope,
            },
        };
        use typed_trees::{expression::StaticMachineArgument, machine::GenericConformanceBound};

        pub(crate) fn validate_forwarded_argument(
            program: &dyn crate::CheckingInput,
            required: &GenericConformanceBound,
            selected: &StaticMachineArgument,
            arguments_match: impl FnOnce(&GenericConformanceBound) -> bool,
        ) -> Result<(), String> {
            if !selected.symbol.is_valid()
                || program.symbols.get(selected.symbol).kind != SymbolKind::ConformanceParameter
            {
                return Err(
                    "forwarded evidence must name a declared implementation binder".to_owned(),
                );
            }
            if selected.application.is_some() {
                return Err("an implementation evidence binder is already selected and takes no application arguments".to_owned());
            }
            let actual = program
                .callable_conformance_bound(
                    program.symbols.get(selected.symbol).parent,
                    selected.symbol,
                )
                .ok_or_else(|| {
                    "forwarded implementation binder has no declared contract".to_owned()
                })?;
            let demanded =
                crate::refinement_fit::resolve_bound_carrier(program, required.trait_symbol);
            let promised =
                crate::refinement_fit::resolve_bound_carrier(program, actual.trait_symbol);
            if promised.base != demanded.base
                || (demanded.refinement.is_some() && actual.trait_symbol != required.trait_symbol)
                || !arguments_match(actual)
            {
                return Err(format!(
                    "forwarded binder `{}` does not guarantee the exact `{}` trait application",
                    actual.binder_name, required.trait_name
                ));
            }
            Ok(())
        }

        #[derive(Debug, PartialEq, Eq)]
        pub(crate) struct TraitApplicationIdentity {
            arguments: Vec<NormalizedTypeIdentity>,
            lifetimes: Vec<typed_trees::type_identity::LifetimeParameterIdentity>,
        }

        pub(crate) fn applied_bound_argument_identities(
            program: &dyn crate::CheckingInput,
            bound: &GenericConformanceBound,
            receiving_owner: symbols::SymbolHandle,
            argument_owner: symbols::SymbolHandle,
            scopes: &[AppliedTypeIdentityScope<'_>],
        ) -> Option<TraitApplicationIdentity> {
            applied_bound_argument_identities_in_region_scope(
                program,
                bound,
                receiving_owner,
                argument_owner,
                scopes,
                &[],
            )
        }

        pub(crate) fn applied_bound_argument_identities_in_region_scope(
            program: &dyn crate::CheckingInput,
            bound: &GenericConformanceBound,
            receiving_owner: symbols::SymbolHandle,
            argument_owner: symbols::SymbolHandle,
            scopes: &[AppliedTypeIdentityScope<'_>],
            regions: &[(LifetimeParameterIdentity, LifetimeParameterIdentity)],
        ) -> Option<TraitApplicationIdentity> {
            let carrier = crate::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
            let root = typed_trees::trait_definition::AppliedTraitScope::new(
                program.typed_program(),
                program.trait_definition_by_symbol(bound.trait_symbol)?,
                typed_trees::trait_definition::TraitApplicationArguments {
                    lexical_owner: receiving_owner,
                    enclosing_data_applications: &[],
                    arguments: &bound.arguments,
                    lifetimes: &bound.lifetime_arguments,
                },
            )?;
            let scope = root.declaring_scope(program.typed_program(), carrier.base)?;
            applied_scope_argument_identities_in_region_scope(
                program,
                &scope,
                argument_owner,
                scopes,
                regions,
            )
        }

        pub(crate) fn applied_scope_argument_identities(
            program: &dyn crate::CheckingInput,
            application: &typed_trees::trait_definition::AppliedTraitScope<'_>,
            argument_owner: symbols::SymbolHandle,
            outer: &[AppliedTypeIdentityScope<'_>],
        ) -> Option<TraitApplicationIdentity> {
            applied_scope_argument_identities_in_region_scope(
                program,
                application,
                argument_owner,
                outer,
                &[],
            )
        }

        fn applied_scope_argument_identities_in_region_scope(
            program: &dyn crate::CheckingInput,
            application: &typed_trees::trait_definition::AppliedTraitScope<'_>,
            argument_owner: symbols::SymbolHandle,
            outer: &[AppliedTypeIdentityScope<'_>],
            regions: &[(LifetimeParameterIdentity, LifetimeParameterIdentity)],
        ) -> Option<TraitApplicationIdentity> {
            application.with_identity_scopes(program.typed_program(), |inner| {
                let identities = program
                    .trait_type_parameters(application.declaration)
                    .iter()
                    .map(|parameter| {
                        let (argument, prefix) =
                            application.parameter_application(parameter.symbol)?;
                        let mut scopes = outer.to_vec();
                        scopes.extend_from_slice(&inner[..prefix]);
                        match parameter.kind {
                            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                                applied_index_identity_in_region_scope(
                                    program.typed_program(),
                                    IndexIdentityArgument::TypeReference(argument),
                                    argument_owner,
                                    &scopes,
                                    regions,
                                )
                            }
                            _ => applied_type_identity_in_region_scope(
                                program.typed_program(),
                                argument,
                                application.argument_owner,
                                argument_owner,
                                &scopes,
                                regions,
                            ),
                        }
                    })
                    .collect::<Option<Vec<_>>>()?;
                let mut scopes = outer.to_vec();
                scopes.extend_from_slice(inner);
                let lifetimes = application
                    .declaration
                    .lifetime_parameters
                    .iter()
                    .map(|parameter| {
                        let slot = typed_trees::type_identity::lifetime_parameter_identity(
                            program.typed_program(),
                            application.declaration.symbol,
                            parameter.as_str(),
                        )?;
                        typed_trees::type_identity::apply_resolved_lifetime_parameter_identity(
                            program.typed_program(),
                            slot,
                            argument_owner,
                            &scopes,
                        )
                        .map(|(argument, _)| {
                            regions
                                .iter()
                                .find(|(parameter, _)| *parameter == argument)
                                .map_or(argument, |(_, resolved)| *resolved)
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(TraitApplicationIdentity {
                    arguments: identities,
                    lifetimes,
                })
            })
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use symbols::{SymbolNameRef, SymbolTableBuilder};
            use typed_trees::{
                TypedTrees, data::DataDefinition, name::Identifier,
                type_aliases::TypeAliasDefinition,
            };

            #[test]
            fn forwarded_promises_include_data_and_alias_headers_without_weakening_checks() {
                for kind in [SymbolKind::Data, SymbolKind::TypeAlias] {
                    let mut builder = SymbolTableBuilder::default();
                    let owner = builder.insert_root(kind, SymbolNameRef::Static("Owner"));
                    let binder = builder
                        .insert_children(
                            owner,
                            [(
                                SymbolKind::ConformanceParameter,
                                SymbolNameRef::Static("Policy"),
                            )],
                        )
                        .start();
                    let required_trait =
                        builder.insert_root(SymbolKind::Trait, SymbolNameRef::Static("Required"));
                    let other_trait =
                        builder.insert_root(SymbolKind::Trait, SymbolNameRef::Static("Other"));
                    let mut program = TypedTrees {
                        symbols: builder.finish(),
                        ..Default::default()
                    };
                    let promised = GenericConformanceBound {
                        binder,
                        binder_name: Identifier::generated("Policy"),
                        trait_symbol: required_trait,
                        trait_name: Identifier::generated("Required"),
                        lifetime_arguments: Vec::new(),
                        arguments: Vec::new(),
                    };
                    if kind == SymbolKind::TypeAlias {
                        program.push_type_alias_definition(TypeAliasDefinition {
                            symbol: owner,
                            conformance_bounds: vec![promised.clone()],
                            ..Default::default()
                        });
                    } else {
                        program.push_data_definition(DataDefinition {
                            symbol: owner,
                            conformance_bounds: vec![promised.clone()],
                            ..Default::default()
                        });
                    }
                    let selected = StaticMachineArgument {
                        symbol: binder,
                        ..Default::default()
                    };
                    assert!(
                        validate_forwarded_argument(
                            &program,
                            &promised,
                            &selected,
                            |actual| actual == &promised
                        )
                        .is_ok()
                    );
                    assert!(
                        validate_forwarded_argument(&program, &promised, &selected, |_| false)
                            .is_err()
                    );
                    let mut incompatible = promised.clone();
                    incompatible.trait_symbol = other_trait;
                    incompatible.trait_name = Identifier::generated("Other");
                    assert!(
                        validate_forwarded_argument(
                            &program,
                            &incompatible,
                            &selected,
                            |_| panic!("trait mismatch must precede tuple comparison")
                        )
                        .is_err()
                    );
                    let mut applied = selected.clone();
                    applied.application =
                        Some(Box::new(typed_trees::expression::StaticSymbolApplication {
                            member: Default::default(),
                            lifetime_arguments: Box::new([]),
                            arguments: Box::new([]),
                        }));
                    assert!(
                        validate_forwarded_argument(&program, &promised, &applied, |_| panic!(
                            "an applied binder must refuse first"
                        ))
                        .unwrap_err()
                        .contains("already selected")
                    );
                    let missing = TypedTrees {
                        symbols: program.symbols.clone(),
                        ..Default::default()
                    };
                    assert!(
                        validate_forwarded_argument(&missing, &promised, &selected, |_| panic!(
                            "a missing promise must refuse first"
                        ))
                        .unwrap_err()
                        .contains("no declared contract")
                    );
                }
            }
        }
    }
    pub(crate) use forwarding::{
        applied_bound_argument_identities, applied_bound_argument_identities_in_region_scope,
        applied_scope_argument_identities, validate_forwarded_argument,
    };

    pub fn validate_data_evidence_application(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        lexical_owner: SymbolHandle,
    ) -> Result<(), Vec<Diagnostic>> {
        let reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            lifetime_arguments,
            ..
        } = program.type_reference_table.type_reference(reference)
        else {
            return Err(vec![Diagnostic::error(
                "data evidence validation requires an exact applied declaration",
            )]);
        };
        let definition = program
            .data_definition_by_symbol(*base_symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "data evidence application lost its declaration",
                )]
            })?;
        let parameters = program.data_type_parameters(definition);
        let arguments = program
            .type_reference_table
            .type_reference_handles(*arguments);
        if parameters.len() != arguments.len()
            || definition.lifetime_parameters.len() != lifetime_arguments.len()
        {
            return Err(vec![Diagnostic::error(format!(
                "data `{}` has an incomplete application telescope",
                definition.name
            ))]);
        }
        let mut diagnostics = Vec::new();
        super::trait_implementation_arguments::validate_implementation_application(
            program,
            definition.symbol,
            parameters,
            &definition.lifetime_parameters,
            &definition.conformance_bounds,
            arguments,
            lifetime_arguments,
            lexical_owner,
            &format!("data `{}`", definition.name),
            &mut diagnostics,
        );
        crate::program_validation::finish_strict_diagnostics(diagnostics)
    }

    pub fn validate_conformance_applications(
        program: &dyn crate::CheckingInput,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        if let Err(failures) =
            super::trait_implementation_arguments::validate_trait_implementation_arguments(program)
        {
            diagnostics.extend(failures);
        }
        for (handle, expression) in program.expression_table.iter_expressions() {
            if let ExpressionNode::Call(call) = expression {
                validate_conformance_call_arguments(
                    program,
                    StaticArgumentCall::Expression(handle, call),
                    &mut diagnostics,
                );
            }
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(call) = statement.as_call() {
                        validate_conformance_call_arguments(
                            program,
                            StaticArgumentCall::Statement(call),
                            &mut diagnostics,
                        );
                    }
                }
            }
        }
        crate::program_validation::finish_strict_diagnostics(diagnostics)
    }

    fn validate_conformance_call_arguments(
        program: &dyn crate::CheckingInput,
        call: StaticArgumentCall<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let role = crate::compiler_operations::static_argument_role(program, call);
        let declarations = if role == StaticArgumentRole::DeclarationDescription {
            program.product_declarations()
        } else {
            program
        };
        validate_arguments(
            declarations,
            program,
            call.lexical_owner(program.typed_program()),
            call.arguments(program.typed_program()),
            role,
            diagnostics,
        );
    }

    fn validate_arguments(
        program: &dyn crate::CheckingInput,
        lexical_program: &dyn crate::CheckingInput,
        lexical_owner: SymbolHandle,
        arguments: &[StaticMachineArgument],
        role: StaticArgumentRole,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for argument in arguments {
            if matches!(
                program.symbols.get(argument.symbol).kind,
                SymbolKind::Conformance
            ) {
                if role == StaticArgumentRole::DeclarationDescription {
                    if argument.application.is_some() {
                        diagnostics.push(Diagnostic::error(format!(
                        "Build declaration operand `{}` names a conformance application; name its declaration without arguments",
                        argument.display_name(),
                    )));
                    }
                    continue;
                }
                if let Err(diagnostic) = close_conformance_application(program, argument) {
                    diagnostics.push(diagnostic);
                }
                if let Err(diagnostic) = validate_conformance_lifetime_arguments(
                    lexical_program,
                    argument,
                    lexical_owner,
                ) {
                    diagnostics.push(diagnostic);
                }
            }
            if let Some(application) = &argument.application {
                validate_arguments(
                    program,
                    lexical_program,
                    lexical_owner,
                    &application.arguments,
                    role,
                    diagnostics,
                );
            }
        }
    }

    pub(crate) fn validate_conformance_lifetime_arguments(
        program: &dyn crate::CheckingInput,
        selected: &StaticMachineArgument,
        lexical_owner: SymbolHandle,
    ) -> Result<(), Diagnostic> {
        let Some(application) = &selected.application else {
            return Ok(());
        };
        let supplier = program
            .typed_program()
            .static_operand_region_supplier(selected, lexical_owner)
            .map_err(Diagnostic::error)?;
        for lifetime in &application.lifetime_arguments {
            if typed_trees::type_identity::lifetime_parameter_identity(
                program.typed_program(),
                supplier,
                lifetime.as_str(),
            )
            .is_none()
            {
                return Err(Diagnostic::error(format!(
                    "conformance application `{}` supplies undeclared lifetime argument `'{}'; declare it in the lexical owner's lifetime telescope",
                    selected.display_name(),
                    lifetime.as_str(),
                )));
            }
        }
        Ok(())
    }

    pub fn close_conformance_application(
        program: &dyn crate::CheckingInput,
        selected: &StaticMachineArgument,
    ) -> Result<ClosedConformanceApplication, Diagnostic> {
        close_selected_conformance(
            program,
            selected.symbol,
            selected
                .application
                .as_ref()
                .map_or(&[][..], |application| application.arguments.as_ref()),
            selected
                .application
                .as_ref()
                .map(|application| application.lifetime_arguments.as_ref()),
        )
    }

    pub fn close_conformance_declaration(
        program: &dyn crate::CheckingInput,
        declaration: SymbolHandle,
    ) -> Result<ClosedConformanceApplication, Diagnostic> {
        close_selected_conformance(program, declaration, &[], None)
    }

    fn close_selected_conformance(
        program: &dyn crate::CheckingInput,
        declaration: SymbolHandle,
        supplied: &[StaticMachineArgument],
        supplied_lifetimes: Option<&[typed_trees::name::Identifier]>,
    ) -> Result<ClosedConformanceApplication, Diagnostic> {
        let conformance = program.conformance_by_symbol(declaration).ok_or_else(|| {
            Diagnostic::error("selected declaration does not name a package-scoped conformance")
        })?;
        let declaration_name = conformance
            .alias
            .as_ref()
            .map_or("<unnamed-conformance>", |name| name.as_str());
        let parameters = program.conformance_type_parameters(conformance);
        if parameters.len() != supplied.len() {
            return Err(Diagnostic::error(format!(
                "generic conformance `{declaration_name}` requires {} explicit non-lifetime argument(s), got {}; expected subject and trait shape never fill its own telescope",
                parameters.len(),
                supplied.len()
            )));
        }

        let lifetime_arguments = match supplied_lifetimes {
            Some(lifetimes) if lifetimes.len() == conformance.lifetime_parameters.len() => {
                lifetimes
                    .iter()
                    .map(|lifetime| lifetime.as_str().to_owned())
                    .collect::<Vec<_>>()
            }
            Some(lifetimes) if !lifetimes.is_empty() => {
                return Err(Diagnostic::error(format!(
                    "generic conformance `{declaration_name}` requires {} lifetime argument(s), got {}",
                    conformance.lifetime_parameters.len(),
                    lifetimes.len()
                )));
            }
            _ if conformance.lifetime_parameters.is_empty() => Vec::new(),
            _ => {
                return Err(Diagnostic::error(format!(
                    "generic conformance `{declaration_name}` has {} erased lifetime parameter(s), but this application supplies none and no unique ordinary borrow constraint is available; write the lifetime argument explicitly",
                    conformance.lifetime_parameters.len()
                )));
            }
        };

        let mut substitutions = Vec::<(SymbolHandle, String)>::new();
        let mut type_arguments = Vec::new();
        let mut const_arguments = Vec::new();
        let mut machine_arguments = Vec::new();
        for (position, (parameter, argument)) in parameters.iter().zip(supplied).enumerate() {
            match &parameter.kind {
                TypeParameterKind::Implementation => {
                    return Err(category_error(
                        declaration_name,
                        parameter.name.as_str(),
                        "conformance-declaration parameter (implementation slots belong to trait headers)",
                        argument,
                    ));
                }
                TypeParameterKind::Type => {
                    validate_type_argument(
                        program,
                        declaration_name,
                        parameter.name.as_str(),
                        argument,
                    )?;
                    let identity = type_argument_identity(program, argument);
                    substitutions.push((parameter.symbol, identity.clone()));
                    type_arguments.push(identity);
                }
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    let mut prior_consts = const_arguments.iter();
                    let carrier_values = parameters[..position]
                        .iter()
                        .filter_map(|prior| {
                            if !prior.kind.is_const_or_value() {
                                return None;
                            }
                            match prior_consts.next()? {
                                ClosedConformanceConstArgument::Evaluated { value, .. } => {
                                    Some((prior.symbol, value))
                                }
                                _ => None,
                            }
                        })
                        .collect::<Vec<_>>();
                    let (argument, substitution_identity) = close_const_argument(
                    program,
                    declaration_name,
                    parameter.name.as_str(),
                    crate::value_custody::type_references::AppliedValueCarrier::applied_operands(
                        *type_reference,
                        &parameters[..position],
                        &supplied[..position],
                        &carrier_values,
                    ),
                    argument,
                )?;
                    substitutions.push((parameter.symbol, substitution_identity));
                    const_arguments.push(argument);
                }
                TypeParameterKind::Machine { .. } => {
                    if !matches!(
                        program.symbols.get(argument.symbol).kind,
                        SymbolKind::State | SymbolKind::MachineParameter
                    ) {
                        return Err(category_error(
                            declaration_name,
                            parameter.name.as_str(),
                            "static machine",
                            argument,
                        ));
                    }
                    substitutions.push((
                        parameter.symbol,
                        static_argument_identity(program, argument),
                    ));
                    machine_arguments.push(argument.symbol);
                }
                TypeParameterKind::Proposition { .. } => {
                    return Err(Diagnostic::error(format!(
                        "conformance `{declaration_name}` retains an unsupported proposition parameter `{}` in its name-owned telescope",
                        parameter.name
                    )));
                }
            }
        }

        let Some(trait_definition) = program
            .trait_definition_by_symbol(conformance.trait_symbol)
            .or_else(|| {
                program
                    .traits()
                    .iter()
                    .find(|definition| definition.name == conformance.trait_name)
            })
        else {
            return Err(Diagnostic::error(format!(
                "conformance `{declaration_name}` names unresolved trait `{}`",
                conformance.trait_name
            )));
        };
        let trait_arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .iter()
            .map(|argument| {
                substituted_type_identity(
                    program,
                    *argument,
                    &substitutions,
                    &conformance
                        .lifetime_parameters
                        .iter()
                        .zip(&lifetime_arguments)
                        .map(|(parameter, argument)| {
                            (parameter.as_str().to_owned(), argument.clone())
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        let trait_lifetime_arguments = conformance
        .trait_lifetime_arguments
        .iter()
        .map(|ordinal| {
            usize::try_from(*ordinal)
                .ok()
                .and_then(|ordinal| lifetime_arguments.get(ordinal))
                .cloned()
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "conformance `{declaration_name}` target-trait lifetime falls outside its closed application"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
        let rows = conformance.rows.as_slice();
        let row_identities = rows
            .iter()
            .map(|row| ClosedConformanceRowIdentity {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
            })
            .collect::<Vec<_>>();
        let identity = application_identity(
            program,
            declaration_name,
            &lifetime_arguments,
            &type_arguments,
            &const_arguments,
            &machine_arguments,
            &parameters
                .iter()
                .zip(supplied)
                .filter_map(|(parameter, argument)| {
                    matches!(parameter.kind, TypeParameterKind::Machine { .. })
                        .then_some(argument.clone())
                })
                .collect::<Vec<_>>(),
            trait_definition.name.as_str(),
            &trait_lifetime_arguments,
            &trait_arguments,
            rows,
        )?;
        Ok(ClosedConformanceApplication {
            declaration: conformance.symbol,
            trait_scope: Default::default(),
            callable_application: Default::default(),
            arguments: supplied.into(),
            lifetime_arguments,
            type_arguments,
            const_arguments,
            machine_arguments,
            trait_definition: trait_definition.symbol,
            trait_lifetime_arguments,
            trait_arguments,
            rows: row_identities,
            report_fingerprint: identity.report_fingerprint,
            commitment: identity.commitment,
            identity_encoding: identity.encoding,
        })
    }

    fn validate_type_argument(
        program: &dyn crate::CheckingInput,
        conformance: &str,
        parameter: &str,
        argument: &StaticMachineArgument,
    ) -> Result<(), Diagnostic> {
        if argument.type_reference.is_valid() {
            return crate::static_arguments::validate_structural_type_operand(program, argument)
                .map_err(Diagnostic::error);
        }
        if argument.const_literal.is_some()
            || argument.evidence_projection.is_some()
            || !matches!(
                program.symbols.get(argument.symbol).kind,
                SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
            )
        {
            return Err(category_error(conformance, parameter, "type", argument));
        }
        Ok(())
    }

    fn category_error(
        conformance: &str,
        parameter: &str,
        category: &str,
        argument: &StaticMachineArgument,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "conformance `{conformance}` parameter `{parameter}` requires a {category} argument, but `{}` has another static category",
            argument.display_name()
        ))
    }

    fn close_const_argument(
        program: &dyn crate::CheckingInput,
        conformance: &str,
        parameter: &str,
        carrier: crate::value_custody::type_references::AppliedValueCarrier<'_>,
        argument: &StaticMachineArgument,
    ) -> Result<(ClosedConformanceConstArgument, String), Diagnostic> {
        let parameter_carrier = carrier.reference();
        let resolved_carrier = carrier.resolved(program).reference();
        if let Some(literal) = argument.const_literal.as_ref() {
            let value = literal.decimal_value_i128().ok_or_else(|| {
            Diagnostic::error(format!(
                "conformance `{conformance}` parameter `{parameter}` has a non-integer const literal `{}`",
                literal,
            ))
        })?;
            let primitive = program
            .type_reference_table
            .primitive_type(resolved_carrier)
            .filter(|primitive| primitive.accepts_integer_literal())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "conformance `{conformance}` parameter `{parameter}` has an integer literal under a non-integer carrier",
                ))
            })?;
            let value = CanonicalConstIdentity::integer(primitive.name(), value);
            crate::validate_exact_const_value_encoding(
            program,
            resolved_carrier,
            value.encoding.as_str(),
        )
        .map_err(|reason| {
            Diagnostic::error(format!(
                "conformance `{conformance}` parameter `{parameter}` has an invalid literal value: {reason}",
            ))
        })?;
            return Ok((
                ClosedConformanceConstArgument::Evaluated {
                    parameter_carrier,
                    declared_carrier: resolved_carrier,
                    value,
                },
                literal.to_string(),
            ));
        }
        if argument.type_reference.is_valid()
            && let TypeReferenceNode::ConstExpression(expression) = program
                .type_reference_table
                .type_reference(argument.type_reference)
        {
            if argument.symbol.is_valid()
                || !argument.path.is_empty()
                || argument.application.is_some()
                || argument.evidence_projection.is_some()
            {
                return Err(category_error(conformance, parameter, "const", argument));
            }
            if typed_trees::domain::index_argument_is_closed(
                program.typed_program(),
                argument.type_reference,
            ) {
                let owner = program
                    .type_reference_table
                    .operand_lifetime_owner(argument.type_reference);
                if !owner.is_valid() {
                    return Err(Diagnostic::error(format!(
                        "conformance `{conformance}` Const parameter `{parameter}` has a closed expression without its supplying declaration",
                    )));
                }
                if let Some(value) =
                    crate::value_custody::type_references::check_closed_const_expression_in_carrier(
                        program,
                        owner,
                        *expression,
                        carrier,
                    )
                    .map_err(|errors| {
                        Diagnostic::error(
                            errors
                                .iter()
                                .map(|error| error.message.as_str())
                                .collect::<Vec<_>>()
                                .join("; "),
                        )
                    })?
                {
                    let identity = value.identity();
                    return Ok((
                        ClosedConformanceConstArgument::Evaluated {
                            parameter_carrier,
                            declared_carrier: resolved_carrier,
                            value: identity,
                        },
                        value.atom(),
                    ));
                }
            }
            if !crate::value_custody::const_expressions::has_carrier(
                program,
                *expression,
                &TypeParameterKind::Const {
                    type_reference: resolved_carrier,
                },
                program
                    .type_reference_table
                    .operand_lifetime_owner(argument.type_reference),
            ) {
                return Err(category_error(conformance, parameter, "const", argument));
            }
            let identity =
                const_expression_identity(program, argument.type_reference).ok_or_else(|| {
                    category_error(conformance, parameter, "canonical const", argument)
                })?;
            return Ok((
                ClosedConformanceConstArgument::CallerExpression {
                    parameter_carrier,
                    expression: argument.type_reference,
                },
                identity,
            ));
        }
        if !argument.symbol.is_valid()
            && !argument.type_reference.is_valid()
            && argument.application.is_none()
            && argument.evidence_projection.is_none()
            && let [atom] = argument.path.as_ref()
            && let Some(value) = CanonicalConstValue::from_atom(atom.as_str())
        {
            crate::value_custody::type_references::validate_exact_const_identity_in_carrier(
            program, carrier, &value.identity(),
        ).map_err(|reason| Diagnostic::error(format!(
            "conformance `{conformance}` parameter `{parameter}` has an invalid canonical value: {reason}"
        )))?;
            return Ok((
                ClosedConformanceConstArgument::Evaluated {
                    parameter_carrier,
                    declared_carrier: resolved_carrier,
                    value: value.identity(),
                },
                value.atom(),
            ));
        }
        if program.symbols.get(argument.symbol).kind == SymbolKind::TypeParameter {
            let Some(binder) = program.data_type_parameter_by_symbol(argument.symbol) else {
                return Err(Diagnostic::error(format!(
                    "conformance `{conformance}` parameter `{parameter}` names const binder `{}` without its declaration",
                    argument.display_name(),
                )));
            };
            let TypeParameterKind::Const {
                type_reference: binder_carrier,
            } = binder.kind
            else {
                return Err(category_error(conformance, parameter, "const", argument));
            };
            return Ok((
                ClosedConformanceConstArgument::CallerBinder {
                    parameter_carrier,
                    binder: binder.symbol,
                    binder_carrier,
                },
                argument.display_name(),
            ));
        }
        if program.symbols.get(argument.symbol).kind != SymbolKind::Const {
            return Err(category_error(conformance, parameter, "const", argument));
        }
        let declarations = program
            .const_declarations()
            .iter()
            .filter(|declaration| declaration.symbol == argument.symbol)
            .collect::<Vec<_>>();
        let [declaration] = declarations.as_slice() else {
            return Err(Diagnostic::error(format!(
                "conformance `{conformance}` parameter `{parameter}` resolves named const `{}` to {} declarations; expected exactly one",
                argument.display_name(),
                declarations.len(),
            )));
        };
        let Some(encoding) = declaration
            .evaluated_value
            .as_ref()
            .and_then(|value| value.encoding())
        else {
            return Err(Diagnostic::error(format!(
                "conformance `{conformance}` parameter `{parameter}` names const `{}` without a canonical checked value",
                argument.display_name(),
            )));
        };
        if !carrier.matches(program, declaration.declared_type) {
            return Err(Diagnostic::error(format!(
                "conformance `{conformance}` parameter `{parameter}` names const `{}` with a different declared carrier",
                argument.display_name(),
            )));
        }
        for checked_carrier in [
            crate::value_custody::type_references::AppliedValueCarrier::ordinary(
                declaration.declared_type,
            ),
            carrier,
        ] {
            crate::value_custody::type_references::validate_exact_const_value_encoding_in_carrier(
            program, checked_carrier, encoding,
        ).map_err(|reason| Diagnostic::error(format!(
            "conformance `{conformance}` parameter `{parameter}` names const `{}` whose canonical value does not replay against its exact carriers: {reason}",
            argument.display_name(),
        )))?;
        }
        let value = CanonicalConstIdentity {
            type_name: program.normalized_type_identity_text(declaration.declared_type),
            encoding: encoding.to_owned(),
        };
        let substitution_identity =
            CanonicalConstValue::new(value.type_name.clone(), value.encoding.clone(), "").atom();
        Ok((
            ClosedConformanceConstArgument::Evaluated {
                parameter_carrier,
                declared_carrier: declaration.declared_type,
                value,
            },
            substitution_identity,
        ))
    }

    fn const_expression_identity(
        program: &dyn crate::CheckingInput,
        expression: TypeReferenceHandle,
    ) -> Option<String> {
        let identity = typed_trees::type_identity::applied_index_identity(
            program.typed_program(),
            typed_trees::type_identity::IndexIdentityArgument::TypeReference(expression),
            program
                .type_reference_table
                .operand_lifetime_owner(expression),
            &[],
        )?;
        program.type_identity_text(identity)
    }

    pub fn static_argument_identity(
        _program: &dyn crate::CheckingInput,
        argument: &StaticMachineArgument,
    ) -> String {
        argument.display_name()
    }

    pub fn type_argument_identity(
        program: &dyn crate::CheckingInput,
        argument: &StaticMachineArgument,
    ) -> String {
        if argument.type_reference.is_valid() {
            return program.normalized_type_identity_text(argument.type_reference);
        }
        program
            .type_reference_table
            .named_references()
            .find(|(_, symbol, _)| *symbol == argument.symbol)
            .map_or_else(
                || argument.display_name(),
                |(handle, _, _)| program.normalized_type_identity_text(handle),
            )
    }

    pub fn substituted_type_identity(
        program: &dyn crate::CheckingInput,
        handle: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, String)],
        lifetime_substitutions: &[(String, String)],
    ) -> String {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Named { symbol, .. } => substitutions
                .iter()
                .find_map(|(parameter, identity)| (*parameter == *symbol).then(|| identity.clone()))
                .unwrap_or_else(|| program.normalized_type_identity_text(handle)),
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => format!(
                "&{}{}{}",
                lifetime.as_ref().map_or(String::new(), |name| {
                    let lifetime = lifetime_substitutions
                        .iter()
                        .rev()
                        .find_map(|(parameter, argument)| {
                            (parameter == name.as_str()).then_some(argument.as_str())
                        })
                        .unwrap_or_else(|| name.as_str());
                    format!("'{lifetime} ")
                }),
                access.borrow_modifier(),
                substituted_type_identity(program, *referee, substitutions, lifetime_substitutions,)
            ),
            TypeReferenceNode::Constrained { base_type, .. } => substituted_type_identity(
                program,
                *base_type,
                substitutions,
                lifetime_substitutions,
            ),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let length = match length {
                    FixedArrayLength::Literal(value) => value.to_string(),
                    FixedArrayLength::ConstParameter { symbol, name } => substitutions
                        .iter()
                        .find_map(|(parameter, identity)| {
                            (*parameter == *symbol).then(|| identity.clone())
                        })
                        .unwrap_or_else(|| name.as_str().to_owned()),
                    FixedArrayLength::Expression(value) => substituted_type_identity(
                        program,
                        *value,
                        substitutions,
                        lifetime_substitutions,
                    ),
                    FixedArrayLength::ConstCall { name, .. } => format!("{}()", name.as_str()),
                };
                format!(
                    "[{}; {length}]",
                    substituted_type_identity(
                        program,
                        *element_type,
                        substitutions,
                        lifetime_substitutions,
                    )
                )
            }
            TypeReferenceNode::Slice { element_type } => format!(
                "[{}]",
                substituted_type_identity(
                    program,
                    *element_type,
                    substitutions,
                    lifetime_substitutions,
                )
            ),
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
                ..
            } => {
                let mut rendered = lifetime_arguments
                    .iter()
                    .map(|lifetime| {
                        let lifetime = lifetime_substitutions
                            .iter()
                            .rev()
                            .find_map(|(parameter, argument)| {
                                (parameter == lifetime.as_str()).then_some(argument.as_str())
                            })
                            .unwrap_or_else(|| lifetime.as_str());
                        format!("'{lifetime}")
                    })
                    .collect::<Vec<_>>();
                rendered.extend(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .map(|argument| {
                            substituted_type_identity(
                                program,
                                *argument,
                                substitutions,
                                lifetime_substitutions,
                            )
                        }),
                );
                format!("{}<{}>", base_name.as_str(), rendered.join(","))
            }
            _ => program.normalized_type_identity_text(handle),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) struct ApplicationIdentity {
        report_fingerprint: u64,
        commitment: typed_trees::typed_trees::ClosedConformanceApplicationCommitment,
        encoding: std::sync::Arc<[u8]>,
    }

    #[allow(clippy::too_many_arguments)]
    fn application_identity(
        program: &dyn crate::CheckingInput,
        declaration: &str,
        lifetime_arguments: &[String],
        type_arguments: &[String],
        const_arguments: &[ClosedConformanceConstArgument],
        machine_arguments: &[SymbolHandle],
        machine_operands: &[StaticMachineArgument],
        trait_name: &str,
        trait_lifetime_arguments: &[String],
        trait_arguments: &[String],
        rows: &[typed_trees::trait_definition::ConformanceRow],
    ) -> Result<ApplicationIdentity, Diagnostic> {
        use language_semantics::conformance_identity::{
            ConformanceConstIdentity, ConformanceIdentityDescription, ConformanceRowIdentity,
        };
        let const_arguments = const_arguments
            .iter()
            .map(|argument| {
                Ok(match argument {
                    ClosedConformanceConstArgument::Evaluated {
                        parameter_carrier,
                        declared_carrier,
                        value,
                    } => ConformanceConstIdentity::Evaluated {
                        parameter_carrier: program
                            .normalized_type_identity_text(*parameter_carrier),
                        declared_carrier: program.normalized_type_identity_text(*declared_carrier),
                        value_type: value.type_name.as_str(),
                        value_encoding: value.encoding.as_str(),
                    },
                    ClosedConformanceConstArgument::CallerExpression {
                        parameter_carrier,
                        expression,
                    } => ConformanceConstIdentity::CallerExpression {
                        parameter_carrier: program
                            .normalized_type_identity_text(*parameter_carrier),
                        expression_identity: const_expression_identity(program, *expression)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "closed const expression lost its canonical identity",
                                )
                            })?,
                    },
                    ClosedConformanceConstArgument::CallerBinder {
                        parameter_carrier,
                        binder,
                        binder_carrier,
                    } => ConformanceConstIdentity::CallerBinder {
                        parameter_carrier: program
                            .normalized_type_identity_text(*parameter_carrier),
                        binder_carrier: program.normalized_type_identity_text(*binder_carrier),
                        binder_identity: program
                            .symbols
                            .display_path(*binder, "::")
                            .as_str()
                            .to_owned(),
                    },
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let machine_arguments = machine_arguments
            .iter()
            .map(|machine| {
                program
                    .machines()
                    .iter()
                    .flat_map(|owner| {
                        program
                            .machine_states(owner)
                            .iter()
                            .map(move |state| (owner, state))
                    })
                    .find(|(_, state)| state.symbol == *machine)
                    .map(|(owner, state)| format!("{}::{}", owner.name, state.name))
                    .unwrap_or_else(|| "<unresolved-machine>".to_owned())
            })
            .collect();
        let machine_operands = machine_operands
            .iter()
            .map(|operand| normalize_machine_operand(program, operand))
            .collect();
        let rows = rows
            .iter()
            .map(|row| {
                let signature = program
                    .trait_definition_by_symbol(row.declaring_trait)
                    .and_then(|definition| {
                        program.trait_machine_signature_by_symbol(definition, row.requirement)
                    });
                Ok(ConformanceRowIdentity {
                    declaring_trait: row.declaring_trait_name.as_str(),
                    requirement: row.requirement_name.as_str(),
                    realization: row.realization_name.as_str(),
                    requirement_signature: signature
                        .map(|signature| {
                            crate::contract_identities::canonical_state_signature_bytes(
                                program, signature,
                            )
                        })
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let description = ConformanceIdentityDescription {
            declaration,
            trait_name,
            lifetime_arguments,
            type_arguments,
            trait_lifetime_arguments,
            trait_arguments,
            const_arguments,
            machine_arguments,
            machine_operands,
            rows,
        };
        let encoding = description.encoded_bytes();
        let (report_fingerprint, commitment) =
            language_semantics::conformance_identity::source_identity_commitments(&encoding);
        Ok(ApplicationIdentity {
            report_fingerprint,
            commitment:
                typed_trees::typed_trees::ClosedConformanceApplicationCommitment::from_digest(
                    commitment,
                ),
            encoding: encoding.into(),
        })
    }

    fn normalize_machine_operand(
        program: &dyn crate::CheckingInput,
        operand: &StaticMachineArgument,
    ) -> language_semantics::conformance_identity::MachineOperandIdentity {
        use language_semantics::conformance_identity::{
            MachineOperandApplicationIdentity, MachineOperandIdentity,
            MachineOperandSelectionIdentity,
        };
        let selection = if let Some(value) = program.static_const_argument_identity(operand) {
            MachineOperandSelectionIdentity::Const(value)
        } else if operand.type_reference.is_valid()
            || (operand.symbol.is_valid()
                && matches!(
                    program.symbols.get(operand.symbol).kind,
                    SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
                ))
        {
            MachineOperandSelectionIdentity::Type(type_argument_identity(program, operand))
        } else {
            MachineOperandSelectionIdentity::Conformance(
                crate::contract_identities::conformance_symbol_identity(program, operand.symbol),
            )
        };
        MachineOperandIdentity {
            selection,
            application: operand.application.as_ref().map(|application| {
                MachineOperandApplicationIdentity {
                    lifetime_arguments: application
                        .lifetime_arguments
                        .iter()
                        .map(|lifetime| lifetime.as_str().to_owned())
                        .collect(),
                    arguments: application
                        .arguments
                        .iter()
                        .map(|argument| normalize_machine_operand(program, argument))
                        .collect(),
                }
            }),
        }
    }
}
pub(crate) mod dynamic {
    pub(crate) mod row_descriptions {
        use typed_trees::finite_family::FamilyProbe;
        use typed_trees::trait_definition::ConformanceRow;
        use typed_trees::type_identity::CallableIdentityId;

        pub fn describe_dynamic_conformances(
            program: &dyn crate::CheckingInput,
        ) -> Result<checked_trees::DynamicConformanceFacts, Vec<diagnostics::Diagnostic>> {
            let mut selections = Vec::new();
            let mut diagnostics = Vec::new();
            for selection in super::collect_dynamic_implementation_selections(program)? {
                let Some(implementation) = program.conformance_by_symbol(selection.implementation)
                else {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "dynamic selection names no exact implementation",
                    ));
                    continue;
                };
                let mut rows = Vec::new();
                for row in &implementation.rows {
                    if !row.realization_machine.is_valid() || !row.realization_state.is_valid() {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "dynamic implementation row `{}::{}` reached checked lowering without an exact checked realization",
                    row.declaring_trait_name, row.requirement_name
                )));
                        continue;
                    }
                    let (requirement_identity, realization_identity) =
                        match normalized_dynamic_row_identities(program, row) {
                            Ok(identities) => identities,
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                continue;
                            }
                        };
                    let source = match row.source {
                        typed_trees::trait_definition::ConformanceRowSource::Inline
                        | typed_trees::trait_definition::ConformanceRowSource::Synthesized => {
                            checked_trees::DynamicConformanceRowSource::Inline
                        }
                        typed_trees::trait_definition::ConformanceRowSource::Reference => {
                            checked_trees::DynamicConformanceRowSource::Reference
                        }
                        typed_trees::trait_definition::ConformanceRowSource::TraitDefault => {
                            checked_trees::DynamicConformanceRowSource::TraitDefault
                        }
                    };
                    match family_roster(program, row) {
                        FamilyRows::Exact => rows.push(checked_trees::DynamicConformanceRowFact {
                            declaring_trait: row.declaring_trait,
                            requirement: row.requirement,
                            requirement_identity,
                            realization_machine: row.realization_machine,
                            realization_state: row.realization_state,
                            realization_identity,
                            family_tuple: Box::default(),
                            source,
                        }),
                        FamilyRows::Finite(tuples) => {
                            for tuple in &tuples {
                                let Some(instance) = family_specialization_instance(
                                    program,
                                    row.realization_machine,
                                    tuple,
                                ) else {
                                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                                "dynamic implementation row `{}::{}` has no checked specialization for declared family tuple {}",
                                row.declaring_trait_name,
                                row.requirement_name,
                                tuple.display.join(", ")
                            )));
                                    continue;
                                };
                                let Some(instance_state) = family_realization_state(
                                    program,
                                    instance,
                                    row.realization_state,
                                ) else {
                                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                                "dynamic family specialization `{}` has no realization state for tuple {}",
                                program.symbols.name(instance),
                                tuple.display.join(", ")
                            )));
                                    continue;
                                };
                                let Some(instance_machine) = program.machine_by_symbol(instance)
                                else {
                                    continue;
                                };
                                rows.push(checked_trees::DynamicConformanceRowFact {
                                    declaring_trait: row.declaring_trait,
                                    requirement: row.requirement,
                                    requirement_identity,
                                    realization_machine: instance,
                                    realization_state: instance_state,
                                    realization_identity: program
                                        .machine_callable_identity(instance_machine),
                                    family_tuple: tuple.identities.clone(),
                                    source,
                                });
                            }
                        }
                    }
                }
                selections.push(checked_trees::DynamicSelectionFact {
                    occurrence: selection.occurrence,
                    machine: selection.machine,
                    state: selection.state,
                    implementation: selection.implementation,
                    target_trait: selection.target_trait,
                    interface_identity: program
                        .normalized_type_identity(selection.selector_type)
                        .to_owned(),
                    rows,
                });
            }
            if !diagnostics.is_empty() {
                return Err(diagnostics);
            }
            Ok(checked_trees::DynamicConformanceFacts { selections })
        }

        pub(crate) enum FamilyRows {
            Exact,
            Finite(Vec<typed_trees::finite_family::FamilyTuple>),
        }

        fn family_roster(program: &dyn crate::CheckingInput, row: &ConformanceRow) -> FamilyRows {
            let Some(definition) = program.trait_definition_by_symbol(row.declaring_trait) else {
                return FamilyRows::Exact;
            };
            let Some(signature) =
                program.trait_machine_signature_by_symbol(definition, row.requirement)
            else {
                return FamilyRows::Exact;
            };
            match program.finite_signature_family(signature) {
                FamilyProbe::Finite { tuples, .. } => FamilyRows::Finite(tuples),
                FamilyProbe::NotFinite(_) => FamilyRows::Exact,
            }
        }

        fn family_specialization_instance(
            program: &dyn crate::CheckingInput,
            template: symbols::SymbolHandle,
            tuple: &typed_trees::finite_family::FamilyTuple,
        ) -> Option<symbols::SymbolHandle> {
            program
                .machine_specializations
                .iter()
                .find(|specialization| {
                    specialization.template == template
                        && specialization.const_argument_identities.as_slice()
                            == tuple.identities.as_ref()
                        && specialization.is_const_only()
                })
                .map(|specialization| specialization.instance)
        }

        fn family_realization_state(
            program: &dyn crate::CheckingInput,
            instance: symbols::SymbolHandle,
            template_state: symbols::SymbolHandle,
        ) -> Option<symbols::SymbolHandle> {
            let machine = program.machine_by_symbol(instance)?;
            let name = program.symbols.name(template_state);
            program
                .machine_states(machine)
                .iter()
                .find(|state| program.symbols.name(state.symbol) == name)
                .or_else(|| program.machine_states(machine).first())
                .map(|state| state.symbol)
        }

        pub fn normalized_dynamic_row_identities(
            program: &dyn crate::CheckingInput,
            row: &ConformanceRow,
        ) -> Result<(CallableIdentityId, CallableIdentityId), diagnostics::Diagnostic> {
            let mut declaring_traits = program
                .traits()
                .iter()
                .filter(|definition| definition.symbol == row.declaring_trait);
            let declaring_trait = declaring_traits.next().ok_or_else(|| {
                diagnostics::Diagnostic::error(
                    "dynamic conformance row has no exact declaring trait for normalized identity",
                )
            })?;
            if declaring_traits.next().is_some() {
                return Err(diagnostics::Diagnostic::error(
                    "dynamic conformance row has an ambiguous declaring trait for normalized identity",
                ));
            }

            let mut requirements = program
                .trait_machine_signatures(declaring_trait)
                .iter()
                .filter(|requirement| requirement.symbol == row.requirement);
            let requirement = requirements.next().ok_or_else(|| {
                diagnostics::Diagnostic::error(
                    "dynamic conformance row has no exact requirement for normalized identity",
                )
            })?;
            if requirements.next().is_some() {
                return Err(diagnostics::Diagnostic::error(
                    "dynamic conformance row has an ambiguous requirement for normalized identity",
                ));
            }

            let mut realization_machines = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == row.realization_machine);
            let realization_machine = realization_machines.next().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "dynamic conformance row has no exact realization machine for normalized identity",
        )
    })?;
            if realization_machines.next().is_some() {
                return Err(diagnostics::Diagnostic::error(
                    "dynamic conformance row has an ambiguous realization machine for normalized identity",
                ));
            }
            let realization_identity = program.machine_callable_identity(realization_machine);
            let requirement_identity =
                program.trait_requirement_callable_identity(declaring_trait, requirement);
            Ok((requirement_identity, realization_identity))
        }
    }
    pub use row_descriptions::{describe_dynamic_conformances, normalized_dynamic_row_identities};
    use symbols::SymbolHandle;
    use typed_trees::finite_family::FamilyProbe;
    use typed_trees::references::DeclarationSelection;
    use typed_trees::trait_definition::AppliedTraitScope;
    use typed_trees::type_identity::TypeIdentityId;

    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::signature::StateSignature;
    use typed_trees::state::State;
    use typed_trees::statement::StatementNode;
    use typed_trees::trait_definition::{DynamicSignatureIneligibility, TraitDefinition};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DynamicImplementationSelection {
        pub occurrence: ExpressionHandle,
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub implementation: SymbolHandle,
        pub selector_type: TypeReferenceHandle,
        pub target_trait: SymbolHandle,
    }

    pub fn collect_dynamic_implementation_selections(
        program: &dyn crate::CheckingInput,
    ) -> Result<Vec<DynamicImplementationSelection>, Vec<Diagnostic>> {
        let mut selections = Vec::new();
        let mut diagnostics = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let mut visited =
                    crate::queries::expression_traversal::EnteredExpressions::default();
                for statement in program.statement_table.statements(state.statement_nodes) {
                    for (_, root) in program.tables.expression_roots(statement) {
                        collect_selections_in(
                            program,
                            machine.symbol,
                            state.symbol,
                            root,
                            &mut visited,
                            &mut selections,
                            &mut diagnostics,
                        );
                    }
                }
            }
        }
        for declaration in program.const_declarations() {
            let root = declaration.materialized_initializer;
            collect_selections_in(
                program,
                SymbolHandle::invalid(),
                SymbolHandle::invalid(),
                root,
                &mut crate::queries::expression_traversal::EnteredExpressions::default(),
                &mut selections,
                &mut diagnostics,
            );
            if let Some(cast) = expression_table(program).cast_node(root)
                && matches!(
                    program
                        .type_reference_table
                        .type_reference(cast.target_type),
                    TypeReferenceNode::DynamicTrait { .. }
                )
                && program.normalized_type_identity(cast.target_type)
                    != program.normalized_type_identity(declaration.declared_type)
            {
                diagnostics.push(Diagnostic::error(
                    "constant implementation selector differs from its declared trait application",
                ));
            }
        }
        if diagnostics.is_empty() {
            Ok(selections)
        } else {
            Err(diagnostics)
        }
    }

    // Every dynamic selector in `expression`'s subtree, children before
    // parents, skipping expressions already entered.
    fn collect_selections_in(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        expression: ExpressionHandle,
        entered: &mut crate::queries::expression_traversal::EnteredExpressions,
        selections: &mut Vec<DynamicImplementationSelection>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        crate::queries::expression_traversal::visit_post_order(
            program,
            expression,
            entered,
            |expression, node| {
                collect_selection_at(
                    program,
                    machine,
                    state,
                    expression,
                    node,
                    selections,
                    diagnostics,
                )
            },
        );
    }

    fn collect_selection_at(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        expression: ExpressionHandle,
        node: &ExpressionNode,
        selections: &mut Vec<DynamicImplementationSelection>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let ExpressionNode::Cast(cast) = node else {
            return;
        };
        let TypeReferenceNode::DynamicTrait {
            symbol: target_trait,
            name: trait_name,
            arguments,
        } = program
            .type_reference_table
            .type_reference(cast.target_type)
        else {
            return;
        };
        let implementation = match expression_table(program).expression(cast.value) {
            ExpressionNode::Name(name)
                if name.symbol.is_valid()
                    && program.symbols.get(name.symbol).kind
                        == symbols::SymbolKind::Conformance =>
            {
                name.symbol
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                "`{}` converts a value to `dyn {trait_name}`; a selector is constructed from a named implementation (`Implementation as dyn {trait_name}<...>`) and the data is passed to each call",
                cast.display_name(expression_table(program))
            )));
                return;
            }
        };
        let Some(conformance) = program.conformance_by_symbol(implementation) else {
            return;
        };
        let implementation_name = program.symbols.name(implementation);
        let selector_arguments = program
            .type_reference_table
            .type_reference_handles(*arguments);
        let implementation_arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let same_application = conformance.trait_symbol == *target_trait
            && selector_arguments.len() == implementation_arguments.len()
            && selector_arguments.iter().zip(implementation_arguments).all(
                |(selector, implementation)| {
                    program.normalized_type_identity(*selector)
                        == program.normalized_type_identity(*implementation)
                },
            );
        if !same_application {
            let display =
                |handle: TypeReferenceHandle| program.type_reference_table.display_name(handle);
            let implemented = format!(
                "{}<{}>",
                conformance.trait_name,
                implementation_arguments
                    .iter()
                    .map(|argument| display(*argument))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let selected = format!(
                "dyn {trait_name}<{}>",
                selector_arguments
                    .iter()
                    .map(|argument| display(*argument))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            diagnostics.push(Diagnostic::error(format!(
            "`{implementation_name}` implements `{implemented}`, not `{selected}`; a selector names exactly the trait application its implementation satisfies"
        )));
            return;
        }
        selections.push(DynamicImplementationSelection {
            occurrence: expression,
            machine,
            state,
            implementation,
            selector_type: cast.target_type,
            target_trait: *target_trait,
        });
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DynamicSlotDemand {
        pub selector_type: TypeReferenceHandle,
        pub declaring_trait: SymbolHandle,
        pub requirement: SymbolHandle,
        pub call_requirement: SymbolHandle,
        pub family_call_requirements: Vec<SymbolHandle>,
        pub type_substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
        pub substitution_scope_starts: Vec<usize>,
    }

    pub fn collect_dynamic_slot_demands(
        program: &dyn crate::CheckingInput,
    ) -> Result<Vec<DynamicSlotDemand>, Vec<Diagnostic>> {
        let mut demands: Vec<(TypeIdentityId, DynamicSlotDemand)> = Vec::new();
        let mut push = |demand: DynamicSlotDemand| {
            let identity = program.normalized_type_identity(demand.selector_type);
            let mut variants = vec![demand.clone()];
            for requirement in &demand.family_call_requirements {
                let mut closed = demand.clone();
                closed.call_requirement = *requirement;
                variants.push(closed);
            }
            for variant in variants {
                if !demands.iter().any(|(existing, known)| {
                    *existing == identity && known.call_requirement == variant.call_requirement
                }) {
                    demands.push((identity, variant));
                }
            }
        };
        for selection in collect_dynamic_implementation_selections(program)? {
            let Some(implementation) = program.conformance_by_symbol(selection.implementation)
            else {
                continue;
            };
            let Some((selector_trait, selector_arguments)) =
                dynamic_selector_application(program, selection.selector_type)
            else {
                continue;
            };
            let Some(selector_definition) = program.trait_definition_by_symbol(selector_trait)
            else {
                continue;
            };
            for row in &implementation.rows {
                let Some(application) = AppliedTraitScope::new(
                    program,
                    selector_definition,
                    typed_trees::trait_definition::TraitApplicationArguments {
                        lexical_owner: selection.machine,
                        enclosing_data_applications: &[],
                        arguments: selector_arguments,
                        lifetimes: &[],
                    },
                )
                .and_then(|scope| scope.declaring_scope(program, row.declaring_trait)) else {
                    continue;
                };
                push(DynamicSlotDemand {
                    selector_type: selection.selector_type,
                    declaring_trait: row.declaring_trait,
                    requirement: row.requirement,
                    call_requirement: row.requirement,
                    family_call_requirements: closed_family_requirements(
                        program,
                        row.requirement,
                        &application,
                    ),
                    type_substitutions: application.substitutions,
                    substitution_scope_starts: application.substitution_scope_starts,
                });
            }
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let mut visited =
                    crate::queries::expression_traversal::EnteredExpressions::default();
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(call) = statement.as_call()
                        && let Some(demand) = dynamic_call_demand(
                            program,
                            machine,
                            state,
                            call.receiver_symbol,
                            call.target_symbol,
                            program.statement_table.expression_handles(call.arguments),
                        )
                    {
                        push(demand);
                    }
                    for (_, root) in program.tables.expression_roots(statement) {
                        collect_call_demands_in(
                            program,
                            machine,
                            state,
                            root,
                            &mut visited,
                            &mut push,
                        );
                    }
                }
            }
        }
        Ok(demands.into_iter().map(|(_, demand)| demand).collect())
    }

    fn collect_call_demands_in(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        entered: &mut crate::queries::expression_traversal::EnteredExpressions,
        push: &mut impl FnMut(DynamicSlotDemand),
    ) {
        crate::queries::expression_traversal::visit_post_order(
            program,
            expression,
            entered,
            |_, node| {
                if let ExpressionNode::Call(call) = node
                    && let Some(demand) =
                        dynamic_expression_call_demand(program, machine, state, call)
                {
                    push(demand);
                }
            },
        );
    }

    pub fn dynamic_expression_call_demand(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        call: &typed_trees::expression::TableCallExpression,
    ) -> Option<DynamicSlotDemand> {
        dynamic_call_demand(
            program,
            machine,
            state,
            crate::machine_calls::calls::dynamic_requirement::expression_trait_receiver(
                program,
                call.receiver,
            ),
            call.target_symbol,
            expression_table(program).expression_handles(call.arguments),
        )
    }

    pub fn dynamic_call_demand(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        receiver: SymbolHandle,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
    ) -> Option<DynamicSlotDemand> {
        let call = crate::machine_calls::calls::dynamic_requirement::dynamic_selector_call(
            program,
            machine,
            Some(state),
            receiver,
            target,
            arguments,
        )
        .ok()??;
        Some(DynamicSlotDemand {
            selector_type: call.selector_type,
            declaring_trait: call.application.declaration.symbol,
            requirement: call.signature.symbol,
            call_requirement: call
                .closed_signature
                .filter(|_| !call.signature.type_parameters.is_empty())
                .map_or(call.signature.symbol, |signature| signature.symbol),
            family_call_requirements: closed_family_requirements(
                program,
                call.signature.symbol,
                &call.application,
            ),
            type_substitutions: call.application.substitutions,
            substitution_scope_starts: call.application.substitution_scope_starts,
        })
    }

    pub(crate) fn dynamic_requirement_ineligibility(
        program: &dyn crate::CheckingInput,
        declaring_trait: &TraitDefinition,
        requirement: &StateSignature,
    ) -> Option<&'static str> {
        if let Err(reason) = program.dynamic_signature_eligibility(declaring_trait, requirement) {
            return Some(match reason {
                DynamicSignatureIneligibility::BoundaryRequirement => {
                    "boundary-machine requirements are not local dynamic calls"
                }
                DynamicSignatureIneligibility::RequirementLocalGenerics => {
                    "the requirement has requirement-local generic parameters outside one finite family"
                }
            });
        }
        program
        .dynamic_non_signature_eligibility(requirement)
        .err()
        .map(|reason| match reason {
            typed_trees::dynamic_traits::DynamicNonSignatureIneligibility::PrivateContractIdentity => {
                "its public contract names an identity a dynamic caller cannot see"
            }
            typed_trees::dynamic_traits::DynamicNonSignatureIneligibility::UnprojectableResultLifetime => {
                "its result names a borrow lifetime no argument binds"
            }
            typed_trees::dynamic_traits::DynamicNonSignatureIneligibility::UnfittableEnvelope => {
                "its operational envelope (installation-bound reach or native callback parameters) cannot be selected at runtime"
            }
        })
    }

    pub(crate) fn dynamic_selector_application(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<(SymbolHandle, &[TypeReferenceHandle])> {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                dynamic_selector_application(program, *base_type)
            }
            TypeReferenceNode::DynamicTrait {
                symbol, arguments, ..
            } => Some((
                *symbol,
                program
                    .type_reference_table
                    .type_reference_handles(*arguments),
            )),
            _ => None,
        }
    }

    pub(crate) fn report_dynamic_receiver_call_error(
        program: &dyn crate::CheckingInput,
        receiver_type_reference: Option<TypeReferenceHandle>,
        target: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        let Some(error) = receiver_type_reference.and_then(|type_reference| {
            dynamic_receiver_call_error(program, type_reference, target)
        }) else {
            return false;
        };
        diagnostics.push(Diagnostic::error(error));
        true
    }

    pub(crate) fn dynamic_receiver_call_error(
        program: &dyn crate::CheckingInput,
        receiver_type: TypeReferenceHandle,
        target: &str,
    ) -> Option<String> {
        let (trait_symbol, _) = dynamic_selector_application(program, receiver_type)?;
        let trait_name = program.symbols.name(trait_symbol);
        Some(format!(
            "a `dyn {trait_name}` selector is a dispatch operand, not a receiver; call `{trait_name}::{target}(selector, ...)`"
        ))
    }

    pub fn qualified_trait_scope<'program>(
        program: &'program dyn crate::CheckingInput,
        lexical_owner: SymbolHandle,
        namespace: SymbolHandle,
        value_type: Option<TypeReferenceHandle>,
    ) -> Option<AppliedTraitScope<'program>> {
        program
            .trait_definition_by_symbol(namespace)
            .map(|declaration| {
                program
                    .lexical_conformance_scope(lexical_owner)
                    .and_then(|(_, scope)| scope.declaring_scope(program, namespace))
                    .unwrap_or_else(|| AppliedTraitScope::abstract_scope(declaration))
            })
            .or_else(|| {
                value_type.and_then(|reference| {
                    AppliedTraitScope::from_type_reference(program, reference, lexical_owner)
                })
            })
    }

    pub fn resolve_dynamic_call_targets(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        resolve_selected_dynamic_call_targets(program, &DeclarationSelection::all())
    }

    pub fn resolve_selected_dynamic_call_targets(
        program: &mut TypedTrees,
        domain: &DeclarationSelection,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut expression_updates = Vec::new();
        let mut statement_updates = Vec::new();
        let mut diagnostics = Vec::new();

        for machine in program.machines() {
            if !domain.contains(machine.symbol) {
                continue;
            }
            for state in program.machine_states(machine) {
                for (statement_index, statement) in program
                    .statement_table
                    .indexed_statements(state.statement_nodes)
                {
                    if let Some(call) = statement.as_call() {
                        let members = program.statement_table.name_path_members(call.receiver);
                        let receiver = members.split_first().and_then(|(first, members)| {
                            let root = crate::receiver_projections::root(
                                program,
                                state,
                                statement_index,
                                call.receiver_root_symbol,
                                first,
                            )?;
                            members.iter().try_fold(root, |receiver, member| {
                                crate::receiver_projections::field(program, receiver, member)
                            })
                        });
                        let dynamic_target = qualified_trait_scope(
                            program,
                            machine.symbol,
                            call.receiver_symbol,
                            receiver.as_ref().map(|receiver| receiver.type_reference),
                        )
                        .and_then(|scope| {
                            resolved_dynamic_requirement_symbol(
                                program,
                                machine.symbol,
                                scope,
                                program
                                    .trait_definition_by_symbol(call.receiver_symbol)
                                    .is_some(),
                                call.target.as_str(),
                                call.target_symbol,
                                call.source_span,
                                &mut diagnostics,
                            )
                        });
                        let target = dynamic_target.or_else(|| {
                            crate::value_custody::access_views::statement_call_target(
                                program, machine, state, call,
                            )
                        });
                        if let Some(target) = target {
                            let endpoint = receiver
                                .as_ref()
                                .filter(|_| dynamic_target.is_some())
                                .map_or_else(SymbolHandle::invalid, |receiver| receiver.symbol);
                            if endpoint.is_valid()
                                && call.receiver_symbol.is_valid()
                                && call.receiver_symbol != endpoint
                            {
                                diagnostics.push(
                                Diagnostic::error(
                                    "projected call receiver disagrees with its exact declared field",
                                )
                                .with_source_span(call.source_span),
                            );
                            }
                            statement_updates.push((
                                state.statement_nodes,
                                statement_index,
                                endpoint,
                                target,
                            ));
                        }
                    }

                    for (_, root) in program.tables.expression_roots(statement) {
                        collect_dynamic_expression_call_updates(
                            program,
                            machine,
                            state,
                            statement_index,
                            root,
                            &mut expression_updates,
                            &mut diagnostics,
                        );
                    }
                }
            }
        }

        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        for (expression, requirement) in expression_updates {
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression)
            else {
                unreachable!("collected qualified expression call changed shape")
            };
            call.target_symbol = requirement;
        }
        for (statements, statement_index, endpoint, requirement) in statement_updates {
            let StatementNode::Call(call) =
                &mut program.statement_table.statements_mut(statements)[statement_index]
            else {
                unreachable!("collected dynamic statement call changed shape")
            };
            if endpoint.is_valid() {
                call.receiver_symbol = endpoint;
            }
            call.target_symbol = requirement;
        }

        Ok(())
    }

    fn collect_dynamic_expression_call_updates(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        before: usize,
        expression: ExpressionHandle,
        updates: &mut Vec<(ExpressionHandle, SymbolHandle)>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !expression.is_valid() {
            return;
        }

        macro_rules! visit {
            ($child:expr) => {
                collect_dynamic_expression_call_updates(
                    program,
                    machine,
                    state,
                    before,
                    $child,
                    updates,
                    diagnostics,
                )
            };
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::MachineValue(_) => {}
            ExpressionNode::Match(dispatch) => {
                for child in
                    crate::queries::expression_traversal::match_children(program, *dispatch)
                {
                    visit!(child);
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                for element in expression_table(program).expression_handles(*elements) {
                    visit!(*element);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                visit!(atomic.value);
                visit!(atomic.result);
            }
            ExpressionNode::Binary(binary) => {
                visit!(binary.left);
                visit!(binary.right);
            }
            ExpressionNode::Cast(cast) => visit!(cast.value),
            ExpressionNode::Call(call) => {
                if call.runtime_call.is_some() {
                    visit!(call.evaluated_callee());
                    for argument in expression_table(program).expression_handles(call.arguments) {
                        visit!(*argument);
                    }
                    return;
                }

                let receiver_expression = call.receiver;
                let receiver_type = match crate::receiver_projections::expression(
                    program,
                    state,
                    before,
                    receiver_expression,
                    &mut Vec::new(),
                ) {
                    Ok(Some(receiver)) => Some(receiver.type_reference),
                    Ok(None) => crate::value_custody::places::declared_place_type_raw(
                        program,
                        machine,
                        Some(state),
                        receiver_expression,
                    ),
                    Err(errors) => {
                        diagnostics.extend(errors);
                        None
                    }
                };
                let namespace = expression_table(program)
                    .name_path_node(receiver_expression)
                    .map_or_else(SymbolHandle::invalid, |path| path.symbol);
                let scope = if call.requirement_application.is_valid() {
                    program.applied_requirement_scope(machine.symbol, call.requirement_application)
                } else {
                    qualified_trait_scope(program, machine.symbol, namespace, receiver_type)
                };
                if let Some(scope) = scope
                    && let Some(requirement) = resolved_dynamic_requirement_symbol(
                        program,
                        machine.symbol,
                        scope,
                        program.trait_definition_by_symbol(namespace).is_some(),
                        call.target.as_str(),
                        call.target_symbol,
                        expression_table(program).source_span(expression),
                        diagnostics,
                    )
                    && !updates
                        .iter()
                        .any(|(candidate, _)| *candidate == expression)
                {
                    updates.push((expression, requirement));
                }
                visit!(call.receiver);
                for argument in expression_table(program).expression_handles(call.arguments) {
                    visit!(*argument);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                visit!(indexed.collection);
                visit!(indexed.index);
            }
            ExpressionNode::Member(member) => visit!(member.receiver),
            ExpressionNode::Borrow(inner) => visit!(inner.target),
            ExpressionNode::Range(range) => {
                visit!(range.start);
                visit!(range.end);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in expression_table(program).struct_fields(literal.fields) {
                    visit!(field.value);
                }
            }
            ExpressionNode::Unary(unary) => visit!(unary.operand),
            ExpressionNode::TypeExpression(_)
            | ExpressionNode::SavedSubject(_)
            | typed_trees::literal_expression_pattern!() => {}
        }
    }

    fn resolved_dynamic_requirement_symbol(
        program: &dyn crate::CheckingInput,
        lexical_owner: SymbolHandle,
        receiver_scope: AppliedTraitScope<'_>,
        static_namespace: bool,
        target: &str,
        retained_target: SymbolHandle,
        span: source::SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<SymbolHandle> {
        let trait_definition = receiver_scope.declaration;
        let mut matches = Vec::new();
        collect_dynamic_requirements_named(program, &receiver_scope, target, &mut matches);
        if static_namespace {
            matches.retain(|(scope, _)| scope.declaration.symbol == trait_definition.symbol);
        }
        let retained_application = program.closed_requirement_application(retained_target);
        if static_namespace
            && let Some((selecting, selecting_scope)) =
                program.lexical_conformance_scope(lexical_owner)
        {
            let capture = program.implementation_capture(
                lexical_owner,
                program.lifetime_declaration_owner(lexical_owner),
                selecting.symbol,
            );
            let retained_entry = capture.and_then(|capture| {
                program
                    .realization_entry_template(retained_target, &capture.conformance_application)
            });
            let retained_requirement = program
                .closed_requirement_application(retained_target)
                .map_or(retained_target, |application| application.template);
            let mut routes = selecting.rows.iter().filter(|row| {
                (row.realization_state == retained_target
                    || retained_entry.is_some_and(|(machine, state)| {
                        machine == row.realization_machine && state == row.realization_state
                    })
                    || row.requirement == retained_requirement)
                    && matches.iter().any(|(scope, requirement)| {
                        row.declaring_trait == scope.declaration.symbol
                            && row.requirement == requirement.symbol
                            && retained_application.is_none_or(|application| {
                                program.requirement_application_matches_scope(
                                    application.signature.symbol,
                                    scope,
                                )
                            })
                            && selecting_scope
                                .declaring_scope(program, row.declaring_trait)
                                .is_some_and(|selected| selected.same_application(program, scope))
                    })
            });
            if let Some(row) = routes.next()
                && routes.next().is_none()
            {
                return Some(if retained_entry.is_some() {
                    retained_target
                } else {
                    row.realization_state
                });
            }
        }
        let template =
            retained_application.map_or(retained_target, |application| application.template);
        let retained_matches = matches.iter().any(|(scope, requirement)| {
            requirement.symbol == template
                && retained_application.is_none_or(|application| {
                    program
                        .requirement_application_matches_scope(application.signature.symbol, scope)
                })
        });
        if retained_target.is_valid() && !retained_matches {
            let expected_application = receiver_scope
                .portable_application_identity(program.typed_program())
                .and_then(|identity| program.type_identity_text(identity))
                .unwrap_or_else(|| "<unavailable application>".into());
            let retained_application = retained_application
                .and_then(|application| {
                    program.type_identity_text(application.portable_trait_application_identity)
                })
                .unwrap_or_else(|| "<unapplied target>".into());
            diagnostics.push(
            Diagnostic::error(format!(
                "qualified call `{}::{target}` in `{}` retains target `{}` from a different requirement application: expected {expected_application}, retained {retained_application}",
                trait_definition.name,
                program.symbols.name(lexical_owner),
                program.symbols.name(retained_target),
            ))
                .with_source_span(span),
        );
            return None;
        }
        match matches.as_slice() {
            [(_, requirement)] => Some(if retained_application.is_some() {
                retained_target
            } else {
                requirement.symbol
            }),
            [] => None,
            many => {
                let declaring_scope = &many[0].0;
                if many
                    .iter()
                    .all(|(candidate, _)| candidate.same_application(program, declaring_scope))
                {
                    return Some(if retained_matches {
                        retained_target
                    } else {
                        many[0].1.symbol
                    });
                }
                let declarations = many
                    .iter()
                    .map(|(declaring_scope, requirement)| {
                        format!("{}::{}", declaring_scope.declaration.name, requirement.name)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                diagnostics.push(Diagnostic::error(format!(
                "dynamic call `{}::{target}` is ambiguous across inherited requirements: {declarations}",
                trait_definition.name
            )));
                None
            }
        }
    }

    fn collect_dynamic_requirements_named<'program>(
        program: &'program dyn crate::CheckingInput,
        receiver_scope: &AppliedTraitScope<'program>,
        target: &str,
        matches: &mut Vec<(AppliedTraitScope<'program>, &'program StateSignature)>,
    ) {
        receiver_scope.visit(program, &mut |scope| {
            for requirement in program
                .trait_machine_signatures(scope.declaration)
                .iter()
                .filter(|requirement| requirement.name.as_str() == target)
            {
                if !matches.iter().any(|(known, candidate)| {
                    candidate.symbol == requirement.symbol && known.same_application(program, scope)
                }) {
                    matches.push((scope.clone(), requirement));
                }
            }
        });
    }

    fn closed_family_requirements(
        program: &dyn crate::CheckingInput,
        requirement: SymbolHandle,
        application: &AppliedTraitScope<'_>,
    ) -> Vec<SymbolHandle> {
        let Some(signature) = program
            .applied_callable_signature(application, requirement)
            .or_else(|| {
                program
                    .trait_machine_signature(requirement)
                    .map(|(_, signature)| signature)
            })
        else {
            return Vec::new();
        };
        let FamilyProbe::Finite { arity, tuples } = program.finite_signature_family(signature)
        else {
            return Vec::new();
        };
        tuples
            .iter()
            .filter_map(|tuple| {
                program
                    .closed_requirement_applications()
                    .find(|instance| {
                        instance.template == requirement
                            && instance.local_arguments.len() == arity
                            && program.requirement_application_matches_scope(
                                instance.signature.symbol,
                                application,
                            )
                            && instance
                                .local_arguments
                                .iter()
                                .map(|argument| {
                                    argument.static_operand().and_then(|reference| {
                                        program.static_const_type_reference_identity(reference)
                                    })
                                })
                                .collect::<Option<Vec<_>>>()
                                .as_deref()
                                == Some(tuple.identities.as_ref())
                    })
                    .map(|instance| instance.signature.symbol)
            })
            .collect()
    }
}
pub(crate) mod named_implementations {
    use super::trait_definition_by_symbol;
    use crate::value_custody::type_references::TypeParameterScope;
    use diagnostics::Diagnostic;
    use typed_trees::trait_definition::TraitDefinition;
    use typed_trees::trait_definition::{AppliedTraitScope, ConformanceRowSource};

    pub(crate) fn validate_member_formations(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let realizations =
            arena::unique_handles_in_order(program.conformances().iter().flat_map(|conformance| {
                conformance.rows.iter().map(|row| row.realization_machine)
            }));
        for realization in realizations {
            let Some(machine) = program.machine(realization) else {
                continue;
            };
            if !machine.target_family_interface {
                validate_member_formation(machine, diagnostics);
            } else if let Some(members) =
                crate::machine_calls::calls::target_family_members(program, machine)
            {
                for member in members {
                    if let Some(body) = program.machine(member.machine) {
                        validate_member_formation(body, diagnostics);
                    }
                }
            }
        }
    }

    fn validate_member_formation(
        machine: &typed_trees::machine::Machine,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        if !machine.body_is_present
            && !matches!(
                machine.supply_mode,
                language_semantics::MachineSupplyMode::ExternalRealization { .. }
            )
        {
            diagnostics.push(Diagnostic::error(format!(
            "inline member `{}` requires a checked body or an admitted external realization; a boundary claim does not supply an implementation",
            machine.name,
        )));
            return false;
        }
        true
    }

    pub(crate) fn validate_conformances(
        program: &dyn crate::CheckingInput,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        for (index, conformance) in program.conformances().iter().enumerate() {
            if program.conformances()[..index].iter().any(|previous| {
                !program
                    .symbols
                    .source_scopes_separate(previous.symbol, conformance.symbol)
                    && previous.alias == conformance.alias
            }) {
                let name = conformance
                    .alias
                    .as_ref()
                    .map_or("<unnamed implementation>", |alias| alias.as_str());
                diagnostics.push(Diagnostic::error(format!(
                    "named implementation `{name}` is declared more than once"
                )));
            }
            validate_named_implementation(
                program,
                service_reaches,
                symbols,
                conformance,
                diagnostics,
                formation_obligations,
            );
        }
    }

    pub fn revalidate_named_implementation(
        program: &dyn crate::CheckingInput,
        conformance: &typed_trees::trait_definition::Conformance,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let mut formation_obligations = Vec::new();
        let symbols =
            crate::declarations::symbols::TopLevelSymbols::build(program, &mut diagnostics);
        let operational = crate::infer_operational_may(program);
        let service_reaches = crate::infer_service_reaches(program, &operational);
        validate_named_implementation(
            program,
            &service_reaches,
            &symbols,
            conformance,
            &mut diagnostics,
            &mut formation_obligations,
        );
        crate::program_validation::finish_strict_diagnostics(diagnostics)
    }

    fn validate_named_implementation(
        program: &dyn crate::CheckingInput,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
        conformance: &typed_trees::trait_definition::Conformance,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        let implementation_name = conformance
            .alias
            .as_ref()
            .map_or("<unnamed implementation>", |alias| alias.as_str());
        let trait_name = conformance.trait_name.as_str();
        let conformance_name = conformance.alias.as_ref().map(|name| name.as_str());
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.trait_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
            "conformance `{implementation_name} satisfies {trait_name}` names unknown trait `{trait_name}`"
        )));
            return;
        };

        let expected_lifetimes = trait_definition.lifetime_parameters.len();
        if conformance.trait_lifetime_arguments.len() != expected_lifetimes {
            diagnostics.push(Diagnostic::error(format!(
            "conformance `{implementation_name} satisfies {trait_name}` expects {expected_lifetimes} target-trait lifetime argument(s), got {}",
            conformance.trait_lifetime_arguments.len()
        )));
            return;
        }
        if conformance.trait_lifetime_arguments.iter().any(|ordinal| {
            usize::try_from(*ordinal).map_or(true, |ordinal| {
                ordinal >= conformance.lifetime_parameters.len()
            })
        }) {
            diagnostics.push(Diagnostic::error(format!(
            "conformance `{implementation_name} satisfies {trait_name}` retains a target-trait lifetime outside its conformance telescope"
        )));
            return;
        }

        let arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let expected = program.trait_type_parameters(trait_definition).len();
        if arguments.len() != expected {
            diagnostics.push(Diagnostic::error(format!(
            "conformance `{implementation_name} satisfies {trait_name}` expects {expected} generic argument(s), got {}",
            arguments.len()
        )));
            return;
        }

        let applied_lifetimes = conformance
            .trait_lifetime_arguments
            .iter()
            .map(|ordinal| conformance.lifetime_parameters[*ordinal as usize].clone())
            .collect::<Vec<_>>();
        super::conformance::validate_trait_application_obligations(
            program,
            trait_definition,
            arguments,
            &applied_lifetimes,
            &format!("conformance `{implementation_name} satisfies {trait_name}`"),
            symbols,
            TypeParameterScope::authored(
                conformance.symbol,
                program
                    .data_type_parameters
                    .span_or_empty(conformance.type_parameters),
                &conformance.lifetime_parameters,
            ),
            diagnostics,
            formation_obligations,
        );

        validate_closed_rows(
            program,
            service_reaches,
            implementation_name,
            trait_definition,
            &conformance.rows,
            conformance_name,
            conformance,
            diagnostics,
        );
    }

    fn validate_closed_rows(
        program: &dyn crate::CheckingInput,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        implementation_name: &str,
        root_trait: &TraitDefinition,
        rows: &[typed_trees::trait_definition::ConformanceRow],
        conformance_name: Option<&str>,
        conformance: &typed_trees::trait_definition::Conformance,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(root_application) =
            AppliedTraitScope::from_conformance(program.typed_program(), conformance)
        else {
            return;
        };
        for row in rows {
            let Some(declaring_trait) = trait_definition_by_symbol(program, row.declaring_trait)
            else {
                diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name} satisfies {}` retains an unresolved declaring trait for row `{}`",
                root_trait.name, row.requirement_name
            )));
                continue;
            };
            let Some(requirement) =
                program.trait_machine_signature_by_symbol(declaring_trait, row.requirement)
            else {
                diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name} satisfies {}` retains an unresolved requirement `{}::{}`",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
                continue;
            };
            if row.source == ConformanceRowSource::TraitDefault && !requirement.is_default {
                diagnostics.push(Diagnostic::error(format!(
                    "closed conformance `{implementation_name} satisfies {}` selects a default for bodyless requirement `{}::{}`",
                    root_trait.name, row.declaring_trait_name, row.requirement_name
                )));
            }
            let Some(machine) = program.machine(row.realization_machine) else {
                diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name} satisfies {}` row `{}::{}` retains no exact realization machine",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
                continue;
            };
            let Some(state) = program.state_in_machine(machine, row.realization_state) else {
                diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name} satisfies {}` row `{}::{}` retains no exact realization state",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
                continue;
            };
            let Some(application) =
                root_application.declaring_scope(program.typed_program(), row.declaring_trait)
            else {
                diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name} satisfies {}` cannot instantiate inherited row `{}::{}`",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
                continue;
            };
            let validate_member =
                |machine: &typed_trees::machine::Machine,
                 state: &typed_trees::state::State,
                 diagnostics: &mut Vec<Diagnostic>| {
                    if !validate_member_formation(machine, diagnostics) {
                        return;
                    }
                    super::conformance::validate_applied_trait_signature(
                        program,
                        service_reaches,
                        machine,
                        state,
                        &application,
                        requirement,
                        Some(conformance),
                        diagnostics,
                    );
                    crate::proof_contracts::contract_entailment::check_law_conformance(
                        program,
                        machine,
                        conformance_name,
                        &application,
                        requirement,
                        diagnostics,
                        &mut Vec::new(),
                    );
                };
            if !machine.target_family_interface {
                validate_member(machine, state, diagnostics);
                continue;
            }
            let Some(members) =
                crate::machine_calls::calls::target_family_members(program, machine)
            else {
                diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name}` target family `{}` retains no complete authored member roster",
                machine.name,
            )));
                continue;
            };
            let Some(state_position) =
                typed_trees::state::state_position(program.machine_states(machine), state.symbol)
            else {
                diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name}` target family `{}` retains a state outside its callable interface",
                machine.name,
            )));
                continue;
            };
            for member in members {
                let Some((body, entry)) = program.machine(member.machine).and_then(|body| {
                    program
                        .machine_states(body)
                        .get(state_position)
                        .map(|entry| (body, entry))
                }) else {
                    diagnostics.push(Diagnostic::error(format!(
                    "closed conformance `{implementation_name}` target family `{}` has no corresponding callable state on target `{}`",
                    machine.name, member.target,
                )));
                    continue;
                };
                validate_member(body, entry, diagnostics);
            }
        }
    }
}
pub(crate) mod provider_applications {
    use language_semantics::MachineSupplyMode;
    use symbols::SymbolHandle;
    use typed_trees::trait_definition::AppliedTraitScope;
    use typed_trees::type_identity::{LifetimeParameterIdentity, NormalizedTypeIdentity};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderApplicationIdentity {
        pub provider: SymbolHandle,
        pub selecting_declaration: SymbolHandle,
        pub template: SymbolHandle,
        pub closed_requirement: SymbolHandle,
        pub declaration_application: NormalizedTypeIdentity,
        pub receiving_application: NormalizedTypeIdentity,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderApplicationAssociation {
        pub identity: ProviderApplicationIdentity,
        pub receiving_regions: Vec<LifetimeParameterIdentity>,
    }

    pub fn provider_application_associations(
        program: &dyn crate::CheckingInput,
    ) -> std::sync::Arc<Vec<ProviderApplicationAssociation>> {
        let derive = || std::sync::Arc::new(derive_provider_application_associations(program));
        match crate::frozen_program::frozen_program_memos(program) {
            Some(memos) => memos.provider_applications.get_or_init(derive).clone(),
            None => derive(),
        }
    }

    pub(crate) fn closed_requirement_provider_associations(
        program: &dyn crate::CheckingInput,
        requirement: SymbolHandle,
    ) -> Option<Vec<ProviderApplicationAssociation>> {
        let application = program.closed_requirement_application(requirement)?;
        let all = provider_application_associations(program);
        let rows = all
            .iter()
            .filter(|row| row.identity.closed_requirement == requirement)
            .cloned()
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return None;
        }
        for implementation in program.conformances() {
            let declares_row = implementation.rows.iter().any(|row| {
                row.declaring_trait == application.declaring_trait
                    && row.requirement == application.template
            });
            if !trait_reaches(
                program,
                implementation.trait_symbol,
                application.declaring_trait,
            )? {
                if declares_row {
                    return None;
                }
                continue;
            }
            if rows
                .iter()
                .any(|row| row.identity.selecting_declaration == implementation.symbol)
            {
                continue;
            }
            if !program
                .conformance_type_parameters(implementation)
                .is_empty()
                || !implementation.lifetime_parameters.is_empty()
            {
                return None;
            }
            let scope =
                AppliedTraitScope::from_conformance(program.typed_program(), implementation)?;
            let scope =
                scope.declaring_scope(program.typed_program(), application.declaring_trait)?;
            if scope.application_identity(program.typed_program())?
                == application.trait_application_identity
            {
                return None;
            }
        }
        for machine in program.machines() {
            if program
                .machine_specializations
                .iter()
                .any(|row| row.instance == machine.symbol)
                || program
                    .captured_machine_applications
                    .iter()
                    .any(|(_, row)| row.instance == machine.symbol)
                || machine.generic_data_template.is_valid()
            {
                continue;
            }
            for conformance in typed_trees::provider_declarations::independent_satisfactions(
                program.typed_program(),
                machine,
            ) {
                let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                    requirement: declared,
                    ..
                }) = typed_trees::machine::resolve_satisfied_declaration(
                    program,
                    machine,
                    conformance,
                )
                else {
                    continue;
                };
                if declared.symbol != application.template {
                    continue;
                }
                let mut covered = false;
                for row in &rows {
                    if row.identity.selecting_declaration == row.identity.provider
                        && program
                            .machine_invokes_declaration(row.identity.provider, machine.symbol)
                            .ok()?
                    {
                        covered = true;
                    }
                }
                if covered {
                    continue;
                }
                if machine.has_specialization_parameters()
                    || !machine.lifetime_parameters.is_empty()
                {
                    return None;
                }
                let scope = AppliedTraitScope::from_machine_conformance(
                    program.typed_program(),
                    machine,
                    conformance,
                )?;
                let scope =
                    scope.declaring_scope(program.typed_program(), application.declaring_trait)?;
                if scope.application_identity(program.typed_program())?
                    == application.trait_application_identity
                {
                    return None;
                }
            }
        }
        Some(rows)
    }

    fn trait_reaches(
        program: &dyn crate::CheckingInput,
        root: SymbolHandle,
        wanted: SymbolHandle,
    ) -> Option<bool> {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(symbol) = pending.pop() {
            if symbol == wanted {
                return Some(true);
            }
            if visited.contains(&symbol) {
                continue;
            }
            let declaration = program.trait_definition_by_symbol(symbol)?;
            visited.push(symbol);
            pending.extend(
                program
                    .trait_requirements(declaration)
                    .iter()
                    .chain(declaration.refines.iter())
                    .map(|parent| parent.symbol),
            );
        }
        Some(false)
    }

    fn derive_provider_application_associations(
        program: &dyn crate::CheckingInput,
    ) -> Vec<ProviderApplicationAssociation> {
        let mut rows = Vec::new();
        for provider in program.machines().iter().filter(|machine| {
            machine.supply_mode == MachineSupplyMode::CheckedBody
                && program.machine_type_parameters(machine).is_empty()
        }) {
            for conformance in typed_trees::provider_declarations::independent_satisfactions(
                program.typed_program(),
                provider,
            ) {
                if conformance.external_binding.is_some() {
                    continue;
                }
                if let Some(scope) = AppliedTraitScope::from_machine_conformance(
                    program.typed_program(),
                    provider,
                    conformance,
                ) {
                    for application in program.closed_requirement_applications() {
                        if conformance.requirement_symbol == application.template
                            && program.requirement_application_matches_scope(
                                application.signature.symbol,
                                &scope,
                            )
                            && program.requirement_realization_matches(
                                application.signature.symbol,
                                provider_template(program, provider.symbol),
                                provider.symbol,
                            )
                            && let Some(declaration_application) =
                                scope.portable_application_identity(program.typed_program())
                        {
                            rows.push((
                                ProviderApplicationIdentity {
                                    provider: provider.symbol,
                                    selecting_declaration: provider.symbol,
                                    template: application.template,
                                    closed_requirement: application.signature.symbol,
                                    declaration_application,
                                    receiving_application: application.trait_application_identity,
                                },
                                Vec::new(),
                            ));
                        }
                    }
                }
                if conformance.symbol != conformance.requirement_symbol {
                    continue;
                }
                let Some(template) =
                    program
                        .machines_with_symbol(conformance.symbol)
                        .find(|candidate| {
                            candidate.supply_mode == MachineSupplyMode::TopLevelRequirement
                                && !program.machine_type_parameters(candidate).is_empty()
                        })
                else {
                    continue;
                };
                if let Ok(closed) =
                    crate::resolve_closed_requirement_application(program, provider, template)
                {
                    rows.push((
                        ProviderApplicationIdentity {
                            provider: provider.symbol,
                            selecting_declaration: provider.symbol,
                            template: template.symbol,
                            closed_requirement: closed.symbol,
                            declaration_application: Default::default(),
                            receiving_application: Default::default(),
                        },
                        Vec::new(),
                    ));
                }
            }
            for application in program.closed_requirement_applications() {
                let Some(receiving_scope) = application.trait_scope.scope(program.typed_program())
                else {
                    continue;
                };
                for implementation in program.conformances().iter().filter(|implementation| {
                    implementation.rows.iter().any(|row| {
                        row.declaring_trait == application.declaring_trait
                            && row.requirement == application.template
                            && program.requirement_realization_matches(
                                application.signature.symbol,
                                row.realization_machine,
                                provider.symbol,
                            )
                    })
                }) {
                    let captures = implementation
                        .rows
                        .iter()
                        .filter(|row| {
                            row.requirement == application.template
                                && row.declaring_trait == application.declaring_trait
                        })
                        .filter_map(|row| {
                            program.typed_program().implementation_capture(
                                provider.symbol,
                                row.realization_machine,
                                implementation.symbol,
                            )
                        })
                        .collect::<Vec<_>>();
                    let correspondence = match captures.as_slice() {
                        [] => crate::provider_region_correspondence(
                            program,
                            implementation,
                            &receiving_scope,
                        ),
                        [capture] => crate::closed_provider_region_correspondence(
                            program,
                            implementation,
                            &receiving_scope,
                            capture,
                        ),
                        _ => None,
                    };
                    let Some(correspondence) = correspondence else {
                        continue;
                    };
                    rows.push((
                        ProviderApplicationIdentity {
                            provider: provider.symbol,
                            selecting_declaration: implementation.symbol,
                            template: application.template,
                            closed_requirement: application.signature.symbol,
                            declaration_application: correspondence.declaration_application,
                            receiving_application: application.trait_application_identity,
                        },
                        correspondence.receiving_regions,
                    ));
                }
            }
        }
        rows.sort_by_key(|(row, _)| {
            (
                row.provider.arena_index(),
                row.provider.generation(),
                row.selecting_declaration.arena_index(),
                row.selecting_declaration.generation(),
                row.template.arena_index(),
                row.template.generation(),
                row.closed_requirement.arena_index(),
                row.closed_requirement.generation(),
                row.declaration_application,
                row.receiving_application,
            )
        });
        rows.dedup();
        rows.into_iter()
            .map(
                |(identity, receiving_regions)| ProviderApplicationAssociation {
                    identity,
                    receiving_regions,
                },
            )
            .collect()
    }

    fn provider_template(
        program: &dyn crate::CheckingInput,
        provider: symbols::SymbolHandle,
    ) -> symbols::SymbolHandle {
        let mut receipts = program
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.instance == provider);
        let Some(receipt) = receipts.next() else {
            return provider;
        };
        if receipts.next().is_some() {
            return symbols::SymbolHandle::invalid();
        }
        receipt.template
    }
}
pub(crate) mod provider_regions {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use crate::queries::type_regions::{
        ReferenceStep, TypeRegionRelationship, TypeRegionScope, type_reference_region_applications,
    };
    use symbols::SymbolHandle;
    use typed_trees::trait_definition::{AppliedTraitScope, Conformance};
    use typed_trees::type_identity::{
        AppliedTypeIdentityScope, LifetimeParameterIdentity, NormalizedTypeIdentity,
        TypeIdentityRequest, applied_lifetime_parameter_identity,
    };
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProviderRegionCorrespondence {
        pub selecting_declaration: SymbolHandle,
        pub declaration_application: NormalizedTypeIdentity,
        pub receiving_regions: Vec<LifetimeParameterIdentity>,
    }

    pub fn provider_region_correspondence(
        program: &dyn CheckingInput,
        provider: &Conformance,
        receiver: &AppliedTraitScope<'_>,
    ) -> Option<ProviderRegionCorrespondence> {
        if !program.conformance_type_parameters(provider).is_empty() {
            return None;
        }
        let root = AppliedTraitScope::from_conformance(program.typed_program(), provider)?;
        provider_region_correspondence_in_scope(
            program,
            provider,
            receiver,
            root,
            vec![None; provider.lifetime_parameters.len()],
        )
    }

    pub fn closed_provider_region_correspondence(
        program: &dyn CheckingInput,
        provider: &Conformance,
        receiver: &AppliedTraitScope<'_>,
        capture: &typed_trees::typed_trees::CapturedMachineApplication,
    ) -> Option<ProviderRegionCorrespondence> {
        if capture.conformance_application.declaration != provider.symbol
            || capture.conformance_application.lifetime_arguments.len()
                != provider.lifetime_parameters.len()
        {
            return None;
        }
        let source =
            expression_table(program).static_machine_application(capture.supplying_application);
        let regions = capture
            .conformance_application
            .lifetime_arguments
            .iter()
            .map(|name| {
                typed_trees::type_identity::lifetime_parameter_identity(
                    program.typed_program(),
                    source.lexical_owner,
                    name,
                )
                .map(Some)
            })
            .collect::<Option<Vec<_>>>()?;
        let scope = capture
            .conformance_application
            .trait_scope
            .scope(program.typed_program())?;
        provider_region_correspondence_in_scope(program, provider, receiver, scope, regions)
    }

    fn provider_region_correspondence_in_scope(
        program: &dyn CheckingInput,
        provider: &Conformance,
        receiver: &AppliedTraitScope<'_>,
        root: AppliedTraitScope<'_>,
        initial_regions: Vec<Option<LifetimeParameterIdentity>>,
    ) -> Option<ProviderRegionCorrespondence> {
        let supplied =
            root.declaring_scope(program.typed_program(), receiver.declaration.symbol)?;
        let mut applications = vec![initial_regions];
        let matched = supplied.with_identity_scopes(program.typed_program(), |supplied_scopes| {
            receiver.with_identity_scopes(program.typed_program(), |receiving_scopes| {
                let supplied_environment = ApplicationRegions::new(&supplied, supplied_scopes);
                let receiving_environment = ApplicationRegions::new(receiver, receiving_scopes);
                for parameter in &receiver.declaration.lifetime_parameters {
                    let Some(actual) = supplied_environment.region(program, parameter.as_str())
                    else {
                        return false;
                    };
                    let Some(required) = receiving_environment.region(program, parameter.as_str())
                    else {
                        return false;
                    };
                    applications.retain_mut(|regions| {
                        constrain_slot(provider.symbol, regions, actual, required)
                    });
                    if applications.is_empty() {
                        return false;
                    }
                }
                let mut constrain =
                    |regions: &mut Vec<Option<LifetimeParameterIdentity>>,
                     actual: ApplicationRegions<'_>,
                     required: ApplicationRegions<'_>,
                     _reference,
                     actual_name: Option<&typed_trees::name::Identifier>,
                     required_name: Option<&typed_trees::name::Identifier>| {
                        match (actual_name, required_name) {
                            (None, None) => true,
                            (Some(actual_name), Some(required_name)) => actual
                                .region(program, actual_name.as_str())
                                .zip(required.region(program, required_name.as_str()))
                                .is_some_and(|(actual, required)| {
                                    constrain_slot(provider.symbol, regions, actual, required)
                                }),
                            _ => false,
                        }
                    };
                for parameter in program.trait_type_parameters(receiver.declaration) {
                    if supplied.parameter_identity(
                        program.typed_program(),
                        parameter.symbol,
                        typed_trees::type_identity::TypeIdentityQualification::Ordinary,
                    ) != receiver.parameter_identity(
                        program.typed_program(),
                        parameter.symbol,
                        typed_trees::type_identity::TypeIdentityQualification::Ordinary,
                    ) {
                        return false;
                    }
                    let Some((actual, actual_scopes)) =
                        supplied.parameter_application(parameter.symbol)
                    else {
                        return false;
                    };
                    let Some((required, required_scopes)) =
                        receiver.parameter_application(parameter.symbol)
                    else {
                        return false;
                    };
                    if !type_reference_region_applications(
                        program,
                        actual,
                        required,
                        supplied_environment.prefix(actual_scopes),
                        receiving_environment.prefix(required_scopes),
                        TypeRegionRelationship::ExactApplication,
                        &mut applications,
                        &mut constrain,
                    ) {
                        return false;
                    }
                }
                true
            })
        });
        matched.then_some(())?;
        let mut complete = Vec::new();
        for regions in applications {
            if let Some(regions) = regions.into_iter().collect::<Option<Vec<_>>>()
                && !complete.contains(&regions)
            {
                complete.push(regions);
            }
        }
        let [regions] = complete.as_slice() else {
            return None;
        };
        Some(ProviderRegionCorrespondence {
            selecting_declaration: provider.symbol,
            declaration_application: supplied
                .portable_application_identity(program.typed_program())?,
            receiving_regions: regions.clone(),
        })
    }

    fn constrain_slot(
        selecting_declaration: SymbolHandle,
        receiving: &mut [Option<LifetimeParameterIdentity>],
        supplied: LifetimeParameterIdentity,
        required: LifetimeParameterIdentity,
    ) -> bool {
        if supplied.declaration != selecting_declaration {
            return supplied == required;
        }
        let Some(slot) = receiving.get_mut(supplied.ordinal) else {
            return false;
        };
        match slot {
            Some(previous) => *previous == required,
            None => {
                *slot = Some(required);
                true
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) struct ApplicationRegions<'scope> {
        application: &'scope AppliedTraitScope<'scope>,
        identities: &'scope [AppliedTypeIdentityScope<'scope>],
        owner: SymbolHandle,
        visible: usize,
    }

    impl<'scope> ApplicationRegions<'scope> {
        fn new(
            application: &'scope AppliedTraitScope<'scope>,
            identities: &'scope [AppliedTypeIdentityScope<'scope>],
        ) -> Self {
            Self {
                application,
                identities,
                owner: application.argument_owner,
                visible: application.substitutions.len(),
            }
        }

        fn prefix(mut self, count: usize) -> Self {
            self.visible = self
                .application
                .substitution_scope_starts
                .get(count)
                .copied()
                .unwrap_or(self.application.substitutions.len());
            self.identities = &self.identities[..count];
            self
        }

        fn region(
            self,
            program: &dyn CheckingInput,
            name: &str,
        ) -> Option<LifetimeParameterIdentity> {
            applied_lifetime_parameter_identity(
                program.typed_program(),
                self.owner,
                name,
                self.identities,
            )
        }
    }

    impl TypeRegionScope for ApplicationRegions<'_> {
        fn erased_identity(
            self,
            program: &dyn CheckingInput,
            reference: TypeReferenceHandle,
        ) -> NormalizedTypeIdentity {
            program.scoped_type_identity(
                TypeIdentityRequest {
                    substitutions: &self.application.substitutions[..self.visible],
                    ..TypeIdentityRequest::ordinary(reference)
                },
                &self.application.substitution_scope_starts[..self.identities.len()],
            )
        }

        fn enter_operand(mut self, program: &dyn CheckingInput, owner: SymbolHandle) -> Self {
            let count = self
                .identities
                .iter()
                .rposition(|identity| {
                    program.lifetime_declaration_owner(identity.declaration)
                        == program.lifetime_declaration_owner(owner)
                })
                .map_or(0, |ordinal| ordinal + 1);
            self = self.prefix(count);
            self.owner = owner;
            self
        }

        fn named_alias(
            mut self,
            _program: &dyn CheckingInput,
            symbol: SymbolHandle,
            reference: TypeReferenceHandle,
        ) -> Option<ReferenceStep<Self>> {
            let substitutions = &self.application.substitutions;
            let visible = self.visible;
            let (ordinal, argument) = substitutions[..visible]
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (parameter, _))| *parameter == symbol)
                .map(|(ordinal, (_, argument))| (ordinal, *argument))?;
            if argument == reference {
                return Some(ReferenceStep::Stop);
            }
            let Some(count) = self
                .application
                .substitution_scope_starts
                .partition_point(|start| *start <= ordinal)
                .checked_sub(1)
            else {
                return Some(ReferenceStep::Refuse);
            };
            self = self.prefix(count);
            Some(ReferenceStep::Chase(self, argument))
        }
    }
}
pub(crate) use provider_applications::closed_requirement_provider_associations;
pub use provider_applications::{
    ProviderApplicationAssociation, ProviderApplicationIdentity, provider_application_associations,
};
pub mod refinement_fit {
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::trait_definition::AppliedTraitScope;
    use typed_trees::trait_definition::{
        Conformance, TraitDefinition, TraitRefinementClause, TraitRefinementReach,
    };

    pub struct ResolvedBoundCarrier<'program> {
        pub base: SymbolHandle,
        pub refinement: Option<&'program TraitDefinition>,
    }

    pub fn resolve_bound_carrier<'program>(
        program: &'program dyn crate::CheckingInput,
        carrier: SymbolHandle,
    ) -> ResolvedBoundCarrier<'program> {
        let Some(definition) = program.trait_definition_by_symbol(carrier) else {
            return ResolvedBoundCarrier {
                base: carrier,
                refinement: None,
            };
        };
        match &definition.refines {
            Some(base) => ResolvedBoundCarrier {
                base: base.symbol,
                refinement: Some(definition),
            },
            None => ResolvedBoundCarrier {
                base: carrier,
                refinement: None,
            },
        }
    }

    fn covering_clauses<'program>(
        refinement: &'program TraitDefinition,
        requirement: &str,
    ) -> Vec<&'program TraitRefinementClause> {
        refinement
            .refinement_clauses
            .iter()
            .filter(|clause| match clause.requirement.as_ref() {
                None => true,
                Some(name) => name.as_str() == requirement,
            })
            .collect()
    }

    pub fn refinement_fit_diagnostics(
        program: &dyn crate::CheckingInput,
        refinement: &TraitDefinition,
        application: Option<&AppliedTraitScope<'_>>,
        conformance: &Conformance,
        template_name: &str,
        binder_name: &str,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let rows = conformance.rows.as_slice();
        let selected_name = conformance
            .alias
            .as_ref()
            .map_or("<unnamed>", |name| name.as_str());
        for row in rows {
            let clauses = covering_clauses(refinement, row.requirement_name.as_str());
            if clauses.is_empty() {
                continue;
            }
            let reject = |axis: String| {
                Diagnostic::error(format!(
                    "generic machine `{template_name}` cannot bind `{binder_name}` to conformance `{selected_name}`: refinement `{}` {axis}",
                    refinement.name.as_str(),
                ))
            };
            let requirement = row.requirement_name.as_str();
            let realization = row.realization_name.as_str();
            let Some(machine) = program.machine(row.realization_machine) else {
                diagnostics.push(reject(format!(
                    "lost the selected realization `{realization}` for `{requirement}`"
                )));
                continue;
            };
            if clauses.iter().any(|clause| {
                program
                    .state_signature_contracts(&clause.signature)
                    .iter()
                    .any(|contract| contract.kind == SignatureContractKind::Ensures)
            }) {
                let fit = (|| {
                    let application =
                        application.ok_or("has no exact applied refinement context")?;
                    let (declaring, _) = program
                        .trait_machine_signature(row.requirement)
                        .ok_or("lost the selected public requirement")?;
                    let declaring = application
                        .declaring_scope(program.typed_program(), declaring.symbol)
                        .ok_or("has no unique applied requirement scope")?;
                    let required = program
                        .applied_callable_signature(&declaring, row.requirement)
                        .ok_or(
                            "has no prepared public contract for its exact refinement application",
                        )?;
                    let entry = program
                        .state_in_machine(machine, row.realization_state)
                        .ok_or("lost the selected realization's entry")?;
                    let actual_generics = program.requirement_callable_parameters(
                        required,
                        &program.machine_callable_parameters(machine),
                    );
                    let required_generics = program.state_signature_type_parameters(required);
                    if required_generics.len() != actual_generics.len() {
                        return Err("has a different callable generic telescope");
                    }
                    let binders = required_generics
                        .iter()
                        .zip(&actual_generics)
                        .map(|(required, actual)| (required.symbol, actual.symbol))
                        .collect::<Vec<_>>();
                    let actual_contracts = program
                        .machine_contracts(machine)
                        .iter()
                        .chain(program.state_contracts(entry))
                        .cloned()
                        .collect::<Vec<_>>();
                    // A selected map must publish the promised facts itself. Its
                    // body and the required guarantees are not proof premises.
                    let implication = crate::proof_contracts::contract_entailment::CallableContractRefinement::prepare(
                        program,
                        crate::proof_contracts::contract_entailment::CallableContract {
                            owner: required.symbol,
                            contracts: program.state_signature_contracts(required),
                            parameters: program.state_signature_parameters(required),
                            result: required.return_type,
                        },
                        crate::proof_contracts::contract_entailment::CallableContract {
                            owner: machine.symbol,
                            contracts: &actual_contracts,
                            parameters: program.state_parameters(entry),
                            result: entry.return_type,
                        },
                        &binders,
                    )
                    .ok_or("has no complete parameter/result correspondence")?;
                    if !implication.proves(program, &SignatureContractKind::Requires) {
                        return Err("does not preserve the public preconditions");
                    }
                    if !implication.proves(program, &SignatureContractKind::Ensures) {
                        return Err("does not prove the strengthened public guarantees");
                    }
                    Ok(())
                })();
                if let Err(reason) = fit {
                    diagnostics.push(reject(format!(
                        "requires a stronger contract on `{requirement}`, but `{realization}` {reason}"
                    )));
                }
            }
            for clause in clauses {
                if !clause.signature.suspends_keyword_source_spans.is_empty()
                    && !clause.signature.suspends
                    && machine.suspends
                {
                    diagnostics.push(reject(format!(
                "removes `suspends` from `{requirement}`, but its selected realization `{realization}` suspends",
            )));
                }
                if !clause.signature.blocks_keyword_source_spans.is_empty()
                    && !clause.signature.blocks
                    && machine.blocks
                {
                    diagnostics.push(reject(format!(
                "removes `blocks` from `{requirement}`, but its selected realization `{realization}` blocks",
            )));
                }

                if matches!(
                    clause.signature.termination_guarantee,
                    language_semantics::TerminationGuarantee::Terminates { .. }
                ) && !matches!(
                    machine.termination_plan.interface.published(),
                    Some(language_semantics::TerminationGuarantee::Terminates { .. })
                ) {
                    diagnostics.push(reject(format!(
                "requires `terminates` on `{requirement}`, but its selected realization `{realization}` publishes no termination guarantee",
            )));
                }

                if let TraitRefinementReach::Concrete(clause_row) = clause.service_reach {
                    let permitted = program.service_reach_rows.services(clause_row);
                    let reached = program
                        .service_reach_rows
                        .services(machine.service_reach_row);
                    if let Some(escaping) = reached
                        .iter()
                        .find(|service| !permitted.contains(service))
                        .copied()
                    {
                        let escaping = program
                            .service_reaches
                            .definition(escaping)
                            .map_or("<service>", |definition| definition.name.as_str());
                        diagnostics.push(reject(format!(
                    "narrows `reaches` on `{requirement}`, but its selected realization `{realization}` reaches `{escaping}`",
                )));
                    }
                }
            }
        }
        diagnostics.dedup_by(|left, right| left.to_string() == right.to_string());
        diagnostics
    }
}
pub(crate) mod requirements {
    use super::trait_definition_by_symbol;
    use crate::declarations::symbols::TopLevelSymbols;
    use crate::value_custody::type_references::TypeParameterScope;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::operator::trait_operator_operand_signature;
    use typed_trees::trait_definition::TraitDefinition;

    pub(crate) fn validate_trait_requirements(
        program: &dyn crate::CheckingInput,
        symbols: &TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        for trait_definition in program.traits() {
            validate_trait_operator_bindings(program, trait_definition, diagnostics);
            let mut seen = Vec::new();
            for requirement in program.trait_requirements(trait_definition) {
                let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                        "trait `{}` requires unknown trait `{}`",
                        trait_definition.name, requirement.name
                    )));
                    continue;
                };

                if seen.contains(&requirement.symbol) {
                    diagnostics.push(Diagnostic::error(format!(
                        "trait `{}` names parent `{}` more than once",
                        trait_definition.name, requirement.name
                    )));
                } else {
                    seen.push(requirement.symbol);
                }

                let expected_lifetimes = required_trait.lifetime_parameters.len();
                let actual_lifetimes = requirement.lifetime_arguments.len();
                if actual_lifetimes != 0 && expected_lifetimes != actual_lifetimes {
                    diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` parent `{}` expects {expected_lifetimes} lifetime argument(s), got {actual_lifetimes}",
                    trait_definition.name, requirement.name
                )));
                }
                for lifetime in &requirement.lifetime_arguments {
                    if !trait_definition
                        .lifetime_parameters
                        .iter()
                        .any(|declared| declared == lifetime)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "trait `{}` parent `{}` uses undeclared lifetime argument `'{}'",
                            trait_definition.name,
                            requirement.name,
                            lifetime.as_str()
                        )));
                    }
                }

                let expected = program.trait_type_parameters(required_trait).len();
                let actual = requirement.arguments.len();
                if expected != actual {
                    diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` parent `{}` expects {expected} generic argument(s), got {actual}",
                    trait_definition.name, requirement.name
                )));
                } else {
                    super::conformance::validate_trait_application_obligations(
                        program,
                        required_trait,
                        program
                            .type_reference_table
                            .type_reference_handles(requirement.arguments),
                        &requirement.lifetime_arguments,
                        &format!(
                            "trait `{}` parent `{}`",
                            trait_definition.name, requirement.name
                        ),
                        symbols,
                        TypeParameterScope::authored(
                            trait_definition.symbol,
                            program.trait_type_parameters(trait_definition),
                            &trait_definition.lifetime_parameters,
                        ),
                        diagnostics,
                        formation_obligations,
                    );
                }

                if !trait_definition.is_boundary && required_trait.is_boundary {
                    diagnostics.push(Diagnostic::error(format!(
                    "ordinary trait `{}` cannot inherit boundary service `{}`; declare the child as `boundary trait`",
                    trait_definition.name, requirement.name
                )));
                }
            }
        }

        let mut reported_cycle_symbols = Vec::new();
        for trait_definition in program.traits() {
            let mut path = Vec::new();
            validate_trait_requirement_cycles(
                program,
                trait_definition,
                &mut path,
                &mut reported_cycle_symbols,
                diagnostics,
            );
        }
    }

    fn validate_trait_operator_bindings(
        program: &dyn crate::CheckingInput,
        trait_definition: &TraitDefinition,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut seen = Vec::new();
        for requirement in program.trait_machine_signatures(trait_definition) {
            let Some(spelling) = requirement.spelling else {
                continue;
            };
            let operands =
                match trait_operator_operand_signature(program, trait_definition, requirement) {
                    Ok(operands) => operands,
                    Err(error) => {
                        diagnostics.push(Diagnostic::error(format!(
                            "trait `{}` operator `{}` has no resolved operand identity: {error}",
                            trait_definition.name,
                            spelling.symbol(),
                        )));
                        continue;
                    }
                };
            if seen.iter().any(|(prior_spelling, prior_operands)| {
                *prior_spelling == spelling && prior_operands == &operands
            }) {
                diagnostics.push(Diagnostic::error(format!(
                "trait `{}` binds operator token `{}` more than once for normalized operands `({operands})`; one trait requirement owns each token meaning",
                trait_definition.name,
                spelling.symbol(),
            )));
            } else {
                seen.push((spelling, operands));
            }
        }
    }

    fn validate_trait_requirement_cycles(
        program: &dyn crate::CheckingInput,
        trait_definition: &TraitDefinition,
        path: &mut Vec<SymbolHandle>,
        reported_cycle_symbols: &mut Vec<SymbolHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if reported_cycle_symbols.contains(&trait_definition.symbol) {
            return;
        }

        if let Some(cycle_start) = path
            .iter()
            .position(|symbol| *symbol == trait_definition.symbol)
        {
            let cycle_symbols = path[cycle_start..]
                .iter()
                .copied()
                .chain(std::iter::once(trait_definition.symbol))
                .collect::<Vec<_>>();
            let mut cycle = path[cycle_start..]
                .iter()
                .filter_map(|symbol| trait_definition_by_symbol(program, *symbol))
                .map(|trait_definition| trait_definition.name.to_string())
                .collect::<Vec<_>>();
            cycle.push(trait_definition.name.to_string());

            diagnostics.push(Diagnostic::error(format!(
                "trait requirement cycle detected: {}",
                cycle.join(" -> ")
            )));
            reported_cycle_symbols.extend(cycle_symbols);
            return;
        }

        path.push(trait_definition.symbol);
        for requirement in program.trait_requirements(trait_definition) {
            let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol)
            else {
                continue;
            };

            validate_trait_requirement_cycles(
                program,
                required_trait,
                path,
                reported_cycle_symbols,
                diagnostics,
            );
        }
        path.pop();
    }
}
pub(crate) mod trait_implementation_arguments {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::data::TypeParameterKind;
    use typed_trees::machine::GenericConformanceBound;
    use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
    use typed_trees::types::TypeReferenceHandle;

    use super::conformance_applications::{
        applied_bound_argument_identities, close_conformance_application,
        validate_conformance_lifetime_arguments,
    };

    pub(crate) fn validate_trait_implementation_arguments(
        program: &dyn CheckingInput,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        if let Err(reason) = validate_retained_implementation_arguments(program) {
            diagnostics.push(Diagnostic::error(reason));
        }
        let mut validate = |symbol,
                            arguments: &[TypeReferenceHandle],
                            lifetime_arguments: &[typed_trees::name::Identifier],
                            owner: SymbolHandle,
                            label: String| {
            let Some(definition) = program.trait_definition_by_symbol(symbol) else {
                return;
            };
            validate_trait_implementation_application(
                program,
                definition,
                arguments,
                lifetime_arguments,
                owner,
                &label,
                &mut diagnostics,
            );
        };
        for definition in program.traits() {
            for relationship in program.trait_requirements(definition) {
                validate(
                    relationship.symbol,
                    program
                        .type_reference_table
                        .type_reference_handles(relationship.arguments),
                    &relationship.lifetime_arguments,
                    definition.symbol,
                    format!("trait `{}` parent `{}`", definition.name, relationship.name),
                );
            }
            if let Some(base) = &definition.refines {
                validate(
                    base.symbol,
                    program
                        .type_reference_table
                        .type_reference_handles(base.arguments),
                    &base.lifetime_arguments,
                    definition.symbol,
                    format!("trait refinement `{}`", definition.name),
                );
            }
            for bound in &definition.conformance_bounds {
                validate(
                    bound.trait_symbol,
                    &bound.arguments,
                    &bound.lifetime_arguments,
                    definition.symbol,
                    format!(
                        "trait `{}` evidence binder `{}`",
                        definition.name, bound.binder_name
                    ),
                );
            }
        }
        for implementation in program.conformances() {
            let lifetimes = implementation
                .trait_lifetime_arguments
                .iter()
                .filter_map(|ordinal| {
                    implementation
                        .lifetime_parameters
                        .get(*ordinal as usize)
                        .cloned()
                })
                .collect::<Vec<_>>();
            validate(
                implementation.trait_symbol,
                program
                    .type_reference_table
                    .type_reference_handles(implementation.arguments),
                &lifetimes,
                implementation.symbol,
                format!(
                    "implementation `{}`",
                    program.symbols.display_path(implementation.symbol, "::")
                ),
            );
        }
        for machine in program.machines() {
            for edge in program.machine_trait_conformances(machine) {
                let owner = match super::conformance::authored_conformance_machine(program, machine)
                {
                    Ok(owner) => owner,
                    Err(diagnostic) => return Err(vec![diagnostic]),
                };
                let lifetimes = edge
                    .trait_lifetime_arguments
                    .iter()
                    .filter_map(|ordinal| owner.lifetime_parameters.get(*ordinal as usize).cloned())
                    .collect::<Vec<_>>();
                validate(
                    edge.symbol,
                    program
                        .type_reference_table
                        .type_reference_handles(edge.arguments),
                    &lifetimes,
                    owner.symbol,
                    format!("machine `{}` satisfaction edge", machine.name),
                );
            }
            for bound in &machine.conformance_bounds {
                validate(
                    bound.trait_symbol,
                    &bound.arguments,
                    &bound.lifetime_arguments,
                    machine.symbol,
                    format!(
                        "machine `{}` evidence binder `{}`",
                        machine.name, bound.binder_name
                    ),
                );
            }
        }
        crate::program_validation::finish_strict_diagnostics(diagnostics)
    }

    pub(crate) fn validate_trait_implementation_application(
        program: &dyn CheckingInput,
        definition: &typed_trees::trait_definition::TraitDefinition,
        arguments: &[TypeReferenceHandle],
        lifetime_arguments: &[typed_trees::name::Identifier],
        owner: SymbolHandle,
        label: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        validate_implementation_application(
            program,
            definition.symbol,
            program.trait_type_parameters(definition),
            &definition.lifetime_parameters,
            &definition.conformance_bounds,
            arguments,
            lifetime_arguments,
            owner,
            label,
            diagnostics,
        );
    }

    pub(crate) fn validate_implementation_application(
        program: &dyn CheckingInput,
        declaration: SymbolHandle,
        parameters: &[typed_trees::data::TypeParameter],
        declared_lifetimes: &[typed_trees::name::Identifier],
        bounds: &[GenericConformanceBound],
        arguments: &[TypeReferenceHandle],
        lifetime_arguments: &[typed_trees::name::Identifier],
        owner: SymbolHandle,
        label: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if parameters.len() != arguments.len() {
            return;
        }
        let lifetimes = declared_lifetimes
            .iter()
            .zip(lifetime_arguments)
            .map(|(parameter, argument)| {
                (parameter.as_str().to_owned(), argument.as_str().to_owned())
            })
            .collect::<Vec<_>>();
        let substitutions = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| {
                (
                    parameter.symbol,
                    if parameter.kind == TypeParameterKind::Type {
                        IndexIdentityArgument::TypeArgument(*argument)
                    } else {
                        IndexIdentityArgument::TypeReference(*argument)
                    },
                )
            })
            .collect::<Vec<_>>();
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if parameter.kind != TypeParameterKind::Implementation {
                continue;
            }
            let Some(bound) = bounds.iter().find(|bound| bound.binder == parameter.symbol) else {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} implementation slot `{}` has no declared evidence contract",
                    parameter.name
                )));
                continue;
            };
            if let Err(reason) = validate_binding(
                program,
                bound,
                *argument,
                declaration,
                owner,
                arguments,
                lifetime_arguments,
                &[AppliedTypeIdentityScope {
                    declaration,
                    arguments: &substitutions,
                    lifetimes: &lifetimes,
                }],
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "{label} cannot bind implementation `{}`: {reason}",
                    parameter.name
                )));
            }
        }
    }

    fn validate_binding(
        program: &dyn CheckingInput,
        bound: &GenericConformanceBound,
        argument: TypeReferenceHandle,
        receiving_owner: SymbolHandle,
        owner: SymbolHandle,
        receiving_arguments: &[TypeReferenceHandle],
        receiving_lifetimes: &[typed_trees::name::Identifier],
        scopes: &[AppliedTypeIdentityScope<'_>],
    ) -> Result<(), String> {
        let selected = program
            .static_operand_from_type_reference(argument)
            .map_err(str::to_owned)?;
        if !selected.symbol.is_valid() {
            return Err(
                "expected an explicitly named implementation, not a data argument".to_owned(),
            );
        }
        let carrier = super::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
        let expected_arguments = applied_bound_argument_identities(
            program,
            bound,
            receiving_owner,
            owner,
            scopes,
        )
        .ok_or_else(|| {
            format!(
                "implementation slot has incomplete applied argument identity in `{}` for `{}`",
                program.symbols.display_path(owner, "::"),
                program.display_type_reference(argument)
            )
        })?;
        if program.symbols.get(selected.symbol).kind == SymbolKind::ConformanceParameter {
            return super::conformance_applications::validate_forwarded_argument(
                program,
                bound,
                &selected,
                |actual| {
                    applied_bound_argument_identities(
                        program,
                        actual,
                        program.symbols.get(actual.binder).parent,
                        owner,
                        &[],
                    )
                    .is_some_and(|actual| actual == expected_arguments)
                },
            );
        }
        let application = close_conformance_application(program, &selected)
            .map_err(|diagnostic| diagnostic.message)?;
        validate_conformance_lifetime_arguments(program, &selected, owner)
            .map_err(|diagnostic| diagnostic.message)?;
        let implementation = program
            .conformance_by_symbol(application.declaration)
            .ok_or_else(|| "closed implementation has no declaration".to_owned())?;
        let actuals = program
            .conformance_type_parameters(implementation)
            .iter()
            .zip(&application.arguments)
            .map(|(parameter, argument)| {
                (
                    parameter.symbol,
                    IndexIdentityArgument::StaticArgument(argument),
                )
            })
            .collect::<Vec<_>>();
        let lifetimes = implementation
            .lifetime_parameters
            .iter()
            .zip(&application.lifetime_arguments)
            .map(|(parameter, argument)| (parameter.as_str().to_owned(), argument.clone()))
            .collect::<Vec<_>>();
        let actual_arguments = typed_trees::trait_definition::AppliedTraitScope::from_conformance(
            program.typed_program(),
            implementation,
        )
        .and_then(|scope| {
            super::conformance_applications::applied_scope_argument_identities(
                program,
                &scope,
                owner,
                &[AppliedTypeIdentityScope {
                    declaration: implementation.symbol,
                    arguments: &actuals,
                    lifetimes: &lifetimes,
                }],
            )
        });
        if application.trait_definition != carrier.base
            || !actual_arguments.is_some_and(|actual| actual == expected_arguments)
        {
            return Err(format!(
                "`{}` must select a complete `{}` implementation with the instantiated trait arguments",
                selected.display_name(),
                bound.trait_name
            ));
        }
        if let Some(refinement) = carrier.refinement {
            let refinement_application = program
                .trait_definition_by_symbol(receiving_owner)
                .and_then(|receiving| {
                    typed_trees::trait_definition::AppliedTraitScope::new(
                        program.typed_program(),
                        receiving,
                        typed_trees::trait_definition::TraitApplicationArguments {
                            lexical_owner: owner,
                            enclosing_data_applications: &[],
                            arguments: receiving_arguments,
                            lifetimes: receiving_lifetimes,
                        },
                    )
                })
                .and_then(|scope| scope.bound_scope(program.typed_program(), bound.binder));
            let diagnostics = super::refinement_fit::refinement_fit_diagnostics(
                program,
                refinement,
                refinement_application.as_ref(),
                implementation,
                "trait application",
                bound.binder_name.as_str(),
            );
            if let Some(diagnostic) = diagnostics.first() {
                return Err(diagnostic.message.clone());
            }
        }
        Ok(())
    }

    fn validate_retained_implementation_arguments(
        program: &dyn CheckingInput,
    ) -> Result<(), String> {
        use typed_trees::static_applications::StaticMachineBindingValue;
        let typed = program.typed_program();
        if !typed.data_implementation_applications.is_empty() {
            let mut owners = typed
                .data_implementation_applications
                .iter()
                .map(|(_, selected)| selected.lexical_owner)
                .collect::<Vec<_>>();
            owners.sort_unstable_by_key(|owner| (owner.arena_index(), owner.generation()));
            owners.dedup();
            let live = typed_trees::references::authored_type_occurrences(typed, &owners)
                .into_iter()
                .map(|occurrence| (occurrence.lexical_owner, occurrence.reference))
                .collect::<std::collections::HashSet<_, symbols::BuildSymbolHasher>>();
            for (_, selected) in typed.data_implementation_applications.iter() {
                if !live.contains(&(selected.lexical_owner, selected.source_application)) {
                    return Err(
                        "data implementation demand lost its live authored constructor occurrence"
                            .into(),
                    );
                }
                let storage = typed
                    .layout_storage_arguments(selected.source_application)
                    .ok_or("data implementation demand lost its exact fixed constructor")?;
                if storage.policy != selected.argument {
                    return Err("data implementation demand changed its actual code operand".into());
                }
                let operand = typed
                    .static_operand_from_type_reference(storage.policy)
                    .map_err(str::to_owned)?;
                let reclosed = close_conformance_application(program, &operand)
                    .map_err(|error| error.message)?;
                let source = expression_table(typed)
                    .static_machine_application(selected.supplying_application);
                let bindings = expression_table(typed)
                    .static_machine_application_bindings(selected.supplying_application);
                if reclosed != selected.application
                    || source.lexical_owner != selected.lexical_owner
                    || bindings
                        != [typed_trees::static_applications::StaticMachineBinding {
                            parameter: selected.parameter,
                            value: StaticMachineBindingValue::Conformance(operand),
                        }]
                {
                    return Err(
                        "data implementation demand changed its complete supplied map application"
                            .into(),
                    );
                }
            }
        }
        if typed.trait_implementation_applications.is_empty() {
            return Ok(());
        }
        let roots = typed.applied_trait_roots();
        for (_, selected) in typed.trait_implementation_applications.iter() {
            let scope = selected
                .source_scope
                .scope(typed)
                .ok_or("trait implementation demand lost its original source scope")?;
            let mut live = false;
            for root in &roots {
                root.visit(typed, &mut |current| {
                    live |= current.argument_owner == scope.argument_owner
                        && current.same_application(typed, &scope);
                });
            }
            if !live {
                return Err(
                    "trait implementation demand has no exact live source application".to_owned(),
                );
            }
            let parameter = typed
                .trait_type_parameters(scope.declaration)
                .iter()
                .find(|parameter| {
                    parameter.symbol == selected.parameter
                        && parameter.kind == TypeParameterKind::Implementation
                })
                .ok_or("trait implementation demand lost its exact implementation slot")?;
            let expected = scope
                .portable_parameter_identity(typed, parameter.symbol)
                .ok_or("trait implementation demand has an unresolved source argument")?;
            let actual = typed_trees::type_identity::portable_applied_type_identity(
                typed,
                selected.argument,
                scope.argument_owner,
                &[],
            )
            .ok_or("trait implementation demand has an unresolved materialized argument")?;
            if actual != expected {
                return Err(
                    "trait implementation demand changed its original source argument".to_owned(),
                );
            }
            let operand = typed
                .static_operand_from_type_reference(selected.argument)
                .map_err(str::to_owned)?;
            let source =
                expression_table(typed).static_machine_application(selected.supplying_application);
            let arguments = expression_table(typed)
                .static_machine_application_arguments(selected.supplying_application);
            let bindings = expression_table(typed)
                .static_machine_application_bindings(selected.supplying_application);
            if source.lexical_owner != scope.argument_owner
                || source.registration_operation != scope.declaration.symbol
                || source.binding_declaration != scope.declaration.symbol
                || arguments != [operand.clone()]
                || bindings.len() != 1
                || bindings[0].parameter != selected.parameter
                || bindings[0].value != StaticMachineBindingValue::Conformance(operand.clone())
                || source.conformance_selection.is_some()
            {
                return Err(
                    "trait implementation demand changed its supplying source application"
                        .to_owned(),
                );
            }
            let closed = close_conformance_application(program, &operand)
                .map_err(|diagnostic| diagnostic.message)?;
            if closed != selected.application {
                return Err(
                    "trait implementation demand changed its exact named map application"
                        .to_owned(),
                );
            }
            validate_conformance_lifetime_arguments(program, &operand, source.lexical_owner)
                .map_err(|diagnostic| diagnostic.message)?;
        }
        Ok(())
    }
}
pub(crate) use trait_implementation_arguments::validate_implementation_application;

pub use conformance::GenericBoundRequirement;
pub use conformance::compose_forwarded_trait_arguments;
pub use conformance::generic_bound_operator_requirement;
pub use conformance::resolve_closed_requirement_application;
pub use conformance::revalidate_machine_trait_conformances;
pub use conformance::revalidate_top_level_requirement_realization;
pub(crate) use conformance::{
    named_conformance_argument_matches, validate_conformance_bounds,
    validate_external_leaf_native_shapes, validate_external_via_expression,
    validate_generic_conformance_bounds, validate_machine_trait_conformances,
    validate_trait_application_obligations, validate_trait_conformance_bounds,
};
pub use dynamic::{
    DynamicImplementationSelection, DynamicSlotDemand, collect_dynamic_implementation_selections,
    collect_dynamic_slot_demands, describe_dynamic_conformances, dynamic_call_demand,
    dynamic_expression_call_demand, normalized_dynamic_row_identities, qualified_trait_scope,
    resolve_dynamic_call_targets, resolve_selected_dynamic_call_targets,
};
pub(crate) use dynamic::{
    dynamic_requirement_ineligibility, dynamic_selector_application,
    report_dynamic_receiver_call_error,
};
pub use named_implementations::revalidate_named_implementation;
pub(crate) use named_implementations::{validate_conformances, validate_member_formations};
pub use provider_regions::{
    ProviderRegionCorrespondence, closed_provider_region_correspondence,
    provider_region_correspondence,
};
pub(crate) use requirements::validate_trait_requirements;

fn trait_definition_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&TraitDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program.trait_definition_by_symbol(symbol)
}
