//! Synthetic reload-value namespace custody after reload-home assignment.

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use selected_instructions_to_register_homes::{
    SyntheticReloadValueError, SyntheticReloadValueId, SyntheticReloadValuePlan,
    SyntheticReloadValuePolicy, ValidatedReloadValueHomes, ValidatedSyntheticReloadValues,
    bind_synthetic_reload_values, validate_synthetic_reload_values,
};
use target::NativeTarget;

use super::reload_value_homes::ReloadSources;
use crate::tests::{
    selected_lowering_budget, stage_optimized_allocation_legality, stage_optimized_live_ranges,
    stage_optimized_liveness, staged_exact_add_conditional,
};

#[test]
fn epoch_zero_namespace_is_exact_and_deterministic_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = ReloadSources::new(target);
        let homes = sources.assign(selected_lowering_budget()).unwrap();
        let first = bind(&sources, &homes, selected_lowering_budget()).unwrap();
        let second = bind(&sources, &homes, selected_lowering_budget()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().binding_count(), 1);
        assert_eq!(first.receipt().function_count(), 1);
        assert_eq!(first.receipt().usage(), exact_usage());
        assert_eq!(
            first.receipt().abstract_spill_insertion(),
            sources.insertion().receipt().identity()
        );
        assert_eq!(
            first.receipt().reload_value_homes(),
            homes.receipt().identity()
        );

        let home = homes.plan().functions[0].assignment.as_ref().unwrap();
        let binding = first.plan().functions[0].binding.unwrap();
        assert_eq!(binding.logical, home.result);
        assert_eq!(
            binding.synthetic,
            SyntheticReloadValueId {
                epoch: 0,
                ordinal: 0,
            }
        );
        assert_eq!(binding.block, home.block);
        assert_eq!(binding.start, home.start);
        assert_eq!(binding.exclusive_end, home.exclusive_end);
        assert_eq!(binding.class, home.class);
        assert_eq!(binding.view, home.view);
    }
}

#[test]
fn independent_replay_rejects_root_namespace_home_and_usage_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let sources = ReloadSources::new(target);
        let homes = sources.assign(selected_lowering_budget()).unwrap();
        let canonical = bind(&sources, &homes, selected_lowering_budget())
            .unwrap()
            .plan()
            .clone();

        let mut root = canonical.clone();
        root.reload_value_homes =
            selected_instructions_to_register_homes::ReloadValueHomeIdentity::from_bytes(
                [0x71; 32],
            );
        assert_eq!(
            validate(&sources, &homes, root),
            Err(SyntheticReloadValueError::RootMismatch)
        );

        for corrupt in [
            |plan: &mut SyntheticReloadValuePlan| {
                plan.functions[0].binding.as_mut().unwrap().synthetic.epoch = 1;
            },
            |plan: &mut SyntheticReloadValuePlan| {
                plan.functions[0]
                    .binding
                    .as_mut()
                    .unwrap()
                    .synthetic
                    .ordinal = 1;
            },
            |plan: &mut SyntheticReloadValuePlan| {
                plan.functions[0].binding.as_mut().unwrap().view.0 += 1;
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                validate(&sources, &homes, changed),
                Err(SyntheticReloadValueError::NonCanonicalNamespace { function: 0 })
            );
        }

        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            validate(&sources, &homes, usage),
            Err(SyntheticReloadValueError::UsageMismatch)
        );
    }
}

#[test]
fn exact_budget_and_empty_pressure_preserve_a_closed_namespace() {
    let sources = ReloadSources::new(NativeTarget::linux_x64());
    let homes = sources.assign(selected_lowering_budget()).unwrap();
    assert!(matches!(
        bind(
            &sources,
            &homes,
            OptimizationWorkBudget::new(1, 1, 6, 1, 1).unwrap(),
        ),
        Err(SyntheticReloadValueError::BudgetExceeded { .. })
    ));
    let exact = bind(
        &sources,
        &homes,
        OptimizationWorkBudget::new(1, 1, 7, 1, 1).unwrap(),
    )
    .unwrap();
    assert_eq!(exact.plan().usage, exact_usage());

    let legality = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional(NativeTarget::linux_x64()))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let empty_sources = ReloadSources::from_legality(legality);
    let empty_homes = empty_sources.assign(selected_lowering_budget()).unwrap();
    let empty = bind(&empty_sources, &empty_homes, selected_lowering_budget()).unwrap();
    assert_eq!(empty.receipt().binding_count(), 0);
    assert_eq!(
        empty.receipt().usage(),
        OptimizationWorkUsage {
            rule_evaluations: 1,
            candidates: 0,
            validation_steps: 0,
            commits: 0,
            iterations: 1,
        }
    );
    assert!(
        empty
            .plan()
            .functions
            .iter()
            .all(|function| function.binding.is_none())
    );
}

fn exact_usage() -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates: 1,
        validation_steps: 7,
        commits: 1,
        iterations: 1,
    }
}

fn bind(
    sources: &ReloadSources,
    homes: &ValidatedReloadValueHomes,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedSyntheticReloadValues, SyntheticReloadValueError> {
    bind_synthetic_reload_values(
        sources.insertion(),
        homes,
        SyntheticReloadValuePolicy::ValidatedSingleSpillEpochZeroCanonicalOrderV1,
        budget,
    )
}

fn validate(
    sources: &ReloadSources,
    homes: &ValidatedReloadValueHomes,
    plan: SyntheticReloadValuePlan,
) -> Result<ValidatedSyntheticReloadValues, SyntheticReloadValueError> {
    validate_synthetic_reload_values(sources.insertion(), homes, plan)
}
