//! Local moves preserve value provenance and declaration-ordered final disposal.

use super::support;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralValue,
};

const SOURCE: &str = r#"
data Limits { value: u64; }
data Value { number: u64; take: bool; }
data Outer { inner: Value; observed: u64; }
machine selected(take: bool, input: u64) -> u64 {
    let keep: Value = Value { number: 17, take: false };
    let first: Value = Value { number: input, take: take };
    let second: Value = first;
    let wrapped: Outer = Outer { observed: second.number, inner: second };
    let last: Outer = wrapped;
    let extra: Value = Value { number: 3, take: false };
    let later: Value = keep;
    transition last.inner.take {
        true -> (last.observed ^ later.number)
        _ -> extra.number
    }
}
"#;

fn integer(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn local_move_chains_preserve_nested_reads_and_dispose_only_final_owners() {
    let two_children = SOURCE
        .replace("inner: Value; observed: u64;", "inner: Value; observed: u64; sibling: Value;")
        .replace("let wrapped: Outer = Outer { observed: second.number, inner: second };",
            "let child: Value = Value { number: 7, take: false }; let wrapped: Outer = Outer { sibling: child, observed: second.number, inner: second };");
    for source in [SOURCE.to_owned(), two_children] {
        for with_parameter in [false, true] {
            let source = if with_parameter {
                source.replace("selected(take:", "selected(limits: Limits, take:")
            } else {
                source.clone()
            };
            let (_, module, bytes, proof) = support::publish(&source, "selected");
            let machine = module
                .machines
                .iter()
                .find(|machine| machine.id == module.entry)
                .unwrap();
            let roots = machine
                .structural_parameters
                .iter()
                .map(|parameter| TerminalStructuralValue {
                    opaque_identity: 71,
                    structural_type: parameter.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                })
                .collect::<Vec<_>>();
            for input in [0, 256, u128::from(u64::MAX)] {
                for (take, expected) in [(true, input ^ 17), (false, 3)] {
                    let mut execution = TerminalExecution::start_artifact_with_structural_arguments_and_scalar_fields(
                    &bytes, &proof, &proof_admission::AdmissionProfile::default(),
                    &[TerminalScalarValue::Boolean(take), integer(input)], &roots, &[],
                ).unwrap();
                    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
                    let mut completed = false;
                    for _ in 0..256 {
                        match execution.resume(&mut meter).unwrap() {
                            TerminalExecutionStatus::Complete(result) => {
                                assert_eq!(
                                    result,
                                    TerminalExecutionResult::Scalar(integer(expected))
                                );
                                completed = true;
                                break;
                            }
                            TerminalExecutionStatus::SponsorExhausted(_) => {
                                meter.replenish(1).unwrap()
                            }
                            other => panic!("unexpected move execution: {other:?}"),
                        }
                    }
                    assert!(completed, "one-unit resumptions complete");
                    assert!(
                        execution.live_affine_frontier().next().is_none(),
                        "no moved source or surviving owner leaks"
                    );
                }
            }
        }
    }
}

#[test]
fn local_move_replay_rejects_changed_receipts_and_source_witnesses() {
    use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
    let (checked, _, _, _) = support::publish(SOURCE, "selected");
    let transfer = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| (event.kind == PermissionEventKind::Transfer).then_some(handle))
        .unwrap();
    for corruption in 0..4 {
        let mut changed = checked.clone();
        let event = changed.facts.flow.ownership.permissions.get_mut(transfer);
        match corruption {
            0 => event.kind = PermissionEventKind::Establish,
            1 => event.provenance = PermissionProvenance::Unknown,
            2 => event.source = PermissionEventSource::Statement { statement_index: 0 },
            _ => event.obligation_live = true,
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "selected").is_err(),
            "receipt corruption {corruption}"
        );
    }
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
    let keep = local("keep");
    let handle = checked.facts.values.structural_values.nodes.iter().find_map(|(handle, node)| {
        match &node.kind {
            checked_trees::CheckedStructuralValueKind::Place(argument)
                if argument.source == checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: first } => Some(handle),
            _ => None,
        }
    }).unwrap();
    let mut changed = checked.clone();
    let checked_trees::CheckedStructuralValueKind::Place(argument) = &mut changed
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(handle)
        .kind
    else {
        panic!("move source");
    };
    argument.source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: keep };
    assert!(
        checked_trees_to_lowered_psi::lower_machine(&changed, "selected").is_err(),
        "same-carrier source substitution"
    );
    let mut stale = checked.clone();
    stale
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(handle)
        .expression = arena::Handle::invalid();
    assert!(
        checked_trees_to_lowered_psi::lower_machine(&stale, "selected").is_err(),
        "stale move expression"
    );
}

#[test]
fn canonical_move_cleanup_rejects_old_homes_and_reordered_disposal() {
    let (_, module, _, _) = support::publish(SOURCE, "selected");
    let machine_position = module
        .machines
        .iter()
        .position(|machine| machine.id == module.entry)
        .unwrap();
    let machine = &module.machines[machine_position];
    let cleanup_position = machine.blocks.iter().position(|block| {
        matches!(&block.terminator, terminal_psi::Terminator::Jump { trivial_affine_discards, .. } if trivial_affine_discards.len() == 3)
    }).unwrap();
    let cleanup = &machine.blocks[cleanup_position];
    let old_home = machine
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            terminal_psi::Terminator::Jump {
                target,
                structural_arguments,
                ..
            } if *target == cleanup.id => Some(structural_arguments[0].place),
            _ => None,
        })
        .unwrap();
    for corruption in 0..3 {
        let mut changed = module.clone();
        let terminal_psi::Terminator::Jump {
            trivial_affine_discards,
            ..
        } = &mut changed.machines[machine_position].blocks[cleanup_position].terminator
        else {
            panic!("cleanup edge");
        };
        match corruption {
            0 => trivial_affine_discards.reverse(),
            1 => trivial_affine_discards[0] = old_home,
            _ => {
                trivial_affine_discards.pop();
            }
        }
        assert!(
            terminal_verifier::validate_module(&changed).is_err(),
            "malformed disposal {corruption} must fail before proof checking"
        );
    }
}
