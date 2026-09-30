//! Case selection consumes its subject; ordinary edges own every other local.
use super::{CLOSED_SUM_UNIT_SOURCE, checked_source_with_core_service, lower_machine, roundtrip};
use crate::TerminalMachineSelection;
use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
use semantic_vocabulary::PlaceId;
use terminal_psi::Terminator;

fn case_return_source(copy: bool) -> String {
    let source = r#"
pub boundary trait Console {
    machine exit_process(value: i32) reaches Console;
}
data Choice { case First; case Second; }
data Output { value: i32; }
machine Output::from_choice(choice: Choice) -> Output {
    transition choice {
        Choice::First -> (Output { value: 70 })
        Choice::Second -> (Output { value: 71 })
    }
}
data Main { console: Binding<Console>; }
machine Main::main(main: &mut Main) reaches Console {
    let output: Output = Output::from_choice(Choice::First);
    main.console.exit_process(output.value);
}
"#;
    if copy {
        source.replace("data Choice {", "data Choice [copy] {")
    } else {
        source.to_owned()
    }
}

#[test]
fn closed_sum_return_cleanup_follows_subject_multiplicity() {
    for copy in [false, true] {
        let source = case_return_source(copy);
        let lowered = roundtrip(&checked_source_with_core_service(&source));
        let (machine, subject, cases) = lowered
            .semantic_module
            .machines
            .iter()
            .find_map(|machine| {
                machine
                    .blocks
                    .iter()
                    .find_map(|block| match &block.terminator {
                        Terminator::StructuralCase { source, cases } => {
                            Some((machine, *source, cases))
                        }
                        _ => None,
                    })
            })
            .expect("the parameter dispatch retains its structural subject");
        assert_eq!(cases.len(), 2);
        for case in cases {
            assert_eq!(
                case.trivial_affine_discards,
                if copy { Vec::new() } else { vec![subject] }
            );
        }
        let mut returns = 0;
        for block in &machine.blocks {
            if let Terminator::ReturnStructural {
                trivial_affine_discards,
                ..
            } = &block.terminator
            {
                returns += 1;
                assert!(
                    !trivial_affine_discards.contains(&subject),
                    "the case edge already settled the affine subject"
                );
            }
        }
        assert_eq!(returns, 2);
        // Independent verification must reject missing or repeated affine
        // cleanup. A copy parameter may legally be discarded in Terminal,
        // but dispatch must still name an established subject.
        for duplicate in [false, true] {
            let mut changed = lowered.semantic_module.clone();
            let changed_machine = changed
                .machines
                .iter_mut()
                .find(|candidate| candidate.id == machine.id)
                .unwrap();
            let cases = changed_machine
                .blocks
                .iter_mut()
                .find_map(|block| match &mut block.terminator {
                    Terminator::StructuralCase { source, cases } => {
                        if copy {
                            *source = PlaceId::new(u64::MAX).unwrap();
                        }
                        Some(cases)
                    }
                    _ => None,
                })
                .unwrap();
            if !copy && duplicate {
                cases[0].trivial_affine_discards.push(subject);
            } else if !copy {
                cases[0].trivial_affine_discards.clear();
            }
            assert!(
                terminal_verifier::verify_module(
                    &changed,
                    &lowered.proof_bundle,
                    &proof_admission::AdmissionProfile::default(),
                )
                .is_err(),
                "copy {copy}, duplicate {duplicate}"
            );
        }
    }
}

#[test]
fn closed_sum_parameter_return_rejects_forged_disposal_evidence() {
    let affine = checked_source_with_core_service(&case_return_source(false));
    roundtrip(&affine);
    let (handle, original) = affine
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find(|(_, event)| {
            event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
                && affine
                    .facts
                    .flow
                    .terminal_unit_effects
                    .composed_machines
                    .iter()
                    .any(|plan| {
                        plan.machine == event.machine_symbol
                            && plan.states.iter().any(|state| {
                                state.state == event.state_symbol
                                    && matches!(state.terminator,
                        checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. })
                            })
                    })
        })
        .map(|(handle, event)| (handle, event.clone()))
        .expect("affine subject exit receipt");
    for mutation in 0..3 {
        let mut changed = affine.clone();
        match mutation {
            0 => {
                changed
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .get_mut(handle)
                    .source = PermissionEventSource::StateEntry
            }
            1 => {
                changed
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .insert(original.clone());
            }
            2 => {
                changed
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .get_mut(handle)
                    .root = facts::PlaceRoot::Unknown
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err(),
            "affine receipt mutation {mutation}"
        );
    }
    let copy = checked_source_with_core_service(&case_return_source(true));
    roundtrip(&copy);
    let plan = copy
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
    let state = plan
        .states
        .iter()
        .find(|state| {
            matches!(
                state.terminator,
                checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
            )
        })
        .unwrap();
    let machine = copy
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
        .unwrap();
    let source_state = copy
        .machine_states(machine)
        .iter()
        .find(|source| source.symbol == state.state)
        .unwrap();
    let mut forged = original;
    forged.machine_symbol = plan.machine;
    forged.state_symbol = state.state;
    forged.root = facts::PlaceRoot::Symbol(copy.state_parameters(source_state)[0].symbol);
    let mut changed = copy.clone();
    changed.facts.flow.ownership.permissions.insert(forged);
    assert!(
        lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err(),
        "a copy subject cannot acquire affine disposal evidence"
    );
    let mut changed = copy.clone();
    let changed_plan = changed
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .find(|candidate| candidate.machine == plan.machine)
        .unwrap();
    let changed_state = changed_plan
        .states
        .iter_mut()
        .find(|candidate| candidate.state == state.state)
        .unwrap();
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, .. } =
        &mut changed_state.terminator
    else {
        unreachable!()
    };
    subject.type_identity.push_str("::forged");
    assert!(
        lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err(),
        "an unrestricted subject still requires its exact type identity"
    );
}

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
