use super::*;

fn type_source(source: &str) -> Result<typed_trees::TypedTrees, Diagnostic> {
    let tokens = crate::frontend_fixture::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = crate::frontend_fixture::parse_syntax_trees(&tokens).unwrap();
    let trees = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .unwrap();
    crate::lower_symbol_resolved_trees(&trees)
}

#[test]
fn callable_field_named_store_keeps_its_value_arguments() {
    let typed = type_source(
        "data Registry {
            store: machine(key: u64, value: u64) -> u64 reaches none crashes none terminates;
        }
        machine call(registry: Registry) -> u64 {
            (registry.store)(1u64, 42u64)
        }",
    )
    .unwrap();
    let table = &typed.expression_table;
    let call = table
        .iter_expressions()
        .find_map(|(_, node)| match node {
            TypedNode::Call(call) if call.runtime_call.is_some() => Some(call),
            _ => None,
        })
        .expect("machine-valued field call");
    let arguments = table.expression_handles(call.arguments);
    assert_eq!(arguments.len(), 2);
    for (&arg, expected) in arguments.iter().zip([1, 42]) {
        assert!(matches!(table.expression(arg), TypedNode::Integer(literal)
            if literal.magnitude_u128() == Some(expected)));
    }
}

#[test]
fn indexed_atomic_store_retains_its_destination() {
    let typed = type_source(
        "machine set(cells: &mut [AtomicU32; 2]) {
            cells[0].store(17, NoOrdering);
        }",
    )
    .unwrap();
    let store = typed.machines().iter()
        .flat_map(|machine| typed.machine_states(machine))
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .find_map(|instruction| match instruction {
            StatementNode::Assignment(store) if matches!(typed.expression_table.expression(store.value),
                TypedNode::Atomic(event) if event.ordering == AtomicOrderingPlan::Store(MemoryOrdering::NoOrdering)) => Some(store),
            _ => None,
        }).expect("typed atomic store");
    assert!(matches!(
        typed.expression_table.expression(store.target),
        TypedNode::Indexed(_)
    ));
}

#[test]
fn selected_cells_check_ordering_and_keep_the_event_and_its_result() {
    for (member, forbidden) in [
        ("load(Publish)", "atomic load"),
        ("store(1, Receive)", "atomic store"),
        (
            "compare_exchange(0, 1, NoOrdering, Publish)",
            "failure ordering",
        ),
    ] {
        let invocation = if member.starts_with("store") {
            format!("cell.{member};")
        } else {
            format!("let prior: u32 = cell.{member};")
        };
        let error = type_source(&format!(
            "machine update(cell: &AtomicU32) {{ {invocation} }}"
        ))
        .unwrap_err();
        assert!(error.message.contains(forbidden), "{}", error.message);
    }
    let typed = type_source(
        "machine update(cell: &AtomicU32) {
            cell.store(1, Publish);
            let read: u32 = cell.load(Receive);
            let prior: u32 = cell.fetch_add(1, GlobalOrder);
            let swapped: u32 = cell.swap(1, NoOrdering);
            let compared: u32 = cell.compare_exchange(0, 1, Receive, NoOrdering);
            let once = cell.compare_exchange_once(0, 1, Receive, NoOrdering);
        }",
    )
    .unwrap();
    let operations = typed
        .expression_table
        .iter_expressions()
        .filter_map(|(_, node)| match node {
            TypedNode::Atomic(event) => Some(event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(operations.len(), 6);
    for event in operations {
        assert!(event.result_custody.is_valid_for(event.ordering));
        if matches!(
            event.ordering,
            AtomicOrderingPlan::CompareExchangeOnce { .. }
        ) {
            let once = typed.atomic_once_operands(event).unwrap();
            assert_eq!(once.operands.len(), 2);
        }
        assert_eq!(
            event.result.is_valid(),
            matches!(
                event.ordering,
                AtomicOrderingPlan::ReadModifyWrite(_)
                    | AtomicOrderingPlan::Swap(_)
                    | AtomicOrderingPlan::CompareExchange { .. }
            )
        );
    }
}

#[test]
fn source_calls_publish_the_selected_atomic_member() {
    use language_core::atomic::AtomicCellMember;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
    };
    let source = "machine main() {
        let mut cell: AtomicU32;
        cell.store(10u32, Publish);
        let prior: u32 = cell.fetch_add(5u32, NoOrdering);
        let current: u32 = cell.load(Receive);
    }";
    let typed =
        crate::qualified_operator_home_tests::packages(&[(1, "main.omg", source)], &[], &[])
            .unwrap();
    let table = &typed.expression_table;
    let mut operations = Vec::new();
    for (handle, node) in table.iter_expressions() {
        let TypedNode::Atomic(event) = node else {
            continue;
        };
        let expected = match event.ordering {
            AtomicOrderingPlan::Store(_) => AtomicCellMember::Store,
            AtomicOrderingPlan::ReadModifyWrite(_) => AtomicCellMember::FetchAdd,
            AtomicOrderingPlan::Load(_) => AtomicCellMember::Load,
            other => panic!("unexpected atomic event {other:?}"),
        };
        let calls: Vec<_> = table
            .authored_selection_occurrences(handle)
            .map(|id| typed.authored_declaration_selections().get(id).unwrap())
            .filter(|row| row.kind() == Kind::Call)
            .map(|row| row.target())
            .collect();
        assert_eq!(
            calls,
            [Target::Intrinsic(Intrinsic::AtomicCellOperation(expected))]
        );
        operations.push(expected);
    }
    assert_eq!(
        operations,
        [
            AtomicCellMember::Store,
            AtomicCellMember::FetchAdd,
            AtomicCellMember::Load
        ]
    );
}
