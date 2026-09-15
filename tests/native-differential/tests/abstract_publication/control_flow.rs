//! Control-flow projection custody.

use super::*;

#[test]
fn private_machine_pruning_projects_exact_roster_and_ledger_custody() {
    let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
    let optimized =
        publish_optimization_run(run(unreachable_private_machine_verified(), selections)).unwrap();

    assert_eq!(optimized.verified_input().plan().functions.len(), 2);
    assert_eq!(optimized.plan().functions.len(), 1);
    assert_eq!(optimized.unit().functions.len(), 1);
    assert_eq!(optimized.unit().pruned_machines.len(), 1);
    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
    assert_eq!(
        optimized.transformation_ledger().records()[0].pruned_machines,
        optimized.unit().pruned_machines
    );
    assert!(
        optimized.transformation_ledger().records()[0]
            .provenance
            .iter()
            .all(|row| matches!(
                row.disposition,
                optimization_unit::ProvenanceDisposition::ProvenUnreachableAt(_)
            ))
    );

    let mut wrong_ordinal = optimized.unit().clone();
    wrong_ordinal.pruned_machines[0].source_ordinal = 0;
    wrong_ordinal.identity =
        optimization_unit::recompute_psi_optimization_unit_identity(&wrong_ordinal);
    assert_eq!(
        abstract_operations_to_abstract_operations::validation::validate_transformed_psi_optimization_unit(
            optimized.verified_input(),
            &wrong_ordinal,
        ),
        Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)
    );
}

#[test]
fn adjacent_terminal_jump_fusion_reaches_verified_one_block_projection() {
    let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
    let optimized =
        publish_optimization_run(run(adjacent_terminal_jump_verified(), selections)).unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.plan().functions[0].block_entries.len(), 1);
    assert_eq!(optimized.unit().functions[0].blocks.len(), 1);
    assert_eq!(
        optimized.unit().functions[0].blocks[0].nodes[0].provenance,
        [
            optimization_unit::PsiProvenance::Edge(EdgeId::new(1_055).unwrap()),
            optimization_unit::PsiProvenance::Edge(EdgeId::new(1_054).unwrap()),
        ]
    );
}

#[test]
fn non_adjacent_block_merges_replay_and_lower_in_both_target_families() {
    let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
    let optimized =
        publish_optimization_run(run(non_adjacent_block_merge_verified(), selections.clone()))
            .unwrap();

    assert_eq!(optimized.commits().len(), 2);
    assert_eq!(optimized.transformation_ledger().records().len(), 2);
    assert_eq!(
        optimized
            .transformation_ledger()
            .records()
            .iter()
            .map(|record| record.provenance.len())
            .collect::<Vec<_>>(),
        [6, 6]
    );
    let retained_outgoing_edge = optimization_unit::PsiRealizationSite::Edge {
        machine: MachineId::new(1_501).unwrap(),
        edge: EdgeId::new(1_517).unwrap(),
    };
    let outgoing_edge_rewrites = optimized
        .transformation_ledger()
        .records()
        .iter()
        .flat_map(|record| &record.provenance)
        .filter(|row| row.input == retained_outgoing_edge)
        .collect::<Vec<_>>();
    assert_eq!(outgoing_edge_rewrites.len(), 1);
    assert_eq!(
        outgoing_edge_rewrites[0].sources,
        [optimization_unit::PsiProvenance::Edge(
            EdgeId::new(1_517).unwrap()
        )]
    );
    assert_eq!(
        outgoing_edge_rewrites[0].disposition,
        optimization_unit::ProvenanceDisposition::RealizedAt(
            optimization_unit::PsiRealizationSite::Node(optimization_unit::NodeLocation {
                machine: MachineId::new(1_501).unwrap(),
                block: BlockId::new(1_504).unwrap(),
                node: 1,
            },)
        )
    );
    assert!(
        optimized
            .transformation_ledger()
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .all(|row| row.disposition.is_realized())
    );
    assert_eq!(optimized.plan().functions[0].block_entries.len(), 3);
    assert_eq!(optimized.plan().functions[0].operations.len(), 6);
    assert_eq!(optimized.unit().functions[0].blocks.len(), 3);
    assert!(matches!(
        optimized.plan().functions[0].operations[2],
        AbstractOperation::BooleanNot { .. }
    ));
    assert!(matches!(
        optimized.plan().functions[0].operations[3],
        AbstractOperation::BooleanNot { .. }
    ));
    assert!(matches!(
        optimized.plan().functions[0].operations[4],
        AbstractOperation::BooleanEqual { .. }
    ));
    assert!(matches!(
        optimized.plan().functions[0].operations[5],
        AbstractOperation::Return { .. }
    ));

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized =
            publish_optimization_run(run(non_adjacent_block_merge_verified(), selections.clone()))
                .unwrap();
        let lowered = lower_optimized_to_target_operations(optimized, target).unwrap();
        assert_eq!(lowered.target(), target);
        assert_eq!(lowered.optimized().commits().len(), 2);
    }
}

#[test]
fn shared_terminal_jump_fusion_replays_to_two_exact_terminal_occurrences() {
    let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
    let optimized =
        publish_optimization_run(run(shared_terminal_jump_verified(), selections)).unwrap();

    assert_eq!(optimized.commits().len(), 2);
    assert_eq!(optimized.plan().functions[0].block_entries.len(), 3);
    assert_eq!(optimized.unit().functions[0].blocks.len(), 3);
    let terminal_source = optimization_unit::PsiProvenance::Edge(EdgeId::new(1_075).unwrap());
    let terminal_nodes = optimized.unit().functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter(|node| node.provenance.contains(&terminal_source))
        .collect::<Vec<_>>();
    assert_eq!(terminal_nodes.len(), 2);
    assert!(
        terminal_nodes
            .iter()
            .all(|node| matches!(node.operation, AbstractOperation::ReturnUnit { .. }))
    );
    let source_site =
        optimization_unit::PsiRealizationSite::Node(optimization_unit::NodeLocation {
            machine: MachineId::new(1_061).unwrap(),
            block: BlockId::new(1_065).unwrap(),
            node: 0,
        });
    assert_eq!(
        optimized.transformation_ledger().records()[0]
            .provenance
            .iter()
            .filter(|row| row.input == source_site)
            .count(),
        2
    );
}
