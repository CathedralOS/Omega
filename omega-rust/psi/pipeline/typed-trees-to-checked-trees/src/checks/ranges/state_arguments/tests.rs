use super::SymbolHandle;
use crate::checks::ranges::collect_state_argument_facts;
use crate::checks::ranges::state_arguments::MAX_PROPAGATION_PASSES;
use crate::checks::ranges::state_arguments::MergedBound;
use crate::checks::ranges::state_arguments::MergedFact;
use crate::checks::ranges::state_arguments::ParameterFacts;
use crate::checks::ranges::state_arguments::ParameterIndexProof;
use crate::checks::ranges::state_arguments::StateArgumentFacts;
use crate::checks::ranges::state_arguments::collect_state_argument_facts_whole_pass;
use crate::checks::ranges::state_arguments::merge_contribution;

thread_local! {
    pub(super) static STATE_TRANSFERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static WHOLE_PASS_REFERENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn compare(source: &str) -> (Vec<StateArgumentFacts>, usize, usize) {
    compare_machine(source, None)
}

fn compare_machine(
    source: &str,
    machine_name: Option<&str>,
) -> (Vec<StateArgumentFacts>, usize, usize) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
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
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let actual = crate::lower_typed_trees(program.clone());
        let _restore = Restore(WHOLE_PASS_REFERENCE.replace(true));
        let reference = crate::lower_typed_trees(program);
        assert_eq!(actual, reference);
        assert_eq!(actual.is_ok(), incoming == "2", "{actual:?}");
    }
}

fn check_source(source: &str) -> Result<(), Vec<String>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    crate::lower_typed_trees(program)
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

/// A bound name that aliases another bound name carries the source's proven
/// bounds into a transition argument: `let j = i` records `j` as a full-extent
/// alias of `i` (`alias_index`), so the collection replay transports `i`'s
/// ensured-result, `requires`, or member-store bound through `j` exactly the
/// way the checking pass does inside the body. Chained copies, reassignment
/// into a mutable local, and member-source aliases take the same path; an
/// unbounded source or a later unbounded write still rejects.
#[test]
fn bound_name_alias_transports_through_transition_arguments() {
    for (prefix, argument, accepted) in [
        // `let j = i` inherits `i`'s ensured-result bound.
        ("let i: u64 = self.pick(); let j: u64 = i;", "j", true),
        // A chained copy inherits through the middle alias.
        (
            "let i: u64 = self.pick(); let j: u64 = i; let k: u64 = j;",
            "k",
            true,
        ),
        // Assigning `i` into a mutable local aliases the same bound.
        (
            "let i: u64 = self.pick(); let mut j: u64 = 0; j = i;",
            "j",
            true,
        ),
        // A member store's seeded bound reaches `j` through `let j =
        // self.slot`.
        (
            "self.slot = self.pick(); let j: u64 = self.slot;",
            "j",
            true,
        ),
        // An unbounded source keeps the ordinary rejection.
        ("let i: u64 = self.raw(); let j: u64 = i;", "j", false),
        // A later unbounded write retires the aliased bound.
        (
            "let i: u64 = self.pick(); let mut j: u64 = i; j = self.raw();",
            "j",
            false,
        ),
    ] {
        let source = format!(
            "data Main {{ cells: [u8; 4]; slot: u64; }}
            machine Main::pick(&self) -> u64 ensures result < 4u64 {{ 2 }}
            machine Main::raw(&self) -> u64 {{ 7 }}
            machine Main::run(&mut self) -> u8 {{
                {prefix}
                transition {{ _ -> load({argument}) }}
                state load(&mut self, index: u64) -> u8 {{ self.cells[index] }}
            }}"
        );
        let result = check_source(&source);
        assert_eq!(
            result.is_ok(),
            accepted,
            "{prefix} | {argument}: {result:?}"
        );
    }

    // A `requires`-proven parameter bound aliases the same way: `let j = i`
    // transports `i < 4` without any call contract in the chain.
    let result = check_source(
        "data Main { cells: [u8; 4]; }
        machine Main::run(&mut self, i: u64) -> u8 requires i < 4u64 {
            let j: u64 = i;
            transition { _ -> load(j) }
            state load(&mut self, index: u64) -> u8 { self.cells[index] }
        }",
    );
    assert!(result.is_ok(), "{result:?}");

    // The collected facts carry the aliased exclusive bound: `j < 4` enters
    // `load`'s `index` parameter as `index < 4`.
    let (facts, _, _) = compare_machine(
        "data Main { cells: [u8; 4]; }
        machine Main::pick(&self) -> u64 ensures result < 4u64 { 2 }
        machine Main::run(&mut self) -> u8 {
            let i: u64 = self.pick();
            let j: u64 = i;
            transition { _ -> load(j) }
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

/// Member stores and subslice bindings record the same seeds in the
/// collection replay that the checking pass records in the body: a folded
/// field integer (`self.slot = 2`), an offset index bound
/// (`self.jp = self.i + 1`), and a subslice's shrunk window floor
/// (`let w = base[1..]`). Each keys on the target's place or the bound
/// name's label, so a later `-> load(...)` transports the fact into the
/// destination parameter's merged facts exactly as the body would. An
/// unbounded or out-of-range store keeps the ordinary rejection.
#[test]
fn member_store_and_window_facts_transport_through_transition_arguments() {
    for (fields, requires, prefix, argument, accepted) in [
        // A folded field integer transports as the parameter's exact value.
        (
            "cells: [u8; 4]; slot: u64;",
            "",
            "self.slot = 2;",
            "self.slot",
            true,
        ),
        // The same folded value out of range still rejects.
        (
            "cells: [u8; 4]; slot: u64;",
            "",
            "self.slot = 9;",
            "self.slot",
            false,
        ),
        // An unbounded call store carries no folded integer.
        (
            "cells: [u8; 4]; slot: u64;",
            "",
            "self.slot = self.raw();",
            "self.slot",
            false,
        ),
        // `self.jp = self.i + 1` transports `self.i < 4` as `jp < 5`.
        (
            "cells: [u8; 8]; i: u64; jp: u64;",
            "requires self.i < 4u64",
            "self.jp = self.i + 1;",
            "self.jp",
            true,
        ),
        // An unbounded `self.i` write retires the bound `self.jp` would
        // have inherited.
        (
            "cells: [u8; 8]; i: u64; jp: u64;",
            "requires self.i < 4u64",
            "self.i = self.raw(); self.jp = self.i + 1;",
            "self.jp",
            false,
        ),
    ] {
        let source = format!(
            "data Main {{ {fields} }}
            machine Main::raw(&self) -> u64 {{ 7 }}
            machine Main::run(&mut self) -> u8 {requires} {{
                {prefix}
                transition {{ _ -> load({argument}) }}
                state load(&mut self, index: u64) -> u8 {{ self.cells[index] }}
            }}"
        );
        let result = check_source(&source);
        assert_eq!(
            result.is_ok(),
            accepted,
            "{requires} {prefix} | {argument}: {result:?}"
        );
    }

    // A subslice binding transports the base's floor minus the window's
    // start: `let w = base[1..]` under `requires base.len >= 4` gives `w` a
    // minimum length of 3, which is exactly what `items[2]` needs. The
    // rebinding form (`w = base[1..]`) mirrors the same window facts. A
    // base without the floor rejects both the subslice and the index.
    for (prefix, accepted) in [
        ("let w: &[u8] = base[1..];", true),
        ("let mut w: &[u8] = base[0..]; w = base[1..];", true),
    ] {
        let source = format!(
            "machine Main::run(&mut self, base: &[u8]) -> u8 requires base.len >= 4u64 {{
                {prefix}
                transition {{ _ -> load(w) }}
                state load(&mut self, items: &[u8]) -> u8 {{ items[2] }}
            }}"
        );
        let result = check_source(&source);
        assert_eq!(result.is_ok(), accepted, "{prefix}: {result:?}");
    }
    let result = check_source(
        "machine Main::run(&mut self, base: &[u8]) -> u8 {
            let w: &[u8] = base[1..];
            transition { _ -> load(w) }
            state load(&mut self, items: &[u8]) -> u8 { items[2] }
        }",
    );
    assert!(result.is_err());

    // The collected facts carry the transported values: the folded field
    // integer enters `load`'s `index` parameter as the exact value 2, the
    // offset bound enters as `jp < 5`, and the shrunk window enters as
    // `items`'s minimum length 3.
    let (facts, _, _) = compare_machine(
        "data Main { cells: [u8; 4]; slot: u64; }
        machine Main::run(&mut self) -> u8 {
            self.slot = 2;
            transition { _ -> load(self.slot) }
            state load(&mut self, index: u64) -> u8 { self.cells[index] }
        }",
        Some("Main::run"),
    );
    let index = facts
        .iter()
        .flat_map(|facts| &facts.parameters)
        .find(|parameter| parameter.name == "index")
        .expect("load index parameter facts");
    assert_eq!(index.integer.get(), Some(2));

    let (facts, _, _) = compare_machine(
        "data Main { cells: [u8; 8]; i: u64; jp: u64; }
        machine Main::run(&mut self) -> u8 requires self.i < 4u64 {
            self.jp = self.i + 1;
            transition { _ -> load(self.jp) }
            state load(&mut self, index: u64) -> u8 { self.cells[index] }
        }",
        Some("Main::run"),
    );
    let index = facts
        .iter()
        .flat_map(|facts| &facts.parameters)
        .find(|parameter| parameter.name == "index")
        .expect("load index parameter facts");
    assert_eq!(index.upper_bound.get(), Some(5));

    let (facts, _, _) = compare_machine(
        "machine Main::run(&mut self, base: &[u8]) -> u8 requires base.len >= 4u64 {
            let w: &[u8] = base[1..];
            transition { _ -> load(w) }
            state load(&mut self, items: &[u8]) -> u8 { items[2] }
        }",
        Some("Main::run"),
    );
    let items = facts
        .iter()
        .flat_map(|facts| &facts.parameters)
        .find(|parameter| parameter.name == "items")
        .expect("load items parameter facts");
    assert_eq!(items.minimum_length.get(), Some(3));
}

/// A recursive state indexes a fixed-extent slice BEFORE its `has_next`
/// arm guard runs: `let found = items[index] == 0` must already know
/// `index` is in range when the state is entered. The
/// `has_next -> find_at(items, count, next_index)` edge evaluates its
/// arguments under the arm guard `next_index < count`, and `count`'s
/// enforced declared range turns that strict comparison into the
/// argument's exclusive bound `next_index < 16` — the endpoint mint sees
/// through the boolean local's name to the aliased comparison. That bound
/// meets the literal entry edge's `0` as the destination parameter's
/// merged `index < 16`, so `items[index]` proves before the guard is
/// evaluated.
///
/// The count must carry an ENFORCED declared range: an unrelated `u64`
/// scalar mints no bound, a `[0..=17]` ceiling mints one element past the
/// extent, `<=` permits `next_index == count == 16` (genuinely out of
/// range, since the strict form's `-1` is what stays inside), a second
/// incoming edge with an unbounded argument poisons the merge, and an
/// argument the guard never mentions carries no bound either.
#[test]
fn declared_count_guard_transports_through_transition_arguments() {
    let find_at = |count_type: &str, entry_index: &str, recurse_argument: &str, guard: &str| {
        format!(
                "data Main {{ cells: [u8; 16]; count: {count_type}; }}
                machine Main::raw(&self) -> u64 {{ 7 }}
                machine Main::run(&mut self) -> u8 {{
                    let items: &[u8] = self.cells.as_slice();
                    transition self.count > 0 {{
                        true -> find_at(items, self.count, {entry_index})
                        false -> 0
                    }}
                    state find_at(&mut self, items: &[u8], count: {count_type}, index: u64 in Wrapping) -> u8 {{
                        let found: bool = items[index] == 0;
                        let next_index: u64 in Wrapping = index + 1;
                        let has_next: bool = {guard};
                        transition {{
                            found -> (items[index])
                            has_next -> find_at(items, count, {recurse_argument})
                            _ -> 0
                        }}
                    }}
                }}"
            )
    };
    for (count_type, entry_index, recurse_argument, guard, accepted) in [
        // `next_index < count` with `count: u64 [0..=16]` mints
        // `next_index < 16` — the recursive edge's `index` bound.
        (
            "u64 [0..=16]",
            "0",
            "next_index",
            "next_index < count",
            true,
        ),
        // An unrelated unbounded scalar count proves nothing about the
        // extent: the edge argument stays unbounded.
        ("u64", "0", "next_index", "next_index < count", false),
        // A ceiling wider than the extent mints `next_index < 17`, one
        // element past the collection.
        (
            "u64 [0..=17]",
            "0",
            "next_index",
            "next_index < count",
            false,
        ),
        // `next_index <= count` permits `next_index == count == 16` —
        // genuinely out of range, so the rejection is the sound answer.
        (
            "u64 [0..=16]",
            "0",
            "next_index",
            "next_index <= count",
            false,
        ),
        // `index` stays provable through the `next_index = index + 1`
        // offset (`index < 15` follows), but `index + 2` pushes the
        // merged argument bound one element past the extent.
        (
            "u64 [0..=16]",
            "0",
            "index + 2",
            "next_index < count",
            false,
        ),
        // A second incoming edge whose argument carries no bound poisons
        // the merged parameter facts.
        (
            "u64 [0..=16]",
            "self.raw()",
            "next_index",
            "next_index < count",
            false,
        ),
    ] {
        let result = check_source(&find_at(count_type, entry_index, recurse_argument, guard));
        assert_eq!(
            result.is_ok(),
            accepted,
            "{count_type} | {entry_index} | {recurse_argument} | {guard}: {result:?}"
        );
    }

    // The collected facts carry the transported exclusive bound:
    // `next_index < 16` under `has_next` enters `find_at`'s `index`
    // parameter as `index < 16`.
    let (facts, _, _) = compare_machine(
        "data Main { cells: [u8; 16]; count: u64 [0..=16]; }
        machine Main::run(&mut self) -> u8 {
            let items: &[u8] = self.cells.as_slice();
            transition self.count > 0 {
                true -> find_at(items, self.count, 0)
                false -> 0
            }
            state find_at(&mut self, items: &[u8], count: u64 [0..=16], index: u64 in Wrapping) -> u8 {
                let found: bool = items[index] == 0;
                let next_index: u64 in Wrapping = index + 1;
                let has_next: bool = next_index < count;
                transition {
                    found -> (items[index])
                    has_next -> find_at(items, count, next_index)
                    _ -> 0
                }
            }
        }",
        Some("Main::run"),
    );
    let index = facts
        .iter()
        .flat_map(|facts| &facts.parameters)
        .find(|parameter| parameter.name == "index")
        .expect("find_at index parameter facts");
    assert_eq!(index.upper_bound.get(), Some(16));
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
