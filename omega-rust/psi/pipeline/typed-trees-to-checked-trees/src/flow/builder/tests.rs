use facts::ScalarValue;
use numerics::bignum::BigInt;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

use super::build_flow_facts;
use crate::flow::{StateMutationSummaryCache, canonical_place_from_symbol};
use crate::{build_borrow_facts, build_domain_facts, build_proof_facts, build_semantic_facts};

thread_local! {
    pub(in crate::flow) static STATE_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(in crate::flow) static WHOLE_PASS_REFERENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static SWEEP_LIMIT: std::cell::Cell<usize> = const { std::cell::Cell::new(usize::MAX) };
}

pub(crate) fn check_against_whole_pass(
    program: typed_trees::TypedTrees,
) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let reference_program = program.clone();
    let result = crate::lower_typed_trees(program);
    let reference = with_whole_pass_reference(|| crate::lower_typed_trees(reference_program));
    assert_eq!(
        result, reference,
        "complete checked facts or diagnostics changed"
    );
    result
}

#[test]
fn exhausted_sweep_budget_discards_provisional_constants() {
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            SWEEP_LIMIT.set(self.0);
        }
    }
    let source = reverse_chain_source(12, 0).replace(
        "machine chain() -> u8",
        "machine chain() -> u8 ensures result == 3",
    );
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let program = lower_symbol_resolved_trees(&resolved).unwrap();
    check_against_whole_pass(program.clone()).expect("the complete fixed point proves 3");
    let _restore = Restore(SWEEP_LIMIT.replace(1));
    let diagnostics = check_against_whole_pass(program)
        .expect_err("fallback must discard every provisional constant");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ensures"))
    );
}

fn with_whole_pass_reference<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            WHOLE_PASS_REFERENCE.set(self.0);
        }
    }
    let _restore = Restore(WHOLE_PASS_REFERENCE.replace(true));
    run()
}

fn reverse_chain_source(length: usize, independent: usize) -> String {
    use std::fmt::Write;
    let mut source = String::from("machine chain() -> u8 { transition { _ -> step0(3) }\n");
    for index in (0..length).rev() {
        write!(source, "state step{index}(value: u8) -> u8 {{ ").unwrap();
        if index + 1 == length {
            source.push_str("value");
        } else {
            write!(source, "transition {{ _ -> step{}(value) }}", index + 1).unwrap();
        }
        source.push_str(" }\n");
    }
    source.push_str("}\n");
    for index in 0..independent {
        writeln!(source, "machine independent{index}() -> u8 {{ 7 }}").unwrap();
    }
    source
}

#[test]
fn reverse_chain_revisits_only_changed_inputs_before_final_materialization() {
    let length = 12;
    let independent = 64;
    let source = reverse_chain_source(length, independent);
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let program = lower_symbol_resolved_trees(&resolved).unwrap();
    let proof_plan = proof::obligations::build_proof_plan(&program);
    let operations = validation::infer_operational_may(&program);
    let borrow = build_borrow_facts(&program);
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&program, &proof);
    let mut reference_semantic = semantic.clone();
    let domains = build_domain_facts(&program, &semantic);
    let before = STATE_BUILDS.get();
    let flow = build_flow_facts(
        &program,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let builds = STATE_BUILDS.get() - before;
    let states = 1 + length + independent;
    assert_eq!(flow.control.states.len(), states);
    assert!(
        builds <= 2 * states + length,
        "{builds} state builds for {states} states: independent machines must not repeat each convergence sweep"
    );
    let before = STATE_BUILDS.get();
    let reference = with_whole_pass_reference(|| {
        build_flow_facts(
            &program,
            &borrow,
            &proof,
            &mut reference_semantic,
            &domains,
            &operations,
        )
    });
    let reference_builds = STATE_BUILDS.get() - before;
    assert_eq!(semantic, reference_semantic);
    assert_eq!(flow, reference);
    assert_eq!(builds, 2 * states + length - 1);
    assert_eq!(reference_builds, states * length);
    eprintln!("reverse chain: {builds} state builds, whole-pass reference {reference_builds}");
}

#[test]
fn complete_checking_matches_reference_with_reverse_chain_and_cycle() {
    let source = format!(
        "{}\n{}",
        reverse_chain_source(12, 64),
        r#"
        machine cycle(flag: bool) -> u8 {
            transition { _ -> first(3, flag) }
            state second(current: u8, again: bool) -> u8 {
                transition again { true -> first(4, again) false -> current }
            }
            state first(current: u8, again: bool) -> u8 {
                transition { _ -> second(current, again) }
            }
        }
        "#
    );
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let program = lower_symbol_resolved_trees(&resolved).unwrap();
    let mut dirty_times = Vec::new();
    let mut reference_times = Vec::new();
    // Alternate order and compare full checking, not just the transfer helper.
    // Cloning input and comparing/dropping output stay outside timed intervals.
    for round in 0..6 {
        let mut results = [None, None];
        for reference in if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let input = program.clone();
            let before = STATE_BUILDS.get();
            let start = std::time::Instant::now();
            let result = if reference {
                with_whole_pass_reference(|| crate::lower_typed_trees(input))
            } else {
                crate::lower_typed_trees(input)
            };
            let elapsed = start.elapsed();
            let builds = STATE_BUILDS.get() - before;
            assert!(result.is_ok(), "checking must succeed: {result:?}");
            if reference {
                reference_times.push(elapsed);
            } else {
                dirty_times.push(elapsed);
            }
            results[usize::from(reference)] = Some(result);
            if round == 0 {
                eprintln!("complete checking: reference={reference}, state builds={builds}");
            }
        }
        assert_eq!(results[0], results[1], "complete checked output differs");
    }
    dirty_times.sort();
    reference_times.sort();
    eprintln!(
        "complete checking, six alternating runs: dirty median {:?}, whole-pass median {:?}",
        dirty_times[3], reference_times[3]
    );
}

#[test]
fn call_free_direct_stores_leave_mutation_summaries_unbuilt() {
    let source = "machine produce() -> u8 { let mut value: u8 = 3; value = 4; value }";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize direct store");
    let syntax = parse_syntax_trees(&tokens).expect("parse direct store");
    let resolved = lower_syntax_trees(&syntax).expect("resolve direct store");
    let program = lower_symbol_resolved_trees(&resolved).expect("type direct store");
    let proof_plan = proof::obligations::build_proof_plan(&program);
    let operations = validation::infer_operational_may(&program);
    let borrow = build_borrow_facts(&program);
    assert!(borrow.calls.is_empty(), "fixture must have no calls");
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let baseline = build_semantic_facts(&program, &proof);
    let domains = build_domain_facts(&program, &baseline);
    let state = &program.machine_states(&program.machines()[0])[0];
    let typed_trees::statement::StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("fixture must declare the local before overwriting it");
    };

    for invocation in 0..2 {
        let mut semantic = baseline.clone();
        let before = StateMutationSummaryCache::build_count();
        let before_states = STATE_BUILDS.get();
        let flow = build_flow_facts(
            &program,
            &borrow,
            &proof,
            &mut semantic,
            &domains,
            &operations,
        );
        assert_eq!(
            StateMutationSummaryCache::build_count() - before,
            0,
            "invocation {invocation}: direct stores need no call-summary table"
        );
        assert_eq!(flow.control.states.len(), 1);
        assert_eq!(
            STATE_BUILDS.get() - before_states,
            1,
            "a first-pass fixed point needs no scratch replay or final rebuild"
        );
        assert!(flow.control.calls.is_empty());
        assert!(
            flow.invalidations.events.iter().any(|(_, event)| {
                event.mutated_root == facts::PlaceRoot::Symbol(local.symbol)
                    && matches!(
                        event.source,
                        checked_trees::FlowInvalidationSource::Statement { statement_index: 1 }
                    )
            }),
            "invocation {invocation}: the direct store must still invalidate the local's facts"
        );
    }
}

#[test]
fn flow_value_input_passes_share_one_mutation_summary_table_per_invocation() {
    let mut summary_builds = Vec::new();
    for written_parameter in ["other", "current"] {
        // Both programs retain the same declarations and call arguments. Only
        // the callee's write changes, so a later invocation needs fresh summaries.
        let source = format!(
            r#"
            machine touch(current: &mut u8, other: &mut u8) {{
                {written_parameter} = 9;
            }}
            machine produce() -> u8 {{
                let mut current: u8 = 3;
                let mut other: u8 = 2;
                touch(&mut current, &mut other);
                transition {{ _ -> first(current) }}
                state finish(value: u8) -> u8 {{ value }}
                state second(value: u8) -> u8 {{ transition {{ _ -> finish(value) }} }}
                state first(value: u8) -> u8 {{ transition {{ _ -> second(value) }} }}
            }}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize flow fixture");
        let syntax = parse_syntax_trees(&tokens).expect("parse flow fixture");
        let resolved = lower_syntax_trees(&syntax).expect("resolve flow fixture");
        let program = lower_symbol_resolved_trees(&resolved).expect("type flow fixture");
        let proof_plan = proof::obligations::build_proof_plan(&program);
        let operations = validation::infer_operational_may(&program);
        let borrow = build_borrow_facts(&program);
        let proof = build_proof_facts(&program, &proof_plan, &borrow);
        let baseline = build_semantic_facts(&program, &proof);
        let domains = build_domain_facts(&program, &baseline);
        let producer = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "produce")
            .expect("producer machine");
        let states = program.machine_states(producer);
        assert_eq!(
            states
                .iter()
                .skip(1)
                .map(|state| state.name.as_str())
                .collect::<Vec<_>>(),
            ["finish", "second", "first"],
            "the fixture must retain reverse declaration order"
        );

        // The initial pass reaches first, which delivers second's input late.
        // Dirty sweeps deliver finish's input, then final materialization
        // visits touch again. Both complete passes share one summary table;
        // a separate invocation must still derive its own callee-write facts.
        for invocation in 0..2 {
            let mut semantic = baseline.clone();
            let before = StateMutationSummaryCache::build_count();
            let flow = build_flow_facts(
                &program,
                &borrow,
                &proof,
                &mut semantic,
                &domains,
                &operations,
            );
            summary_builds.push((
                written_parameter,
                invocation,
                StateMutationSummaryCache::build_count() - before,
            ));

            for state in states.iter().skip(1) {
                let state_flow = flow
                    .control
                    .states
                    .iter()
                    .map(|(_, state)| state)
                    .find(|candidate| candidate.state_symbol == state.symbol)
                    .expect("named state flow");
                let parameter = program.state_parameters(state)[0].symbol;
                let place = canonical_place_from_symbol(parameter).expect("parameter place");
                let retained = crate::values::scalar_value_at_place(
                    &program,
                    &semantic,
                    flow.contexts
                        .semantic_context_refs
                        .span_or_empty(state_flow.entry_semantic_contexts)
                        .iter()
                        .map(|reference| semantic.contexts.get(reference.context)),
                    &place,
                );
                assert_eq!(
                    retained,
                    if written_parameter == "other" {
                        Some(ScalarValue::Integer(BigInt::from_u64(3)))
                    } else {
                        None
                    },
                    "write to {written_parameter}, invocation {invocation}, state {}: \
                     disjoint writes preserve the input; overlapping writes retire it",
                    state.name.as_str()
                );
            }
        }
    }

    // Check all semantic outcomes before the cost assertion, including on the
    // unfixed builder. Deltas isolate invocations from earlier test activity.
    assert_eq!(
        summary_builds,
        [
            ("other", 0, 1),
            ("other", 1, 1),
            ("current", 0, 1),
            ("current", 1, 1)
        ],
        "each build_flow_facts invocation owns one table across value-input passes"
    );
}
