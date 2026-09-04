use crate::tests::*;

pub(super) fn staged_object_artifact(
    target: NativeTarget,
) -> StagedValidatedOptimizedObjectArtifact {
    let (semantic, proof) = conditional_u64_integer_less_or_equal_parameters_artifact();
    let selections = match target.architecture {
        omega_target::Architecture::X86_64 => OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::X86RelaxConditionalBranchesToRel8V1,
        ])
        .unwrap(),
        omega_target::Architecture::Aarch64 => OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
        ])
        .unwrap(),
    };
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let source = match physical {
        StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } => {
            StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization))
        }
        StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } => {
            StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(
                realization,
            ))
        }
        StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } => {
            StagedOptimizedFunctionFragmentEmissionSource::SelectedLowering(Box::new(realization))
        }
        _ => panic!("runtime inclusive-order fixture must complete function-relative realization"),
    };
    let fragments = stage_optimized_function_fragment_emission(source).unwrap();
    let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
        .unwrap()
}
