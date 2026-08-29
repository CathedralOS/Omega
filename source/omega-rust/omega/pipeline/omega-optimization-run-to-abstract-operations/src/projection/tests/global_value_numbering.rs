use super::*;

#[test]
fn global_value_numbering_projects_local_cse_and_return_substitution() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let optimized = project_optimization_run(run(local_cse_verified(), selections)).unwrap();
    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.plan().functions[0].operations.len(), 2);
    assert!(
        matches!(optimized.plan().functions[0].operations[0], AbstractOperation::IntegerBitwiseNot { result, .. } if result == ValueId::new(1_324).unwrap())
    );
    assert!(
        matches!(optimized.plan().functions[0].operations[1], AbstractOperation::Return { value, .. } if value == ValueId::new(1_324).unwrap())
    );
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
    assert_eq!(
        optimized.transformation_ledger().records()[0]
            .provenance
            .len(),
        2
    );
}

#[test]
fn global_value_numbering_projects_proof_certified_cse_with_exact_fact_custody() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let optimized =
        project_optimization_run(run(proof_certified_local_cse_verified(), selections)).unwrap();
    let leader = ValueId::new(1_385).unwrap();
    let redundant_operation = OperationId::new(1_389).unwrap();
    let redundant_fact = optimized
        .unit()
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == redundant_operation)
        .unwrap()
        .identity;

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.plan().functions[0].operations.len(), 4);
    assert!(
        matches!(optimized.plan().functions[0].operations[2], AbstractOperation::ExactIntegerAdd { result, .. } if result == leader)
    );
    assert!(
        matches!(optimized.plan().functions[0].operations[3], AbstractOperation::Return { value, .. } if value == leader)
    );
    assert_eq!(
        optimized.unit().functions[0]
            .facts
            .iter()
            .filter(|fact| matches!(
                fact,
                omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
            ))
            .count(),
        1
    );
    assert_eq!(optimized.unit().accepted_obligation_facts.len(), 2);
    assert_eq!(optimized.pass_manifests().len(), 1);
    assert_eq!(
        optimized.pass_manifests()[0].decisions()[0].consumed_facts(),
        &[omega_optimization_core::OptimizationFactReference::AcceptedObligation(redundant_fact)]
    );
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
}

#[test]
fn compatible_policy_gvn_projects_and_lowers_with_exact_fact_custody() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let optimized =
            project_optimization_run(run(compatible_policy_local_cse_verified(), selections))
                .unwrap();
        let leader = ValueId::new(1_405).unwrap();
        let redundant_operation = OperationId::new(1_409).unwrap();
        let redundant_fact = optimized
            .unit()
            .accepted_obligation_facts
            .iter()
            .find(|fact| fact.operation == redundant_operation)
            .unwrap()
            .identity;

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.plan().functions[0].operations.len(), 4);
        assert!(matches!(
            optimized.plan().functions[0].operations[2],
            AbstractOperation::SaturatingIntegerAdd { result, .. } if result == leader
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[3],
            AbstractOperation::Return { value, .. } if value == leader
        ));
        assert_eq!(
            optimized.unit().functions[0]
                .facts
                .iter()
                .filter(|fact| matches!(
                    fact,
                    omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
                ))
                .count(),
            0
        );
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        assert_eq!(
            optimized.pass_manifests()[0].decisions()[0].consumed_facts(),
            &[
                omega_optimization_core::OptimizationFactReference::AcceptedObligation(
                    redundant_fact
                )
            ]
        );
        let lowered = lower_optimized_to_target_operations(optimized, target).unwrap();
        assert_eq!(lowered.target(), target);
        assert_eq!(lowered.target_operations().functions.len(), 1);
    }
}

#[test]
fn global_value_numbering_projects_a_non_roster_order_dominating_leader() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let optimized = project_optimization_run(run(dominator_gvn_verified(), selections)).unwrap();
    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(
        optimized.plan().functions[0].block_entries[0].block,
        BlockId::new(1_362).unwrap()
    );
    assert!(
        matches!(optimized.plan().functions[0].operations[0], AbstractOperation::Return { value, .. } if value == ValueId::new(1_365).unwrap())
    );
    assert!(
        matches!(optimized.plan().functions[0].operations[1], AbstractOperation::IntegerBitwiseNot { result, .. } if result == ValueId::new(1_365).unwrap())
    );
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
    assert!(
        optimized.transformation_ledger().records()[0]
            .provenance
            .iter()
            .all(|row| row.disposition.is_realized())
    );
}

#[test]
fn global_value_numbering_projects_phi_translated_join_bindings() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let optimized =
        project_optimization_run(run(phi_translated_gvn_verified(), selections)).unwrap();
    let join = BlockId::new(1_452).unwrap();
    let redundant = ValueId::new(1_462).unwrap();
    let function = &optimized.unit().functions[0];
    let join_block = function
        .blocks
        .iter()
        .find(|block| block.id == join)
        .unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(join_block.parameters.len(), 2);
    assert_eq!(join_block.parameters[1].value, redundant);
    assert_eq!(join_block.nodes.len(), 1);
    assert!(
        matches!(join_block.nodes[0].operation, AbstractOperation::Return { value, .. } if value == redundant)
    );
    let mut supplied = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == join)
        .map(|edge| edge.bindings[1].argument)
        .collect::<Vec<_>>();
    supplied.sort();
    assert_eq!(
        supplied,
        vec![ValueId::new(1_460).unwrap(), ValueId::new(1_461).unwrap()]
    );
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
}

#[test]
fn global_value_numbering_projects_proof_certified_phi_custody() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let optimized = project_optimization_run(run(
        proof_certified_phi_translated_gvn_verified(),
        selections,
    ))
    .unwrap();
    let join = BlockId::new(1_452).unwrap();
    let redundant = ValueId::new(1_462).unwrap();
    let redundant_fact = optimized
        .unit()
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == OperationId::new(1_463).unwrap())
        .unwrap()
        .identity;
    let function = &optimized.unit().functions[0];
    let join_block = function
        .blocks
        .iter()
        .find(|block| block.id == join)
        .unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(join_block.parameters[1].value, redundant);
    assert_eq!(join_block.nodes.len(), 1);
    assert_eq!(optimized.unit().accepted_obligation_facts.len(), 3);
    assert_eq!(
        function
            .facts
            .iter()
            .filter(|fact| matches!(
                fact,
                omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
            ))
            .count(),
        2
    );
    assert_eq!(
        optimized.pass_manifests()[0].decisions()[0].consumed_facts(),
        &[omega_optimization_core::OptimizationFactReference::AcceptedObligation(redundant_fact,)]
    );
    let supplied = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == join)
        .map(|edge| edge.bindings[1].argument)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        supplied,
        BTreeSet::from([ValueId::new(1_460).unwrap(), ValueId::new(1_461).unwrap(),])
    );
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
}

#[test]
fn compatible_policy_phi_gvn_and_wrapping_shift_identities_project_and_lower() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized = project_optimization_run(run(
            compatible_policy_phi_translated_gvn_verified(),
            selections.clone(),
        ))
        .unwrap();
        let join = BlockId::new(1_452).unwrap();
        let redundant = ValueId::new(1_462).unwrap();
        let redundant_operation = OperationId::new(1_463).unwrap();
        let function = &optimized.unit().functions[0];
        let join_block = function
            .blocks
            .iter()
            .find(|block| block.id == join)
            .unwrap();
        // The compatible-policy phi rewrite is followed by one exact
        // zero-count shift elimination on each predecessor.
        assert_eq!(optimized.commits().len(), 3);
        assert_eq!(join_block.parameters[1].value, redundant);
        assert_eq!(join_block.nodes.len(), 1);
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        assert!(function.facts.iter().all(|fact| {
                !matches!(fact, omega_optimization_unit::OptimizationFact::OperationObligationReference { support, .. }
                    if *support == redundant_operation)
            }));
        assert_eq!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.successors)
                .filter(|edge| edge.target == join)
                .map(|edge| edge.bindings[1].argument)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([ValueId::new(1_457).unwrap(), ValueId::new(1_458).unwrap(),])
        );
        let lowered = lower_optimized_to_target_operations(optimized, target).unwrap();
        assert_eq!(lowered.target(), target);
        assert_eq!(
            lowered.optimized().transformation_ledger().records().len(),
            3
        );
    }
}
