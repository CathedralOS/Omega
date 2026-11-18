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
    assert!(
        typed
            .expression_table
            .iter_expressions()
            .any(|(_, node)| matches!(node, TypedNode::Call(call) if call.runtime_call.is_some()))
    );
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
