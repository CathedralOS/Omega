//! X86 XOR-zero realization and complete object/callable projection.

use crate::tests::{
    AdmissionProfile, ExplicitOptimizationRequest, FunctionFragmentEmissionSourceKind, IntegerSign,
    IntegerType, NativeTarget, Optimization, OptimizationSelections, ProofBundle,
    StagedOptimizedFunctionFragmentEmissionSource,
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedVerifiedPhysicalPipeline,
    WholeFunctionExitLayoutCustody, canonical_artifact, conditional_immediate_machine,
    conditional_immediate_module, optimize_artifact_sections, selected_lowering_budget,
    stage_optimized_function_fragment_emission, stage_optimized_relocation_free_object_container,
    stage_optimized_relocation_free_text_section,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    stage_validated_optimized_object_artifact, stage_validated_optimized_ordinary_callable_entry,
    validate_optimized_object_artifact, validate_optimized_ordinary_callable_entry,
    validate_post_allocation_machine_function_relative_realization_custody,
};

#[test]
fn x86_xor_zero_uses_the_generic_post_allocation_join_for_both_source_routes() {
    for selected_lowering in [false, true] {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let machine = conditional_immediate_machine(18_100, integer_type, [0, 1]);
        let module = conditional_immediate_module(machine.id, vec![machine]);
        let semantic = psi_terminal_codec::encode_module(&module).unwrap();
        let proof = psi_terminal_codec::encode_proof_bundle(&ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        })
        .unwrap();
        let selections = if selected_lowering {
            OptimizationSelections::new([
                Optimization::SelectedIncomingU12ExactAddImmediate,
                Optimization::X86SelectXorZeroI64MaterializationV1,
            ])
            .unwrap()
        } else {
            OptimizationSelections::new([Optimization::X86SelectXorZeroI64MaterializationV1])
                .unwrap()
        };
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_x64(),
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
            &staged
        else {
            panic!("XOR-zero must reach the generic post-allocation realization")
        };
        assert_eq!(staged.selections(), selections.identity());
        let allocation = realization.allocation().current();
        assert_eq!(
            staged.selected_lowering_completion(),
            allocation
                .post_allocation_manifest()
                .record()
                .selected_lowering_completion
        );
        assert_eq!(
            staged.selected_lowering_completion().is_some(),
            selected_lowering
        );
        assert!(matches!(
            (selected_lowering, allocation.evidence()),
            (
                false,
                omega_selected_instructions_to_register_homes::AllocationEvidence::RegisterHomes(_)
            ) | (
                true,
                omega_selected_instructions_to_register_homes::AllocationEvidence::SelectedLowering(
                    _
                )
            )
        ));
        let StagedOptimizedPostAllocationMachineOptimization::X86XorZero(materialization) =
            realization.optimization()
        else {
            panic!("the generic realization must retain the XOR-zero result")
        };
        let custody = realization.optimization().custody().unwrap();
        assert!(custody.action_count() > 0);
        let action_count = u64::try_from(custody.action_count()).unwrap();
        assert_eq!(custody.baseline_bytes(), action_count * 10);
        assert_eq!(custody.selected_bytes(), action_count * 3);
        assert_eq!(custody.selections(), selections.identity());
        assert_eq!(
            realization
                .manifest()
                .record()
                .post_allocation_machine_optimization,
            Some(custody)
        );
        assert_eq!(
            validate_post_allocation_machine_function_relative_realization_custody(realization)
                .unwrap(),
            *realization.custody()
        );
        assert_eq!(
            materialization.materialization().plan().actions.len(),
            custody.action_count()
        );
        assert!(matches!(
            realization.exit_contract().contract().layout_custody,
            WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
                optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
                artifact_identity,
            } if artifact_identity == custody.artifact_identity()
        ));

        let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = staged
        else {
            unreachable!()
        };
        let emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(
                realization,
            )),
        )
        .unwrap();
        assert_eq!(
            emitted.manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
            }
        );
        let text = stage_optimized_relocation_free_text_section(emitted).unwrap();
        let object = stage_optimized_relocation_free_object_container(text).unwrap();
        let artifact = stage_validated_optimized_object_artifact(
            canonical_artifact(&semantic, &proof),
            object,
        )
        .unwrap();
        validate_optimized_object_artifact(&artifact).unwrap();
        let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
        validate_optimized_ordinary_callable_entry(&callable).unwrap();
    }
}
