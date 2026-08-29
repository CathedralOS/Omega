//! Dead-scalar projection custody.

use super::*;

#[test]
fn dead_scalar_literal_elimination_replays_transitive_fuel_to_the_terminal() {
    let selections =
        OptimizationSelections::new([Optimization::DeadPureScalarElimination]).unwrap();
    let optimized =
        project_optimization_run(run(dead_scalar_literals_verified(), selections)).unwrap();
    assert_eq!(optimized.commits().len(), 2);
    assert_eq!(optimized.plan().functions[0].operations.len(), 1);
    assert_eq!(optimized.unit().functions[0].facts.len(), 0);
    let terminal = &optimized.unit().functions[0].blocks[0].nodes[0];
    assert!(matches!(
        terminal.operation,
        AbstractOperation::ReturnUnit { .. }
    ));
    assert_eq!(terminal.provenance.len(), 3);
    assert_eq!(terminal.fuel.len(), 3);
    assert!(
        optimized
            .transformation_ledger()
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .all(|row| row.disposition.is_realized())
    );
}

#[test]
fn dead_scalar_suite_removes_total_arithmetic_then_its_dead_operands() {
    let selections =
        OptimizationSelections::new([Optimization::DeadPureScalarElimination]).unwrap();
    let optimized =
        project_optimization_run(run(dead_wrapping_add_verified(), selections)).unwrap();
    assert_eq!(optimized.commits().len(), 3);
    assert_eq!(optimized.plan().functions[0].operations.len(), 1);
    assert_eq!(optimized.unit().functions[0].facts.len(), 0);
    let terminal = &optimized.unit().functions[0].blocks[0].nodes[0];
    assert!(matches!(
        terminal.operation,
        AbstractOperation::ReturnUnit { .. }
    ));
    assert_eq!(terminal.provenance.len(), 4);
    assert_eq!(terminal.fuel.len(), 4);
    assert!(
        optimized
            .transformation_ledger()
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .all(|row| row.disposition.is_realized())
    );
}
