use checked_trees::{CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan};

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
fn local_record_graph_retains_nested_copies_and_fences_affine_moves() {
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
            assert_eq!(
                checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .for_machine(machine.symbol)
                    .is_some(),
                !copy.is_empty(),
                "{copy}: {initializer}"
            );
        }
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
