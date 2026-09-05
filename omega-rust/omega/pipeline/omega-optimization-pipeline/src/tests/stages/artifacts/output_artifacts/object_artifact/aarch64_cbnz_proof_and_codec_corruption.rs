//! AArch64 CBNZ artifact proof custody, zero-span retention, and codec rejection.

use crate::FunctionFragmentReplayInputs;
use crate::tests::*;

#[test]
fn optimized_cbnz_object_artifact_retains_zero_span_and_rejects_detached_proof() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1])
            .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let realization = (physical)
        .into_post_allocation_machine_for_test()
        .unwrap_or_else(|| panic!("CBNZ must complete its direct realization"));
    let emitted = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::PostAllocationMachine(Box::new(realization)).into(),
    )
    .unwrap();
    let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let object = stage_optimized_relocation_free_object_container(placed).unwrap();

    let module = psi_terminal_codec::decode_module(&semantic).unwrap();
    let mut detached_proof = psi_terminal_codec::decode_proof_bundle(&proof).unwrap();
    detached_proof.evidence.pop();
    let optimization =
        psi_terminal_codec::build_identity_optimization_execution_record(&module, &detached_proof)
            .unwrap();
    let detached = psi_terminal_codec::CanonicalTerminalArtifact::from_parts(
        &module,
        &detached_proof,
        &optimization,
        None,
    )
    .unwrap();
    assert!(matches!(
        stage_validated_optimized_object_artifact(detached, object),
        Err(OptimizedObjectArtifactError::ProofMismatch)
    ));

    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let realization = (physical)
        .into_post_allocation_machine_for_test()
        .unwrap_or_else(|| panic!("CBNZ must complete its direct realization"));
    let emitted = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::PostAllocationMachine(Box::new(realization)).into(),
    )
    .unwrap();
    let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let object = stage_optimized_relocation_free_object_container(placed).unwrap();
    let staged =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .unwrap();
    assert!(
        staged.source().source().text_section().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| instruction.byte_count == 0)
    );
    assert_eq!(
        validate_optimized_object_artifact(&staged).unwrap(),
        staged.custody()
    );

    let mut wrong_magic = staged.artifact().encode();
    wrong_magic[0] ^= 1;
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&wrong_magic),
        Err(OptimizedObjectArtifactRecordDecodeError::WrongMagic)
    );
    let mut wrong_version = staged.manifest().record().encode();
    wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&wrong_version),
        Err(OptimizedObjectArtifactManifestDecodeError::UnsupportedVersion(2))
    );
    let mut trailing = staged.artifact().encode();
    trailing.push(0);
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&trailing),
        Err(OptimizedObjectArtifactRecordDecodeError::TrailingBytes)
    );
    let mut stale = staged.manifest().record().encode();
    stale[12] ^= 1;
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&stale),
        Err(OptimizedObjectArtifactManifestDecodeError::IdentityMismatch)
    );
}
