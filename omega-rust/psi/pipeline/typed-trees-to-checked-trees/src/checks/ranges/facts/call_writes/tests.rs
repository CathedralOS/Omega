use super::{RangeCallContext, RangeFacts};
use crate::CallSite;

fn program() -> typed_trees::TypedTrees {
    let source = r#"
        machine leaf(value: u64) -> u64 { value }
        machine pair(left: u64, right: u64) -> u64 { left }
        machine first(value: u64) -> u64 {
            pair(leaf(value), leaf(leaf(value)));
            let result: u64 = pair(leaf(value), leaf(value));
            transition { _ -> next(result) }
            state next(value: u64) -> u64 {
                pair(leaf(value), leaf(value));
                leaf(value)
            }
        }
        machine second(value: u64) -> u64 {
            pair(leaf(value), leaf(value));
            leaf(value)
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn owner_local_calls_rejoin_exact_nested_sibling_and_statement_occurrences() {
    let program = program();
    let borrows = crate::build_borrow_facts(&program);
    let flow = crate::checks::ranges::cache_tests::range_flow_fixture(&program, &borrows);
    let frames = validation::CallFrameResolver::new(&program);
    let mut expressions = 0;
    let mut statements = 0;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let context = RangeCallContext::new(machine, state, &borrows, &flow, frames.as_ref());
            for expected in context.borrow_calls {
                let site = crate::find_call_site(
                    &program,
                    machine.symbol,
                    state.symbol,
                    expected.statement_index,
                    expected.call_ordinal,
                )
                .expect("authored occurrence");
                if matches!(site, CallSite::TransitionNamed { .. }) {
                    continue;
                }
                let actual = context
                    .find_call(&program, machine, state, expected.statement_index, &site)
                    .expect("exact call survives the owner-local join");
                assert!(std::ptr::eq(actual, expected));
                assert!(
                    context
                        .find_call(
                            &program,
                            machine,
                            state,
                            expected.statement_index + 100,
                            &site,
                        )
                        .is_none()
                );
                for other in program
                    .machines()
                    .iter()
                    .filter(|other| other.symbol != machine.symbol)
                {
                    assert!(
                        context
                            .find_call(&program, other, state, expected.statement_index, &site,)
                            .is_none()
                    );
                }
                for other in program
                    .machine_states(machine)
                    .iter()
                    .filter(|other| other.symbol != state.symbol)
                {
                    assert!(
                        context
                            .find_call(&program, machine, other, expected.statement_index, &site,)
                            .is_none()
                    );
                }
                match site {
                    CallSite::Expression { .. } => expressions += 1,
                    CallSite::Statement(statement) => {
                        statements += 1;
                        let copied = statement.clone();
                        assert!(
                            context
                                .find_call(
                                    &program,
                                    machine,
                                    state,
                                    expected.statement_index,
                                    &CallSite::Statement(&copied),
                                )
                                .is_none(),
                            "equal payload is not the authored statement"
                        );
                    }
                    CallSite::TransitionNamed { .. } => unreachable!(),
                }
            }
        }
    }
    assert!(
        expressions >= 12,
        "fixture exercises nested and sibling expressions"
    );
    assert_eq!(statements, 3);
}

#[test]
fn missing_call_evidence_is_opaque_not_a_complete_empty_write_frame() {
    let program = program();
    let borrows = crate::build_borrow_facts(&program);
    let flow = crate::checks::ranges::cache_tests::range_flow_fixture(&program, &borrows);
    let frames = validation::CallFrameResolver::new(&program);
    let machine = &program.machines()[2];
    let state = &program.machine_states(machine)[0];
    let site = crate::find_call_site(&program, machine.symbol, state.symbol, 0, 0).unwrap();
    let empty_flow = checked_trees::FlowFacts::default();
    let empty_borrows = checked_trees::BorrowFacts::default();
    for (borrows, flow) in [(&borrows, &empty_flow), (&empty_borrows, &flow)] {
        let context = RangeCallContext::new(machine, state, borrows, flow, frames.as_ref());
        let mut facts = RangeFacts::new(&[]);
        facts.checked_calls = Some(&context);
        assert_eq!(
            facts.structured_call_writes(&program, machine, state, &site),
            None
        );
        let mut branch = facts.clone();
        assert!(std::ptr::eq(branch.checked_calls.unwrap(), &context));
        assert_eq!(
            branch.structured_call_writes(&program, machine, state, &site),
            None
        );
    }
}
