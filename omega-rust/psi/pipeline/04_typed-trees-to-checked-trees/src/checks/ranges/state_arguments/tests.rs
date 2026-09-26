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
