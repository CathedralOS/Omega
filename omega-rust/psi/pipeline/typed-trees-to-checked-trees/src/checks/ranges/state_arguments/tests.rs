use super::*;

thread_local! {
    pub(super) static STATE_TRANSFERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static WHOLE_PASS_REFERENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn compare(source: &str) -> (Vec<StateArgumentFacts>, usize, usize) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let borrows = crate::build_borrow_facts(&program);
    let proof_plan = proof::obligations::build_proof_plan(&program);
    let values = crate::values::build_value_facts(&program, &proof_plan);
    let operators = crate::operators::build_operator_facts(&program, &values);
    let flow = super::super::cache_tests::range_flow_fixture(&program, &borrows);
    let frames = validation::CallFrameResolver::new(&program).unwrap();
    let machine = program.machines().first().unwrap();
    let calls: Vec<_> = program
        .machine_states(machine)
        .iter()
        .map(|state| {
            super::super::facts::RangeCallContext::new(
                machine,
                state,
                &borrows,
                &flow,
                Some(&frames),
            )
        })
        .collect();
    let fields = super::super::arrays::fixed_array_field_lengths(&program);
    let summaries = crate::flow::StateMutationSummaryCache::default();
    let before = STATE_TRANSFERS.get();
    let actual = collect_state_argument_facts(
        &program,
        &fields,
        machine,
        Some(&frames),
        &calls,
        &operators,
        &summaries,
    );
    let transfers = STATE_TRANSFERS.get() - before;
    let before = STATE_TRANSFERS.get();
    let expected = collect_state_argument_facts_whole_pass(
        &program,
        &fields,
        machine,
        Some(&frames),
        &calls,
        &operators,
        &summaries,
    );
    let reference_transfers = STATE_TRANSFERS.get() - before;
    assert_eq!(
        actual, expected,
        "retained contributions changed range inference"
    );
    (actual, transfers, reference_transfers)
}

fn chain(length: usize) -> String {
    use std::fmt::Write;
    let mut source = String::from("machine chain() -> u8 { transition { _ -> step0(3) }\n");
    for state_index in (0..length).rev() {
        write!(source, "state step{state_index}(value: u8) -> u8 {{ ").unwrap();
        if state_index + 1 == length {
            source.push_str("value");
        } else {
            write!(
                source,
                "transition {{ _ -> step{}(value) }}",
                state_index + 1
            )
            .unwrap();
        }
        source.push_str(" }\n");
    }
    source.push_str(
        "state disconnected(value: u8) -> u8 { transition { _ -> disconnected(value) } }\n}",
    );
    source
}

#[test]
fn reverse_chain_skips_unchanged_and_disconnected_state_transfers() {
    let (facts, transfers, reference) = compare(&chain(12));
    assert_eq!(facts.len(), 12);
    assert!(
        facts
            .iter()
            .all(|state| state.parameters[0].integer.get() == Some(3))
    );
    assert_eq!(transfers, 13);
    assert_eq!(reference, 91);
    eprintln!("range reverse chain: {transfers} transfers; whole-pass {reference}");
}

#[test]
fn unchanged_first_pass_does_not_replay_entry() {
    let (facts, transfers, reference) = compare("machine leaf() -> u8 { 3 }");
    assert!(facts.is_empty());
    assert_eq!((transfers, reference), (1, 1));
}

#[test]
fn original_round_budget_withholds_long_chain_inference() {
    let (facts, transfers, _) = compare(&chain(MAX_PROPAGATION_PASSES));
    assert!(facts.is_empty());
    assert_eq!(transfers, MAX_PROPAGATION_PASSES);
}

#[test]
fn late_cycle_predecessor_weakens_retained_outputs() {
    for value in ["3", "9", "unknown"] {
        let source = format!(
            "machine cycle(flag: bool, unknown: u8) -> u8 {{
                transition {{ _ -> first(3, flag, unknown) }}
                state tail(value: u8, flag: bool, unknown: u8) -> u8 {{
                    transition flag {{ true -> first({value}, flag, unknown) false -> value }}
                }}
                state first(value: u8, flag: bool, unknown: u8) -> u8 {{
                    transition {{ _ -> tail(value, flag, unknown) }}
                }}
            }}"
        );
        let (facts, _, _) = compare(&source);
        assert_eq!(facts.len(), 2);
        for state in facts {
            assert_eq!(
                state.parameters[0].integer.get(),
                (value == "3").then_some(3)
            );
            assert_eq!(
                state.parameters[0].upper_bound.get(),
                match value {
                    "3" => Some(4),
                    "9" => Some(10),
                    _ => None,
                }
            );
        }
    }
}

#[test]
fn recursive_entry_never_acquires_internal_argument_facts() {
    let (facts, _, _) = compare(
        "machine recursive(value: u8, again: bool) -> u8 {
            transition again { true -> recursive(3, again) false -> value }
        }",
    );
    assert!(facts.is_empty());
}

#[test]
fn complete_checked_evidence_and_bounds_diagnostics_match_whole_pass() {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            WHOLE_PASS_REFERENCE.set(self.0);
        }
    }
    for incoming in ["2", "unknown"] {
        let source = format!(
            "machine read(items: &[u8; 4], unknown: u64, flag: bool) -> u8 {{
                transition {{ _ -> first(items, 2, unknown, flag) }}
                state tail(items: &[u8; 4], position: u64, unknown: u64, flag: bool) -> u8 {{
                    let output: u8 = items[position];
                    transition flag {{ true -> first(items, {incoming}, unknown, flag) false -> output }}
                }}
                state first(items: &[u8; 4], position: u64, unknown: u64, flag: bool) -> u8 {{
                    transition {{ _ -> tail(items, position, unknown, flag) }}
                }}
            }}"
        );
        let (facts, _, _) = compare(&source);
        for state in facts {
            assert_eq!(state.parameters[0].length.get(), Some(4));
            assert_eq!(state.parameters[0].minimum_length.get(), Some(4));
            assert_eq!(
                state.index_proofs.get().contains(&ParameterIndexProof {
                    collection_parameter: 0,
                    index_parameter: 1,
                }),
                incoming == "2",
            );
        }
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let actual = crate::lower_typed_trees(program.clone());
        let _restore = Restore(WHOLE_PASS_REFERENCE.replace(true));
        let reference = crate::lower_typed_trees(program);
        assert_eq!(actual, reference);
        assert_eq!(actual.is_ok(), incoming == "2", "{actual:?}");
    }
}

#[test]
fn grouped_scalar_meets_preserve_unseen_unknown_and_conflicting_inputs() {
    let values = [None, Some(3), Some(9)];
    for first in values {
        for second in values {
            for third in values {
                let mut sequential = MergedFact::Unseen;
                let mut minimum = MergedBound::Unseen;
                let mut maximum = MergedBound::Unseen;
                for value in [first, second, third] {
                    sequential.merge(value);
                    minimum.merge_lower(value);
                    maximum.merge(value);
                }
                let parameter = |values: &[Option<i64>]| {
                    let mut facts = ParameterFacts {
                        symbol: SymbolHandle::default(),
                        name: String::new(),
                        is_self: false,
                        length: MergedFact::Unseen,
                        integer: MergedFact::Unseen,
                        minimum_length: MergedBound::Unseen,
                        upper_bound: MergedBound::Unseen,
                    };
                    for value in values {
                        facts.integer.merge(*value);
                        facts.minimum_length.merge_lower(*value);
                        facts.upper_bound.merge(*value);
                    }
                    StateArgumentFacts {
                        parameters: vec![facts],
                        ..Default::default()
                    }
                };
                let mut grouped = vec![parameter(&[first])];
                merge_contribution(&mut grouped, &parameter(&[second, third]));
                merge_contribution(&mut grouped, &parameter(&[]));
                assert_eq!(grouped[0].parameters[0].integer, sequential);
                assert_eq!(grouped[0].parameters[0].minimum_length, minimum);
                assert_eq!(grouped[0].parameters[0].upper_bound, maximum);
                assert_eq!(grouped[0].parameters[0].length, MergedFact::Unseen);
            }
        }
    }
}
