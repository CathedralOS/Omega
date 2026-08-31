//! X86 rel8 artifact binding, replay, reporting, and corruption rejection.

use crate::tests::*;

#[test]
fn optimized_rel8_object_artifact_binds_replays_and_reports_without_authority() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let terminal = canonical_artifact(&semantic, &proof);
    let selections =
        OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_x64(),
        &[],
    )
    .unwrap();
    let physical_report = optimization_pipeline_report(&physical);
    assert_eq!(physical_report.function_fragment(), None);
    assert_eq!(physical_report.text_section(), None);
    assert_eq!(physical_report.object_container(), None);
    assert_eq!(physical_report.object_artifact(), None);
    let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } = physical
    else {
        panic!("rel8 must complete its direct realization")
    };
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
    )
    .unwrap();
    let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let object = stage_optimized_relocation_free_object_container(placed).unwrap();
    let mut staged = stage_validated_optimized_object_artifact(terminal, object).unwrap();

    assert_eq!(
        validate_optimized_object_artifact(&staged).unwrap(),
        staged.custody()
    );
    let artifact = staged.artifact();
    assert_eq!(artifact.psi, staged.source().object().psi);
    assert_eq!(
        artifact.semantic_entry,
        staged.source().object().semantic_entry
    );
    assert_eq!(artifact.statistics.relocation_records, 0);
    assert_eq!(
        artifact.pre_physical_manifest,
        staged
            .source()
            .source()
            .source()
            .function_relative_manifest()
            .record()
            .pre_physical_manifest
    );
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&artifact.encode()),
        Ok(artifact.clone())
    );
    let manifest = staged.manifest().record();
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&manifest.encode()),
        Ok(manifest.clone())
    );
    assert_eq!(
        manifest.external_entry_bridge,
        OptimizedObjectArtifactUnavailableData::Unavailable
    );
    assert_eq!(
        manifest.executable_image,
        OptimizedObjectArtifactUnavailableData::Unavailable
    );
    assert_eq!(
        manifest.installation,
        OptimizedObjectArtifactUnavailableData::Unavailable
    );
    assert_eq!(
        manifest.publication,
        OptimizedObjectArtifactUnavailableData::Unavailable
    );

    let artifact_identity = artifact.identity;
    let object_bytes = staged.source().container().bytes.clone();
    let report = optimization_pipeline_report_from_object_artifact(&staged);
    assert_eq!(
        report.render_human_text(OptimizationReportRequest::Suppressed),
        None
    );
    let rendered = report
        .render_human_text(OptimizationReportRequest::EmitHumanText)
        .unwrap();
    assert!(rendered.contains("[optimized Omega object artifact]"));
    assert!(rendered.contains("publication: unavailable"));
    assert_eq!(staged.artifact().identity, artifact_identity);
    assert_eq!(staged.source().container().bytes, object_bytes);
    assert_eq!(
        report.function_fragment().unwrap().identity,
        artifact.function_fragment_manifest
    );
    assert_eq!(
        report.text_section().unwrap().identity,
        artifact.text_section_manifest
    );
    assert_eq!(
        report.object_container().unwrap().identity,
        artifact.object_container_manifest
    );
    assert_eq!(
        report.object_artifact().unwrap().artifact,
        artifact.identity
    );

    let original_artifact = staged.artifact().clone();
    staged.artifact_mut().statistics.relocation_records = 1;
    let corrupted_artifact_identity = staged.artifact().recomputed_identity();
    staged.artifact_mut().identity = corrupted_artifact_identity;
    assert_eq!(
        validate_optimized_object_artifact(&staged),
        Err(OptimizedObjectArtifactError::ArtifactMismatch)
    );
    *staged.artifact_mut() = original_artifact;

    let original_manifest = staged.manifest().record().clone();
    staged
        .manifest_mut()
        .record_mut()
        .statistics
        .function_symbols += 1;
    let corrupted_manifest_identity = staged.manifest().record().recomputed_identity();
    staged.manifest_mut().record_mut().identity = corrupted_manifest_identity;
    assert_eq!(
        validate_optimized_object_artifact(&staged),
        Err(OptimizedObjectArtifactError::ManifestMismatch)
    );
    *staged.manifest_mut().record_mut() = original_manifest;
    staged.corrupt_custody_for_test();
    assert_eq!(
        validate_optimized_object_artifact(&staged),
        Err(OptimizedObjectArtifactError::ReceiptMismatch)
    );
}
