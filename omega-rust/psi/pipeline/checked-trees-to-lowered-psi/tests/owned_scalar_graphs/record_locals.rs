//! Local records stay alive until the selected transition completes its reads.

use super::support;
use checked_trees::CheckedUnitEffectOperationPlan;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};

const SOURCE: &str = r#"
data Value { value: u64; flag: bool; }
data Outer { inner: Value; }
machine adjust(value: u64) -> u64 { value ^ 17 }
machine selected(take: bool, input: u64) -> u64 {
    let before: u64 = adjust(input);
    let first: Outer = Outer { inner: Value { flag: take, value: adjust(before) } };
    let second: Outer = Outer { inner: Value { value: 3, flag: false } };
    let after: u64 = first.inner.value;
    transition first.inner.flag {
        true -> yes(adjust(first.inner.value), after)
        _ -> no(second.inner.value)
    }
    state yes(value: u64, snapshot: u64) { value ^ snapshot }
    state no(value: u64) { value }
}
"#;

fn integer(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

const COPY_SOURCE: &str = r#"
data Value [copy] { number: u64; take: bool; }
data Outer [copy] { inner: Value; }
machine selected(take: bool, input: u64) -> u64 {
    let before: u64 = input ^ 17;
    let first: Value = Value { take: take, number: before };
    let other: Value = Value { number: 3, take: false };
    let nested: Outer = Outer { inner: first };
    let copied: Outer = nested;
    let last: Outer = copied;
    let after: u64 = last.inner.number ^ 17;
    transition last.inner.take {
        true -> yes(after, other.number)
        _ -> no(other.number)
    }
    state yes(value: u64, snapshot: u64) { value ^ snapshot }
    state no(value: u64) { value }
}
"#;

#[test]
fn record_copies_bind_distinct_homes_before_nested_transition_reads() {
    let (_, module, bytes, proof) = support::publish(COPY_SOURCE, "selected");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let mut copies = 0;
    for block in &machine.blocks {
        let terminal_psi::Terminator::Jump {
            target,
            structural_arguments,
            ..
        } = &block.terminator
        else {
            continue;
        };
        let destination = machine
            .blocks
            .iter()
            .find(|block| block.id == *target)
            .unwrap();
        for (source, destination) in structural_arguments
            .iter()
            .zip(&destination.structural_parameters)
        {
            assert_ne!(
                source.place, destination.place,
                "copy cannot alias the source home"
            );
            assert_eq!(source.access, terminal_psi::StructuralAccess::Owned);
            assert_eq!(
                destination.multiplicity,
                terminal_psi::StructuralMultiplicity::Unrestricted
            );
            copies += 1;
        }
    }
    assert_eq!(copies, 3, "nested field and two whole-local copies");
    for input in [0, 256, u128::from(u64::MAX)] {
        for (take, expected) in [(true, input ^ 3), (false, 3)] {
            let result = terminal_interpreter::interpret_terminal_artifact_measured(
                &bytes,
                &proof,
                &proof_admission::AdmissionProfile::default(),
                &[TerminalScalarValue::Boolean(take), integer(input)],
            )
            .unwrap();
            assert_eq!(
                result.value(),
                TerminalExecutionResult::Scalar(integer(expected))
            );
        }
    }
}

#[test]
fn record_copy_replay_rejects_same_carrier_source_and_access_substitution() {
    let (checked, _, _, _) = support::publish(COPY_SOURCE, "selected");
    let local = |name: &str| {
        checked
            .machines()
            .iter()
            .flat_map(|machine| checked.machine_states(machine))
            .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
            .find_map(|statement| match statement {
                checked_trees::statement::StatementNode::LocalData(local)
                    if local.name.as_str() == name =>
                {
                    Some(local.symbol)
                }
                _ => None,
            })
            .unwrap()
    };
    let first = local("first");
    let other = local("other");
    let handle = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .find_map(|(handle, node)| match &node.kind {
            checked_trees::CheckedStructuralValueKind::Place(argument)
                if argument.source == checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: first } => Some(handle),
            _ => None,
        })
        .unwrap();
    for corruption in 0..3 {
        let mut forged = checked.clone();
        let checked_trees::CheckedStructuralValueKind::Place(argument) = &mut forged
            .facts
            .values
            .structural_values
            .nodes
            .get_mut(handle)
            .kind
        else {
            panic!("whole-place copy");
        };
        match corruption {
            0 => {
                argument.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol: other,
                    }
            }
            1 => argument.access = checked_trees::CheckedStructuralAccess::SharedBorrow,
            _ => argument.type_identity.push_str("substituted"),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&forged, "selected").is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn record_locals_complete_fields_and_selected_arguments_before_cleanup() {
    let (_, _, bytes, proof) = support::publish(SOURCE, "selected");
    for input in [0, 256, u128::from(u64::MAX)] {
        for (take, expected) in [(true, 17), (false, 3)] {
            let result = terminal_interpreter::interpret_terminal_artifact_measured(
                &bytes,
                &proof,
                &proof_admission::AdmissionProfile::default(),
                &[TerminalScalarValue::Boolean(take), integer(input)],
            )
            .unwrap();
            assert_eq!(
                result.value(),
                TerminalExecutionResult::Scalar(integer(expected))
            );
        }
    }
}

#[test]
fn record_local_graph_replay_rejects_missing_moved_and_substituted_producers() {
    let (checked, _, _, _) = support::publish(SOURCE, "selected");
    let graph_position = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .position(|graph| {
            graph
                .states
                .iter()
                .any(|state| state.unit_operations.len() == 2)
        })
        .unwrap();
    for corruption in 0..3 {
        let mut forged = checked.clone();
        let operations = &mut forged.facts.flow.terminal_scalar_graphs.machines[graph_position]
            .states[0]
            .unit_operations;
        match corruption {
            0 => {
                operations.remove(0);
            }
            1 => {
                operations.swap(0, 1);
            }
            _ => {
                let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    value: replacement,
                    ..
                } = operations[1]
                else {
                    panic!("record producer");
                };
                let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } =
                    &mut operations[0]
                else {
                    panic!("record producer");
                };
                *value = replacement;
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&forged, "selected").is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn record_local_graph_replay_rejects_missing_affine_drop() {
    use language_semantics::{PermissionEventKind, PermissionEventSource};
    let (mut checked, _, _, _) = support::publish(SOURCE, "selected");
    let drop = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.kind == PermissionEventKind::AffineDrop
                && event.source == PermissionEventSource::StateExit)
                .then_some(handle)
        })
        .unwrap();
    checked.facts.flow.ownership.permissions.get_mut(drop).kind = PermissionEventKind::Transfer;
    assert!(checked_trees_to_lowered_psi::lower_machine(&checked, "selected").is_err());
}

#[test]
fn record_locals_preserve_effectful_field_order_and_scalar_returns() {
    let source = r#"
        data Pair { first: u64; second: u64; }
        machine stamp(destination: &mut u64, value: u64) -> u64 { destination = value; value }
        machine direct() -> u64 {
            let mut scratch: u64 = 0;
            let pair: Pair = Pair { second: stamp(&mut scratch, 1), first: stamp(&mut scratch, 2) };
            pair.first ^ pair.second ^ scratch
        }
        machine branch(take: bool) -> u64 {
            let pair: Pair = Pair { second: 1, first: 2 };
            transition take { true -> pair.first _ -> pair.second }
        }
    "#;
    for (entry, arguments, expected) in [
        ("direct", vec![], 1),
        ("branch", vec![TerminalScalarValue::Boolean(true)], 2),
        ("branch", vec![TerminalScalarValue::Boolean(false)], 1),
    ] {
        let (_, _, bytes, proof) = support::publish(source, entry);
        let result = terminal_interpreter::interpret_terminal_artifact_measured(
            &bytes,
            &proof,
            &proof_admission::AdmissionProfile::default(),
            &arguments,
        )
        .unwrap();
        assert_eq!(
            result.value(),
            TerminalExecutionResult::Scalar(integer(expected)),
            "{entry}"
        );
    }
}

#[test]
fn record_local_transition_does_not_execute_the_unselected_argument() {
    let source = r#"
        data Value { number: u64; flag: bool; }
        machine fail() -> u64 crashes Abort { crash Abort; }
        machine selected(take: bool) -> u64 crashes Abort {
            let value: Value = Value { number: 256, flag: take };
            transition value.flag { true -> yes(fail()) _ -> no(value.number) }
            state yes(number: u64) { number }
            state no(number: u64) { number }
        }
    "#;
    let (_, _, bytes, proof) = support::publish(source, "selected");
    let result = terminal_interpreter::interpret_terminal_artifact_measured(
        &bytes,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(false)],
    )
    .unwrap();
    assert_eq!(
        result.value(),
        TerminalExecutionResult::Scalar(integer(256))
    );
}

#[test]
fn record_locals_separate_owned_parameters_and_retain_the_cycle_boundary() {
    let source = r#"
        data Limits { value: u64; }
        data Local { value: u64; }
        machine finish(limits: Limits, marker: u64) -> u64 {
            let local: Local = Local { value: marker };
            local.value
        }
        machine walk(limits: Limits, remaining: u64[0..=3], marker: u64)
        terminates by remaining -> Nat::Descending in 0..4;
        -> u64 {
            let local: Local = Local { value: marker };
            transition remaining > 0 {
                true -> walk(limits, remaining - 1, local.value)
                false -> local.value
            }
        }
    "#;
    let checked = support::check(source);
    assert!(
        matches!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "walk"),
            Err(
                checked_trees_to_lowered_psi::LoweringError::InvalidTerminalModule(
                    terminal_verifier::ModuleError::ControlCycle(_)
                )
            )
        ),
        "fresh record cycle lifetime remains explicitly unadmitted"
    );
    let (_, module, bytes, proof) = support::publish(source, "finish");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let root = terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 71,
        structural_type: machine.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments_and_scalar_fields(&bytes, &proof, &proof_admission::AdmissionProfile::default(), &[integer(17)], &[root], &[]).unwrap();
    let result = execution
        .resume(&mut terminal_fuel::TerminalFuelMeter::unbounded())
        .unwrap();
    assert_eq!(
        result,
        terminal_interpreter::TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            integer(17)
        ))
    );
    assert!(execution.live_affine_frontier().next().is_none());
}
