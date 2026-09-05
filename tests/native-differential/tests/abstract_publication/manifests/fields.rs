use super::super::*;
use super::fixture::{donor, optimized, validate};

type Mutation = fn(&mut PrePhysicalOptimizationManifest, &ValidatedOptimizedAbstractPlan);

#[test]
fn every_mutable_manifest_field_is_bound_by_independent_validation() {
    let optimized = optimized();
    let donor = donor();
    let baseline = optimized.pre_physical_manifest().record();
    assert_eq!(MUTATIONS.len(), 35, "complete mutable logical-field matrix");

    for (name, mutate) in MUTATIONS {
        let mut candidate = baseline.clone();
        mutate(&mut candidate, &donor);
        candidate.identity = candidate.recomputed_identity();

        assert_eq!(
            PrePhysicalOptimizationManifest::decode(&candidate.encode()),
            Ok(candidate.clone()),
            "mutation `{name}` must remain a canonical manifest"
        );
        assert_eq!(
            validate(&optimized, &candidate),
            Err(PrePhysicalOptimizationManifestError::ContentMismatch),
            "independent replay must reject mutation `{name}`"
        );
    }
}

#[test]
fn stale_outer_identity_is_rejected_before_custody_is_granted() {
    let optimized = optimized();
    let mut candidate = optimized.pre_physical_manifest().record().clone();
    candidate.source_statistics.nodes += 1;

    assert_eq!(
        PrePhysicalOptimizationManifest::decode(&candidate.encode()),
        Err(PrePhysicalOptimizationManifestDecodeError::IdentityMismatch)
    );
    assert_eq!(
        validate(&optimized, &candidate),
        Err(PrePhysicalOptimizationManifestError::ContentMismatch)
    );
}

const MUTATIONS: &[(&str, Mutation)] = &[
    ("psi.program_fingerprint", mutate_program_fingerprint),
    ("fuel_schedule", mutate_fuel_schedule),
    ("initial_unit", mutate_initial_unit),
    ("final_unit", mutate_final_unit),
    ("projection", mutate_projection),
    ("selections", mutate_selections),
    ("psi_selections", mutate_psi_selections),
    ("budget.rule_evaluations", mutate_budget_rule_evaluations),
    ("budget.candidates", mutate_budget_candidates),
    ("budget.validation_steps", mutate_budget_validation_steps),
    ("budget.commits", mutate_budget_commits),
    ("budget.iterations", mutate_budget_iterations),
    ("usage.rule_evaluations", mutate_usage_rule_evaluations),
    ("usage.candidates", mutate_usage_candidates),
    ("usage.validation_steps", mutate_usage_validation_steps),
    ("usage.commits", mutate_usage_commits),
    ("usage.iterations", mutate_usage_iterations),
    ("decision_log", mutate_decision_log),
    ("pass_manifests", mutate_pass_manifests),
    ("transformation_ledger", mutate_transformation_ledger),
    ("identity_bundle", mutate_identity_bundle),
    ("source.functions", mutate_source_functions),
    ("source.blocks", mutate_source_blocks),
    ("source.nodes", mutate_source_nodes),
    (
        "source.scalar_definitions",
        mutate_source_scalar_definitions,
    ),
    ("source.scalar_uses", mutate_source_scalar_uses),
    (
        "source.optimization_facts",
        mutate_source_optimization_facts,
    ),
    (
        "source.ownership_frontier_facts",
        mutate_source_ownership_frontier_facts,
    ),
    ("optimized.functions", mutate_optimized_functions),
    ("optimized.blocks", mutate_optimized_blocks),
    ("optimized.nodes", mutate_optimized_nodes),
    (
        "optimized.scalar_definitions",
        mutate_optimized_scalar_definitions,
    ),
    ("optimized.scalar_uses", mutate_optimized_scalar_uses),
    (
        "optimized.optimization_facts",
        mutate_optimized_optimization_facts,
    ),
    (
        "optimized.ownership_frontier_facts",
        mutate_optimized_ownership_frontier_facts,
    ),
];

fn mutate_program_fingerprint(
    manifest: &mut PrePhysicalOptimizationManifest,
    _: &ValidatedOptimizedAbstractPlan,
) {
    manifest.psi.program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes([0x51; 32]);
}

fn mutate_fuel_schedule(
    manifest: &mut PrePhysicalOptimizationManifest,
    _: &ValidatedOptimizedAbstractPlan,
) {
    manifest.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(99_960).unwrap();
}

fn mutate_initial_unit(
    manifest: &mut PrePhysicalOptimizationManifest,
    _: &ValidatedOptimizedAbstractPlan,
) {
    manifest.initial_unit =
        optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"mutated initial");
}

fn mutate_final_unit(
    manifest: &mut PrePhysicalOptimizationManifest,
    _: &ValidatedOptimizedAbstractPlan,
) {
    manifest.final_unit =
        optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"mutated final");
}

fn mutate_projection(
    manifest: &mut PrePhysicalOptimizationManifest,
    _: &ValidatedOptimizedAbstractPlan,
) {
    manifest.projection =
        optimization_core::OptimizedAbstractPlanProjectionIdentity::from_canonical_bytes(
            b"mutated projection",
        );
}

fn mutate_selections(
    manifest: &mut PrePhysicalOptimizationManifest,
    donor: &ValidatedOptimizedAbstractPlan,
) {
    manifest.selections = donor.selections().clone();
}

fn mutate_psi_selections(
    manifest: &mut PrePhysicalOptimizationManifest,
    donor: &ValidatedOptimizedAbstractPlan,
) {
    manifest.psi_selections = donor.psi_selections().clone();
}

fn replace_budget(manifest: &mut PrePhysicalOptimizationManifest, values: [u64; 5]) {
    manifest.budget_per_pass =
        OptimizationWorkBudget::new(values[0], values[1], values[2], values[3], values[4]).unwrap();
}

fn budget_values(manifest: &PrePhysicalOptimizationManifest) -> [u64; 5] {
    let budget = manifest.budget_per_pass;
    [
        budget.rule_evaluations(),
        budget.candidates(),
        budget.validation_steps(),
        budget.commits(),
        budget.iterations(),
    ]
}

macro_rules! budget_mutation {
    ($name:ident, $index:expr) => {
        fn $name(
            manifest: &mut PrePhysicalOptimizationManifest,
            _: &ValidatedOptimizedAbstractPlan,
        ) {
            let mut values = budget_values(manifest);
            values[$index] += 1;
            replace_budget(manifest, values);
        }
    };
}

budget_mutation!(mutate_budget_rule_evaluations, 0);
budget_mutation!(mutate_budget_candidates, 1);
budget_mutation!(mutate_budget_validation_steps, 2);
budget_mutation!(mutate_budget_commits, 3);
budget_mutation!(mutate_budget_iterations, 4);

macro_rules! usage_mutation {
    ($name:ident, $field:ident) => {
        fn $name(
            manifest: &mut PrePhysicalOptimizationManifest,
            _: &ValidatedOptimizedAbstractPlan,
        ) {
            manifest.usage.$field += 1;
        }
    };
}

usage_mutation!(mutate_usage_rule_evaluations, rule_evaluations);
usage_mutation!(mutate_usage_candidates, candidates);
usage_mutation!(mutate_usage_validation_steps, validation_steps);
usage_mutation!(mutate_usage_commits, commits);
usage_mutation!(mutate_usage_iterations, iterations);

fn mutate_decision_log(
    manifest: &mut PrePhysicalOptimizationManifest,
    donor: &ValidatedOptimizedAbstractPlan,
) {
    manifest.decision_log = donor.decisions().clone();
}

fn mutate_pass_manifests(
    manifest: &mut PrePhysicalOptimizationManifest,
    donor: &ValidatedOptimizedAbstractPlan,
) {
    manifest.pass_manifests = donor.pass_manifests().to_vec();
}

fn mutate_transformation_ledger(
    manifest: &mut PrePhysicalOptimizationManifest,
    donor: &ValidatedOptimizedAbstractPlan,
) {
    manifest.transformation_ledger = donor.transformation_ledger().clone();
}

fn mutate_identity_bundle(
    manifest: &mut PrePhysicalOptimizationManifest,
    donor: &ValidatedOptimizedAbstractPlan,
) {
    manifest.identity_bundle = donor.identity_bundle();
}

macro_rules! statistics_mutation {
    ($name:ident, $side:ident, $field:ident) => {
        fn $name(
            manifest: &mut PrePhysicalOptimizationManifest,
            _: &ValidatedOptimizedAbstractPlan,
        ) {
            manifest.$side.$field += 1;
        }
    };
}

statistics_mutation!(mutate_source_functions, source_statistics, functions);
statistics_mutation!(mutate_source_blocks, source_statistics, blocks);
statistics_mutation!(mutate_source_nodes, source_statistics, nodes);
statistics_mutation!(
    mutate_source_scalar_definitions,
    source_statistics,
    scalar_definitions
);
statistics_mutation!(mutate_source_scalar_uses, source_statistics, scalar_uses);
statistics_mutation!(
    mutate_source_optimization_facts,
    source_statistics,
    optimization_facts
);
statistics_mutation!(
    mutate_source_ownership_frontier_facts,
    source_statistics,
    ownership_frontier_facts
);
statistics_mutation!(mutate_optimized_functions, optimized_statistics, functions);
statistics_mutation!(mutate_optimized_blocks, optimized_statistics, blocks);
statistics_mutation!(mutate_optimized_nodes, optimized_statistics, nodes);
statistics_mutation!(
    mutate_optimized_scalar_definitions,
    optimized_statistics,
    scalar_definitions
);
statistics_mutation!(
    mutate_optimized_scalar_uses,
    optimized_statistics,
    scalar_uses
);
statistics_mutation!(
    mutate_optimized_optimization_facts,
    optimized_statistics,
    optimization_facts
);
statistics_mutation!(
    mutate_optimized_ownership_frontier_facts,
    optimized_statistics,
    ownership_frontier_facts
);
