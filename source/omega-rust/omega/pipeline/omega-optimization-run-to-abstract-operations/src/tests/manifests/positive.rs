use super::super::*;
use super::fixture::{optimized, validate};

#[test]
fn pre_physical_manifest_is_deterministic_structured_and_independently_validated() {
    let first = optimized();
    let second = optimized();
    let manifest = first.pre_physical_manifest().record();

    assert_eq!(manifest, second.pre_physical_manifest().record());
    assert_eq!(manifest.identity, manifest.recomputed_identity());
    assert_eq!(
        PrePhysicalOptimizationManifest::decode(&manifest.encode()),
        Ok(manifest.clone())
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

    assert_eq!(
        validate(&first, manifest).unwrap(),
        *first.pre_physical_manifest()
    );
}
