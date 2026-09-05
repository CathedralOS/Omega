use crate::realization::callback_machine_code::emit_callback_thunks;
use crate::realization::diagnostics::realization_error;
use crate::realization::model::{NativeRealizationCoreRequest, NativeRealizationInput};
use crate::realization::optimization_stage::lower_realization_optimization_stage;
use crate::realization::optimized_fragment_projection::{
    OptimizedFragmentPublicationRequest, emit_return_only_optimized_fragments,
};
use crate::realization::physical_stage::{
    NativePhysicalStageResult, lower_realization_physical_stage,
};
use crate::realization::target_stage::lower_realization_target_stage;
use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_boundary_applications::TerminalBoundaryApplicationCoverage;
use omega_machine_code::MachineCodePlanWithPrivateFunctions;
use omega_native_artifact::NativePhysicalEvidenceScope;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use psi_diagnostics::Diagnostic;

pub(crate) struct EmittedRealizationMachineCode {
    pub(crate) machine_code: MachineCodePlanWithPrivateFunctions,
    pub(crate) physical_evidence_scope: NativePhysicalEvidenceScope,
}

pub(crate) fn emit_realization_machine_code(
    input: NativeRealizationInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    boundary_application_coverage: Option<&TerminalBoundaryApplicationCoverage>,
    initial_physical_evidence_scope: NativePhysicalEvidenceScope,
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<EmittedRealizationMachineCode, Vec<Diagnostic>> {
    let optimization_stage = lower_realization_optimization_stage(input, request)?;
    let target_stage = lower_realization_target_stage(
        optimization_stage,
        provider_installation,
        settlements,
        request,
    )?;
    let physical_stage = lower_realization_physical_stage(target_stage, request)?;
    match physical_stage {
        NativePhysicalStageResult::Assigned(assigned) => {
            let private_functions =
                emit_callback_thunks(request.callback_thunks, request.target, request.profile)?;
            let plan = omega_machine_emission::emit_machine_code_with_native_callbacks(&assigned)
                .map_err(|error| realization_error("machine-code emission", error))?;
            Ok(EmittedRealizationMachineCode {
                machine_code: MachineCodePlanWithPrivateFunctions {
                    plan,
                    private_functions,
                },
                physical_evidence_scope: initial_physical_evidence_scope,
            })
        }
        NativePhysicalStageResult::Optimized(optimized) => {
            let (plan, physical_evidence_scope) = emit_return_only_optimized_fragments(
                optimized.physical,
                OptimizedFragmentPublicationRequest {
                    has_provider_installation: optimized.has_provider_installation,
                    has_boundary_settlements: !settlements.is_empty(),
                    boundary_application_coverage,
                    optimized_plan: &optimized.optimized_plan,
                    terminal: optimized.terminal,
                    validation: optimized.validation,
                    final_unit: optimized.final_unit,
                },
            )?;
            Ok(EmittedRealizationMachineCode {
                machine_code: MachineCodePlanWithPrivateFunctions {
                    plan,
                    private_functions: Vec::new(),
                },
                physical_evidence_scope,
            })
        }
    }
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
        let optimized = psi_checked_trees_to_terminal::run_psi_optimization(
            lowered.terminal,
            psi_optimization::PsiOptimizationSelections::default(),
        )
        .expect("callback identity Psi optimization");
        let artifact = psi_checked_trees_to_terminal::finalize_terminal_artifact(&optimized)
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
