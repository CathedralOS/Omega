use super::*;

#[test]
fn guarded_division_obligation_retains_its_selected_arm_facts() {
    let checked = checked_source(
        "machine value(denominator: u8) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ transition (1 <= denominator) { true -> (7u8 / denominator) false -> 7 } }",
    );
    let graph = &checked.facts.flow.terminal_scalar_graphs.machines[0];
    let prepared = prepare_scalar_graph_machine(&checked, graph.machine, graph).expect("prepare");
    let lowered = build_scalar_graph_module(
        &prepared.states,
        prepared.result_type,
        prepared.contract_value,
        prepared.result_predicate,
        prepared.crash_routes,
        prepared.identity_reshuffles,
        prepared.partition_compositions,
        machine_id(1),
        0,
        &[(graph.machine, machine_id(1))],
        &[(graph.machine, 1)],
    )
    .expect("module");
    let validated = terminal_verifier::validate_module(&lowered.semantic_module).expect("validate");
    let machine = &lowered.semantic_module.machines[0];
    let entry = machine
        .blocks
        .iter()
        .find(|block| block.id == machine.entry)
        .expect("entry");
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &entry.terminator
    else {
        panic!("selected branch")
    };
    let divisions: Vec<_> = machine
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(move |operation| match operation.kind {
                    OperationKind::ExactIntegerDivide { obligation, .. } => {
                        Some((block.id, obligation))
                    }
                    _ => None,
                })
        })
        .collect();
    assert_eq!(divisions.len(), 1);
    assert_eq!(divisions[0].0, when_true.target);
    assert_ne!(divisions[0].0, entry.id);
    assert_ne!(divisions[0].0, when_false.target);
    let context = validated.value_context(machine).expect("value context");
    let parameters = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect();
    let mut certificates = 0;
    for site in reconstruct_operation_obligations(&lowered.semantic_module).expect("obligations") {
        if site.canonical_certificate {
            certificates += 1;
            assert_eq!(site.obligation.id, divisions[0].1);
            assert!(site.semantic_axioms.contains(&site.obligation.proposition));
            let proof = crate::nonzero_divisor_certificate::produce_checked_canonical_integer_proof(
                &context,
                &site.obligation.proposition,
                &machine.contract.requires,
                &site.semantic_axioms,
                &parameters,
            );
            assert!(proof.is_some(), "{site:#?}");
        }
    }
    assert_eq!(certificates, 1);
}
