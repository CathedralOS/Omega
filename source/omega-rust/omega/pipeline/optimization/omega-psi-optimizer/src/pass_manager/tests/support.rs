//! Shared budget, external-log, and multi-pass execution helpers.

use super::*;

pub(super) fn budget(iterations: u64) -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(96, 64, 64, 64, iterations).unwrap()
}

pub(super) fn external_log_with(
    context: ExternalDecisionContext,
    points: impl IntoIterator<Item = ExternalDecisionPoint>,
) -> ExternalDecisionLog {
    ExternalDecisionLog::new(context, points).unwrap()
}

pub(super) fn run_test_pipeline(
    mut unit: PsiOptimizationUnit,
    registries: &[OrderedRuleRegistry],
) -> (
    PsiOptimizationUnit,
    Vec<OptimizationPassManifestRecord>,
    PsiTransformationLedger,
) {
    let input = unit.identity;
    let psi = unit.psi;
    let fuel_schedule = unit.fuel_schedule;
    let mut manifests = Vec::with_capacity(registries.len());
    let mut records = Vec::new();
    for registry in registries {
        let (output, _, _, _, manifest, ledger) = run_unit(unit, registry, budget(8)).unwrap();
        manifests.push(manifest.expect("a selected pass emits a manifest row"));
        records.extend_from_slice(ledger.records());
        unit = output;
    }
    let ledger =
        PsiTransformationLedger::new(psi, fuel_schedule, input, unit.identity, records).unwrap();
    (unit, manifests, ledger)
}
