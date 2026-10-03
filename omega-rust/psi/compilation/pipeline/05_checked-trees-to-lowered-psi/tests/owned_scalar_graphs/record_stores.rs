//! Local stores compose with copying, scalar snapshots and selected exits.

use super::support;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
};

const SOURCE: &str = r#"
data Value [copy] { number: u64; enabled: bool; }
data Outer [copy] { left: Value; right: Value; }
machine identity(input: u64) -> u64 { input }
machine changed(input: u64, take: bool) -> u64 {
    let mut first: Outer = Outer {
        left: Value { number: 17, enabled: false },
        right: Value { number: 256, enabled: true }
    };
    let snapshot: Outer = first;
    first.right.number = input;
    first.left.enabled = take;
    let mut second: Outer = first;
    second.right.number = 7;
    let observed: u64 = first.right.number;
    first.right.number = 3;
    transition first.left.enabled {
        true -> done(observed ^ snapshot.right.number ^ second.right.number)
        _ -> done(first.right.number)
    }
    state done(value: u64) { value }
}
"#;

fn integer(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn restricted_local_stores_prove_each_replacement() {
    for (declaration, field_type, invalid_value) in [
        (
            "domain u64::Allowed(value) requires value != 5;",
            "u64 in Allowed",
            5,
        ),
        ("", "u64 [0..=10]", 15),
    ] {
        let source = format!(
            "{declaration}
             data Value [copy] {{ number: {field_type}; }}
             machine changed() -> u64 {{
                 let mut first: Value = Value {{ number: 7 }};
                 first.number = 8;
                 first.number
             }}"
        );
        let (_, module, bytes, proof) = support::publish(&source, "changed");
        assert!(
            module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    operation.kind,
                    terminal_psi::OperationKind::StructuralScalarFieldStore {
                        range_obligation: Some(_),
                        ..
                    }
                ))
        );
        let mut execution = TerminalExecution::start_artifact(
            &bytes,
            &proof,
            &proof_admission::AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs::default(),
        )
        .unwrap();
        let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(10000);
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(integer(8)))
        );
        let invalid = source.replace(
            "first.number = 8",
            &format!("first.number = {invalid_value}"),
        );
        if let Ok(checked) = crate::front_end::checked_program_result(&invalid) {
            assert!(
                terminal_production::TerminalProductionRequest::new(
                    &checked,
                    TerminalMachineSelection::Name("changed"),
                )
                .produce(
                    terminal_production::TerminalProductionCustody::artifact_only(
                        &mut terminal_production::TerminalProductionTimings::default(),
                    )
                )
                .is_err(),
                "replacement must establish the declared restriction"
            );
        }
    }
}

#[test]
fn local_stores_preserve_record_copies_and_captured_scalar_values() {
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace("= input;", "= identity(input);"),
        SOURCE.replace("let mut ", "let "),
    ] {
        let (_, _, bytes, proof) = support::publish(&source, "changed");
        for input in [0, 256, u128::from(u64::MAX)] {
            for (take, expected) in [(true, input ^ 256 ^ 7), (false, 3)] {
                let mut execution = TerminalExecution::start_artifact(
                    &bytes,
                    &proof,
                    &proof_admission::AdmissionProfile::default(),
                    &[integer(input), TerminalScalarValue::Boolean(take)],
                    TerminalStructuralInputs::default(),
                )
                .unwrap();
                let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
                let mut completed = false;
                for _ in 0..256 {
                    match execution
                        .resume(&mut meter, &mut AcceptTerminalEffects)
                        .unwrap()
                    {
                        TerminalExecutionStatus::Complete(result) => {
                            assert_eq!(result, TerminalExecutionResult::Scalar(integer(expected)));
                            completed = true;
                            break;
                        }
                        TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
                        other => panic!("unexpected local store execution: {other:?}"),
                    }
                }
                assert!(completed);
                assert!(execution.live_affine_frontier().next().is_none());
            }
        }
    }
}

#[test]
fn store_cannot_resurrect_a_moved_local_from_its_declaration() {
    for binding in ["let", "let mut"] {
        let source = format!(
            "data Value {{ number: u64; }}
             machine changed() -> u64 {{
                 {binding} first: Value = Value {{ number: 256 }};
                 let second: Value = first;
                 first.number = 5;
                 second.number
             }}"
        );
        // Stage 04 replays the transfer before the store, so the moved local
        // never reaches lowering: a store strictly inside a consumed owner
        // rejects, and only a whole assignment re-establishes it.
        let diagnostics = crate::front_end::checked_program_result(&source)
            .expect_err("moved source cannot regain its old home");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.message
                == "cannot store into `first.number` in machine `changed` state `entry` \
                    (statement 2) after `first` was transferred or consumed at statement 1: \
                    assign a whole value to `first` to re-establish it"),
            "{binding}: {diagnostics:?}"
        );
    }
}

#[test]
fn store_replay_rejects_substituted_local_and_field_custody() {
    use checked_trees::{CheckedStoreRoot, CheckedUnitEffectOperationPlan};
    let (checked, _, _, _) = support::publish(SOURCE, "changed");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "changed")
        .unwrap()
        .symbol;
    let graph_position = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .position(|graph| graph.machine == machine)
        .unwrap();
    let graph = &checked.facts.flow.terminal_scalar_graphs.machines[graph_position];
    let position = graph.states[0]
        .unit_operations
        .iter()
        .position(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            )
        })
        .unwrap();
    let (_, state) = checked
        .machines()
        .iter()
        .find_map(|owner| {
            checked
                .machine_states(owner)
                .iter()
                .find(|state| state.symbol == graph.states[0].state)
                .map(|state| (owner, state))
        })
        .unwrap();
    let snapshot = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "snapshot" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .unwrap();
    for corruption in 0..4 {
        let mut changed = checked.clone();
        let operations = &mut changed.facts.flow.terminal_scalar_graphs.machines[graph_position]
            .states[0]
            .unit_operations;
        let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) =
            &mut operations[position]
        else {
            panic!("store");
        };
        match corruption {
            0 => store.destination = CheckedStoreRoot::Local { symbol: snapshot },
            1 => store.field_identity = "enabled".into(),
            2 => store.carrier_path.clear(),
            _ => {
                operations.remove(position);
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("changed")
            )
            .is_err(),
            "store substitution {corruption}"
        );
    }
}
