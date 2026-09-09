use super::*;

#[test]
fn repeated_call_checking_retains_shared_context_storage() {
    let mut source = String::from(
        "data Helper {} machine Helper::touch() {}\n\
         machine main(value: u64) -> u64 requires value > 0 {\n",
    );
    for _ in 0..64 {
        source.push_str("Helper::touch();\n");
    }
    source.push_str("value }");
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = crate::lower_typed_trees(typed).expect("check repeated calls");
    let flow = &checked.facts.flow;
    assert_eq!(flow.control.calls.len(), 64);
    let selected_context_rows: usize = flow
        .control
        .calls
        .iter()
        .map(|(_, call)| call.entry_semantic_contexts.len() + call.exit_semantic_contexts.len())
        .sum();
    let stored_context_rows = flow.contexts.semantic_context_refs.len();
    eprintln!(
        "64 checked calls: {stored_context_rows} stored context rows for {selected_context_rows} entry/exit context occurrences; {} constraint rows",
        flow.contexts.constraint_refs.len()
    );
    assert!(selected_context_rows >= 128);
    assert!(
        stored_context_rows < selected_context_rows / 4,
        "unchanged call contexts must not be materialized per snapshot"
    );
}

#[test]
fn extending_recycled_tail_normalizes_generation_without_changing_snapshot() {
    let mut references = arena::Arena::<FlowConstraintRef>::default();
    let original = references.append(FlowConstraintRef::default());
    assert!(references.free(original));
    let recycled = references.insert(FlowConstraintRef::default());
    let snapshot = HandleSpan::from_parts(recycled, 1);
    let mut extended = retained_constraint_refs(&references, snapshot);
    append_flow_reference(&mut references, &mut extended, FlowConstraintRef::default());
    assert_eq!(references.len(), 3);
    assert_eq!(extended.start().generation(), 1);
    assert_eq!(references.span_or_empty(extended).len(), 2);
    assert_eq!(references.span_or_empty(snapshot).len(), 1);
}

#[test]
fn filtered_reference_spans_match_every_selection_without_copying_contiguous_runs() {
    let values: Vec<_> = (1..=8)
        .map(|context_index| FlowConstraintRef {
            kind: FlowConstraintKind::SemanticContext {
                context: Handle::from_arena_index(context_index),
            },
        })
        .collect();
    for selection_mask in 0u16..256 {
        let mut references = arena::Arena::default();
        let source = references.insert_many(values.iter().copied());
        // Force the source away from the tail, including for suffix selections.
        references.append(FlowConstraintRef::default());
        let before = references.len();
        let mut visited = Vec::new();
        let filtered = filter_constraint_refs(&mut references, source, |reference| {
            let position = visited.len();
            visited.push(reference);
            selection_mask & (1 << position) != 0
        });
        let expected: Vec<_> = values
            .iter()
            .enumerate()
            .filter(|(position, _)| selection_mask & (1 << position) != 0)
            .map(|(_, reference)| *reference)
            .collect();
        assert_eq!(visited, values, "predicate order and exactly-once calls");
        assert_eq!(references.span_or_empty(filtered), expected);
        assert_eq!(references.span_or_empty(source), values);
        let selected_positions: Vec<_> = (0..8)
            .filter(|position| selection_mask & (1 << position) != 0)
            .collect();
        let contiguous = selected_positions
            .windows(2)
            .all(|pair| pair[1] == pair[0] + 1);
        assert_eq!(
            references.len() - before,
            if contiguous { 0 } else { expected.len() }
        );
        if selection_mask == 255 {
            assert_eq!(filtered, source);
        }
        let mut extended = filtered;
        append_flow_reference(&mut references, &mut extended, FlowConstraintRef::default());
        let mut expected_extended = expected.clone();
        expected_extended.push(FlowConstraintRef::default());
        assert_eq!(references.span_or_empty(extended), expected_extended);
        assert_eq!(references.span_or_empty(filtered), expected);
        assert_eq!(references.span_or_empty(source), values);
    }
}

#[test]
fn appended_reference_snapshots_preserve_siblings_and_duplicate_occurrences() {
    let mut references = arena::Arena::default();
    let repeated = FlowSemanticContextRef {
        context: Handle::from_arena_index(7),
    };
    let source = references.insert_many([repeated, repeated]);
    let mut first = retained_flow_contexts(&references, source);
    let mut sibling = retained_flow_contexts(&references, source);
    assert_eq!(references.len(), 2);
    append_flow_reference(
        &mut references,
        &mut first,
        FlowSemanticContextRef::default(),
    );
    assert_eq!(references.len(), 3, "tail extension copies no prefix");
    assert_eq!(references.span_or_empty(sibling), [repeated, repeated]);
    append_flow_reference(&mut references, &mut sibling, repeated);
    assert_eq!(
        references.len(),
        6,
        "non-tail sibling copies only on extension"
    );
    assert_eq!(references.span_or_empty(source), [repeated, repeated]);
    assert_eq!(
        references.span_or_empty(first),
        [repeated, repeated, FlowSemanticContextRef::default()]
    );
    assert_eq!(
        references.span_or_empty(sibling),
        [repeated, repeated, repeated]
    );
}

#[test]
fn reference_selection_preserves_generation_and_rejects_whole_stale_spans() {
    let mut references = arena::Arena::<FlowConstraintRef>::default();
    let source = references.insert_many([FlowConstraintRef::default(); 3]);
    let middle = Handle::from_arena_index(source.start().arena_index() + 1);
    assert!(references.free(middle));
    let replacement = references.insert(FlowConstraintRef::default());
    assert_ne!(replacement.generation(), middle.generation());
    let invalid_sources = [
        source,
        HandleSpan::from_parts(Handle::invalid(), 1),
        HandleSpan::from_parts(Handle::from_arena_index(u32::MAX), 2),
        HandleSpan::from_parts(source.start(), u32::MAX),
        HandleSpan::empty(),
    ];
    for invalid in invalid_sources {
        assert_eq!(
            retained_constraint_refs(&references, invalid),
            HandleSpan::empty()
        );
        let before = references.len();
        assert_eq!(
            filter_constraint_refs(&mut references, invalid, |_| panic!(
                "invalid source reached predicate"
            )),
            HandleSpan::empty()
        );
        assert_eq!(references.len(), before);
    }
    let selected = HandleSpan::from_parts(replacement, 1);
    assert_eq!(
        filter_constraint_refs(&mut references, selected, |_| true),
        selected
    );
    let mut extended = selected;
    append_flow_reference(&mut references, &mut extended, FlowConstraintRef::default());
    assert_eq!(extended.start().generation(), 1);
    assert_eq!(references.span_or_empty(extended).len(), 2);
    assert_eq!(references.span_or_empty(selected).len(), 1);
}

#[test]
fn unchanged_reference_snapshots_remove_eager_materialization_work() {
    let mut references = arena::Arena::<FlowConstraintRef>::default();
    let source = references.insert_many([FlowConstraintRef::default(); 128]);
    let mut eager = references.clone();
    let mut active = source;
    let mut eager_active = source;
    for _ in 0..512 {
        active = retained_constraint_refs(&references, active);
        active = filter_constraint_refs(&mut references, active, |_| true);
        // Independent old implementation: snapshot then copy the whole list.
        for _ in 0..2 {
            let copied = eager.span_or_empty(eager_active).to_vec();
            eager_active = eager.insert_many(copied);
        }
        assert_eq!(
            references.span_or_empty(active),
            eager.span_or_empty(eager_active)
        );
    }
    assert_eq!(references.len(), 128);
    assert_eq!(eager.len(), 128 + 512 * 2 * 128);
}

#[test]
fn projection_retains_borrow_kinds_order_duplicates_and_exact_context_generations() {
    let active = Handle::from_parts(4, 2);
    let inactive = Handle::from_parts(4, 1);
    let mut contexts = arena::Arena::default();
    let active_contexts = contexts.insert_many([FlowSemanticContextRef { context: active }]);
    let kinds = [
        FlowConstraintKind::Unknown,
        FlowConstraintKind::SemanticContext { context: inactive },
        FlowConstraintKind::BorrowState {
            state: Handle::invalid(),
        },
        FlowConstraintKind::BorrowCall {
            call: Handle::invalid(),
        },
        FlowConstraintKind::BorrowWritableRoot {
            root: Handle::invalid(),
        },
        FlowConstraintKind::BorrowAccess {
            access: Handle::invalid(),
        },
        FlowConstraintKind::BorrowLoan {
            loan: Handle::invalid(),
        },
        FlowConstraintKind::SemanticContext { context: active },
        FlowConstraintKind::SemanticContext { context: active },
    ];
    let mut constraints = arena::Arena::default();
    let source = constraints.insert_many(kinds.map(|kind| FlowConstraintRef { kind }));
    let selected = project_constraint_refs_to_active_contexts(
        &mut constraints,
        source,
        active_contexts,
        &contexts,
    );
    let expected: Vec<_> = kinds
        .into_iter()
        .filter(|kind| *kind != FlowConstraintKind::SemanticContext { context: inactive })
        .map(|kind| FlowConstraintRef { kind })
        .collect();
    assert_eq!(constraints.span_or_empty(selected), expected);
}

#[test]
fn prepared_flow_lists_match_independent_scans_with_late_state_contexts() {
    for machine_count in [8, 32, 128] {
        let mut semantic = FactPlan::default();
        semantic.append_fact_context(facts::Fact::default());
        for index in 1..=machine_count {
            let machine_symbol = SymbolHandle::from_arena_index(index);
            semantic.append_context(
                ProgramPoint::Machine { machine_symbol },
                HandleSpan::empty(),
            );
            for state_index in 0..4 {
                let state_symbol =
                    SymbolHandle::from_arena_index(machine_count + index * 4 + state_index);
                semantic.append_context(
                    ProgramPoint::State {
                        machine_symbol,
                        state_symbol,
                    },
                    HandleSpan::empty(),
                );
                for statement_index in 0..16 {
                    semantic.append_context(
                        ProgramPoint::Statement {
                            machine_symbol,
                            state_symbol,
                            statement_index,
                        },
                        HandleSpan::empty(),
                    );
                }
            }
        }

        let mut selected_contexts = 0;
        let mut reference_inspections = 0;
        let global_contexts = semantic.context_group_at_point(ProgramPoint::Global);
        for index in 1..=machine_count {
            let machine_symbol = SymbolHandle::from_arena_index(index);
            let machine_contexts =
                semantic.context_group_at_point(ProgramPoint::Machine { machine_symbol });
            for state_index in 0..4 {
                let state_symbol =
                    SymbolHandle::from_arena_index(machine_count + index * 4 + state_index);
                let state_point = ProgramPoint::State {
                    machine_symbol,
                    state_symbol,
                };
                // State-input inference appends facts after initial preparation.
                semantic.append_fact_context(facts::Fact {
                    point: state_point,
                    ..Default::default()
                });
                let points = [
                    ProgramPoint::Global,
                    ProgramPoint::Machine { machine_symbol },
                    state_point,
                ];
                let mut contexts = arena::Arena::default();
                let mut constraints = arena::Arena::default();
                let mut context_span = HandleSpan::empty();
                let mut constraint_span = HandleSpan::empty();
                append_flow_contexts(
                    semantic
                        .context_handles_in_group(global_contexts)
                        .chain(semantic.context_handles_in_group(machine_contexts)),
                    &mut contexts,
                    &mut context_span,
                    &mut constraints,
                    &mut constraint_span,
                );
                append_flow_contexts_for_points(
                    &semantic,
                    &mut contexts,
                    &mut context_span,
                    &mut constraints,
                    &mut constraint_span,
                    &[state_point],
                );

                let mut reference_contexts = arena::Arena::default();
                let mut reference_constraints = arena::Arena::default();
                let mut reference_context_span = HandleSpan::empty();
                let mut reference_constraint_span = HandleSpan::empty();
                for point in points {
                    for (context, value) in semantic.contexts.iter() {
                        reference_inspections += 1;
                        if value.point == point {
                            reference_contexts.append_to_span(
                                &mut reference_context_span,
                                FlowSemanticContextRef { context },
                            );
                        }
                    }
                }
                for point in points {
                    for (context, value) in semantic.contexts.iter() {
                        reference_inspections += 1;
                        if value.point == point {
                            append_constraint_ref(
                                &mut reference_constraints,
                                &mut reference_constraint_span,
                                FlowConstraintKind::SemanticContext { context },
                            );
                        }
                    }
                }
                assert_eq!(contexts, reference_contexts);
                assert_eq!(constraints, reference_constraints);
                assert_eq!(context_span, reference_context_span);
                assert_eq!(constraint_span, reference_constraint_span);
                selected_contexts += contexts.len();
            }
        }
        assert_eq!(selected_contexts, (machine_count * 4 * 4) as usize);
        assert!(reference_inspections > selected_contexts * machine_count as usize);
    }
}
