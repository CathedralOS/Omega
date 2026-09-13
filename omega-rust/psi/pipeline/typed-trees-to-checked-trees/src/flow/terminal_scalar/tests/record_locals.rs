use checked_trees::{CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan};

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
            checked_trees::CheckedStructuralScalarFieldStoreDestination::Local {
                symbol: local.symbol
            }
        );
        assert!(store.carrier_path.is_empty());
        assert_eq!(store.destination.parameter_position(), None);
        assert!(graph.states[0].bindings.is_empty());
        if value.contains("identity") {
            assert!(matches!(
                store.value,
                checked_trees::CheckedStructuralScalarFieldStoreValue::Computation(_)
            ));
        }
    }
}

#[test]
fn nested_local_field_store_retains_its_exact_carrier() {
    let checked = check_source(
        "data Inner [copy] { value: u64; }
         data Outer [copy] { left: Inner; right: Inner; }
         machine enter(value: u64) -> u64 {
             let mut record: Outer = Outer { left: Inner { value: 256 }, right: Inner { value: 2 } };
             record.right.value = value;
             record.left.value
         }"
    );
    let machine = checked.machines()[0].symbol;
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine)
        .expect("nested record store graph");
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) =
        &graph.states[0].unit_operations[1]
    else {
        panic!("store");
    };
    assert_eq!(
        store.carrier_path,
        vec![checked_trees::CheckedUnitStructuralPathSegment::Field(
            "right".into()
        )]
    );
    assert_eq!(store.field_identity, "value");
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
            crate::flow::terminal_unit::build_local_scalar_field_store(
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
fn bounded_local_field_store_waits_for_a_write_range_obligation() {
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
            .is_none()
    );
}

#[test]
fn source_rejects_record_rebinding_and_invalid_bounds() {
    for (field_type, result_type, body) in [
        (
            "u64",
            "u64",
            "let first: Value = Value { value: 256 }; first = Value { value: 5 }; first.value",
        ),
        (
            "u64[0..=256]",
            "u64",
            "let mut first: Value = Value { value: 256 }; first.value = 257; first.value",
        ),
        (
            "u64",
            "u64[256..=256]",
            "let mut first: Value = Value { value: 256 }; first.value = 5; first.value",
        ),
    ] {
        let source = format!(
            "data Value {{ value: {field_type}; }} machine enter() -> {result_type} {{ {body} }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        assert!(crate::lower_typed_trees(typed).is_err(), "{source}");
    }
}

#[test]
fn field_fill_without_mutable_binding_retains_its_store() {
    // Source permits member/index filling without `let mut`; only whole-local
    // rebinding is gated by that modifier. Owned field storage must retain the
    // same ordered write regardless of whole-binding mutability.
    let checked = check_source(
        "data Value { value: u64; }
         machine enter() -> u64 {
             let first: Value = Value { value: 256 };
             first.value = 5;
             first.value
         }",
    );
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(checked.machines()[0].symbol)
        .expect("owned field fill graph");
    assert!(matches!(
        graph.states[0].unit_operations[1],
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
    ));
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
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

#[test]
fn fresh_record_local_and_owned_loop_parameter_keep_separate_custody_ledgers() {
    let checked = check_source(
        "data Limits { value: u64; }
         data Local { value: u64; }
         machine walk(limits: Limits, remaining: u64[0..=3], marker: u64)
         terminates by remaining -> Nat::Descending in 0..4;
         -> u64 {
             let local: Local = Local { value: marker };
             transition remaining > 0 {
                 true -> walk(limits, remaining - 1, local.value)
                 false -> local.value
             }
         }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .unwrap()
        .symbol;
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine)
        .expect("owned loop plus fresh local");
    assert!(graph.ranked_scc.is_some());
    assert_eq!(graph.states[0].unit_operations.len(), 1);
    for kind in [
        language_semantics::PermissionEventKind::Transfer,
        language_semantics::PermissionEventKind::Establish,
    ] {
        let mut changed = checked.clone();
        let receipt = changed
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .find_map(|(handle, event)| {
                (event.machine_symbol == machine && event.kind == kind).then_some(handle)
            })
            .expect("parameter transfer or local establishment");
        changed
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(receipt)
            .obligation_live = true;
        super::super::finalize_checked_scalar_graph_plans(
            &changed.typed,
            &changed.facts.flow.ownership,
            &changed.facts.values.scalar_computations,
            &mut changed.facts.flow.terminal_scalar_graphs,
        );
        super::super::unit_operations::finalize(&changed.typed, &mut changed.facts);
        assert!(
            changed
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine)
                .is_none(),
            "corrupted {kind:?}"
        );
    }
}

#[test]
fn fresh_nested_record_transition_retains_shapes_operations_and_scalar_occurrences() {
    for copy in ["[copy]", ""] {
        let checked = checked(copy, "", "identity(7)");
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "enter")
            .unwrap();
        let graphs = &checked.facts.flow.terminal_scalar_graphs;
        let graph = graphs
            .for_machine(machine.symbol)
            .expect("fresh local record graph");
        let state = &graph.states[0];
        let [
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                value,
                calls,
                discard_result_on_return,
            },
        ] = state.unit_operations.as_slice()
        else {
            panic!("one exact record establishment");
        };
        assert_eq!(result.statement_index, 0);
        assert_eq!(result.binding_ordinal, 0);
        assert!(!discard_result_on_return);
        assert!(calls.is_empty());
        assert_eq!(
            *value,
            checked
                .facts
                .values
                .structural_values
                .root_at(state.state, 0)
                .unwrap()
                .root
        );
        assert_eq!(state.bindings.len(), 1);
        assert_eq!(state.bindings[0].statement_ordinal, 1);
        for (ordinal, role) in [
            (
                1,
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
            ),
            (2, CheckedScalarExpressionRole::Guard),
            (
                2,
                CheckedScalarExpressionRole::TransitionArgument {
                    argument_ordinal: 0,
                },
            ),
        ] {
            assert!(
                checked
                    .facts
                    .values
                    .scalar_computations
                    .root_at(state.state, ordinal, role)
                    .is_some(),
                "missing occurrence {role:?}"
            );
        }
        assert!(graphs.structural_types.iter().any(|shape| matches!(
            &shape.shape,
            checked_trees::CheckedUnitStructuralTypeShape::Record { fields }
            if fields.iter().any(|field| matches!(field.field_type, checked_trees::CheckedUnitStructuralFieldType::BoundedInteger(_)))
        )));
        assert!(graphs.structural_types.iter().any(|shape| matches!(
            &shape.shape,
            checked_trees::CheckedUnitStructuralTypeShape::Record { fields }
            if fields.iter().any(|field| matches!(field.field_type, checked_trees::CheckedUnitStructuralFieldType::Structural { .. }))
        )));
    }
}

#[test]
fn local_record_graph_retains_whole_copy_initializers() {
    let checked = checked("[copy]", "let copied: Outer = bounded;", "7");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine.symbol)
            .is_some()
    );
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine.symbol)
        .unwrap();
    assert_eq!(graph.states[0].unit_operations.len(), 2);
    let root = checked
        .facts
        .values
        .structural_values
        .root_at(graph.states[0].state, 1)
        .unwrap();
    assert!(matches!(
        checked
            .facts
            .values
            .structural_values
            .nodes
            .get(root.root)
            .kind,
        checked_trees::CheckedStructuralValueKind::Place(_)
    ));
}

#[test]
fn local_record_graph_retains_nested_copies_and_affine_moves() {
    for copy in ["[copy]", ""] {
        for initializer in ["first", "Outer { inner: child }"] {
            let checked = check_source(&format!(
                "data Value {copy} {{ value: u64; }}
                 data Outer {copy} {{ inner: Value; }}
                 machine enter() -> u64 {{
                     let first: Outer = Outer {{ inner: Value {{ value: 9 }} }};
                     let child: Value = Value {{ value: 7 }};
                     let copied: Outer = {initializer};
                     transition true {{
                         true -> done(copied.inner.value)
                         _ -> 0
                     }}
                     state done(value: u64) {{ value }}
                 }}"
            ));
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "enter")
                .unwrap();
            assert!(
                checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .for_machine(machine.symbol)
                    .is_some(),
                "{copy}: {initializer}"
            );
        }
    }
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
fn record_move_disposition_retains_only_final_owners_and_original_provenance() {
    let checked = check_source(MOVED_RECORDS);
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
        .expect("record move graph");
    assert_eq!(graph.states[0].unit_operations.len(), 4);
    for (ordinal, expected) in [true, false, false, true].into_iter().enumerate() {
        assert_eq!(
            validation::record_local_disposition(
                &checked.typed,
                &checked.facts,
                machine,
                graph.states[0].state,
                ordinal as u32
            ),
            Some(expected)
        );
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
            let checked_trees::CheckedStructuralValueKind::Place(argument) = &mut node.kind else {
                panic!("whole record copy");
            };
            if corruption == 1 {
                argument.access = checked_trees::CheckedStructuralAccess::SharedBorrow;
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
fn local_record_graph_rejects_missing_affine_exit_receipt() {
    let mut checked = checked("", "", "7");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap()
        .symbol;
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
            .is_some()
    );
    let receipt = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.machine_symbol == machine
                && event.kind == language_semantics::PermissionEventKind::AffineDrop)
                .then_some(handle)
        })
        .expect("whole affine local exit receipt");
    checked
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(receipt)
        .obligation_live = true;
    super::super::unit_operations::finalize(&checked.typed, &mut checked.facts);
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
            .is_none()
    );
}
