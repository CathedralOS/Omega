//! Case selection consumes its subject; ordinary edges own every other local.
use super::{CLOSED_SUM_UNIT_SOURCE, checked_source_with_core_service, lower_machine, roundtrip};
use crate::TerminalMachineSelection;
use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
use terminal_psi::Terminator;

fn source(transfer_before: bool) -> String {
    let source = CLOSED_SUM_UNIT_SOURCE
        .replace("data Main {", "data Scratch { value: i32; } data Main {")
        .replace(
            "let result: ByteRead = self.console.read_byte();",
            "let before: Scratch = Scratch { value: 7 };\n\
             let result: ByteRead = self.console.read_byte();\n\
             let after: Scratch = Scratch { value: 9 };",
        )
        .replace("eof()", "eof(before.value)")
        .replace("state eof(&mut self)", "state eof(&mut self, saved: i32)")
        .replace(
            "self.console.exit_process(70);",
            "self.console.exit_process(saved);",
        );
    if transfer_before {
        source
            .replace("-> byte(value)", "-> byte(value, before, after.value)")
            .replace(
                "state byte(&mut self, value: i32 [0..=255])",
                "state byte(&mut self, value: i32 [0..=255], kept: Scratch, saved: i32)",
            )
            .replace(
                "consume(identity(value));",
                "consume(kept.value); consume(saved);",
            )
    } else {
        source
            .replace("-> byte(value)", "-> byte(value, after.value)")
            .replace(
                "state byte(&mut self, value: i32 [0..=255])",
                "state byte(&mut self, value: i32 [0..=255], saved: i32)",
            )
            .replace("consume(identity(value));", "consume(saved);")
    }
}

#[test]
fn closed_sum_successor_catalogs_reject_duplicate_and_mixed_value_lanes() {
    use checked_trees::{
        CheckedLocatedScalarExpression, CheckedScalarExpression, CheckedScalarExpressionBindings,
        CheckedScalarExpressionRole,
    };

    let checked = checked_source_with_core_service(
        &source(false).replace("-> eof(before.value)", "-> eof(7)"),
    );
    roundtrip(&checked);
    let root = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .find(|root| {
            matches!(
                root.role,
                CheckedScalarExpressionRole::TransitionArgument { .. }
            )
        })
        .unwrap()
        .clone();
    let pure = checked
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .find(|expression| {
            matches!(
                expression.role,
                CheckedScalarExpressionRole::TransitionArgument { .. }
            ) && matches!(
                expression.expression,
                CheckedScalarExpression::IntegerLiteral { .. }
            )
        })
        .unwrap()
        .clone();

    // Two computation rows must not look like absence and expose a pure fallback.
    let mut changed = checked.clone();
    let mut extra = root.clone();
    extra.state = pure.state;
    extra.statement_ordinal = pure.statement_ordinal;
    extra.role = pure.role;
    changed
        .facts
        .values
        .scalar_computations
        .roots
        .insert(extra.clone());
    changed.facts.values.scalar_computations.roots.insert(extra);
    assert!(lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err());

    // An incomplete or duplicated pure roster cannot hide behind a valid root.
    for (binding_count, expression_count) in [(1, 0), (2, 0), (0, 1), (0, 2), (2, 2)] {
        let mut changed = checked.clone();
        for _ in 0..binding_count {
            changed
                .facts
                .values
                .scalar_expressions
                .source_bindings
                .insert(CheckedScalarExpressionBindings {
                    state: root.state,
                    statement_ordinal: root.statement_ordinal,
                    role: root.role,
                    ..Default::default()
                });
        }
        for _ in 0..expression_count {
            changed.facts.values.scalar_expressions.expressions.push(
                CheckedLocatedScalarExpression {
                    state: root.state,
                    statement_ordinal: root.statement_ordinal,
                    role: root.role,
                    expression: pure.expression.clone(),
                },
            );
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err(),
            "binding rows {binding_count}, expression rows {expression_count}"
        );
    }
}

#[test]
fn closed_sum_exit_roster_rejects_unknown_duplicate_and_missing_receipts() {
    let checked = checked_source_with_core_service(&source(true));
    roundtrip(&checked);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| {
            plan.states.iter().any(|state| {
                matches!(
                    state.terminator,
                    checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
        })
        .unwrap();
    let state = &plan.states[0];
    let drops = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == plan.machine
                && event.state_symbol == state.state
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
        })
        .map(|(handle, event)| (handle, event.clone()))
        .collect::<Vec<_>>();
    assert_eq!(drops.len(), 3);
    let source_state = checked.machine_states(
        checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == plan.machine)
            .unwrap(),
    )[0]
    .clone();
    let receiver = checked
        .state_parameters(&source_state)
        .iter()
        .find(|parameter| parameter.is_self)
        .unwrap()
        .symbol;
    for (handle, original) in drops {
        for mutation in 0..6 {
            let mut changed = checked.clone();
            let event = changed.facts.flow.ownership.permissions.get_mut(handle);
            match mutation {
                0 => event.source = PermissionEventSource::StateEntry,
                1 => event.provenance = PermissionProvenance::Unknown,
                2 => {
                    event.segments =
                        arena::HandleSpan::from_parts(arena::Handle::from_arena_index(u32::MAX), 1)
                }
                3 => {
                    changed
                        .facts
                        .flow
                        .ownership
                        .permissions
                        .insert(original.clone());
                }
                4 | 5 => {
                    let mut extra = original.clone();
                    extra.root = if mutation == 4 {
                        facts::PlaceRoot::Unknown
                    } else {
                        facts::PlaceRoot::Symbol(receiver)
                    };
                    changed.facts.flow.ownership.permissions.insert(extra);
                }
                _ => unreachable!(),
            }
            assert!(
                lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err(),
                "receipt {handle:?}, mutation {mutation}"
            );
        }
    }
}

#[test]
fn closed_sum_terminal_cleanup_rejects_missing_reordered_and_repeated_owners() {
    let lowered = roundtrip(&checked_source_with_core_service(&source(false)));
    let entry = lowered.semantic_module.entry;
    for mutation in 0..4 {
        let mut changed = lowered.semantic_module.clone();
        let machine = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == entry)
            .unwrap();
        let subject = machine
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                Terminator::StructuralCase { source, .. } => Some(*source),
                _ => None,
            })
            .unwrap();
        let staged = machine
            .blocks
            .iter_mut()
            .find(|block| matches!(&block.terminator, Terminator::Jump { trivial_affine_discards, .. } if trivial_affine_discards.len() == 2))
            .unwrap();
        let Terminator::Jump {
            trivial_affine_discards,
            ..
        } = &mut staged.terminator
        else {
            unreachable!()
        };
        assert_eq!(trivial_affine_discards.len(), 2);
        match mutation {
            0 => {
                trivial_affine_discards.pop();
            }
            1 => trivial_affine_discards.reverse(),
            2 => trivial_affine_discards.push(subject),
            3 => trivial_affine_discards.push(trivial_affine_discards[0]),
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "Terminal mutation {mutation}"
        );
    }
}
