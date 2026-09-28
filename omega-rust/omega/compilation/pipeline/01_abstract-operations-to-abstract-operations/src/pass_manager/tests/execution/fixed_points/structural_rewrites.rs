//! Control-flow cleanup and copy-propagation structural fixed points.

use super::super::super::super::run_unit;
use super::super::super::{
    Optimization, OptimizationSelections, budget, built_in_psi_registry, non_adjacent_merge_unit,
};

#[test]
fn named_control_flow_cleanup_merges_non_adjacent_blocks_to_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    for target_before_predecessor in [false, true] {
        let unit = non_adjacent_merge_unit(target_before_predecessor);
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit, &registry, budget(8)).unwrap();

        assert_eq!(commits.len(), 2);
        assert_eq!(usage.commits, 2);
        assert_eq!(usage.iterations, 3);
        assert_eq!(
            usage.rule_evaluations,
            if target_before_predecessor { 21 } else { 18 }
        );
        assert_eq!(output.functions[0].blocks.len(), 3);
        assert_eq!(ledger.records().len(), 2);
        assert!(ledger.records().iter().all(|record| {
            record
                .provenance
                .iter()
                .all(|row| row.disposition.is_realized())
        }));
        assert_eq!(manifest.unwrap().ordered_rules().len(), 7);

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second, output);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert_eq!(second_usage.rule_evaluations, 7);
        assert!(second_ledger.records().is_empty());
    }
}
