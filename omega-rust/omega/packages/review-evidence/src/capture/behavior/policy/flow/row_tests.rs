use super::{FlowRows, exactly_one};
use checked_trees::{
    CheckFacts, CheckedMachineResourceEnvelopes, CheckedTrees, MachineContractCommitment,
    MachineServiceReachRows, RealizedMachineContractEnvelope,
};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, machine::Machine, state::State};

fn envelope(machine: SymbolHandle) -> RealizedMachineContractEnvelope {
    let commitment = MachineContractCommitment::from_digest([0; 32]);
    RealizedMachineContractEnvelope {
        machine,
        contract_report_fingerprint: 0,
        contract_commitment: commitment,
        effective_service_reach: Vec::new(),
        concrete_service_reach: Vec::new(),
        unresolved_installation_reaches: Vec::new(),
        effective_synchronous_invocations: Vec::new(),
        checked_may_suspend: false,
        checked_may_block: false,
        checked_termination: Default::default(),
        checked_crash: Default::default(),
        mutation: Vec::new(),
        capabilities: Vec::new(),
        resources: CheckedMachineResourceEnvelopes::from_checked_contract_entries(
            machine,
            0,
            commitment,
            [],
        ),
    }
}

fn program(rows: &[(SymbolHandle, Vec<SymbolHandle>)]) -> CheckedTrees {
    let mut typed = TypedTrees::default();
    let mut facts = CheckFacts::default();
    for (symbol, states) in rows {
        let mut machine = Machine {
            symbol: *symbol,
            ..Default::default()
        };
        for state in states {
            typed.push_machine_state(
                &mut machine,
                State {
                    symbol: *state,
                    ..Default::default()
                },
            );
        }
        typed.push_machine(machine);
        facts
            .contract_plans
            .realized_envelopes
            .push(envelope(*symbol));
        let graph = &mut facts.service_reaches;
        graph.machines.append_to_span(
            &mut graph.root_machines,
            MachineServiceReachRows {
                machine: *symbol,
                ..Default::default()
            },
        );
    }
    CheckedTrees::with_roots(typed, facts)
}

fn selection<'a, T: 'a>(rows: impl Iterator<Item = &'a T>) -> Result<usize, Vec<String>> {
    exactly_one(rows, "literal raw selection", "row")
        .map(|row| std::ptr::from_ref(row) as usize)
        .map_err(|errors| errors.into_iter().map(|error| error.message).collect())
}

fn compare(program: &CheckedTrees, keys: &[SymbolHandle]) {
    let rows = FlowRows::new(program);
    for &symbol in keys {
        assert_eq!(
            selection(rows.machines.get(&symbol).into_iter().flatten().copied()),
            selection(program.machines().iter().filter(|row| row.symbol == symbol)),
        );
        assert_eq!(
            selection(rows.envelopes.get(&symbol).into_iter().flatten().copied()),
            selection(
                program
                    .facts
                    .contract_plans
                    .realized_envelopes
                    .iter()
                    .filter(|row| row.machine == symbol)
            ),
        );
        assert_eq!(
            selection(rows.reaches.get(&symbol).into_iter().flatten().copied()),
            selection(
                program
                    .facts
                    .service_reaches
                    .machines()
                    .iter()
                    .filter(|row| row.machine == symbol)
            ),
        );
        let original: Vec<_> = program
            .machines()
            .iter()
            .filter(|machine| {
                symbol.is_valid()
                    && typed_trees::state::state_by_symbol(program.machine_states(machine), symbol)
                        .is_some()
            })
            .map(std::ptr::from_ref)
            .collect();
        assert_eq!(
            rows.state_owners(symbol)
                .map(std::ptr::from_ref)
                .collect::<Vec<_>>(),
            original
        );
    }
}

#[test]
fn capability_rows_match_original_searches_with_duplicates_generations_and_replacement() {
    let a = SymbolHandle::from_parts(1, 1);
    let b = SymbolHandle::from_parts(1, 2);
    let c = SymbolHandle::from_parts(2, 1);
    let s = SymbolHandle::from_parts(3, 1);
    let newer = SymbolHandle::from_parts(3, 2);
    let missing = SymbolHandle::from_parts(4, 1);
    let invalid = SymbolHandle::invalid();
    let keys = [a, b, c, s, newer, missing, invalid];
    compare(&CheckedTrees::default(), &keys);
    let mut input = program(&[
        (a, vec![s, s]),
        (b, vec![s]),
        (a, vec![newer]),
        (invalid, vec![invalid]),
    ]);
    compare(&input, &keys);
    let rows = FlowRows::new(&input);
    // Repeated states in one machine count once; duplicate machine IDs do not
    // collapse distinct owner rows. Invalid target states never gain an owner.
    assert_eq!(rows.state_owners(s).count(), 2);
    assert_eq!(rows.machines.get(&a).unwrap().len(), 2);
    assert_eq!(rows.state_owners(invalid).count(), 0);
    drop(rows);
    let retained = input.clone();
    // A fresh request sees arbitrary whole-table replacement, even when its
    // dimensions and local mutation counts equal those of a retained input.
    input = program(&[
        (c, vec![newer, newer]),
        (a, vec![s]),
        (c, vec![s]),
        (invalid, vec![invalid]),
    ]);
    compare(&input, &keys);
    compare(&retained, &keys);
    // Independent missing/duplicate envelope and reach rosters preserve the
    // same exactly_one diagnostic, independently of machine row availability.
    input.facts.contract_plans.realized_envelopes.clear();
    input.facts.service_reaches.root_machines = Default::default();
    compare(&input, &keys);
}
