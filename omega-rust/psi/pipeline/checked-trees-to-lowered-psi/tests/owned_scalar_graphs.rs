//! Owned inputs retain authored identity through scalar graph publication.

#[path = "owned_scalar_graphs/integer_inputs.rs"]
mod integer_inputs;
#[path = "owned_scalar_graphs/permissions.rs"]
mod permissions;
#[path = "owned_scalar_graphs/selected_owned.rs"]
mod selected_owned;
#[path = "owned_scalar_graphs/source_custody.rs"]
mod source_custody;
#[path = "owned_scalar_graphs/support.rs"]
mod support;
#[path = "owned_scalar_graphs/type_custody.rs"]
mod type_custody;

use terminal_psi::{
    OperationKind, StructuralAccess, StructuralMultiplicity, TerminalAffineCleanupAction,
    TerminalMachineResult, Terminator,
};

const LIMITS: &str = r#"
data Limits { limit: u64; divisor: u64 [3..=5]; }
machine reset(value: &mut u64) -> u64 { value = 0; 0 }
machine inspect(marker: u64, limits: Limits) -> u64 {
    let mut scratch: u64 = marker;
    let reset_result: u64 = reset(&mut scratch);
    limits.limit
}
machine enter(limits: Limits, marker: u64) -> u64 {
    let inspected: u64 = inspect(marker, limits);
    inspected
}
"#;

// Nested operands intentionally observe an input after an earlier owned actual.
// Explicit copy semantics keep this order legal; Limits remains ordinary affine data.
const ORDERED: &str = r#"
data Flags [copy] { value: bool; spare: bool; }
machine stamp(value: &mut bool, number: bool) -> bool { value = number; number }
machine choose(before: bool, left: Flags, after: bool, right: Flags) -> bool {
    let selected: bool = before;
    selected
}
machine inspect(marker: bool, left: Flags, other: bool, right: Flags) -> bool {
    let mut scratch: bool = marker;
    let answer: bool = choose(stamp(&mut scratch, left.value), right,
                              stamp(&mut scratch, right.value), left);
    let snapshot: bool = scratch;
    scratch = answer;
    snapshot
}
machine enter(left: Flags, marker: bool, right: Flags, other: bool) -> bool {
    let inspected: bool = inspect(marker, right, other, left);
    inspected
}
"#;

#[test]
fn source_debug_follows_selected_entry_after_ordered_helpers() {
    let source = "machine answer(row: [u8; 2], value: u8) -> u8 { value }
        machine helper(value: u8) -> u8 {
            let row: [u8; 2] = [7u8, 9u8];
            answer(row, value)
        }
        machine selected(input: u8) -> u8 { helper(input) }";
    let (checked, _, _, _) = support::publish(source, "selected");
    let selected = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "selected")
        .unwrap();
    let parameter = &checked.state_parameters(&checked.machine_states(selected)[0])[0];
    let machine_span = checked
        .typed
        .symbols
        .symbol_source_span(selected.symbol)
        .unwrap();
    let parameter_span = checked
        .typed
        .symbols
        .symbol_source_span(parameter.symbol)
        .unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "selected").unwrap();
    let module = &lowered.semantic_module;
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_ne!(
        module.machines[0].id, entry.id,
        "ordered helper precedes selected entry"
    );
    let debug = lowered.debug_map.as_ref().unwrap();
    for (subject, expected) in [
        (terminal_psi::DebugSubject::Machine(entry.id), machine_span),
        (
            terminal_psi::DebugSubject::Value(entry.parameters[0].id),
            parameter_span,
        ),
        (
            terminal_psi::DebugSubject::Value(entry.result.scalar().unwrap().id),
            machine_span,
        ),
    ] {
        let site = debug
            .sites
            .iter()
            .find(|site| site.subject == subject)
            .unwrap_or_else(|| panic!("selected source subject missing: {subject:?}"));
        assert_eq!(site.span.start, expected.span.start as u64, "{subject:?}");
        assert_eq!(site.span.end, expected.span.end as u64, "{subject:?}");
    }
    assert!(
        !debug
            .sites
            .iter()
            .any(|site| site.subject == terminal_psi::DebugSubject::Machine(module.machines[0].id)),
        "selected entry source must not be attributed to its helper"
    );
}

#[test]
fn source_debug_parameters_follow_scalar_positions_among_owned_inputs() {
    for entry in ["inspect", "enter"] {
        let (checked, _, _, _) = support::publish(ORDERED, entry);
        let selection =
            checked_trees_to_lowered_psi::select_terminal_machine(&checked, entry).unwrap();
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(selection.machine)
            .unwrap();
        let source_machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == selection.machine)
            .unwrap();
        let source_parameters =
            checked.state_parameters(&checked.machine_states(source_machine)[0]);
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry).unwrap();
        let root = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        let debug = lowered.debug_map.unwrap();
        assert_eq!(
            root.parameters.len(),
            graph.states[0].scalar_parameters.len()
        );
        for (parameter, retained) in root
            .parameters
            .iter()
            .zip(&graph.states[0].scalar_parameters)
        {
            let source = &source_parameters[retained.source_position as usize];
            let expected = checked
                .typed
                .symbols
                .symbol_source_span(source.symbol)
                .unwrap();
            let site = debug
                .sites
                .iter()
                .find(|site| site.subject == terminal_psi::DebugSubject::Value(parameter.id))
                .unwrap();
            assert_eq!(
                site.span.start, expected.span.start as u64,
                "{entry}: {}",
                source.name
            );
            assert_eq!(
                site.span.end, expected.span.end as u64,
                "{entry}: {}",
                source.name
            );
        }
    }
}

#[test]
fn limits_root_and_forwarding_caller_publish_integer_field_and_local_mutation() {
    for (entry, machines) in [("inspect", 2), ("enter", 3)] {
        let (_, module, _, _) = support::publish(LIMITS, entry);
        assert_eq!(module.machines.len(), machines);
        let root = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        assert_eq!(root.parameters.len(), 1);
        assert_eq!(root.structural_parameters.len(), 1);
        assert_eq!(
            root.structural_parameters[0].access,
            StructuralAccess::Owned
        );
        assert_eq!(
            root.structural_parameters[0].multiplicity,
            StructuralMultiplicity::Affine
        );
        assert!(root.entry_claims.is_empty());
        assert!(
            module
                .machines
                .iter()
                .all(|machine| matches!(machine.result, TerminalMachineResult::Scalar(_)))
        );
        let fields = module
            .machines
            .iter()
            .flat_map(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(move |operation| {
                        if let OperationKind::IntegerStructuralField { source, field, .. } =
                            operation.kind
                        {
                            Some((machine, source, field))
                        } else {
                            None
                        }
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(fields.len(), 1);
        let (owner, source, field) = fields[0];
        assert_eq!(source, owner.structural_parameters[0].place);
        assert_eq!(
            owner.structural_parameters[0].multiplicity,
            StructuralMultiplicity::Affine
        );
        assert!(owner.entry_claims.is_empty());
        assert!(owner.structural_parameters[0].qualifications.is_empty());
        assert!(
            owner.structural_parameters[0]
                .projected_qualifications
                .is_empty()
        );
        let returns = owner
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator {
                Terminator::Return {
                    cleanup_actions, ..
                } => Some(cleanup_actions),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(returns.len(), 1, "inspect has one normal return");
        assert_eq!(
            returns[0].as_slice(),
            &[TerminalAffineCleanupAction::DiscardRoot(source)],
            "inspect discards the observed affine Limits exactly once"
        );
        assert_eq!(
            field,
            support::field(
                &module,
                owner.structural_parameters[0].structural_type,
                "limit"
            )
        );
        let operations = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(
                    operation.kind,
                    OperationKind::EstablishPrimitiveLocal { .. }
                ))
                .count(),
            1
        );
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(
                    operation.kind,
                    OperationKind::WriteOnlyPrimitiveStore { .. }
                ))
                .count(),
            1
        );
        if entry == "enter" {
            let returns = root
                .blocks
                .iter()
                .filter_map(|block| match &block.terminator {
                    Terminator::Return {
                        cleanup_actions, ..
                    } => Some(cleanup_actions),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(returns.len(), 1);
            assert!(
                returns[0].is_empty(),
                "forwarded Limits leaves no caller disposal"
            );
            let call = root
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| match &operation.kind {
                    OperationKind::CallStructuralScalar {
                        arguments,
                        structural_arguments,
                        ..
                    } => Some((arguments, structural_arguments)),
                    _ => None,
                })
                .expect("owned Limits forwarding call");
            assert_eq!(call.0.len(), 1);
            let mut argument = call.0[0];
            let mut staged = Vec::new();
            while !root
                .parameters
                .iter()
                .any(|parameter| parameter.id == argument)
            {
                assert!(!staged.contains(&argument), "acyclic argument staging");
                staged.push(argument);
                let destinations = root
                    .blocks
                    .iter()
                    .filter_map(|block| {
                        block
                            .parameters
                            .iter()
                            .position(|parameter| parameter.id == argument)
                            .map(|position| (block.id, position))
                    })
                    .collect::<Vec<_>>();
                let [(destination, position)] = destinations.as_slice() else {
                    panic!(
                        "forwarded marker must be an entry value or one exact block parameter: {argument:?}"
                    );
                };
                let incoming = root
                    .blocks
                    .iter()
                    .filter_map(|block| match &block.terminator {
                        Terminator::Jump {
                            target, arguments, ..
                        } if target == destination => Some(arguments[*position]),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let [source] = incoming.as_slice() else {
                    panic!("straight-line marker staging has one incoming argument: {incoming:?}");
                };
                argument = *source;
            }
            assert_eq!(
                argument, root.parameters[0].id,
                "staging preserves the authored marker's provenance"
            );
            assert_eq!(call.1.len(), 1);
            assert_eq!(call.1[0].place, root.structural_parameters[0].place);
            assert_eq!(call.1[0].access, StructuralAccess::Owned);
            assert!(call.1[0].path.is_empty());
        }
    }
}

#[test]
fn owned_boolean_fields_preserve_nested_order_current_local_and_returned_snapshot() {
    for entry in ["inspect", "enter"] {
        for (left, right) in [(false, true), (true, false)] {
            let (first, last) = if entry == "inspect" {
                (left, right)
            } else {
                (right, left)
            };
            for (returned, expected) in [("snapshot", last), ("scratch", first), ("answer", first)]
            {
                let source = ORDERED.replace("\n    snapshot\n", &format!("\n    {returned}\n"));
                support::execute(&source, entry, left, right, expected);
            }
        }
    }
}

#[test]
fn mixed_formals_keep_both_scalar_positions_and_same_typed_owned_actuals() {
    for entry in ["inspect", "enter"] {
        for (selected, inspect_expected) in [
            ("before", false),
            ("after", true),
            ("left.value", true),
            ("right.value", false),
        ] {
            let source = ORDERED
                .replace(
                    "selected: bool = before",
                    &format!("selected: bool = {selected}"),
                )
                .replace("\n    snapshot\n", "\n    answer\n");
            support::execute(
                &source,
                entry,
                false,
                true,
                if entry == "inspect" {
                    inspect_expected
                } else {
                    !inspect_expected
                },
            );
        }
    }
}
