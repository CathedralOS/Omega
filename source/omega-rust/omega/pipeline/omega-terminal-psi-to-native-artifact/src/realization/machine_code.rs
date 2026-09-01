use crate::realization::diagnostics::{
    realization_error, selected_physical_pipeline_failed,
    selected_physical_pipeline_not_publishable,
};
use crate::realization::model::{NativeRealizationInput, NativeRealizationRequest};
use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_machine_code::{CompilerPrivateMachineCodeFunction, MachineCodePlanWithPrivateFunctions};
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use psi_diagnostics::Diagnostic;

pub(crate) fn emit_realization_machine_code(
    input: NativeRealizationInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    request: &NativeRealizationRequest<'_>,
) -> Result<MachineCodePlanWithPrivateFunctions, Vec<Diagnostic>> {
    match input {
        NativeRealizationInput::Unoptimized(
            omega_psi_to_abstract_operations::NativeArtifactOperationPlan::Ordinary(plan),
        ) => {
            let target = match provider_installation {
                Some(installation) => {
                    omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions_installation_ieee_float_fma_and_native_callbacks(
                        &plan,
                        request.target,
                        settlements,
                        Some(&installation),
                        request.ieee_float_fma,
                        request.native_callbacks,
                    )
                }
                None => omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions_installation_ieee_float_fma_and_native_callbacks(
                    &plan,
                    request.target,
                    settlements,
                    None,
                    request.ieee_float_fma,
                    request.native_callbacks,
                ),
            }
            .map_err(|error| realization_error("ordinary target lowering", error))?;
            let assigned = omega_target_operations_to_assigned_target_operations::assign_registers_with_native_callbacks(&target)
                .map_err(|error| realization_error("ordinary physical assignment", error))?;
            let private_functions =
                emit_callback_thunks(request.callback_thunks, request.target, request.profile)?;
            validate_callback_thunk_assignments(
                request.callback_thunks,
                &assigned.native_callback_arguments,
            )?;
            let plan = omega_machine_emission::emit_machine_code_with_native_callbacks(&assigned)
                .map_err(|error| realization_error("machine-code emission", error))?;
            Ok(MachineCodePlanWithPrivateFunctions {
                plan,
                private_functions,
            })
        }
        NativeRealizationInput::Unoptimized(
            omega_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(
                ranked,
            ),
        ) => {
            if provider_installation.is_some()
                || !settlements.is_empty()
                || !request.native_callbacks.is_empty()
                || !request.callback_thunks.is_empty()
            {
                return Err(realization_error(
                    "ranked native provider isolation",
                    "the exact ranked countdown admits no provider installation or boundary settlement",
                ));
            }
            let target =
                omega_abstract_operations_to_target_operations::lower_ranked_to_target_operations(
                    &ranked,
                    request.target,
                )
                .map_err(|error| realization_error("ranked target lowering", error))?;
            let assigned =
                omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                    .map_err(|error| realization_error("ranked physical assignment", error))?;
            let plan = omega_machine_emission::emit_machine_code(&assigned)
                .map_err(|error| realization_error("ranked machine-code emission", error))?;
            Ok(MachineCodePlanWithPrivateFunctions {
                plan,
                private_functions: Vec::new(),
            })
        }
        NativeRealizationInput::ExplicitOptimization(input) => {
            if !request.native_callbacks.is_empty() || !request.callback_thunks.is_empty() {
                return Err(realization_error(
                    "optimized native callback custody",
                    "retained callbacks require the ordinary custody-preserving pipeline",
                ));
            }
            if !request.ieee_float_fma.is_empty() {
                return Err(realization_error(
                    "optimized nearest-FMA custody",
                    "retained nearest-FMA occurrences require the ordinary custody-preserving pipeline",
                ));
            }
            let optimization_request = omega_optimization_pipeline::compiler_baseline_request_v1(
                request.optimization_selections,
            )
            .map_err(|error| realization_error("canonical optimization request", error))?;
            let optimized = omega_optimization_pipeline::optimize_verified_psi_input(
                input,
                optimization_request,
            )
            .map_err(|error| realization_error("canonical optimization", error))?;
            let continuation = match provider_installation {
                Some(installation) => omega_optimization_pipeline::stage_optimized_native_continuation_with_provider_executions_and_installation(
                    optimized,
                    request.target,
                    settlements,
                    installation,
                ),
                None => omega_optimization_pipeline::stage_optimized_native_continuation_with_provider_executions(
                    optimized,
                    request.target,
                    settlements,
                ),
            }
            .map_err(|error| match error {
                omega_optimization_pipeline::OptimizedNativeContinuationError::CoverageFallbackAssigned(
                    error,
                ) => realization_error("optimized physical assignment", error),
                omega_optimization_pipeline::OptimizedNativeContinuationError::SelectedPhysical(
                    error,
                ) => selected_physical_pipeline_failed(request.optimization_selections, error),
            })?;
            let assigned = match continuation {
                omega_optimization_pipeline::StagedOptimizedNativeContinuation::CoverageFallbackAssigned(
                    assigned,
                ) => assigned,
                omega_optimization_pipeline::StagedOptimizedNativeContinuation::SelectedPhysical(
                    physical,
                ) => {
                    return Err(selected_physical_pipeline_not_publishable(
                        request.optimization_selections,
                        &physical,
                    ));
                }
            };
            let plan = omega_machine_emission::emit_machine_code(assigned.assigned())
                .map_err(|error| realization_error("machine-code emission", error))?;
            Ok(MachineCodePlanWithPrivateFunctions {
                plan,
                private_functions: Vec::new(),
            })
        }
    }
}

fn emit_callback_thunks(
    thunks: &[crate::realization::model::NativeCallbackThunkSettlement<'_>],
    target: omega_target::NativeTarget,
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<Vec<CompilerPrivateMachineCodeFunction>, Vec<Diagnostic>> {
    if thunks.len() > 1 {
        return Err(realization_error(
            "native callback thunk emission",
            "ordinary native realization currently admits exactly one callback thunk",
        ));
    }
    let mut emitted = Vec::with_capacity(thunks.len());
    for thunk in thunks {
        if thunk.private_symbol.is_empty()
            || thunk.callback_function.callback_thunk_placement_index()
                != Some(thunk.placement_index)
        {
            return Err(realization_error(
                "native callback thunk emission",
                "callback thunk has invalid private identity custody",
            ));
        }
        thunk
            .artifact
            .validate()
            .map_err(|error| realization_error("callback thunk artifact replay", error))?;
        let lowered =
            omega_psi_to_abstract_operations::lower_artifact_sections_for_native_realization(
                thunk.artifact.semantic_bytes(),
                thunk.artifact.proof_bytes(),
                profile,
            )
            .map_err(|error| realization_error("callback thunk abstract lowering", error))?;
        let omega_psi_to_abstract_operations::NativeArtifactOperationPlan::Ordinary(abstract_plan) =
            lowered
        else {
            return Err(realization_error(
                "native callback thunk emission",
                "callback thunk cannot use ranked native authority",
            ));
        };
        let target_plan =
            omega_abstract_operations_to_target_operations::lower_to_target_operations(
                &abstract_plan,
                target,
            )
            .map_err(|error| realization_error("callback thunk target lowering", error))?;
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .map_err(|error| realization_error("callback thunk physical assignment", error))?;
        let machine_code = omega_machine_emission::emit_machine_code(&assigned)
            .map_err(|error| realization_error("callback thunk machine-code emission", error))?;
        let [function] = machine_code.functions.as_slice() else {
            return Err(realization_error(
                "native callback thunk emission",
                "bounded callback thunk must emit exactly one native function",
            ));
        };
        let Some(abi) = function.fixed_integer_scalar_abi.as_ref() else {
            return Err(realization_error(
                "native callback thunk emission",
                "bounded callback thunk did not retain its fixed-integer scalar ABI",
            ));
        };
        if machine_code.target != target
            || machine_code.entry != thunk.lowering_receipt.terminal_machine
            || function.machine != thunk.lowering_receipt.terminal_machine
            || abi.call_plan != thunk.boundary_entry_plan.call
        {
            return Err(realization_error(
                "native callback thunk emission",
                "callback thunk machine, target, or inbound ABI drifted from its lowering receipt",
            ));
        }
        emitted.push(CompilerPrivateMachineCodeFunction {
            identity: thunk.callback_function,
            private_symbol: std::sync::Arc::from(thunk.private_symbol),
            source_psi: machine_code.psi,
            function: function.clone(),
        });
    }
    Ok(emitted)
}

fn validate_callback_thunk_assignments(
    thunks: &[crate::realization::model::NativeCallbackThunkSettlement<'_>],
    assigned: &[omega_assigned_target_operations::AssignedNativeCallbackArgument],
) -> Result<(), Vec<Diagnostic>> {
    if thunks.len() != assigned.len() {
        return Err(realization_error(
            "native callback thunk assignment",
            "callback body and assigned argument rosters differ",
        ));
    }
    for thunk in thunks {
        let matching = assigned
            .iter()
            .filter(|argument| argument.target.placement_index == thunk.placement_index)
            .collect::<Vec<_>>();
        let [argument] = matching.as_slice() else {
            return Err(realization_error(
                "native callback thunk assignment",
                "callback body does not rejoin exactly one assigned registrar argument",
            ));
        };
        if argument.target.terminal_operation != thunk.terminal_operation
            || argument.target.callback_function != thunk.callback_function
        {
            return Err(realization_error(
                "native callback thunk assignment",
                "callback body identity drifted from its assigned registrar argument",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::realization::model::NativeCallbackThunkSettlement;

    fn callback_fixture() -> (
        psi_terminal_codec::CanonicalTerminalArtifact,
        psi_checked_trees_to_terminal::CallbackTerminalLoweringReceipt,
        omega_function_identity::MachineFunctionIdentity,
        omega_calling_conventions::BoundaryEntryPlan,
    ) {
        let checked = crate::tests::fixtures::checked_source::checked(
            r#"
                machine callback(message: u64) -> u64 {
                    message
                }
            "#,
        );
        let selection = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .find(|selection| selection.name == "callback")
            .expect("callback machine selection");
        let state = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(selection.machine)
            .and_then(|graph| graph.states.first())
            .expect("callback checked scalar entry");
        let lowered = psi_checked_trees_to_terminal::lower_bounded_callback_identity_machine(
            &checked,
            selection.machine,
            state.state,
        )
        .expect("bounded callback Terminal lowering");
        let artifact = psi_terminal_codec::CanonicalTerminalArtifact::from_parts(
            &lowered.terminal.semantic_module,
            &lowered.terminal.proof_bundle,
            lowered.terminal.debug_map.as_ref(),
        )
        .expect("canonical callback artifact");
        let signature = omega_calling_conventions::CallSignature {
            parameters: vec![omega_calling_conventions::ValueShape::integer(8, 8)],
            result: Some(omega_calling_conventions::ValueShape::integer(8, 8)),
        };
        let boundary = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            omega_calling_conventions::CallingPolicy::MicrosoftX64,
            &signature,
        )
        .expect("Microsoft x64 callback plan")
        .plan()
        .clone();
        let identity = omega_function_identity::MachineFunctionIdentity::callback_thunk(
            omega_function_identity::StateKey {
                machine: selection.machine,
                state: state.state,
                segment_index: 0,
            },
            0,
        )
        .expect("callback thunk identity");
        (artifact, lowered.receipt, identity, boundary)
    }

    #[test]
    fn bounded_callback_artifact_emits_one_private_x86_function() {
        let (artifact, receipt, identity, boundary) = callback_fixture();
        let settlement = NativeCallbackThunkSettlement {
            terminal_operation: psi_core::OperationId::new(1).expect("operation"),
            placement_index: 0,
            callback_function: identity,
            private_symbol: "__omega_test_callback",
            artifact: &artifact,
            lowering_receipt: receipt,
            boundary_entry_plan: &boundary,
        };
        let emitted = emit_callback_thunks(
            &[settlement],
            omega_target::NativeTarget::windows_x64(),
            &psi_proof_admission::AdmissionProfile::default(),
        )
        .expect("bounded callback thunk emission");
        let [private] = emitted.as_slice() else {
            panic!("one private callback function must be emitted");
        };
        assert_eq!(private.identity, identity);
        assert_eq!(private.private_symbol.as_ref(), "__omega_test_callback");
        assert_eq!(private.function.machine, receipt.terminal_machine);
        assert!(!private.function.bytes.is_empty());
        assert_eq!(
            private
                .function
                .fixed_integer_scalar_abi
                .as_ref()
                .expect("fixed callback ABI")
                .call_plan,
            boundary.call,
        );

        let mut drifted_boundary = boundary.clone();
        drifted_boundary.call.parameters[0].shape =
            omega_calling_conventions::ValueShape::integer(4, 4);
        assert!(
            emit_callback_thunks(
                &[NativeCallbackThunkSettlement {
                    boundary_entry_plan: &drifted_boundary,
                    ..settlement
                }],
                omega_target::NativeTarget::windows_x64(),
                &psi_proof_admission::AdmissionProfile::default(),
            )
            .is_err(),
            "callback ABI drift must reject",
        );

        let semantic_function = private.function.clone();
        let semantic_psi = psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: private.source_psi.vocabulary_marker,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x44; 32]),
        };
        let object_input = omega_machine_code::MachineCodePlanWithPrivateFunctions {
            plan: omega_machine_code::MachineCodePlan {
                psi: semantic_psi,
                target: omega_target::NativeTarget::windows_x64(),
                entry: semantic_function.machine,
                functions: vec![semantic_function],
            },
            private_functions: emitted,
        };
        let object =
            omega_image_emission::build_object_artifact_with_private_functions(&object_input)
                .expect("private callback object custody");
        let [private] = object.private_functions() else {
            panic!("object must retain one private callback function");
        };
        assert_eq!(object.functions().len(), 1);
        assert_eq!(private.identity, identity);
        assert_ne!(private.source_psi, object.psi());
        assert_eq!(
            private.function.machine,
            object.functions()[0].machine,
            "artifact-local callback MachineId may equal a semantic MachineId without joining namespaces",
        );
        assert_eq!(
            private.function.text_offset,
            object.functions()[0].byte_count
        );
        assert_eq!(
            private.bytes(&object),
            &object_input.private_functions[0].function.bytes
        );
        let (symbol, plan) = omega_object_file::object_function_symbol(object.object(), identity)
            .expect("exact callback identity symbol");
        assert_eq!(symbol, private.function.symbol);
        assert_eq!(plan.name, "__omega_test_callback");
        assert_eq!(plan.offset, private.function.text_offset);
        assert_eq!(plan.size, private.function.byte_count);

        let image = omega_image_emission::emit_executable_image(&object, 3)
            .expect("private callback executable image custody");
        assert_eq!(image.private_functions(), object.private_functions());
        omega_image_emission::validate_executable_image(&object, &image)
            .expect("private callback executable replay");
        let installation = omega_image_emission::build_installation_record(
            &image,
            psi_core::ProfileDecisionId::new(1).expect("profile decision"),
        )
        .expect("private callback installation custody");
        let [installed_private] = installation.private_functions() else {
            panic!("installation must retain one private callback function");
        };
        assert_eq!(installed_private.identity, private.identity);
        assert_eq!(installed_private.source_psi, private.source_psi);
        assert_eq!(installed_private.machine, private.function.machine);
        assert_eq!(
            Some(&installed_private.fixed_integer_scalar_abi),
            private.function.fixed_integer_scalar_abi.as_ref()
        );
        assert_eq!(installed_private.text_offset, private.function.text_offset);
        assert_eq!(installed_private.byte_count, private.function.byte_count);
        let installation_bytes = omega_image_emission::encode_installation_record(&installation)
            .expect("private callback installation encoding");
        let decoded = omega_image_emission::decode_installation_record(&installation_bytes)
            .expect("private callback installation decoding");
        assert_eq!(decoded, installation);
        omega_image_emission::validate_installation_record(&decoded, &image)
            .expect("private callback installation replay");

        let mut wrong_role = object_input.clone();
        wrong_role.private_functions[0].identity =
            omega_function_identity::MachineFunctionIdentity::source(
                identity.associated_source_continuation(),
            );
        assert!(matches!(
            omega_image_emission::build_object_artifact_with_private_functions(&wrong_role),
            Err(omega_image_emission::ObjectError::InvalidPrivateFunctionIdentity)
        ));

        let mut empty_symbol = object_input.clone();
        empty_symbol.private_functions[0].private_symbol = std::sync::Arc::from("");
        assert!(matches!(
            omega_image_emission::build_object_artifact_with_private_functions(&empty_symbol),
            Err(omega_image_emission::ObjectError::EmptyPrivateFunctionSymbol)
        ));

        let mut colliding_symbol = object_input.clone();
        colliding_symbol.private_functions[0].private_symbol = std::sync::Arc::from("main");
        assert!(matches!(
            omega_image_emission::build_object_artifact_with_private_functions(&colliding_symbol),
            Err(omega_image_emission::ObjectError::PrivateFunctionSymbolCollision)
        ));

        let mut missing_abi = object_input.clone();
        missing_abi.private_functions[0]
            .function
            .fixed_integer_scalar_abi = None;
        assert!(matches!(
            omega_image_emission::build_object_artifact_with_private_functions(&missing_abi),
            Err(omega_image_emission::ObjectError::InvalidPrivateFunctionAbi)
        ));

        let mut empty_body = object_input.clone();
        empty_body.private_functions[0].function.bytes.clear();
        assert!(matches!(
            omega_image_emission::build_object_artifact_with_private_functions(&empty_body),
            Err(omega_image_emission::ObjectError::InvalidPrivateFunctionBody)
        ));

        let mut duplicate = object_input;
        duplicate
            .private_functions
            .push(duplicate.private_functions[0].clone());
        assert!(matches!(
            omega_image_emission::build_object_artifact_with_private_functions(&duplicate),
            Err(omega_image_emission::ObjectError::TooManyPrivateFunctions)
        ));
    }
}
