//! The stage's catalog route is closed: every source-visible optimization
//! declared in the `SelectedLowering` phase must own exactly one
//! `SELECTED_LOWERING_RULE_CATALOG` row, in the vocabulary's canonical order.
//! `project_phase` already excludes foreign-phase variants before the
//! resolver runs, so a phase variant without a row is the only reachable
//! `UnsupportedSelection` — this pin makes that missing row a test failure
//! rather than a run-time rejection of an authored selection.

use optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

use crate::{
    ORDERED_SELECTED_LOWERING_RULES, SELECTED_LOWERING_RULE_CATALOG,
    resolve_selected_lowering_rules,
};

fn selected_lowering_variants() -> Vec<Optimization> {
    Optimization::ALL
        .iter()
        .copied()
        .filter(|optimization| {
            optimization.execution_phase() == OptimizationExecutionPhase::SelectedLowering
        })
        .collect()
}

#[test]
fn every_selected_lowering_variant_owns_exactly_one_catalog_row() {
    let phase_variants = selected_lowering_variants();
    assert_eq!(phase_variants.len(), SELECTED_LOWERING_RULE_CATALOG.len());
    for variant in phase_variants {
        let rows = SELECTED_LOWERING_RULE_CATALOG
            .iter()
            .filter(|entry| entry.optimization() == variant)
            .count();
        assert_eq!(
            rows, 1,
            "catalog route must bind exactly one row to {variant:?}"
        );
    }
}

#[test]
fn catalog_route_order_matches_the_phase_vocabulary() {
    assert_eq!(
        ORDERED_SELECTED_LOWERING_RULES.as_slice(),
        selected_lowering_variants()
    );
}

/// A selected phase variant resolves through its catalog row to the row's
/// declared fold policy.
#[test]
fn a_selected_variant_resolves_to_its_catalog_payload() {
    let variant = Optimization::SelectedIncomingBitwiseAndOnesIdentityCopy;
    let selections = OptimizationSelections::new([variant]).unwrap();
    let phase = selections.project_phase(OptimizationExecutionPhase::SelectedLowering);
    let (selected, policy) = resolve_selected_lowering_rules(&phase).unwrap();
    assert_eq!(selected.as_slice(), &[variant]);
    let entry = SELECTED_LOWERING_RULE_CATALOG
        .iter()
        .find(|entry| entry.optimization() == variant)
        .unwrap();
    assert!(policy.contains(entry.payload().policy()));
}
