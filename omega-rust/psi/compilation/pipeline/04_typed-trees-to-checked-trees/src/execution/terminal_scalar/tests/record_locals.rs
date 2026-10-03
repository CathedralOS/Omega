use crate::tests::front_end::checked_program_result;
use checked_trees::CheckedUnitEffectOperationPlan;

#[test]
fn projected_parameter_stores_keep_the_complete_effect_body() {
    for target in ["context.value", "context.samples[0]"] {
        let checked = check_source(&format!(
            "data Cell {{ value: i32; samples: [i32; 2]; }}
             machine stamp(context: &mut Cell) -> i32 {{ {target} = 90; 7 }}"
        ));
        let machine = checked.machines()[0].symbol;
        let operations = if let Some(graph) = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
        {
            &graph.states[0].unit_operations
        } else {
            let body = checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine)
                .expect("the ordinary effect body retains the store and scalar result");
            assert!(body.scalar_result.is_some());
            &body.operations
        };
        assert_eq!(
            operations
                .iter()
                .filter_map(CheckedUnitEffectOperationPlan::store_statement_index)
                .collect::<Vec<_>>(),
            [0],
            "the selected body retains {target}'s store exactly once"
        );
    }
}

#[test]
fn mutable_record_field_stores_keep_local_identity_and_authored_order() {
    for (copy, target, result, value) in [
        ("[copy]", "first", "second.value", "5"),
        ("[copy]", "first", "first.value", "identity(5)"),
        ("", "second", "second.value", "identity(5)"),
    ] {
        let checked = check_source(&format!(
            "data Value {copy} {{ value: u64; }}
             machine identity(value: u64) -> u64 {{ value }}
             machine enter() -> u64 {{
                 let mut first: Value = Value {{ value: 256 }};
                 let mut second: Value = first;
                 {target}.value = {value};
                 {result}
             }}"
        ));
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "enter")
            .unwrap();
        let state = &checked.typed.machine_states(machine)[0];
        let statements = checked
            .typed
            .statement_table
            .statements(state.statement_nodes);
        let typed_trees::statement::StatementNode::LocalData(local) =
            &statements[if target == "first" { 0 } else { 1 }]
        else {
            panic!("local");
        };
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine.symbol)
            .expect("mutable record scalar graph");
        let [
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. },
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. },
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
        ] = graph.states[0].unit_operations.as_slice()
        else {
            panic!("ordered record operations");
        };
        assert_eq!(store.statement_index, 2);
        assert_eq!(
            store.destination,
            checked_trees::CheckedStoreRoot::Local {
                symbol: local.symbol
            }
        );
        assert!(store.carrier_path.is_empty());
        assert_eq!(store.destination.parameter_position(), None);
        assert!(graph.states[0].bindings.is_empty());
        if value.contains("identity") {
            assert!(matches!(
                store.value,
                checked_trees::CheckedStoreValue::Computation(_)
            ));
        }
    }
}

#[test]
fn local_store_rejects_missing_destination_and_rhs_custody() {
    let original = check_source(
        "data Value [copy] { value: u64; }
         machine enter() -> u64 {
             let mut record: Value = Value { value: 256 };
             record.value = 5;
             record.value
         }",
    );
    let machine = original.machines()[0].clone();
    let state = original.typed.machine_states(&machine)[0].clone();
    for destination in [true, false] {
        let mut changed = original.clone();
        let statements = changed
            .typed
            .statement_table
            .statements_mut(state.statement_nodes);
        let typed_trees::statement::StatementNode::Assignment(assignment) = &mut statements[1]
        else {
            panic!("assignment");
        };
        if destination {
            assignment.target = typed_trees::expression::ExpressionHandle::invalid();
        } else {
            assignment.value = typed_trees::expression::ExpressionHandle::invalid();
        }
        let typed_trees::statement::StatementNode::Assignment(assignment) = &changed
            .typed
            .statement_table
            .statements(state.statement_nodes)[1]
        else {
            panic!("assignment");
        };
        assert!(
            crate::execution::terminal_unit::build_local_scalar_field_store(
                &changed.typed,
                &changed.facts,
                &machine,
                &state,
                1,
                assignment,
            )
            .is_none()
        );
    }
}

#[test]
fn bounded_local_field_store_retains_a_write_range_obligation() {
    let checked = check_source(
        "data Value [copy] { value: u64[0..=256]; }
         machine enter() -> u64 {
             let mut record: Value = Value { value: 256 };
             record.value = 5;
             record.value
         }",
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(checked.machines()[0].symbol)
            .is_some()
    );
}

fn checked(copy: &str, prefix: &str, initializer: &str) -> checked_trees::CheckedTrees {
    let source = format!(
        "data Value {copy} {{ value: u64[0..257]; flag: bool; }}
         data Outer {copy} {{ inner: Value; }}
         machine identity(value: u64) -> u64 {{ value }}
         machine enter() -> u64 {{
             let bounded: Outer = Outer {{ inner: Value {{ value: 256, flag: true }} }};
             {prefix}
             let number: u64 = {initializer};
             transition bounded.inner.flag {{
                 true -> yes(bounded.inner.value)
                 _ -> no()
             }}
             state yes(value: u64) {{ value }}
             state no() {{ 0 }}
         }}"
    );
    check_source(&source)
}

fn check_source(source: &str) -> checked_trees::CheckedTrees {
    checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

const MOVED_RECORDS: &str = "data Owned { value: u64; }
    machine moved() -> u64 {
        let keep: Owned = Owned { value: 17 };
        let first: Owned = Owned { value: 256 };
        let second: Owned = first;
        let third: Owned = second;
        transition true {
            true -> done(keep.value ^ third.value)
            _ -> 0
        }
        state done(value: u64) { value }
    }";

#[test]
fn record_wrapper_provenance_uses_common_child_origin_or_new_parent_origin() {
    use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
    for (second, origin) in [("Owned { value: 3 }", 0), ("second", 2)] {
        let checked = check_source(&format!(
            "const OFFSET: u64 = 17;
             data Owned {{ value: u64; }}
             data Pair {{ first: Owned; second: Owned; observed: u64; }}
             machine wrapped() -> u64 {{
                 let first: Owned = Owned {{ value: OFFSET }};
                 let second: Owned = Owned {{ value: 3 }};
                 let pair: Pair = Pair {{ observed: first.value, first: first, second: {second} }};
                 let moved: Pair = pair;
                 transition true {{ true -> done(moved.observed) _ -> 0 }}
                 state done(value: u64) {{ value }}
             }}"
        ));
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "wrapped")
            .unwrap()
            .symbol;
        let state = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
            .expect("wrapper origin graph")
            .states[0]
            .state;
        for ordinal in [2, 3] {
            assert_eq!(
                validation::record_local_disposition(
                    &checked.typed,
                    &checked.facts,
                    machine,
                    state,
                    ordinal
                ),
                Some(ordinal == 3)
            );
            let handle = checked
                .facts
                .flow
                .ownership
                .permissions
                .iter()
                .find_map(|(handle, event)| {
                    (event.machine_symbol == machine
                        && event.state_symbol == state
                        && event.kind == PermissionEventKind::Establish
                        && event.source
                            == PermissionEventSource::Statement {
                                statement_index: ordinal as usize,
                            })
                    .then_some(handle)
                })
                .unwrap();
            assert_eq!(
                checked
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .get(handle)
                    .provenance,
                PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: PermissionEventSource::Statement {
                        statement_index: origin
                    }
                }
            );
            let mut forged = checked.clone();
            forged
                .facts
                .flow
                .ownership
                .permissions
                .get_mut(handle)
                .provenance = PermissionProvenance::Established {
                machine_symbol: machine,
                state_symbol: state,
                source: PermissionEventSource::Statement {
                    statement_index: if origin == 0 { 2 } else { 0 },
                },
            };
            assert_eq!(
                validation::record_local_disposition(
                    &forged.typed,
                    &forged.facts,
                    machine,
                    state,
                    ordinal
                ),
                None
            );
        }
    }
}

#[test]
fn record_move_disposition_rejects_forged_transfer_or_destination_receipts() {
    use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
    let original = check_source(MOVED_RECORDS);
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "moved")
        .unwrap()
        .symbol;
    let state = original
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine)
        .unwrap()
        .states[0]
        .state;
    let transfer = original
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.machine_symbol == machine
                && event.state_symbol == state
                && event.kind == PermissionEventKind::Transfer
                && event.source == PermissionEventSource::Statement { statement_index: 2 })
            .then_some(handle)
        })
        .unwrap();
    let destination = original
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.machine_symbol == machine
                && event.state_symbol == state
                && event.kind == PermissionEventKind::Establish
                && event.source == PermissionEventSource::Statement { statement_index: 2 })
            .then_some(handle)
        })
        .unwrap();
    let keep = original
        .machines()
        .iter()
        .flat_map(|machine| original.machine_states(machine))
        .flat_map(|state| original.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "keep" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .unwrap();
    for corruption in 0..6 {
        let mut forged = original.clone();
        match corruption {
            0 => {
                let duplicate = forged
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .get(transfer)
                    .clone();
                forged.facts.flow.ownership.permissions.append(duplicate);
            }
            1 => {
                forged
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .get_mut(transfer)
                    .source = PermissionEventSource::Statement { statement_index: 3 }
            }
            2 => {
                forged
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .get_mut(transfer)
                    .kind = PermissionEventKind::AffineDrop
            }
            3 => {
                forged
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .get_mut(transfer)
                    .access = language_semantics::PermissionAccess::Shared
            }
            4 => {
                let event = forged.facts.flow.ownership.permissions.get_mut(destination);
                event.provenance = PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: event.source,
                };
            }
            _ => {
                let root = forged
                    .facts
                    .values
                    .structural_values
                    .root_at(state, 2)
                    .unwrap()
                    .root;
                let checked_trees::CheckedStructuralValueKind::Place(argument) = &mut forged
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .get_mut(root)
                    .kind
                else {
                    panic!("move source");
                };
                argument.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol: keep,
                    };
            }
        }
        let ordinal = if corruption == 4 { 2 } else { 1 };
        assert_eq!(
            validation::record_local_disposition(
                &forged.typed,
                &forged.facts,
                machine,
                state,
                ordinal
            ),
            None,
            "corruption {corruption}"
        );
        super::super::unit_operations::finalize(&forged.typed, &mut forged.facts);
        assert!(
            forged
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine)
                .is_none(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn local_record_copy_rejects_stale_roots_and_changed_access_or_type() {
    let original = checked("[copy]", "let copied: Outer = bounded;", "7");
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap()
        .symbol;
    let state = original
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine)
        .unwrap()
        .states[0]
        .state;
    let root = original
        .facts
        .values
        .structural_values
        .root_at(state, 1)
        .unwrap()
        .root;
    for corruption in 0..3 {
        let mut changed = original.clone();
        let node = changed.facts.values.structural_values.nodes.get_mut(root);
        if corruption == 0 {
            node.expression = arena::Handle::invalid();
        } else {
            // A copyable local's later read is a snapshot of its current
            // contents, planned as a shared read of the whole local.
            let checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace {
                source: argument,
                ..
            } = &mut node.kind
            else {
                panic!("whole record copy");
            };
            if corruption == 1 {
                argument.access = checked_trees::CheckedStructuralAccess::Owned;
            } else {
                argument.type_identity = "unrelated".to_owned();
            }
        }
        super::super::unit_operations::finalize(&changed.typed, &mut changed.facts);
        assert!(
            changed
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine)
                .is_none(),
            "corrupted copy {corruption}"
        );
    }
}

#[test]
fn parameter_origin_record_local_joins_its_own_custody() {
    use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
    let checked = check_source(
        "data Owned { value: u64; }
         machine moved(owned: Owned) -> u64 {
             let keep: Owned = owned;
             let handed: Owned = keep;
             handed.value
         }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "moved")
        .unwrap()
        .symbol;
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine)
        .expect("parameter-origin record graph");
    let state = graph.states[0].state;
    // `keep` transfers into `handed`; `handed` still owns at exit.
    assert_eq!(
        validation::record_local_disposition(&checked.typed, &checked.facts, machine, state, 0),
        Some(false),
        "parameter-origin local kept its exact initializer move",
    );
    assert_eq!(
        validation::record_local_disposition(&checked.typed, &checked.facts, machine, state, 1),
        Some(true),
        "parameter-origin destination retained its exit obligation",
    );
    // An affine parameter mints no state-entry claim, so the move stamps the
    // receiving local's own statement as the establishment provenance.
    let establish = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .find(|event| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.kind == PermissionEventKind::Establish
                && event.source == PermissionEventSource::Statement { statement_index: 0 }
        })
        .expect("parameter-origin local establishment");
    assert_eq!(
        establish.provenance,
        PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: PermissionEventSource::Statement { statement_index: 0 },
        }
    );
    let mut forged = checked.clone();
    let handle = forged
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.machine_symbol == machine
                && event.state_symbol == state
                && event.kind == PermissionEventKind::Establish
                && event.source == PermissionEventSource::Statement { statement_index: 0 })
            .then_some(handle)
        })
        .unwrap();
    forged
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(handle)
        .provenance = PermissionProvenance::Established {
        machine_symbol: machine,
        state_symbol: state,
        source: PermissionEventSource::StateEntry,
    };
    assert_eq!(
        validation::record_local_disposition(&forged.typed, &forged.facts, machine, state, 0),
        None,
        "a re-pointed parameter origin must not join custody",
    );
}
