//! Structured caller-visible mutation summaries.
//!
//! `validation::CallFrameResolver` remains the single owner of the
//! complete-or-opaque call and cycle law. This module retains symbol-based
//! field/range places for flow invalidation and propagates those places across
//! the calls which the shared resolver admitted as complete.

use super::local_origins::rebase_local_write_places;
use super::*;
use crate::flow::mutation::receiver::canonical_receiver_place_for_call_site;

#[derive(Debug, Clone, Default)]
pub(crate) struct StateMutationSummaryCache {
    // Completed summaries depend only on the immutable program and borrow
    // facts. Publish after the fixed point, never during recursive inference.
    states: std::sync::OnceLock<Vec<StateMutationSummary>>,
}

#[cfg(test)]
thread_local! {
    static SUMMARY_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SUMMARY_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
impl StateMutationSummaryCache {
    pub(crate) fn build_count() -> usize {
        SUMMARY_BUILDS.get()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateMutationSummary {
    state_symbol: SymbolHandle,
    complete: bool,
    writes: Vec<CanonicalPlace>,
}

pub(super) fn instantiate_known_call_mutation_summary_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    cache: &StateMutationSummaryCache,
    namespace: WritePlaceNamespace,
) -> Option<Vec<CanonicalPlace>> {
    let target_state = find_state(program, borrow_call.target_symbol)?;
    let summary_places = state_mutation_summary_places(program, borrow, cache, target_state)?;

    let mut instantiated = Vec::new();
    for summary_place in summary_places {
        let places = instantiate_call_relative_places(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
            summary_place,
            namespace,
        )?;
        for place in places {
            if !instantiated.contains(&place) {
                instantiated.push(place);
            }
        }
    }

    Some(instantiated)
}

fn state_mutation_summary_places<'cache>(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    cache: &'cache StateMutationSummaryCache,
    state: &typed_trees::state::State,
) -> Option<&'cache [CanonicalPlace]> {
    cache
        .states
        .get_or_init(|| build_state_mutation_summaries(program, borrow))
        .iter()
        .find(|entry| entry.state_symbol == state.symbol)
        .filter(|entry| entry.complete)
        .map(|entry| entry.writes.as_slice())
}

fn build_state_mutation_summaries(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
) -> Vec<StateMutationSummary> {
    #[cfg(test)]
    SUMMARY_BUILDS.set(SUMMARY_BUILDS.get() + 1);
    let mut states = Vec::new();
    let mut inferred_completeness = Vec::new();
    if let Some(resolver) = validation::CallFrameResolver::new(program) {
        for machine in program.machines() {
            for (state, frame) in program
                .machine_states(machine)
                .iter()
                .zip(resolver.inferred_machine_state_write_frames(machine))
            {
                inferred_completeness.push((state.symbol, frame.is_complete()));
            }
        }
    }

    for borrow_state in borrow.states.iter().map(|(_, state)| state) {
        let Some(state) = find_state(program, borrow_state.state_symbol) else {
            continue;
        };
        let direct_writes = collect_state_mutation_summary_places(program, state);
        let direct_complete = direct_writes.is_some();
        let writes = direct_writes.unwrap_or_default();
        states.push(StateMutationSummary {
            state_symbol: state.symbol,
            complete: direct_complete
                && state_has_concrete_body_signature(program, state)
                && inferred_completeness
                    .iter()
                    .find_map(|(symbol, complete)| (*symbol == state.symbol).then_some(*complete))
                    .unwrap_or(false),
            writes,
        });
    }

    propagate_state_mutation_summaries(program, borrow, &mut states);
    states
}

fn propagate_state_mutation_summaries(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    states: &mut [StateMutationSummary],
) {
    // These are invocation-local dense summary positions, not durable symbol
    // identities. Resolve each dependency once while retaining authored call order.
    let calls: Vec<_> = states
        .iter()
        .map(|state| {
            borrow_state_for_symbol(borrow, state.state_symbol)
                .map(|borrow_state| borrow.calls.span_or_empty(borrow_state.calls))
                .unwrap_or_default()
                .iter()
                .map(|call| (call, summary_index_from(states, call.target_symbol)))
                .collect::<Vec<_>>()
        })
        .collect();
    let owners: Vec<_> = states
        .iter()
        .map(|summary| {
            find_state(program, summary.state_symbol)
                .map(|state| (state, machine_symbol_for_state(program, state)))
        })
        .collect();
    let mut callers = vec![Vec::new(); states.len()];
    for (caller_index, dependencies) in calls.iter().enumerate() {
        for (_, target_index) in dependencies {
            if let Some(target_index) = target_index
                && !callers[*target_index].contains(&caller_index)
            {
                callers[*target_index].push(caller_index);
            }
        }
    }
    let mut dirty: Vec<_> = (0..states.len()).collect();
    let mut next_dirty = Vec::new();
    let mut scheduled = vec![false; states.len()];
    let mut updates = Vec::new();
    while !dirty.is_empty() {
        // Publish after the round: later callers must not observe this round's
        // additions early, which would change first-discovery write ordering.
        for caller_index in dirty.drain(..) {
            if !states[caller_index].complete {
                continue;
            }
            #[cfg(test)]
            SUMMARY_VISITS.set(SUMMARY_VISITS.get() + 1);
            let caller_symbol = states[caller_index].state_symbol;
            let Some((caller_state, caller_machine)) = owners[caller_index] else {
                continue;
            };
            let mut additions = Vec::new();
            let mut complete = true;
            for (call, target_index) in &calls[caller_index] {
                let Some(target_index) = target_index else {
                    complete = false;
                    break;
                };
                let target = &states[*target_index];
                if !target.complete {
                    complete = false;
                    break;
                }
                for write in &target.writes {
                    let Some(instantiated_places) = instantiate_call_relative_places(
                        program,
                        caller_machine,
                        caller_symbol,
                        call,
                        write,
                        WritePlaceNamespace::Storage,
                    ) else {
                        complete = false;
                        break;
                    };
                    for instantiated in instantiated_places {
                        if state_summary_exposes_place(program, caller_state, &instantiated)
                            && !states[caller_index].writes.contains(&instantiated)
                            && !additions.contains(&instantiated)
                        {
                            additions.push(instantiated);
                        }
                    }
                }
                if !complete {
                    break;
                }
            }
            if !complete || !additions.is_empty() {
                updates.push((caller_index, complete, additions));
            }
        }
        for (caller_index, complete, additions) in updates.drain(..) {
            states[caller_index].complete = complete;
            if complete {
                states[caller_index].writes.extend(additions);
            } else {
                states[caller_index].writes.clear();
            }
            for &dependent in &callers[caller_index] {
                if !scheduled[dependent] {
                    scheduled[dependent] = true;
                    next_dirty.push(dependent);
                }
            }
        }
        next_dirty.sort_unstable();
        for &caller_index in &next_dirty {
            scheduled[caller_index] = false;
        }
        std::mem::swap(&mut dirty, &mut next_dirty);
    }
}

fn borrow_state_for_symbol(
    borrow: &BorrowFacts,
    state_symbol: SymbolHandle,
) -> Option<&StateBorrowFact> {
    borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == state_symbol)
}

fn summary_index_from(
    summaries: &[StateMutationSummary],
    state_symbol: SymbolHandle,
) -> Option<usize> {
    summaries
        .iter()
        .position(|summary| summary.state_symbol == state_symbol)
}

fn machine_symbol_for_state(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
) -> SymbolHandle {
    program
        .machines()
        .iter()
        .find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
        })
        .map(|machine| machine.symbol)
        .unwrap_or_else(SymbolHandle::invalid)
}

fn state_has_concrete_body_signature(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
) -> bool {
    program
        .machines()
        .iter()
        .find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
        })
        .is_some_and(|machine| {
            machine.body_is_present
                && program
                    .state_parameters(state)
                    .iter()
                    .all(|parameter| parameter.type_reference.is_valid())
        })
}

// Owned primitive formals contain no references: writes change callee storage,
// not the caller's delivered argument. Keep reference-bearing roots visible.
fn state_summary_exposes_place(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    place: &CanonicalPlace,
) -> bool {
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return false;
    };
    root == machine_symbol_for_state(program, state)
        || program.state_parameters(state).iter().any(|parameter| {
            parameter.symbol == root
                && (parameter.is_self
                    || program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none())
        })
}

fn collect_state_mutation_summary_places(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
) -> Option<Vec<CanonicalPlace>> {
    let machine_symbol = program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
                .then_some(machine.symbol)
        })
        .unwrap_or_else(SymbolHandle::invalid);
    let mut writes = Vec::new();

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        if let StatementNode::RootBinding(binding) = statement {
            let receiver = canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                binding.receiver,
            )?;
            let places = super::local_origins::rebase_local_write_places(
                program,
                state.symbol,
                statement_index,
                receiver,
            )?;
            for place in places {
                if state_summary_exposes_place(program, state, &place) && !writes.contains(&place) {
                    writes.push(place);
                }
            }
            continue;
        }
        let StatementNode::Assignment(_) = statement else {
            continue;
        };
        let places = super::local_origins::assignment_storage_places(
            program,
            machine_symbol,
            state.symbol,
            statement_index,
            statement,
        )?;
        for place in places {
            if state_summary_exposes_place(program, state, &place) && !writes.contains(&place) {
                writes.push(place);
            }
        }
    }

    Some(writes)
}

fn instantiate_call_relative_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
    relative_place: &CanonicalPlace,
    namespace: WritePlaceNamespace,
) -> Option<Vec<CanonicalPlace>> {
    let facts::PlaceRoot::Symbol(parameter_symbol) = relative_place.root else {
        return None;
    };
    let call_site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    if matches!(namespace, WritePlaceNamespace::Storage)
        && !super::operand_coordinates::call_operands_have_builtin_coordinates(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            &call_site,
        )
    {
        return None;
    }
    let target_state = find_state(program, borrow_call.target_symbol)?;
    let target_machine_symbol = program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == target_state.symbol)
                .then_some(machine.symbol)
        })
        .unwrap_or_else(SymbolHandle::invalid);
    let mut argument_index = 0usize;

    for parameter in program.state_parameters(target_state) {
        let base_place = if parameter.is_self {
            if parameter.symbol != parameter_symbol && target_machine_symbol != parameter_symbol {
                continue;
            }
            canonical_receiver_place_for_call_site(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                &call_site,
            )
        } else {
            let argument = call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);
            if parameter.symbol != parameter_symbol {
                continue;
            }
            if let Some(expression) = argument
                && matches!(namespace, WritePlaceNamespace::AccessRoute)
                && matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
                )
                && let Some(places) = crate::flow::literal_argument_access_places(
                    program,
                    caller_state_symbol,
                    borrow_call.statement_index,
                    expression,
                    parameter.type_reference,
                    &relative_place.segments,
                )
            {
                return Some(places);
            }
            argument.and_then(|expression| {
                canonical_place_from_expression_in_state(
                    program,
                    caller_state_symbol,
                    borrow_call.statement_index,
                    expression,
                )
                .or_else(|| canonical_place_from_expression(program, expression))
            })
        }?;

        let mut instantiated = base_place;
        instantiated
            .segments
            .extend(relative_place.segments.iter().copied());
        return match namespace {
            WritePlaceNamespace::AccessRoute => Some(vec![instantiated]),
            WritePlaceNamespace::Storage if !storage_place_has_declared_identity(&instantiated) => {
                // Expression identity describes evaluation, not storage. The
                // shared complete frame owns origins of literal/computed
                // actuals, including their reference-bearing leaves.
                super::shared_call_storage_places(
                    program,
                    caller_machine_symbol,
                    caller_state_symbol,
                    borrow_call,
                    validation::CallFrameResolver::new(program).as_ref(),
                )
            }
            WritePlaceNamespace::Storage => rebase_local_write_places(
                program,
                caller_state_symbol,
                borrow_call.statement_index,
                instantiated,
            ),
        };
    }

    None
}

fn storage_place_has_declared_identity(place: &CanonicalPlace) -> bool {
    matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        && place.segments.iter().all(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
            facts::PlaceSegment::Case { variant } => variant.is_valid(),
            _ => true,
        })
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    fn typed(source: &str) -> typed_trees::TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
    }

    #[test]
    fn vec_method_spelling_cannot_complete_a_signature_only_body() {
        for method in ["push", "pop", "as_mut_slice", "index_mut"] {
            let mut program = typed(&format!(
                "data Vec {{ value: u64; }} machine Vec::{method}(&mut self) {{ self.value = 1; }}"
            ));
            program.machines_mut()[0].body_is_present = false;
            let borrows = crate::build_borrow_facts(&program);
            let state = &program.machine_states(&program.machines()[0])[0];
            assert!(
                state_mutation_summary_places(
                    &program,
                    &borrows,
                    &StateMutationSummaryCache::default(),
                    state
                )
                .is_none(),
                "{method} is an ordinary declaration, not completeness authority"
            );
        }
    }

    #[test]
    fn vec_named_concrete_method_retains_only_its_actual_write() {
        let program =
            typed("data Vec { value: u64; } machine Vec::push(&mut self) { self.value = 1; }");
        let borrows = crate::build_borrow_facts(&program);
        let state = &program.machine_states(&program.machines()[0])[0];
        let cache = StateMutationSummaryCache::default();
        let places = state_mutation_summary_places(&program, &borrows, &cache, state).unwrap();
        assert_eq!(places.len(), 1);
        assert_eq!(
            places,
            collect_state_mutation_summary_places(&program, state).unwrap()
        );
        assert!(
            !places[0].segments.is_empty(),
            "no name-based whole-receiver injection"
        );
    }

    #[test]
    fn retained_core_vector_surface_keeps_mutable_receiver_invalidation() {
        let source = format!(
            "{}\nmachine probe(items: &mut Vec<u8>) {{ items.push(1); }}",
            include_str!("../../../../../../../source/library/core/vec.omg")
        );
        for origin in [source::SourceOrigin::Toolchain, source::SourceOrigin::User] {
            let root = std::path::PathBuf::from("fixture/core");
            let mut sources = source::SourceMap::default();
            let source_id = sources
                .add_with_metadata(root.join("vec.omg"), source.clone(), root, None, origin)
                .source_id;
            let tokens = source_files_to_tokens::Lexer::new(&source)
                .tokenize()
                .unwrap();
            let syntax =
                tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
            let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
                &syntax,
                std::sync::Arc::new(sources),
            )
            .unwrap();
            let program =
                symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                    .unwrap();
            let borrow = crate::build_borrow_facts(&program);
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "probe")
                .unwrap();
            let state = &program.machine_states(machine)[0];
            let borrow_state = borrow_state_for_symbol(&borrow, state.symbol).unwrap();
            let call = &borrow.calls.span_or_empty(borrow_state.calls)[0];
            let target =
                find_state(&program, call.target_symbol).expect("resolved Vec::push machine");
            let target_machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol_for_state(&program, target))
                .unwrap();
            assert_eq!(target_machine.name.as_str(), "Vec::push");
            assert!(
                program
                    .statement_table
                    .statements(target.statement_nodes)
                    .is_empty()
            );
            let resolver = validation::CallFrameResolver::new(&program).unwrap();
            let frame = &resolver.inferred_machine_state_write_frames(target_machine)[0];
            let cache = StateMutationSummaryCache::default();
            let summary = state_mutation_summary_places(&program, &borrow, &cache, target);
            let places = super::super::call_mutated_places(
                &program,
                machine.symbol,
                state.symbol,
                &borrow,
                call,
                &cache,
                Some(&resolver),
            )
            .expect("ordinary receiver fallback retains a conservative storage place");
            if origin == source::SourceOrigin::Toolchain {
                assert!(
                    !frame.is_complete(),
                    "the core surface is not a runtime body"
                );
                assert!(summary.is_none());
                assert_eq!(
                    places,
                    vec![
                        canonical_place_from_symbol(program.state_parameters(state)[0].symbol)
                            .unwrap()
                    ]
                );
            } else {
                assert!(frame.is_complete(), "ordinary no-op code uses its body");
                assert_eq!(summary, Some([].as_slice()));
                assert!(places.is_empty());
            }
        }
    }

    // The previous synchronous algorithm is kept only as a test oracle. Its
    // complete snapshot and all-owner replay deliberately do not share scheduling.
    fn full_sweep_reference(
        program: &typed_trees::TypedTrees,
        borrow: &BorrowFacts,
        states: &mut [StateMutationSummary],
    ) {
        loop {
            let snapshot = states.to_vec();
            let mut changed = false;
            for caller_index in 0..snapshot.len() {
                if !snapshot[caller_index].complete {
                    continue;
                }
                let caller_symbol = snapshot[caller_index].state_symbol;
                let caller_state = find_state(program, caller_symbol).unwrap();
                let caller_machine = machine_symbol_for_state(program, caller_state);
                let borrow_state = borrow_state_for_symbol(borrow, caller_symbol).unwrap();
                let mut additions = Vec::new();
                let mut complete = true;
                for call in borrow.calls.span_or_empty(borrow_state.calls) {
                    let Some(target_index) = summary_index_from(&snapshot, call.target_symbol)
                    else {
                        complete = false;
                        break;
                    };
                    let target = &snapshot[target_index];
                    if !target.complete {
                        complete = false;
                        break;
                    }
                    for write in &target.writes {
                        let Some(places) = instantiate_call_relative_places(
                            program,
                            caller_machine,
                            caller_symbol,
                            call,
                            write,
                            WritePlaceNamespace::Storage,
                        ) else {
                            complete = false;
                            break;
                        };
                        for place in places {
                            if state_summary_exposes_place(program, caller_state, &place)
                                && !snapshot[caller_index].writes.contains(&place)
                                && !additions.contains(&place)
                            {
                                additions.push(place);
                            }
                        }
                    }
                    if !complete {
                        break;
                    }
                }
                if !complete {
                    states[caller_index].complete = false;
                    states[caller_index].writes.clear();
                    changed = true;
                } else if !additions.is_empty() {
                    states[caller_index].writes.extend(additions);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    fn direct_summaries(program: &typed_trees::TypedTrees) -> Vec<StateMutationSummary> {
        program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .map(|state| StateMutationSummary {
                state_symbol: state.symbol,
                complete: true,
                writes: collect_state_mutation_summary_places(program, state).unwrap(),
            })
            .collect()
    }

    #[test]
    fn dirty_rounds_preserve_discovery_order_without_replaying_unrelated_owners() {
        let program = typed(
            r#"
            machine outer(first: &mut u64, second: &mut u64) { middle(first); leaf(second); }
            machine middle(value: &mut u64) { leaf(value); }
            machine leaf(value: &mut u64) { value = 1; }
            machine unrelated(value: &mut u64) { value = 2; }
        "#,
        );
        let borrows = crate::build_borrow_facts(&program);
        let mut actual = direct_summaries(&program);
        let mut expected = actual.clone();
        full_sweep_reference(&program, &borrows, &mut expected);
        SUMMARY_VISITS.set(0);
        propagate_state_mutation_summaries(&program, &borrows, &mut actual);
        assert_eq!(actual, expected);
        assert_eq!(
            SUMMARY_VISITS.get(),
            5,
            "four initial owners, then only outer"
        );
        let outer = &program.machine_states(&program.machines()[0])[0];
        let parameters = program.state_parameters(outer);
        assert_eq!(
            actual[0].writes,
            vec![
                canonical_place_from_symbol(parameters[1].symbol).unwrap(),
                canonical_place_from_symbol(parameters[0].symbol).unwrap(),
            ],
            "the shorter path discovers second before the longer path discovers first"
        );
    }

    #[test]
    fn dirty_rounds_match_full_sweeps_for_cycles_and_late_opaque_dependencies() {
        let program = typed(
            r#"
            machine first(value: &mut u64) { second(value); }
            machine second(value: &mut u64) { first(value); leaf(value); }
            machine leaf(value: &mut u64) { value = 1; }
        "#,
        );
        let borrows = crate::build_borrow_facts(&program);
        for opaque_leaf in [false, true] {
            let mut actual = direct_summaries(&program);
            actual[2].complete = !opaque_leaf;
            let mut expected = actual.clone();
            full_sweep_reference(&program, &borrows, &mut expected);
            propagate_state_mutation_summaries(&program, &borrows, &mut actual);
            assert_eq!(actual, expected);
            assert_eq!(actual[0].complete, !opaque_leaf);
            assert_eq!(actual[1].complete, !opaque_leaf);
            if opaque_leaf {
                assert!(actual[0].writes.is_empty());
                assert!(actual[1].writes.is_empty());
            } else {
                assert_eq!(actual[0].writes.len(), 1);
                assert_eq!(actual[1].writes.len(), 1);
            }
        }
    }

    #[test]
    fn shared_cache_publishes_one_complete_table_on_first_demand() {
        let source = "machine fill(values: &write [u16; 4]) { values[1..3] = [7, 8]; }";
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let borrows = crate::build_borrow_facts(&program);
        let state = &program.machine_states(&program.machines()[0])[0];
        let cache = StateMutationSummaryCache::default();
        let first = std::borrow::Cow::Borrowed(&cache);
        let second = first.clone();
        assert!(cache.states.get().is_none());
        let expected = state_mutation_summary_places(&program, &borrows, &first, state).unwrap();
        assert!(!expected.is_empty(), "fixture must retain a real write");
        let observed = state_mutation_summary_places(&program, &borrows, &second, state).unwrap();
        assert!(
            std::ptr::eq(expected, observed),
            "branch queries borrow the same initialized table"
        );
        let independent = StateMutationSummaryCache::default();
        assert!(
            independent.states.get().is_none(),
            "a new analysis has no inherited facts"
        );
        assert_eq!(
            expected,
            state_mutation_summary_places(&program, &borrows, &independent, state).unwrap()
        );
    }
}
