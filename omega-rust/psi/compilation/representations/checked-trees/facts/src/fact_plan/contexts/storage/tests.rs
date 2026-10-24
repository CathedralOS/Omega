use super::{Arena, FactContext, FactContextGroup, FactContexts, Handle, ProgramPoint};
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::statement::TransitionTargetHandle;

fn context(point: ProgramPoint, fact_ordinal: u32) -> FactContext {
    FactContext {
        point,
        facts: HandleSpan::from_parts(Handle::from_arena_index(fact_ordinal), 1),
    }
}

#[test]
fn clone_from_restores_baseline_groups_and_discards_divergent_tail_links() {
    let machine = ProgramPoint::Machine {
        machine_symbol: SymbolHandle::from_arena_index(7),
    };
    let state = ProgramPoint::State {
        machine_symbol: SymbolHandle::from_arena_index(7),
        state_symbol: SymbolHandle::from_arena_index(8),
    };
    let mut baseline = FactContexts::default();
    let first_context = baseline.append(context(ProgramPoint::Global, 1));
    baseline.append(context(machine, 2));
    let first_link = baseline.groups[&ProgramPoint::Global].first;
    let declaration = baseline.group_at_point(machine);
    let expected = baseline.handles_in_group(declaration).collect::<Vec<_>>();
    let mut scratch = baseline.clone();
    scratch.append(context(machine, 3));
    scratch.append(context(state, 4));
    let contexts_pointer = std::ptr::from_ref(scratch.contexts.get(first_context));
    let links_pointer = std::ptr::from_ref(scratch.links.get(first_link));
    for _ in 0..3 {
        scratch.clone_from(&baseline);
        assert_eq!(scratch, baseline);
        assert_eq!(
            std::ptr::from_ref(scratch.contexts.get(first_context)),
            contexts_pointer
        );
        assert_eq!(
            std::ptr::from_ref(scratch.links.get(first_link)),
            links_pointer
        );
        assert_eq!(
            scratch.handles_in_group(declaration).collect::<Vec<_>>(),
            expected
        );
        assert!(scratch.handles_at_point(state).next().is_none());
        let appended = scratch.append(context(machine, 5));
        assert_eq!(
            scratch.handles_in_group(declaration).collect::<Vec<_>>(),
            [expected[0], appended]
        );
        scratch.append(context(state, 6));
    }
    assert_eq!(
        baseline.handles_in_group(declaration).collect::<Vec<_>>(),
        expected
    );
}

fn assert_matches_scan(
    storage: &FactContexts,
    reference: &Arena<FactContext>,
    points: &[ProgramPoint],
) {
    assert_eq!(storage.len(), reference.len());
    assert_eq!(storage.is_empty(), reference.is_empty());
    assert_eq!(
        storage.iter().collect::<Vec<_>>(),
        reference.iter().collect::<Vec<_>>()
    );
    for point in points {
        let expected = reference
            .iter()
            .filter(|(_, context)| context.point == *point)
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        assert_eq!(
            storage.handles_at_point(*point).collect::<Vec<_>>(),
            expected,
            "{point:?}"
        );
    }
}

fn distinct_points() -> Vec<ProgramPoint> {
    let mut points = vec![ProgramPoint::Global];
    for machine_generation in [0, 1, 2] {
        let machine_symbol = SymbolHandle::from_parts(17, machine_generation);
        points.push(ProgramPoint::Definition {
            symbol: machine_symbol,
        });
        points.push(ProgramPoint::Machine { machine_symbol });
        for state_generation in [1, 2] {
            let state_symbol = SymbolHandle::from_parts(23, state_generation);
            points.push(ProgramPoint::State {
                machine_symbol,
                state_symbol,
            });
            for statement_index in [0, 1, usize::MAX] {
                points.push(ProgramPoint::Statement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                });
                for call_ordinal in [0, 1, usize::MAX] {
                    points.extend([
                        ProgramPoint::Call {
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            call_ordinal,
                        },
                        ProgramPoint::CallRequires {
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            call_ordinal,
                        },
                        ProgramPoint::CallEnsures {
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            call_ordinal,
                        },
                    ]);
                }
                for transition_target in [
                    TransitionTargetHandle::invalid(),
                    TransitionTargetHandle::from_parts(29, 1),
                    TransitionTargetHandle::from_parts(29, 2),
                ] {
                    points.extend([
                        ProgramPoint::TransitionArm {
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            transition_target,
                        },
                        ProgramPoint::Exit {
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            transition_target,
                        },
                    ]);
                }
            }
        }
    }
    points.extend([
        ProgramPoint::Definition {
            symbol: SymbolHandle::invalid(),
        },
        ProgramPoint::Machine {
            machine_symbol: SymbolHandle::invalid(),
        },
        ProgramPoint::State {
            machine_symbol: SymbolHandle::invalid(),
            state_symbol: SymbolHandle::invalid(),
        },
    ]);
    points
}

#[test]
fn exact_point_variants_generations_and_coordinates_match_the_original_scan() {
    let points = distinct_points();
    let mut storage = FactContexts::default();
    let mut reference = Arena::default();
    for (position, point) in points.iter().enumerate() {
        assert!(
            !points[..position].contains(point),
            "fixture points must differ"
        );
        let value = context(*point, position as u32 + 1);
        assert_eq!(storage.append(value.clone()), reference.append(value));
    }
    // Interleave a second row for each point rather than requiring contiguous groups.
    for (position, point) in points.iter().rev().enumerate() {
        let value = context(*point, position as u32 + 1);
        assert_eq!(storage.append(value.clone()), reference.append(value));
    }
    assert_matches_scan(&storage, &reference, &points);
    for point in points {
        assert_eq!(storage.handles_at_point(point).count(), 2);
    }
    assert_eq!(
        storage
            .handles_at_point(ProgramPoint::Definition {
                symbol: SymbolHandle::from_parts(17, 3),
            })
            .count(),
        0
    );
}

#[test]
fn freed_first_middle_and_last_rows_preserve_later_append_order() {
    let points = [
        ProgramPoint::Global,
        ProgramPoint::Machine {
            machine_symbol: SymbolHandle::from_arena_index(7),
        },
    ];
    let mut storage = FactContexts::default();
    let mut reference = Arena::default();
    let mut handles = Vec::new();
    for position in 0..8 {
        let value = context(points[position % 2], position as u32 + 1);
        let handle = storage.append(value.clone());
        assert_eq!(handle, reference.append(value));
        handles.push(handle);
        assert_matches_scan(&storage, &reference, &points);
    }
    for position in [0, 4, 6, 2, 7] {
        assert!(storage.free(handles[position]));
        assert!(reference.free(handles[position]));
        assert!(!storage.free(handles[position]));
        assert_matches_scan(&storage, &reference, &points);
    }
    assert_eq!(storage.handles_at_point(ProgramPoint::Global).count(), 0);
    for point in points {
        let value = context(point, 100);
        let handle = storage.append(value.clone());
        assert_eq!(handle, reference.append(value));
        assert!(!handles.contains(&handle));
        assert_matches_scan(&storage, &reference, &points);
    }
}

#[test]
fn empty_invalid_and_stale_handles_keep_zero_initialization_behavior() {
    let mut storage = FactContexts::default();
    assert!(storage.is_empty());
    assert_eq!(storage.iter().count(), 0);
    assert_eq!(storage.handles_at_point(ProgramPoint::Global).count(), 0);
    assert_eq!(storage.get(Handle::invalid()), &FactContext::default());
    assert!(!storage.free(Handle::invalid()));
    let live = storage.append(context(ProgramPoint::Global, 1));
    let wrong_generation = Handle::from_parts(live.arena_index(), live.generation() + 1);
    assert!(!storage.free(wrong_generation));
    assert_eq!(storage.get(wrong_generation), &FactContext::default());
    assert_eq!(
        storage
            .handles_at_point(ProgramPoint::Global)
            .collect::<Vec<_>>(),
        vec![live]
    );
    assert!(storage.free(live));
    assert_eq!(storage.get(live), &FactContext::default());
    assert!(storage.is_empty());
    assert_eq!(storage.handles_at_point(ProgramPoint::Global).count(), 0);
    assert!(!storage.free(wrong_generation));
}

#[test]
fn selected_groups_keep_append_order_and_survive_matching_baseline_clones() {
    let point = ProgramPoint::Machine {
        machine_symbol: SymbolHandle::from_parts(19, 2),
    };
    let mut storage = FactContexts::default();
    let absent = storage.group_at_point(point);
    let first = storage.append(context(point, 1));
    let group = storage.group_at_point(point);
    storage.append(context(ProgramPoint::Global, 2));
    let second = storage.append(context(point, 3));
    assert_eq!(
        storage.handles_in_group(group).collect::<Vec<_>>(),
        [first, second]
    );
    // A missing selection is not a reservation for a future group.
    assert_eq!(storage.handles_in_group(absent).count(), 0);
    assert_eq!(
        storage
            .handles_in_group(FactContextGroup::default())
            .count(),
        0
    );
    let baseline = storage.clone();
    assert!(storage.free(first));
    assert_eq!(
        storage.handles_in_group(group).collect::<Vec<_>>(),
        [second]
    );
    let third = storage.append(context(point, 4));
    assert_eq!(
        storage.handles_in_group(group).collect::<Vec<_>>(),
        [second, third]
    );
    storage = baseline;
    assert_eq!(
        storage.handles_in_group(group).collect::<Vec<_>>(),
        [first, second]
    );
}

#[test]
fn clones_restore_independent_indexes_and_preserve_arena_equality_and_debug() {
    let points = [
        ProgramPoint::Global,
        ProgramPoint::Definition {
            symbol: SymbolHandle::from_arena_index(9),
        },
    ];
    let mut original = FactContexts::with_capacity(32);
    let first = original.append(context(points[0], 1));
    original.append(context(points[1], 2));
    original.append(context(points[0], 3));
    let reference = original.contexts.clone();
    let mut changed = original.clone();
    assert_eq!(changed, original);
    assert_eq!(format!("{original:?}"), format!("{reference:?}"));
    // Different hash-table capacity is implementation state, not semantic identity.
    changed.groups.reserve(128);
    assert_eq!(changed, original);
    assert_eq!(format!("{changed:?}"), format!("{original:?}"));
    assert!(changed.free(first));
    changed.append(context(points[1], 4));
    assert_ne!(changed, original);
    assert_matches_scan(&original, &reference, &points);
    assert_matches_scan(&changed, &changed.contexts, &points);
    // Flow convergence resets a working plan from its cloned baseline.
    changed = original.clone();
    assert_eq!(changed, original);
    assert_matches_scan(&changed, &reference, &points);
    let later = changed.append(context(points[0], 5));
    assert_eq!(changed.handles_at_point(points[0]).last(), Some(later));
    assert_matches_scan(&original, &reference, &points);
}

#[test]
fn mixed_append_and_free_sequences_match_the_original_scan_after_every_change() {
    let points = distinct_points();
    let selected = [
        points[0],
        points[1],
        points[6],
        points[7],
        points[8],
        *points.last().unwrap(),
    ];
    let mut storage = FactContexts::default();
    let mut reference = Arena::default();
    let mut handles = Vec::new();
    for step in 0..128 {
        let value = context(
            selected[(step * 7 + step / 3) % selected.len()],
            step as u32 + 1,
        );
        let handle = storage.append(value.clone());
        assert_eq!(handle, reference.append(value));
        handles.push(handle);
        if step % 3 != 0 {
            let candidate = handles[(step * 11) % handles.len()];
            assert_eq!(storage.free(candidate), reference.free(candidate));
        }
        assert_matches_scan(&storage, &reference, &selected);
    }
}

#[test]
fn adopted_worker_pages_equal_sequential_partition_pages() {
    let state = |machine, state| ProgramPoint::State {
        machine_symbol: SymbolHandle::from_arena_index(machine),
        state_symbol: SymbolHandle::from_arena_index(state),
    };
    // The first partition extends a group the baseline already holds, so its
    // first append rewrites a baseline tail link.
    let mut baseline = FactContexts::default();
    baseline.append(context(state(1, 2), 1));
    baseline.append(context(ProgramPoint::Global, 2));

    let mut sequential = baseline.clone();
    sequential.begin_page(2);
    let first = sequential.append(context(state(1, 2), 3));
    sequential.append(context(state(1, 3), 4));
    sequential.begin_page(3);
    let second = sequential.append(context(state(4, 5), 5));

    let mut worker = |page, appends: &[(ProgramPoint, u32)]| {
        let mut copy = baseline.clone();
        copy.begin_page(page);
        for (point, ordinal) in appends {
            copy.append(context(*point, *ordinal));
        }
        copy.take_additional_pages()
    };
    let first_pages = worker(2, &[(state(1, 2), 3), (state(1, 3), 4)]);
    let second_pages = worker(3, &[(state(4, 5), 5)]);
    let mut merged = baseline.clone();
    merged.adopt_additional_pages(second_pages);
    merged.adopt_additional_pages(first_pages);

    assert_eq!(merged, sequential);
    assert_eq!(merged.get(first), sequential.get(first));
    assert_eq!(merged.get(second), sequential.get(second));
    for point in [state(1, 2), state(1, 3), state(4, 5), ProgramPoint::Global] {
        assert_eq!(
            merged.handles_at_point(point).collect::<Vec<_>>(),
            sequential.handles_at_point(point).collect::<Vec<_>>(),
        );
    }
}

#[test]
fn shared_context_checkpoint_detaches_tail_links_and_restores_groups() {
    let point = ProgramPoint::Global;
    let mut baseline = FactContexts::default();
    let first = baseline.append(context(point, 1));
    let group = baseline.group_at_point(point);
    let tail = baseline.groups[&point].last;
    baseline.share_pages();
    let mut worker = baseline.clone();
    let sibling = baseline.clone();
    assert!(std::ptr::eq(
        baseline.contexts.get(first),
        worker.contexts.get(first)
    ));
    assert!(std::ptr::eq(
        baseline.links.get(tail),
        worker.links.get(tail)
    ));
    worker.begin_page(2);
    let added = worker.append(context(point, 2));
    assert!(!std::ptr::eq(
        baseline.links.get(tail),
        worker.links.get(tail)
    ));
    assert!(std::ptr::eq(
        baseline.contexts.get(first),
        worker.contexts.get(first)
    ));
    assert_eq!(
        worker.handles_in_group(group).collect::<Vec<_>>(),
        [first, added]
    );
    assert_eq!(
        baseline.handles_in_group(group).collect::<Vec<_>>(),
        [first]
    );
    assert_eq!(sibling.handles_in_group(group).collect::<Vec<_>>(), [first]);
    assert!(worker.free(first));
    assert_eq!(worker.handles_in_group(group).collect::<Vec<_>>(), [added]);
    worker.clone_from(&baseline);
    assert!(std::ptr::eq(
        baseline.contexts.get(first),
        worker.contexts.get(first)
    ));
    assert!(std::ptr::eq(
        baseline.links.get(tail),
        worker.links.get(tail)
    ));
    assert_eq!(worker.handles_in_group(group).collect::<Vec<_>>(), [first]);
    assert_eq!(worker.groups[&point].last, tail);
    worker.begin_page(3);
    let replacement = worker.append(context(point, 3));
    assert_eq!(
        worker.handles_in_group(group).collect::<Vec<_>>(),
        [first, replacement]
    );
    assert_eq!(sibling.handles_in_group(group).collect::<Vec<_>>(), [first]);
}
