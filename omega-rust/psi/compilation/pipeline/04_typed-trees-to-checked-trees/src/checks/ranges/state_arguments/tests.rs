use super::SymbolHandle;
use crate::checks::ranges::collect_state_argument_facts;
use crate::checks::ranges::state_arguments::MergedBound;
use crate::checks::ranges::state_arguments::MergedFact;
use crate::checks::ranges::state_arguments::ParameterFacts;
use crate::checks::ranges::state_arguments::StateArgumentFacts;
use crate::checks::ranges::state_arguments::collect_state_argument_facts_whole_pass;
use crate::checks::ranges::state_arguments::merge_contribution;
use crate::tests::front_end::{checked_program_result, typed_program};

thread_local! {
    pub(super) static STATE_TRANSFERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static WHOLE_PASS_REFERENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn compare_machine(
    source: &str,
    machine_name: Option<&str>,
) -> (Vec<StateArgumentFacts>, usize, usize) {
    let program = typed_program(source);
    let borrows = crate::borrow::build_borrow_facts(&program);
    let proof_plan = proof::obligations::build_proof_plan(&program);
    let values = crate::values::build_value_facts(&program, &proof_plan);
    let operators = crate::operators::build_operator_facts(&program, &values);
    let flow = super::super::cache_tests::range_flow_fixture(&program, &borrows);
    let frames = validation::CallFrameResolver::new(&program).unwrap();
    let machine = match machine_name {
        Some(name) => program
            .machines()
            .iter()
            .find(|machine| program.symbols.name(machine.symbol) == name)
            .unwrap_or_else(|| panic!("{name} machine")),
        None => program.machines().first().unwrap(),
    };
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

fn check_source(source: &str) -> Result<(), Vec<String>> {
    checked_program_result(source)
        .map(|_| ())
        .map_err(|diagnostics| {
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect()
        })
}

#[test]
fn projected_bounds_cross_states_that_do_not_read_the_projection() {
    for (statement, accepted) in [("", true), ("next.cursor = 4;", false)] {
        let source = format!(
            "data Context {{ items: [u8; 3]; cursor: u64; }}
             machine read(context: &mut Context) -> u8 {{
                 transition context.cursor < 3 {{
                     true -> relay(context)
                     _ -> (0)
                 }}
                 state relay(next: &mut Context) -> u8 {{
                     {statement}
                     transition {{ _ -> finish(next) }}
                 }}
                 state finish(last: &mut Context) -> u8 {{
                     last.items[last.cursor]
                 }}
             }}"
        );
        let outcome = check_source(&source);
        if accepted {
            assert!(outcome.is_ok(), "{outcome:?}");
        } else {
            assert!(
                outcome
                    .unwrap_err()
                    .iter()
                    .any(|message| message.contains("cannot prove index"))
            );
        }
    }
}

#[test]
fn receiver_bounds_from_parameter_contracts_cross_states_with_exact_write_invalidation() {
    for (comparison, statement, accepted) in [
        ("<", "", true),
        ("<", "self.other = 4;", true),
        ("<", "self.index = 4;", false),
        ("<", "self.overwrite();", false),
        ("<=", "", false),
    ] {
        let source = format!(
            "data Context {{ items: [u8; 3]; index: u64; other: u64; }}
             machine Context::overwrite(&mut self) {{ self.index = 4; }}
             machine Context::read(&mut self, bound: u64) -> u8
             requires bound <= 3u64; {{
                 transition self.index {comparison} bound {{
                     true -> relay()
                     _ -> (0)
                 }}
                 state relay(&mut self) -> u8 {{
                     {statement}
                     transition {{ _ -> finish() }}
                 }}
                 state finish(&mut self) -> u8 {{ self.items[self.index] }}
             }}"
        );
        let outcome = check_source(&source);
        if accepted {
            assert!(outcome.is_ok(), "{comparison} | {statement}: {outcome:?}");
        } else {
            assert!(
                outcome
                    .unwrap_err()
                    .iter()
                    .any(|message| message.contains("cannot prove index")),
                "{comparison} | {statement}: expected an index rejection"
            );
        }
    }
}

#[test]
fn receiver_bounds_from_parameter_contracts_meet_every_incoming_edge() {
    let source = "data Context { items: [u8; 3]; index: u64; }
         machine Context::read(&mut self, bound: u64) -> u8
         requires bound <= 3u64; {
             transition self.index < bound {
                 true -> relay()
                 _ -> unguarded()
             }
             state unguarded(&mut self) -> u8 {
                 transition { _ -> relay() }
             }
             state relay(&mut self) -> u8 {
                 transition { _ -> finish() }
             }
             state finish(&mut self) -> u8 { self.items[self.index] }
         }";
    let diagnostics = check_source(source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|message| message.contains("cannot prove index")),
        "{diagnostics:?}"
    );
}

#[test]
fn receiver_bounds_from_parameter_contracts_use_immutable_record_where_facts() {
    let source = "data Limit where count <= 3u64 { count: u64; }
         data Context { items: [u8; 3]; index: u64; }
         machine Context::read(&mut self, limit: Limit) -> u8 {
             transition self.index < limit.count {
                 true -> relay()
                 _ -> (0)
             }
             state relay(&mut self) -> u8 {
                 transition { _ -> finish() }
             }
             state finish(&mut self) -> u8 { self.items[self.index] }
         }";
    let outcome = check_source(source);
    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn receiver_bounds_from_parameter_contracts_do_not_assume_open_receiver_domain() {
    let source = "data Context where bound <= 3u64 {
             items: [u8; 3]; index: u64; bound: u64;
         }
         machine Context::read(&mut self) -> u8 {
             self.bound = 4;
             transition self.index < self.bound {
                 true -> relay()
                 _ -> (0)
             }
             state relay(&mut self) -> u8 { self.items[self.index] }
         }";
    assert!(check_source(source).is_err());
    // Integer default-domain checking currently rejects the store before the
    // range pass runs. Exercise that pass independently with its ordinary fact
    // producers so the earlier refusal cannot conceal an admitted stale bound.
    let program = typed_program(source);
    let borrows = crate::borrow::build_borrow_facts(&program);
    let proof_plan = proof::obligations::build_proof_plan(&program);
    let values = crate::values::build_value_facts(&program, &proof_plan);
    let operators = crate::operators::build_operator_facts(&program, &values);
    let flow = super::super::cache_tests::range_flow_fixture(&program, &borrows);
    let frames = validation::CallFrameResolver::new(&program).unwrap();
    let incoming =
        super::super::incoming_guards::IncomingGuardIndex::build(&program, Some(&frames));
    let summaries = crate::flow::StateMutationSummaryCache::default();
    let diagnostics = super::super::check_indexed_accesses(
        &program,
        &operators,
        &borrows,
        &flow,
        Some(&frames),
        &incoming,
        &summaries,
    )
    .unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove index")),
        "{diagnostics:?}"
    );
}

/// An ensured call result handed across a transition substitutes the callee's
/// discharged exit proof for the destination parameter's bounds: the consumer
/// reads the contract (`ensures result < K` bounds this occurrence below `K`)
/// instead of weakening the index admission downstream. The same transport
/// covers a bound name and a member store's display label — the collection
/// replay mirrors the checking pass's ensured-result seeding — and the
/// `>= 0` half a signed parameter still owes. A missing or insufficient
/// contract keeps the ordinary rejection.
#[test]
fn ensured_result_bounds_transport_through_transition_arguments() {
    for (callee, argument, parameter_type, accepted) in [
        (
            "machine Main::pick(&self) -> u64 ensures result < 4u64 { 2 }",
            "self.pick()",
            "u64",
            true,
        ),
        (
            "machine Main::pick(&self) -> u64 ensures result <= 3u64 { 2 }",
            "self.pick()",
            "u64",
            true,
        ),
        (
            "machine Main::pick(&self) -> u64 ensures result == 2u64 { 2 }",
            "self.pick()",
            "u64",
            true,
        ),
        // `result <= 4` still permits 4, which is out of range.
        (
            "machine Main::pick(&self) -> u64 ensures result <= 4u64 { 2 }",
            "self.pick()",
            "u64",
            false,
        ),
        // No contract bound keeps the ordinary rejection.
        (
            "machine Main::pick(&self) -> u64 { 2 }",
            "self.pick()",
            "u64",
            false,
        ),
        // A signed parameter owes its lower half to the ensured `>= 0`.
        (
            "machine Main::pick(&self) -> i64 ensures result >= 0i64 && result < 4i64 { 2 }",
            "self.pick()",
            "i64",
            true,
        ),
        (
            "machine Main::pick(&self) -> i64 ensures result < 4i64 { 2 }",
            "self.pick()",
            "i64",
            false,
        ),
        // A bound name transports the same contract through its label.
        (
            "machine Main::pick(&self) -> u64 ensures result < 4u64 { 2 }",
            "i",
            "u64",
            true,
        ),
        // A member store transports it through the display label.
        (
            "machine Main::pick(&self) -> u64 ensures result < 4u64 { 2 }",
            "self.slot",
            "u64",
            true,
        ),
        // Reassignment to an unbounded call retires the stale bound.
        (
            "machine Main::pick(&self) -> u64 ensures result < 4u64 { 2 }",
            "reassigned",
            "u64",
            false,
        ),
    ] {
        let (prefix, argument) = match argument {
            "i" => ("let i: u64 = self.pick();", "i"),
            "self.slot" => ("self.slot = self.pick();", "self.slot"),
            "reassigned" => ("let mut i: u64 = self.pick(); i = self.raw();", "i"),
            _ => ("", argument),
        };
        let source = format!(
            "data Main {{ cells: [u8; 4]; slot: u64; }}
            {callee}
            machine Main::raw(&self) -> u64 {{ 7 }}
            machine Main::run(&mut self) -> u8 {{
                {prefix}
                transition {{ _ -> load({argument}) }}
                state load(&mut self, index: {parameter_type}) -> u8 {{ self.cells[index] }}
            }}"
        );
        let result = check_source(&source);
        assert_eq!(
            result.is_ok(),
            accepted,
            "{callee} | {argument}: {result:?}"
        );
    }

    // The collected facts carry the transported exclusive bound: `ensures
    // result < 4` enters `load`'s `index` parameter as `index < 4`.
    let (facts, _, _) = compare_machine(
        "data Main { cells: [u8; 4]; }
        machine Main::pick(&self) -> u64 ensures result < 4u64 { 2 }
        machine Main::run(&mut self) -> u8 {
            transition { _ -> load(self.pick()) }
            state load(&mut self, index: u64) -> u8 { self.cells[index] }
        }",
        Some("Main::run"),
    );
    let index = facts
        .iter()
        .flat_map(|facts| &facts.parameters)
        .find(|parameter| parameter.name == "index")
        .expect("load index parameter facts");
    assert_eq!(index.upper_bound.get(), Some(4));
}

/// A receiver length crosses each transition once. Both the receiver-length
/// rows and the receiver storage rows render `self.text`'s exact length, and
/// the destination seeds one dependency row per incoming row; keeping both
/// renderings doubled the seeded rows at every state, so the 29- to 45-state
/// sample machines never finished collecting. One row per state still carries
/// the length to the end of the chain.
#[test]
fn receiver_lengths_cross_a_state_chain_once_per_extent() {
    const CHAIN: usize = 12;
    let mut states = String::new();
    for position in 1..CHAIN {
        states.push_str(&format!(
            "state s{position}(&mut self) -> u8 {{ transition {{ _ -> s{}() }} }}\n",
            position + 1
        ));
    }
    let source = format!(
        "domain [u8; 4]::Utf8 requires valid_utf8(self);
         data Main {{ text: [u8; 4] in Utf8; }}
         machine Main::run(&mut self) -> u8 {{
             self.text = \"abcd\";
             transition {{ _ -> s1() }}
             {states}
             state s{CHAIN}(&mut self) -> u8 {{ self.text[3] }}
         }}"
    );
    let outcome = check_source(&source);
    assert!(outcome.is_ok(), "{outcome:?}");
    let (facts, _, _) = compare_machine(&source, Some("Main::run"));
    assert_eq!(facts.len(), CHAIN);
    for state in &facts {
        assert_eq!(
            state.receiver_lengths.as_ref().map(Vec::len),
            Some(1),
            "{state:?}"
        );
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
                        non_negative: MergedFact::Unseen,
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
