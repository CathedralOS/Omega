//! Pre-physical manifest projection custody.

use super::*;

#[test]
fn pre_physical_manifest_is_deterministic_structured_and_independently_validated() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let first = project_optimization_run(run(exact_add_verified(), selections.clone())).unwrap();
    let second = project_optimization_run(run(exact_add_verified(), selections)).unwrap();
    let manifest = first.pre_physical_manifest().record();

    assert_eq!(manifest, second.pre_physical_manifest().record());
    assert_eq!(manifest.identity, manifest.recomputed_identity());
    let encoded = manifest.encode();
    assert_eq!(
        PrePhysicalOptimizationManifest::decode(&encoded),
        Ok(manifest.clone())
    );
    let mut identity_tamper = encoded.clone();
    identity_tamper[12] ^= 1;
    assert_eq!(
        PrePhysicalOptimizationManifest::decode(&identity_tamper),
        Err(PrePhysicalOptimizationManifestDecodeError::IdentityMismatch)
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        PrePhysicalOptimizationManifest::decode(&trailing),
        Err(PrePhysicalOptimizationManifestDecodeError::TrailingBytes)
    );
    assert_eq!(
        PrePhysicalOptimizationManifest::decode(&encoded[..encoded.len() - 1]),
        Err(PrePhysicalOptimizationManifestDecodeError::Truncated)
    );
    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        PrePhysicalOptimizationManifest::decode(&wrong_magic),
        Err(PrePhysicalOptimizationManifestDecodeError::WrongMagic)
    );
    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        PrePhysicalOptimizationManifest::decode(&wrong_version),
        Err(PrePhysicalOptimizationManifestDecodeError::UnsupportedVersion(2))
    );
    assert_eq!(
        manifest.physical_data,
        PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization
    );
    assert_eq!(manifest.initial_unit, first.transformation_ledger().input());
    assert_eq!(manifest.final_unit, first.unit().identity);
    assert_eq!(manifest.projection, first.validation().identity());
    assert_eq!(manifest.decision_log, *first.decisions());
    assert_eq!(manifest.pass_manifests, first.pass_manifests());
    assert_eq!(
        manifest.transformation_ledger,
        *first.transformation_ledger()
    );
    assert_eq!(manifest.source_statistics.functions, 1);
    assert_eq!(manifest.source_statistics.blocks, 1);
    assert_eq!(manifest.source_statistics.nodes, 4);
    assert_eq!(manifest.optimized_statistics.nodes, 4);
    let text = manifest.render_text();
    assert!(text.contains("SparseConditionalConstantPropagation"));
    assert!(text.contains("physical data: unavailable before physical realization"));
    assert!(text.contains("candidate verdicts: applied=1, skipped=0, rejected=0"));
    assert!(text.contains("fact: accepted-obligation:"));
    assert!(text.contains("source: operation:"));
    assert!(text.contains("source-scheduled-fuel: operation:"));
    assert!(text.contains("runtime-charge=1"));

    let replay = validate_pre_physical_optimization_manifest(
        manifest,
        first.verified_input(),
        first.unit(),
        first.selections(),
        first.psi_selections(),
        first.budget_per_pass(),
        work_usage(first.usage()),
        first.decisions(),
        first.pass_manifests(),
        first.transformation_ledger(),
        first.identity_bundle(),
        first.validation(),
    )
    .unwrap();
    assert_eq!(replay, *first.pre_physical_manifest());

    let mut corrupted = manifest.clone();
    corrupted.optimized_statistics.nodes += 1;
    corrupted.identity = corrupted.recomputed_identity();
    assert_eq!(
        validate_pre_physical_optimization_manifest(
            &corrupted,
            first.verified_input(),
            first.unit(),
            first.selections(),
            first.psi_selections(),
            first.budget_per_pass(),
            work_usage(first.usage()),
            first.decisions(),
            first.pass_manifests(),
            first.transformation_ledger(),
            first.identity_bundle(),
            first.validation(),
        ),
        Err(PrePhysicalOptimizationManifestError::ContentMismatch)
    );

    let mut omitted_pass = manifest.clone();
    omitted_pass.pass_manifests.clear();
    omitted_pass.identity = omitted_pass.recomputed_identity();
    assert_eq!(
        validate_pre_physical_optimization_manifest(
            &omitted_pass,
            first.verified_input(),
            first.unit(),
            first.selections(),
            first.psi_selections(),
            first.budget_per_pass(),
            work_usage(first.usage()),
            first.decisions(),
            first.pass_manifests(),
            first.transformation_ledger(),
            first.identity_bundle(),
            first.validation(),
        ),
        Err(PrePhysicalOptimizationManifestError::ContentMismatch)
    );

    let mut wrong_selections = manifest.clone();
    wrong_selections.selections =
        OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    wrong_selections.identity = wrong_selections.recomputed_identity();
    assert_eq!(
        validate_pre_physical_optimization_manifest(
            &wrong_selections,
            first.verified_input(),
            first.unit(),
            first.selections(),
            first.psi_selections(),
            first.budget_per_pass(),
            work_usage(first.usage()),
            first.decisions(),
            first.pass_manifests(),
            first.transformation_ledger(),
            first.identity_bundle(),
            first.validation(),
        ),
        Err(PrePhysicalOptimizationManifestError::ContentMismatch)
    );
}

#[test]
fn multi_pass_projection_retains_zero_commit_manifest_in_canonical_order() {
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::CopyPropagation,
    ])
    .unwrap();
    let optimized =
        project_optimization_run(run_pipeline(exact_add_verified(), selections)).unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.pass_manifests().len(), 2);
    assert_eq!(optimized.pass_manifests()[0].work_usage().commits, 1);
    assert_eq!(optimized.pass_manifests()[1].work_usage().commits, 0);
}

#[test]
fn multi_pass_projection_rejects_reordered_or_omitted_manifests() {
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::CopyPropagation,
    ])
    .unwrap();
    let mut reordered = run_pipeline(exact_add_verified(), selections.clone());
    reordered.pass_manifests.swap(0, 1);
    assert!(matches!(
        project_optimization_run(reordered),
        Err(OptimizedAbstractProjectionError::AppliedDecisionCustody {
            axis: AppliedDecisionCustodyAxis::ManifestPass,
            ..
        })
    ));

    let mut omitted = run_pipeline(exact_add_verified(), selections);
    omitted.pass_manifests.pop();
    assert!(matches!(
        project_optimization_run(omitted),
        Err(OptimizedAbstractProjectionError::AppliedDecisionCustody {
            axis: AppliedDecisionCustodyAxis::ManifestRoster,
            ..
        })
    ));
}
