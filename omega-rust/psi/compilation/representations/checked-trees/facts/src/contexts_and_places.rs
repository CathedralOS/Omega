/*
We represent program-point fact contexts (`contexts`) and projected storage
places, view coordinates, member-symbol resolution, and write frames (`places`)
in `FactPlan`.
*/

pub mod contexts {
    pub(crate) mod storage {
        type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;
        use std::fmt;
        use std::sync::Arc;

        type Arena<T> = arena::PagedArenaSet<T>;

        use super::{FactContext, ProgramPoint};
        use crate::FactContextHandle;

        #[cfg(test)]
        pub(crate) mod tests {
            use super::{Arena, FactContext, FactContextGroup, FactContexts, ProgramPoint};
            use arena::{Handle, HandleSpan};
            use symbols::SymbolHandle;
            use typed_trees::statement::TransitionTargetHandle;

            fn context(point: ProgramPoint, fact_ordinal: u32) -> FactContext {
                FactContext {
                    point,
                    facts: HandleSpan::from_parts(Handle::from_arena_index(fact_ordinal), 1),
                    assignments: HandleSpan::empty(),
                }
            }

            #[test]
            fn clone_from_restores_baseline_groups_and_discards_divergent_additions() {
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
                let declaration = baseline.group_at_point(machine);
                let expected = baseline.handles_in_group(declaration).collect::<Vec<_>>();
                let mut scratch = baseline.clone();
                scratch.append(context(machine, 3));
                scratch.append(context(state, 4));
                let contexts_pointer = std::ptr::from_ref(scratch.contexts.get(first_context));
                for _ in 0..3 {
                    scratch.clone_from(&baseline);
                    assert_eq!(scratch, baseline);
                    assert_eq!(
                        std::ptr::from_ref(scratch.contexts.get(first_context)),
                        contexts_pointer
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
                let wrong_generation =
                    Handle::from_parts(live.arena_index(), live.generation() + 1);
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
                changed.groups.reserve(128);
                assert_eq!(changed, original);
                assert_eq!(format!("{changed:?}"), format!("{original:?}"));
                assert!(changed.free(first));
                changed.append(context(points[1], 4));
                assert_ne!(changed, original);
                assert_matches_scan(&original, &reference, &points);
                assert_matches_scan(&changed, &changed.contexts, &points);
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
                let mut baseline = FactContexts::default();
                baseline.append(context(state(1, 2), 1));
                baseline.append(context(ProgramPoint::Global, 2));

                let mut sequential = baseline.clone();
                sequential.begin_page(2);
                let first = sequential.append(context(state(1, 2), 3));
                sequential.append(context(state(1, 3), 4));
                sequential.begin_page(3);
                let second = sequential.append(context(state(4, 5), 5));

                let worker = |page, appends: &[(ProgramPoint, u32)]| {
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
            fn shared_baseline_forks_keep_appends_frees_and_restoration_local() {
                let point = ProgramPoint::Machine {
                    machine_symbol: SymbolHandle::from_parts(7, 2),
                };
                let other = ProgramPoint::State {
                    machine_symbol: SymbolHandle::from_parts(7, 2),
                    state_symbol: SymbolHandle::from_parts(8, 3),
                };
                let mut baseline = FactContexts::default();
                let first = baseline.append(context(point, 1));
                let selected = baseline.group_at_point(point);
                let absent = baseline.group_at_point(other);
                baseline.share_pages();
                let first_pointer = std::ptr::from_ref(baseline.get(first));

                let mut worker = baseline.clone();
                assert!(std::sync::Arc::ptr_eq(&baseline.baseline, &worker.baseline));
                worker.begin_page(2);
                let second = worker.append(context(point, 2));
                let local_first = worker.append(context(other, 3));
                let local_selected = worker.group_at_point(other);
                assert_eq!(std::ptr::from_ref(worker.get(first)), first_pointer);
                assert!(std::sync::Arc::ptr_eq(&baseline.baseline, &worker.baseline));
                assert_eq!(
                    worker.handles_in_group(selected).collect::<Vec<_>>(),
                    [first, second]
                );
                assert_eq!(worker.handles_in_group(absent).count(), 0);
                assert_eq!(
                    baseline.handles_in_group(selected).collect::<Vec<_>>(),
                    [first]
                );

                assert!(worker.free(first));
                assert!(worker.free(second));
                assert_eq!(worker.handles_in_group(selected).count(), 0);
                let third = worker.append(context(point, 4));
                assert_eq!(
                    worker.handles_in_group(selected).collect::<Vec<_>>(),
                    [third]
                );
                worker.share_pages();
                worker.begin_page(3);
                let fourth = worker.append(context(point, 5));
                let local_second = worker.append(context(other, 6));
                assert_eq!(
                    worker.handles_in_group(local_selected).collect::<Vec<_>>(),
                    [local_first, local_second]
                );
                worker.share_pages();
                assert_eq!(
                    worker.handles_in_group(local_selected).collect::<Vec<_>>(),
                    [local_first, local_second]
                );
                assert_eq!(worker.handles_in_group(absent).count(), 0);
                assert_eq!(
                    worker.handles_in_group(selected).collect::<Vec<_>>(),
                    [third, fourth]
                );
                assert_eq!(
                    baseline.handles_in_group(selected).collect::<Vec<_>>(),
                    [first]
                );

                worker.clone_from(&baseline);
                assert!(std::sync::Arc::ptr_eq(&baseline.baseline, &worker.baseline));
                assert_eq!(std::ptr::from_ref(worker.get(first)), first_pointer);
                assert_eq!(
                    worker.handles_in_group(selected).collect::<Vec<_>>(),
                    [first]
                );
                assert_eq!(worker.handles_at_point(other).count(), 0);
                assert_eq!(worker, baseline);
            }

            #[test]
            fn overlapping_reverse_adoption_survives_sealing_and_page_transfer() {
                let point = ProgramPoint::Global;
                let other = ProgramPoint::Machine {
                    machine_symbol: SymbolHandle::from_parts(11, 4),
                };
                let mut baseline = FactContexts::default();
                let first = baseline.append(context(point, 1));
                let selected = baseline.group_at_point(point);
                baseline.share_pages();
                let mut first_worker = baseline.clone();
                first_worker.begin_page(2);
                let second = first_worker.append(context(point, 2));
                let first_pages = first_worker.take_additional_pages();
                let mut second_worker = baseline.clone();
                second_worker.begin_page(3);
                let third = second_worker.append(context(point, 3));
                let second_pages = second_worker.take_additional_pages();

                let mut merged = baseline.clone();
                merged.adopt_additional_pages(second_pages);
                merged.adopt_additional_pages(first_pages);
                assert_eq!(
                    merged.handles_in_group(selected).collect::<Vec<_>>(),
                    [first, third, second]
                );
                assert_eq!(
                    merged.iter().map(|(handle, _)| handle).collect::<Vec<_>>(),
                    [first, second, third]
                );
                merged.share_pages();
                assert_eq!(
                    merged.handles_in_group(selected).collect::<Vec<_>>(),
                    [first, third, second]
                );

                let mut retained = merged.clone();
                let transferred = retained.take_additional_pages();
                assert_eq!(transferred.len(), 2);
                assert_eq!(
                    retained.handles_in_group(selected).collect::<Vec<_>>(),
                    [first]
                );
                retained.begin_page(2);
                let replacement = retained.append(context(other, 4));
                assert_eq!(replacement, second);
                assert_eq!(
                    retained.handles_in_group(selected).collect::<Vec<_>>(),
                    [first]
                );
                assert_eq!(
                    retained.handles_at_point(other).collect::<Vec<_>>(),
                    [replacement]
                );
                assert_eq!(
                    merged.handles_in_group(selected).collect::<Vec<_>>(),
                    [first, third, second]
                );
            }
        }

        #[cfg(test)]
        pub(crate) mod measurements {
            use super::{Arena, FactContext, FactContextGroup, FactContexts, ProgramPoint};
            use std::hint::black_box;
            use std::time::Instant;

            use arena::HandleSpan;
            use symbols::SymbolHandle;

            #[test]
            #[ignore = "manual prepared entry-group cost measurement"]
            fn prepared_entry_group_cost() {
                for (machine_count, states_per_machine, declared, passes) in [
                    (1, 1, true, 1),
                    (64, 1, false, 1),
                    (64, 4, true, 20),
                    (1, 512, true, 20),
                    (1024, 4, true, 1),
                    (1024, 4, true, 20),
                    (1024, 4, false, 20),
                ] {
                    let mut baseline = FactContexts::default();
                    if declared {
                        baseline.append(FactContext::default());
                    }
                    let mut machines = Vec::new();
                    for machine_index in 1..=machine_count {
                        let machine_symbol = SymbolHandle::from_arena_index(machine_index);
                        if declared {
                            baseline.append(FactContext {
                                point: ProgramPoint::Machine { machine_symbol },
                                facts: HandleSpan::empty(),
                                assignments: HandleSpan::empty(),
                            });
                        }
                        let mut states = Vec::new();
                        for state_index in 0..states_per_machine {
                            let state_symbol = SymbolHandle::from_arena_index(
                                machine_count + machine_index * states_per_machine + state_index,
                            );
                            let point = ProgramPoint::State {
                                machine_symbol,
                                state_symbol,
                            };
                            for _ in 0..4 {
                                baseline.append(FactContext {
                                    point,
                                    facts: HandleSpan::empty(),
                                    assignments: HandleSpan::empty(),
                                });
                            }
                            for call_ordinal in 0..8 {
                                baseline.append(FactContext {
                                    point: ProgramPoint::CallEnsures {
                                        machine_symbol,
                                        state_symbol,
                                        statement_index: 1,
                                        call_ordinal,
                                    },
                                    facts: HandleSpan::empty(),
                                    assignments: HandleSpan::empty(),
                                });
                            }
                            states.push(point);
                        }
                        machines.push((machine_symbol, states));
                    }
                    let start = Instant::now();
                    let mut old_count = 0;
                    for _ in 0..passes {
                        let semantic = black_box(baseline.clone());
                        for (machine_symbol, states) in &machines {
                            for state in states {
                                for point in [
                                    ProgramPoint::Global,
                                    ProgramPoint::Machine {
                                        machine_symbol: *machine_symbol,
                                    },
                                    *state,
                                ] {
                                    for handle in semantic.handles_at_point(black_box(point)) {
                                        black_box(handle);
                                        old_count += 1;
                                    }
                                }
                            }
                        }
                    }
                    let old_elapsed = start.elapsed();
                    let start = Instant::now();
                    baseline.share_pages();
                    let global = baseline.group_at_point(ProgramPoint::Global);
                    let groups: Vec<_> = machines
                        .iter()
                        .map(|(machine_symbol, _)| {
                            baseline.group_at_point(ProgramPoint::Machine {
                                machine_symbol: *machine_symbol,
                            })
                        })
                        .collect();
                    let preparation = start.elapsed();
                    let mut new_count = 0;
                    for _ in 0..passes {
                        let semantic = black_box(baseline.clone());
                        for ((_, states), group) in machines.iter().zip(&groups) {
                            for state in states {
                                for handle in semantic
                                    .handles_in_group(black_box(global))
                                    .chain(semantic.handles_in_group(black_box(*group)))
                                    .chain(semantic.handles_at_point(black_box(*state)))
                                {
                                    black_box(handle);
                                    new_count += 1;
                                }
                            }
                        }
                    }
                    let new_elapsed = start.elapsed();
                    assert_eq!(old_count, new_count);
                    let old_lookups = passes * machine_count * states_per_machine * 3;
                    let new_lookups =
                        1 + machine_count + passes * machine_count * states_per_machine;
                    println!(
                        "machines={machine_count} states_per_machine={states_per_machine} declared={declared} passes={passes} contexts={} unshared={old_elapsed:?} shared_prepared_including_setup={new_elapsed:?} setup={preparation:?} unprepared_lookups={old_lookups} prepared_lookups={new_lookups} selector_capacity_bytes={} selected={new_count}",
                        baseline.len(),
                        (groups.capacity() + 1) * size_of::<FactContextGroup>()
                    );
                }
            }

            #[test]
            #[ignore = "manual context-index cost measurement"]
            fn context_index_cost() {
                for machine_count in [64, 256, 1024] {
                    let mut source = Vec::new();
                    source.push(FactContext::default());
                    let mut queries = Vec::new();
                    for index in 1..=machine_count {
                        let machine_symbol = SymbolHandle::from_arena_index(index);
                        let machine_point = ProgramPoint::Machine { machine_symbol };
                        source.push(FactContext {
                            point: machine_point,
                            facts: HandleSpan::empty(),
                            assignments: HandleSpan::empty(),
                        });
                        for state_index in 0..4 {
                            let state_symbol = SymbolHandle::from_arena_index(
                                machine_count + index * 4 + state_index,
                            );
                            let point = ProgramPoint::State {
                                machine_symbol,
                                state_symbol,
                            };
                            queries.extend([ProgramPoint::Global, machine_point, point]);
                            for _ in 0..4 {
                                source.push(FactContext {
                                    point,
                                    facts: HandleSpan::empty(),
                                    assignments: HandleSpan::empty(),
                                });
                            }
                        }
                    }
                    let start = Instant::now();
                    let mut baseline = Arena::with_capacity(source.len());
                    for context in &source {
                        baseline.append(context.clone());
                    }
                    let baseline_setup = start.elapsed();
                    let start = Instant::now();
                    let mut indexed = FactContexts::with_capacity(source.len());
                    for context in source {
                        indexed.append(context);
                    }
                    let indexed_setup = start.elapsed();
                    let start = Instant::now();
                    let mut reference_selected = 0;
                    for point in &queries {
                        for (handle, context) in baseline.iter() {
                            if context.point == *point {
                                black_box(handle);
                                reference_selected += 1;
                            }
                        }
                    }
                    let baseline_lookup = start.elapsed();
                    let start = Instant::now();
                    let mut indexed_selected = 0;
                    for point in &queries {
                        for handle in indexed.handles_at_point(*point) {
                            black_box(handle);
                            indexed_selected += 1;
                        }
                    }
                    let indexed_lookup = start.elapsed();
                    assert_eq!(indexed_selected, reference_selected);
                    let membership_payload = indexed.members.iter().map(Vec::len).sum::<usize>()
                        * size_of::<crate::FactContextHandle>();
                    let group_payload = indexed.groups.len()
                        * size_of::<(ProgramPoint, FactContextGroup)>()
                        + indexed.members.len() * size_of::<Vec<crate::FactContextHandle>>();
                    println!(
                        "machines={machine_count} contexts={} queries={} baseline_setup={baseline_setup:?} indexed_setup={indexed_setup:?} baseline_lookup={baseline_lookup:?} indexed_lookup={indexed_lookup:?} scanned={} selected={indexed_selected} membership_payload={membership_payload} group_payload={group_payload} group_capacity={}",
                        indexed.len(),
                        queries.len(),
                        baseline.len() * queries.len(),
                        indexed.groups.capacity()
                    );
                }
            }
        }

        #[derive(Clone, Default, compiler_storage::Record)]
        struct ContextIndex {
            points: HashMap<ProgramPoint, FactContextGroup>,
            members: Vec<Vec<FactContextHandle>>,
            maximum_generation: u32,
        }

        #[derive(Default, compiler_storage::Record)]
        pub struct FactContexts {
            contexts: Arena<FactContext>,
            baseline: Arc<ContextIndex>,
            groups: HashMap<ProgramPoint, FactContextGroup>,
            members: Vec<Vec<FactContextHandle>>,
            extensions: Vec<Vec<FactContextHandle>>,
        }

        impl Clone for FactContexts {
            fn clone(&self) -> Self {
                Self {
                    contexts: self.contexts.clone(),
                    baseline: self.baseline.clone(),
                    groups: self.groups.clone(),
                    members: self.members.clone(),
                    extensions: self.extensions.clone(),
                }
            }

            fn clone_from(&mut self, source: &Self) {
                self.contexts.clone_from(&source.contexts);
                self.baseline.clone_from(&source.baseline);
                self.groups.clone_from(&source.groups);
                self.members.clone_from(&source.members);
                self.extensions.clone_from(&source.extensions);
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct FactContextGroup {
            ordinal: usize,
        }

        impl PartialEq for FactContexts {
            fn eq(&self, other: &Self) -> bool {
                self.contexts == other.contexts
            }
        }

        impl Eq for FactContexts {}

        impl fmt::Debug for FactContexts {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.contexts.fmt(formatter)
            }
        }

        impl FactContexts {
            pub(crate) fn with_capacity(capacity: usize) -> Self {
                Self {
                    contexts: Arena::with_capacity(capacity),
                    ..Self::default()
                }
            }

            pub fn shares_sealed_index(&self, other: &Self) -> bool {
                Arc::ptr_eq(&self.baseline, &other.baseline)
            }

            pub fn share_pages(&mut self) {
                self.contexts.share_pages();
                if self.groups.is_empty() && self.extensions.iter().all(Vec::is_empty) {
                    return;
                }
                let baseline = Arc::make_mut(&mut self.baseline);
                for (position, additions) in self.extensions.iter_mut().enumerate() {
                    for handle in additions.iter() {
                        baseline.maximum_generation =
                            baseline.maximum_generation.max(handle.generation());
                    }
                    baseline.members[position].append(additions);
                }
                for group in &self.members {
                    for handle in group {
                        baseline.maximum_generation =
                            baseline.maximum_generation.max(handle.generation());
                    }
                }
                baseline.members.append(&mut self.members);
                baseline.points.extend(self.groups.drain());
                self.extensions.clear();
            }

            pub fn begin_page(&mut self, generation: u32) {
                self.contexts.begin_page(generation);
            }

            pub fn take_additional_pages(&mut self) -> Self {
                let first_generation = self.contexts.first_generation();
                let contexts = self.contexts.take_additional_pages();
                if self.baseline.maximum_generation > first_generation {
                    let baseline = Arc::make_mut(&mut self.baseline);
                    for members in &mut baseline.members {
                        members.retain(|handle| handle.generation() == first_generation);
                    }
                    baseline.maximum_generation = if baseline.members.iter().all(Vec::is_empty) {
                        0
                    } else {
                        first_generation
                    };
                }
                for members in self.members.iter_mut().chain(&mut self.extensions) {
                    members.retain(|handle| handle.generation() == first_generation);
                }
                Self {
                    contexts,
                    ..Self::default()
                }
            }

            pub fn adopt_additional_pages(&mut self, other: Self) {
                let adopted = other
                    .contexts
                    .iter()
                    .map(|(handle, context)| (handle, context.point))
                    .collect::<Vec<_>>();
                self.contexts.adopt_additional_pages(other.contexts);
                for (context, point) in adopted {
                    self.index_context(context, point);
                }
            }

            fn index_context(&mut self, context: FactContextHandle, point: ProgramPoint) {
                if let Some(group) = self.baseline.points.get(&point) {
                    let position = group.ordinal - 1;
                    if self.extensions.len() <= position {
                        self.extensions.resize_with(position + 1, Vec::new);
                    }
                    self.extensions[position].push(context);
                } else if let Some(group) = self.groups.get(&point) {
                    let position = group.ordinal - self.baseline.members.len() - 1;
                    self.members[position].push(context);
                } else {
                    let ordinal = self.baseline.members.len() + self.members.len() + 1;
                    self.groups.insert(point, FactContextGroup { ordinal });
                    self.members.push(vec![context]);
                }
            }

            pub fn append(&mut self, context: FactContext) -> FactContextHandle {
                let point = context.point;
                let context = self.contexts.append(context);
                self.index_context(context, point);
                context
            }

            #[inline]
            pub fn get(&self, context: FactContextHandle) -> &FactContext {
                self.contexts.get(context)
            }

            pub fn free(&mut self, context: FactContextHandle) -> bool {
                self.contexts.free(context)
            }

            pub fn len(&self) -> usize {
                self.contexts.len()
            }

            pub fn is_empty(&self) -> bool {
                self.contexts.is_empty()
            }

            pub fn iter(&self) -> impl Iterator<Item = (FactContextHandle, &FactContext)> {
                self.contexts.iter()
            }

            pub(crate) fn handles_at_point(
                &self,
                point: ProgramPoint,
            ) -> impl Iterator<Item = FactContextHandle> + '_ {
                self.handles_in_group(self.group_at_point(point))
            }

            pub(crate) fn group_at_point(&self, point: ProgramPoint) -> FactContextGroup {
                self.baseline
                    .points
                    .get(&point)
                    .or_else(|| self.groups.get(&point))
                    .copied()
                    .unwrap_or_default()
            }

            pub(crate) fn handles_in_group(
                &self,
                group: FactContextGroup,
            ) -> impl Iterator<Item = FactContextHandle> + '_ {
                let empty: &[FactContextHandle] = &[];
                let (baseline, additions) = match group.ordinal.checked_sub(1) {
                    Some(position) if position < self.baseline.members.len() => (
                        self.baseline.members[position].as_slice(),
                        self.extensions.get(position).map_or(empty, Vec::as_slice),
                    ),
                    Some(position) => (
                        empty,
                        self.members
                            .get(position - self.baseline.members.len())
                            .map_or(empty, Vec::as_slice),
                    ),
                    None => (empty, empty),
                };
                baseline
                    .iter()
                    .chain(additions)
                    .copied()
                    .filter(|context| self.contexts.is_valid(*context))
            }
        }
    }
    pub mod view {
        use arena::HandleSpan;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;

        use crate::{
            Fact, FactPayload, FactPlan, FactRef, PlaceHandle, ProgramPoint, TypeConstraintFact,
        };

        #[derive(Debug, Clone, Copy)]
        pub struct FactContextView<'facts> {
            pub(crate) plan: &'facts FactPlan,
            pub point: ProgramPoint,
            pub(crate) facts: HandleSpan<FactRef>,
        }

        impl<'facts> FactContextView<'facts> {
            pub fn facts(self) -> impl Iterator<Item = &'facts Fact> {
                self.plan
                    .refs
                    .span_or_empty(self.facts)
                    .iter()
                    .map(move |reference| self.plan.facts.get(reference.fact))
            }

            pub fn proves_proposition_label(
                self,
                program: &TypedTrees,
                required_label: &str,
            ) -> bool {
                self.facts().any(|fact| {
                    self.plan
                        .proposition_fact_label(program, fact)
                        .is_some_and(|candidate| candidate == required_label)
                })
            }

            pub fn proves_place_domain_membership(
                self,
                program: &TypedTrees,
                place: PlaceHandle,
                domain_symbol: SymbolHandle,
            ) -> bool {
                if !typed_trees::domain::has_symbol_only_proof(program, domain_symbol) {
                    return false;
                }
                self.facts().any(|fact| {
            matches!(
                fact.payload,
                FactPayload::DomainMembership { domain_symbol: fact_domain, .. }
                    | FactPayload::ContractDomainMembership { domain_symbol: fact_domain, .. }
                    if typed_trees::domain::has_symbol_only_proof(program, fact_domain)
                        && self.plan.fact_place_equals(fact.place, place)
                && self.plan.domain_implies(fact_domain, domain_symbol)
            )
        })
            }

            pub fn proves_place_domain_membership_in_program(
                self,
                program: &TypedTrees,
                place: PlaceHandle,
                domain_symbol: SymbolHandle,
            ) -> bool {
                if !typed_trees::domain::has_symbol_only_proof(program, domain_symbol) {
                    return false;
                }
                self.facts().any(|fact| {
                    let (fact_domain, fact_place) = match fact.payload {
                        crate::domain_membership_pattern!(domain_symbol) => {
                            let Some(place) = fact.place.as_place() else {
                                return false;
                            };
                            (domain_symbol, place)
                        }
                        _ => return false,
                    };

                    typed_trees::domain::has_symbol_only_proof(program, fact_domain)
                        && self.plan.domain_implies(fact_domain, domain_symbol)
                        && self.plan.places_match(program, fact_place, place)
                })
            }

            pub fn type_constraints(self) -> impl Iterator<Item = TypeConstraintFact> + 'facts {
                self.facts().filter_map(|fact| match fact.payload {
                    FactPayload::TypeConstraint { constraint } => {
                        Some(TypeConstraintFact { constraint })
                    }
                    _ => None,
                })
            }
        }
    }
    pub use storage::{FactContextGroup, FactContexts};

    use crate::FactHandle;
    use arena::{Handle, HandleSpan};
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::name::Identifier;
    use typed_trees::types::TypeConstraintNode;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum ProgramPoint {
        #[default]
        Global,
        Definition {
            symbol: SymbolHandle,
        },
        Machine {
            machine_symbol: SymbolHandle,
        },
        State {
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
        },
        Statement {
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
        },
        Call {
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
            call_ordinal: usize,
        },
        CallRequires {
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
            call_ordinal: usize,
        },
        CallEnsures {
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
            call_ordinal: usize,
        },
        TransitionArm {
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
            transition_target: typed_trees::statement::TransitionTargetHandle,
        },
        Exit {
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
            transition_target: typed_trees::statement::TransitionTargetHandle,
        },
    }
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FactRef {
        pub fact: FactHandle,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FactContext {
        pub point: ProgramPoint,
        pub facts: HandleSpan<FactRef>,
        pub assignments: HandleSpan<AssignedFactRef>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct AssignedFactRef {
        pub root: crate::PlaceRoot,
        pub fact: FactHandle,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct SymbolFactSet {
        pub symbol: SymbolHandle,
        pub facts: HandleSpan<FactRef>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BooleanFact {
        pub expression: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DomainMembershipFact {
        pub value: ExpressionHandle,
        pub domain: HandleSpan<Identifier>,
        pub domain_symbol: SymbolHandle,
        pub semantic_domain: language_semantics::SemanticDomainId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TypeConstraintFact {
        pub constraint: Handle<TypeConstraintNode>,
    }

    #[macro_export]
    macro_rules! call_point_triple_pattern {
    ($($fields:tt)*) => {
        $crate::ProgramPoint::Call { $($fields)* }
            | $crate::ProgramPoint::CallEnsures { $($fields)* }
            | $crate::ProgramPoint::CallRequires { $($fields)* }
    };
}
}

pub mod places {
    pub mod resolution {
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        use crate::{FactPlan, Place, PlaceHandle, PlaceRoot, PlaceSegment};

        pub fn payload_variant_for_field(
            program: &TypedTrees,
            field_symbol: SymbolHandle,
        ) -> Option<SymbolHandle> {
            if !field_symbol.is_valid() {
                return None;
            }
            let parent = program.symbols.get(field_symbol).parent;
            (parent.is_valid() && program.symbols.get(parent).kind == symbols::SymbolKind::Variant)
                .then_some(parent)
        }

        pub fn effective_member_symbol(
            program: &TypedTrees,
            receiver: ExpressionHandle,
            member: &TableMemberExpression,
        ) -> SymbolHandle {
            if let Some(symbol) =
                resolve_member_symbol_from_receiver(program, receiver, member.member.as_str())
            {
                return symbol;
            }

            if member.member_symbol.is_valid() {
                return member.member_symbol;
            }

            SymbolHandle::invalid()
        }

        fn resolve_member_symbol_from_receiver(
            program: &TypedTrees,
            receiver: ExpressionHandle,
            member_name: &str,
        ) -> Option<SymbolHandle> {
            if let Some(reference) = expression_type_reference(program, receiver)
                && let Some(data) = program.data_field_definition(reference)
                && let Some(symbol) = data_member_symbol_by_name(program, data, member_name)
            {
                return Some(symbol);
            }

            let type_symbol = expression_type_symbol(program, receiver)?;

            if let Some(data) = program.data_definition_by_symbol(type_symbol)
                && let Some(symbol) = data_member_symbol_by_name(program, data, member_name)
            {
                return Some(symbol);
            }

            if let Some(machine) = program.machine(type_symbol) {
                if let Some(data) = program.attached_data_definition(machine)
                    && let Some(symbol) = data_member_symbol_by_name(program, data, member_name)
                {
                    return Some(symbol);
                }
                for owned in program.machine_owned_data(machine) {
                    if owned.name.as_str() == member_name {
                        return Some(owned.symbol);
                    }
                }
            }

            None
        }

        pub fn canonical_place_label_from_parts(
            program: &TypedTrees,
            root: PlaceRoot,
            segments: &[PlaceSegment],
        ) -> String {
            let mut label = String::new();
            write_canonical_place_label(program, root, segments, &mut label);
            label
        }

        pub(crate) fn write_canonical_place_label(
            program: &TypedTrees,
            root: PlaceRoot,
            segments: &[PlaceSegment],
            label: &mut String,
        ) {
            use std::fmt::Write as _;

            match root {
                PlaceRoot::Unknown => label.push_str("unknown"),
                PlaceRoot::Symbol(symbol) => push_symbol_label(program, symbol, label),
                PlaceRoot::InvocationReferent { parameter, .. } => {
                    label.push_str("invocation referent ");
                    push_symbol_label(program, parameter, label);
                }
                PlaceRoot::InvocationOutput(output) => {
                    let _ = write!(
                        label,
                        "invocation output {:?}:{}:{}:{:?}",
                        output.state_symbol,
                        output.statement_index,
                        output.call_ordinal,
                        output.parameter_symbol
                    );
                }
                PlaceRoot::Expression(expression) => {
                    program
                        .expression_table
                        .write_display_name(expression, label);
                }
                PlaceRoot::TypeReference(type_reference) => {
                    label.push_str(&program.display_type_reference(type_reference));
                }
            }

            for segment in segments {
                match segment {
                    PlaceSegment::Field { symbol } => {
                        label.push(language_core::PLACE_MEMBER_SEPARATOR);
                        push_symbol_label(program, *symbol, label);
                    }
                    PlaceSegment::Case { variant } => {
                        label.push_str("::");
                        push_symbol_label(program, *variant, label);
                    }
                    PlaceSegment::FixedIndex { index } => {
                        let _ = write!(label, "[{index}]");
                    }
                    PlaceSegment::FixedRange { start, end } => {
                        let _ = write!(label, "[{start}..{end}]");
                    }
                    PlaceSegment::OpaqueRange => label.push_str("[?..?]"),
                    PlaceSegment::Index { expression } => {
                        label.push('[');
                        program
                            .expression_table
                            .write_display_name(*expression, label);
                        label.push(']');
                    }
                }
            }
        }

        fn push_symbol_label(program: &TypedTrees, symbol: SymbolHandle, label: &mut String) {
            if symbol.is_valid() {
                label.push_str(program.symbols.name(symbol));
            } else {
                label.push_str("unknown");
            }
        }

        fn expression_type_symbol(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Option<SymbolHandle> {
            if !expression.is_valid() {
                return None;
            }

            match program.expression_table.expression(expression) {
                ExpressionNode::Borrow(inner) => expression_type_symbol(program, inner.target),
                ExpressionNode::Name(path) => {
                    let symbol = if path.head_symbol.is_valid() {
                        path.head_symbol
                    } else {
                        path.symbol
                    };
                    symbol_type_symbol(program, symbol)
                }
                ExpressionNode::Member(member) => {
                    let symbol = effective_member_symbol(program, member.receiver, member);
                    symbol_type_symbol(program, symbol)
                }
                ExpressionNode::Indexed(_) => expression_type_reference(program, expression)
                    .map(|reference| type_reference_base_symbol(program, reference)),
                _ => None,
            }
        }

        fn symbol_type_symbol(program: &TypedTrees, symbol: SymbolHandle) -> Option<SymbolHandle> {
            if !symbol.is_valid() {
                return None;
            }

            if program.symbols.get(symbol).kind == symbols::SymbolKind::Machine {
                for machine in program.machines() {
                    if machine.symbol == symbol
                        && let Some(data) = program.attached_data_definition(machine)
                    {
                        return Some(data.symbol);
                    }
                }
                return None;
            }
            symbol_type_reference(program, symbol)
                .map(|reference| type_reference_base_symbol(program, reference))
        }

        fn symbol_type_reference(
            program: &TypedTrees,
            symbol: SymbolHandle,
        ) -> Option<TypeReferenceHandle> {
            if !symbol.is_valid() {
                return None;
            }
            let declaration = program.symbols.get(symbol);
            match declaration.kind {
                symbols::SymbolKind::Parameter | symbols::SymbolKind::Local => {
                    let state_symbol = declaration.parent;
                    let machine_symbol = program.symbols.get(state_symbol).parent;
                    if program.symbols.get(machine_symbol).kind != symbols::SymbolKind::Machine {
                        return None;
                    }
                    let (_machine, state) = program.machine_state(machine_symbol, state_symbol)?;
                    if declaration.kind == symbols::SymbolKind::Parameter {
                        program
                            .state_parameter_by_symbol(state, symbol)
                            .map(|parameter| parameter.type_reference)
                    } else {
                        program
                            .state_local_declaration(state, symbol)
                            .filter(|(_, local)| {
                                program.symbols.name(symbol) == local.name.as_str()
                            })
                            .map(|(_, local)| local.type_reference)
                    }
                }
                symbols::SymbolKind::Field => match program.symbols.get(declaration.parent).kind {
                    symbols::SymbolKind::Machine => {
                        let machine = program.machine(declaration.parent)?;
                        program
                            .machine_owned_data_by_symbol(machine, symbol)
                            .map(|owned| owned.type_reference)
                    }
                    symbols::SymbolKind::Variant => {
                        let data_symbol = program.symbols.get(declaration.parent).parent;
                        let data = program.data_definition_by_symbol(data_symbol)?;
                        let variant =
                            program
                                .data_members(data)
                                .iter()
                                .find_map(|member| match member {
                                    typed_trees::data::DataMember::Variant(variant)
                                        if variant.symbol == declaration.parent =>
                                    {
                                        Some(variant)
                                    }
                                    _ => None,
                                })?;
                        program
                            .data_payload_field_by_symbol(variant, symbol)
                            .map(|field| field.type_reference)
                    }
                    symbols::SymbolKind::Data => {
                        let data = program.data_definition_by_symbol(declaration.parent)?;
                        program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                typed_trees::data::DataMember::Field(field)
                                    if field.symbol == symbol =>
                                {
                                    Some(field.type_reference)
                                }
                                _ => None,
                            })
                    }
                    _ => None,
                },
                _ => None,
            }
        }

        fn expression_type_reference(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            match program.expression_table.expression(expression) {
                ExpressionNode::Borrow(borrow) => expression_type_reference(program, borrow.target),
                ExpressionNode::Name(name) => symbol_type_reference(
                    program,
                    if name.head_symbol.is_valid() {
                        name.head_symbol
                    } else {
                        name.symbol
                    },
                ),
                ExpressionNode::Member(member) => symbol_type_reference(
                    program,
                    effective_member_symbol(program, member.receiver, member),
                ),
                ExpressionNode::Indexed(indexed)
                    if !matches!(
                        program.expression_table.expression(indexed.index),
                        ExpressionNode::Range(_)
                    ) =>
                {
                    collection_element_type(
                        program,
                        expression_type_reference(program, indexed.collection)?,
                    )
                }
                _ => None,
            }
        }

        fn collection_element_type(
            program: &TypedTrees,
            reference: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            let mut reference = program
                .type_reference_table
                .peeled_type_reference(reference);
            for _ in 0..8 {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => return Some(*element_type),
                    TypeReferenceNode::Named { symbol, .. } => {
                        let origin = program
                            .data_definition_by_symbol(*symbol)?
                            .generic_instance
                            .filter(|origin| origin.is_valid() && *origin != reference)?;
                        reference = program.type_reference_table.peeled_type_reference(origin);
                    }
                    _ => return None,
                }
            }
            None
        }

        fn type_reference_base_symbol(
            program: &TypedTrees,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> SymbolHandle {
            let peeled = program
                .type_reference_table
                .peeled_type_reference(type_reference);
            match program.type_reference_table.type_reference(peeled) {
                typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
                | typed_trees::types::TypeReferenceNode::DynamicTrait {
                    symbol: base_symbol,
                    ..
                }
                | typed_trees::types::TypeReferenceNode::Named {
                    symbol: base_symbol,
                    ..
                } => *base_symbol,
                _ => SymbolHandle::invalid(),
            }
        }

        pub fn resolve_place_member_symbol(
            program: &TypedTrees,
            facts: &FactPlan,
            place: PlaceHandle,
            member_name: &str,
        ) -> Option<SymbolHandle> {
            let place = facts.places.get(place);
            let reference = fact_place_type_reference(program, facts, place)?;
            if let Some(data) = program.data_field_definition(reference)
                && let Some(symbol) = data_member_symbol_by_name(program, data, member_name)
            {
                return Some(symbol);
            }
            let base_symbol = type_reference_base_symbol(program, reference);

            if let Some(machine) = program.machine(base_symbol)
                && let Some(data) = program.attached_data_definition(machine)
                && let Some(symbol) = data_member_symbol_by_name(program, data, member_name)
            {
                return Some(symbol);
            }

            if let Some(data) = program.data_definition_by_symbol(base_symbol)
                && let Some(symbol) = data_member_symbol_by_name(program, data, member_name)
            {
                return Some(symbol);
            }

            None
        }

        fn data_member_symbol_by_name(
            program: &TypedTrees,
            data: &typed_trees::data::DataDefinition,
            member_name: &str,
        ) -> Option<SymbolHandle> {
            program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) => {
                        (field.name.as_str() == member_name).then_some(field.symbol)
                    }
                    typed_trees::data::DataMember::Variant(variant) => (variant.name.as_str()
                        == member_name)
                        .then_some(variant.symbol)
                        .or_else(|| {
                            program
                                .data_payload_fields(variant)
                                .iter()
                                .find_map(|field| {
                                    (field.name.as_str() == member_name).then_some(field.symbol)
                                })
                        }),
                })
        }

        fn fact_place_type_reference(
            program: &TypedTrees,
            facts: &FactPlan,
            place: &Place,
        ) -> Option<TypeReferenceHandle> {
            let mut reference = match place.root {
                PlaceRoot::Symbol(symbol) => {
                    symbol_type_symbol(program, symbol)?;
                    symbol_type_reference(program, symbol)?
                }
                PlaceRoot::Expression(expression) => {
                    expression_type_symbol(program, expression)?;
                    expression_type_reference(program, expression)?
                }
                PlaceRoot::Unknown
                | PlaceRoot::TypeReference(_)
                | PlaceRoot::InvocationReferent { .. }
                | PlaceRoot::InvocationOutput(_) => {
                    return None;
                }
            };

            for segment in facts.place_segments.span_or_empty(place.segments) {
                match segment {
                    PlaceSegment::Field { symbol } => {
                        reference = symbol_type_reference(program, *symbol)?;
                    }
                    PlaceSegment::Case { .. } => {}
                    PlaceSegment::Index { expression }
                        if matches!(
                            program.expression_table.expression(*expression),
                            ExpressionNode::Range(_)
                        ) =>
                    {
                        return None;
                    }
                    PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. } => {
                        reference = collection_element_type(program, reference)?;
                    }
                    PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange => return None,
                }
            }

            reference.is_valid().then_some(reference)
        }
    }
    pub(crate) mod view_coordinates {
        use super::PlaceSegment;

        pub fn compose_captured_view_segments(segments: &mut Vec<PlaceSegment>) -> Option<()> {
            let mut composed = Vec::with_capacity(segments.len());
            for &segment in segments.iter() {
                if let Some(previous) = composed.last().copied()
                    && let Some(combined) = compose_boundary(previous, segment)?
                {
                    *composed.last_mut()? = combined;
                } else {
                    composed.push(segment);
                }
            }
            *segments = composed;
            Some(())
        }

        pub fn project_captured_view_segments(
            captured: &[PlaceSegment],
            projection: &[PlaceSegment],
        ) -> Option<Vec<PlaceSegment>> {
            let mut segments = captured.to_vec();
            let Some((&first, remaining)) = projection.split_first() else {
                return Some(segments);
            };
            let first = match (segments.last(), first) {
                (
                    Some(PlaceSegment::FixedRange { start, end }),
                    PlaceSegment::FixedRange {
                        start: 0,
                        end: usize::MAX,
                    },
                ) => PlaceSegment::FixedRange {
                    start: 0,
                    end: end.checked_sub(*start)?,
                },
                _ => first,
            };
            if let Some(previous) = segments.last().copied()
                && let Some(combined) = compose_boundary(previous, first)?
            {
                *segments.last_mut()? = combined;
            } else {
                segments.push(first);
            }
            segments.extend_from_slice(remaining);
            Some(segments)
        }

        fn compose_boundary(
            previous: PlaceSegment,
            next: PlaceSegment,
        ) -> Option<Option<PlaceSegment>> {
            Some(match (previous, next) {
                (
                    PlaceSegment::OpaqueRange,
                    PlaceSegment::OpaqueRange | PlaceSegment::FixedRange { .. },
                )
                | (PlaceSegment::FixedRange { .. }, PlaceSegment::OpaqueRange) => {
                    Some(PlaceSegment::OpaqueRange)
                }
                (
                    PlaceSegment::OpaqueRange,
                    PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. },
                )
                | (PlaceSegment::FixedRange { .. }, PlaceSegment::Index { .. }) => {
                    Some(PlaceSegment::Index {
                        expression: typed_trees::expression::ExpressionHandle::invalid(),
                    })
                }
                (PlaceSegment::FixedRange { start, end }, PlaceSegment::FixedIndex { index }) => {
                    if index >= end.checked_sub(start)? {
                        return None;
                    }
                    Some(PlaceSegment::FixedIndex {
                        index: start.checked_add(index)?,
                    })
                }
                (
                    PlaceSegment::FixedRange { start, end },
                    PlaceSegment::FixedRange {
                        start: next_start,
                        end: next_end,
                    },
                ) => {
                    if next_start > next_end || next_end > end.checked_sub(start)? {
                        return None;
                    }
                    Some(PlaceSegment::FixedRange {
                        start: start.checked_add(next_start)?,
                        end: start.checked_add(next_end)?,
                    })
                }
                _ => None,
            })
        }

        #[cfg(test)]
        mod tests {
            use super::{PlaceSegment, compose_captured_view_segments};

            #[test]
            fn opaque_window_composition_preserves_dimensions_not_bounds() {
                let mut windows = vec![
                    PlaceSegment::FixedRange { start: 1, end: 8 },
                    PlaceSegment::OpaqueRange,
                    PlaceSegment::FixedRange { start: 0, end: 3 },
                ];
                assert_eq!(compose_captured_view_segments(&mut windows), Some(()));
                assert_eq!(windows, [PlaceSegment::OpaqueRange]);

                let mut element = vec![
                    PlaceSegment::OpaqueRange,
                    PlaceSegment::FixedIndex { index: 2 },
                ];
                assert_eq!(compose_captured_view_segments(&mut element), Some(()));
                assert!(
                    matches!(element.as_slice(), [PlaceSegment::Index { expression }] if !expression.is_valid())
                );
            }
        }
    }
    pub mod write_frame {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum WriteFrameCompleteness {
            Complete,
            #[default]
            Opaque,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct NormalizedWriteFrame {
            completeness: WriteFrameCompleteness,
            paths: Vec<String>,
            compatibility_report_fingerprint: u64,
        }

        impl Default for NormalizedWriteFrame {
            fn default() -> Self {
                Self::opaque()
            }
        }

        impl NormalizedWriteFrame {
            pub fn complete(mut paths: Vec<String>) -> Self {
                paths.sort();
                paths.dedup();
                let mut canonical: Vec<String> = Vec::with_capacity(paths.len());
                for path in paths {
                    if !canonical.iter().any(|ancestor| {
                        path.strip_prefix(ancestor.as_str()).is_some_and(|suffix| {
                            suffix.starts_with('.') || suffix.starts_with('[')
                        })
                    }) {
                        canonical.push(path);
                    }
                }
                let paths = canonical;
                let compatibility_report_fingerprint =
                    non_authoritative_write_frame_compatibility_fingerprint(
                        WriteFrameCompleteness::Complete,
                        &paths,
                    );
                Self {
                    completeness: WriteFrameCompleteness::Complete,
                    paths,
                    compatibility_report_fingerprint,
                }
            }

            pub fn opaque() -> Self {
                Self {
                    completeness: WriteFrameCompleteness::Opaque,
                    paths: Vec::new(),
                    compatibility_report_fingerprint:
                        non_authoritative_write_frame_compatibility_fingerprint(
                            WriteFrameCompleteness::Opaque,
                            &[],
                        ),
                }
            }

            pub fn completeness(&self) -> WriteFrameCompleteness {
                self.completeness
            }

            pub fn is_complete(&self) -> bool {
                self.completeness == WriteFrameCompleteness::Complete
            }

            pub fn paths(&self) -> &[String] {
                &self.paths
            }

            pub fn complete_paths(&self) -> Option<&[String]> {
                self.is_complete().then_some(self.paths.as_slice())
            }

            pub fn into_complete_paths(self) -> Option<Vec<String>> {
                self.is_complete().then_some(self.paths)
            }

            pub fn compatibility_report_fingerprint(&self) -> u64 {
                self.compatibility_report_fingerprint
            }
        }

        fn non_authoritative_write_frame_compatibility_fingerprint(
            completeness: WriteFrameCompleteness,
            normalized_paths: &[String],
        ) -> u64 {
            let mut hash = semantic_vocabulary::Fnv1a::new();
            hash.byte(match completeness {
                WriteFrameCompleteness::Complete => 1,
                WriteFrameCompleteness::Opaque => 2,
            });
            for path in normalized_paths {
                hash.bytes_framed(path.as_bytes());
            }
            hash.finish()
        }

        #[cfg(test)]
        mod tests {
            use super::NormalizedWriteFrame;

            #[test]
            fn complete_frames_normalize_order_and_duplicates_before_identity() {
                let first = NormalizedWriteFrame::complete(vec![
                    "$P0.value".to_owned(),
                    "self.count".to_owned(),
                    "$P0.value".to_owned(),
                ]);
                let second = NormalizedWriteFrame::complete(vec![
                    "self.count".to_owned(),
                    "$P0.value".to_owned(),
                ]);

                assert_eq!(first, second);
                assert_eq!(
                    first.paths(),
                    &["$P0.value".to_owned(), "self.count".to_owned()]
                );
            }

            #[test]
            fn complete_frames_remove_only_covered_storage_descendants() {
                let normalized = NormalizedWriteFrame::complete(vec![
                    "$P0.focus".into(),
                    "$P0".into(),
                    "$P0.focus_visible".into(),
                    "$P01.focus".into(),
                    "$P1.items[2]".into(),
                    "$P1.items".into(),
                    "$P1.items_extra".into(),
                ]);
                assert_eq!(
                    normalized,
                    NormalizedWriteFrame::complete(vec![
                        "$P0".into(),
                        "$P01.focus".into(),
                        "$P1.items".into(),
                        "$P1.items_extra".into()
                    ])
                );
                assert_ne!(
                    normalized,
                    NormalizedWriteFrame::complete(vec!["$P0".into(), "$P1.items".into()])
                );
            }

            #[test]
            fn opaque_frame_is_distinct_from_complete_empty_frame() {
                let opaque = NormalizedWriteFrame::opaque();
                let pure = NormalizedWriteFrame::complete(Vec::new());

                assert_ne!(
                    opaque.compatibility_report_fingerprint(),
                    pure.compatibility_report_fingerprint()
                );
                assert_eq!(NormalizedWriteFrame::default(), opaque);
                assert!(opaque.complete_paths().is_none());
                assert_eq!(pure.complete_paths(), Some([].as_slice()));
            }
        }
    }

    pub use view_coordinates::{compose_captured_view_segments, project_captured_view_segments};

    use crate::PlaceHandle;
    use arena::HandleSpan;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub struct InvocationOutputIdentity {
        pub state_symbol: SymbolHandle,
        pub statement_index: usize,
        pub call_ordinal: usize,
        pub parameter_symbol: SymbolHandle,
    }

    impl InvocationOutputIdentity {
        pub fn is_valid(self) -> bool {
            self.state_symbol.is_valid() && self.parameter_symbol.is_valid()
        }

        pub fn compare_identity(self, other: Self) -> std::cmp::Ordering {
            let key = |value: Self| {
                (
                    (
                        value.state_symbol.arena_index(),
                        value.state_symbol.generation(),
                    ),
                    value.statement_index,
                    value.call_ordinal,
                    (
                        value.parameter_symbol.arena_index(),
                        value.parameter_symbol.generation(),
                    ),
                )
            };
            key(self).cmp(&key(other))
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum PlaceRoot {
        #[default]
        Unknown,
        Symbol(SymbolHandle),
        InvocationReferent {
            parameter: SymbolHandle,
            type_reference: TypeReferenceHandle,
        },
        InvocationOutput(InvocationOutputIdentity),
        Expression(ExpressionHandle),
        TypeReference(TypeReferenceHandle),
    }

    macro_rules! place_root_handle_accessors {
    ($($variant:ident => $method:ident -> $handle:ty),* $(,)?) => {
        $(
            pub fn $method(&self) -> Option<$handle> {
                let Self::$variant(handle) = self else {
                    return None;
                };
                Some(*handle)
            }
        )*
    };
}

    impl PlaceRoot {
        pub fn is_valid(&self) -> bool {
            match self {
                Self::Unknown => false,
                Self::Symbol(symbol) => symbol.is_valid(),
                Self::InvocationReferent {
                    parameter,
                    type_reference,
                } => parameter.is_valid() && type_reference.is_valid(),
                Self::InvocationOutput(output) => output.is_valid(),
                Self::Expression(expression) => expression.is_valid(),
                Self::TypeReference(reference) => reference.is_valid(),
            }
        }

        pub fn group_key(&self) -> (u8, u32, u32) {
            match self {
                Self::Unknown => (0, 0, 0),
                Self::Symbol(symbol) => (1, symbol.arena_index(), symbol.generation()),
                Self::InvocationReferent { parameter, .. } => {
                    (2, parameter.arena_index(), parameter.generation())
                }
                Self::InvocationOutput(output) => {
                    let parameter = output.parameter_symbol;
                    (3, parameter.arena_index(), parameter.generation())
                }
                Self::Expression(expression) => {
                    (4, expression.arena_index(), expression.generation())
                }
                Self::TypeReference(reference) => {
                    (5, reference.arena_index(), reference.generation())
                }
            }
        }

        place_root_handle_accessors!(
            Symbol => as_symbol -> SymbolHandle,
            Expression => as_expression -> ExpressionHandle,
            TypeReference => as_type_reference -> TypeReferenceHandle,
        );
    }

    impl PlaceSegment {
        pub fn order_key(&self) -> (u8, u64, u64) {
            let handle =
                |index: u32, generation: u32| (u64::from(index) << 32) | u64::from(generation);
            match self {
                Self::Field { symbol } => (0, handle(symbol.arena_index(), symbol.generation()), 0),
                Self::Case { variant } => {
                    (1, handle(variant.arena_index(), variant.generation()), 0)
                }
                Self::FixedIndex { index } => (2, *index as u64, 0),
                Self::FixedRange { start, end } => (3, *start as u64, *end as u64),
                Self::OpaqueRange => (4, 0, 0),
                Self::Index { expression } => (
                    5,
                    handle(expression.arena_index(), expression.generation()),
                    0,
                ),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum PlaceSegment {
        Field { symbol: SymbolHandle },
        Case { variant: SymbolHandle },
        FixedIndex { index: usize },
        FixedRange { start: usize, end: usize },
        OpaqueRange,
        Index { expression: ExpressionHandle },
    }

    impl PlaceSegment {
        pub fn erase_runtime_selector(
            self,
            is_range: impl FnOnce(ExpressionHandle) -> bool,
        ) -> Self {
            match self {
                Self::Index { expression } if expression.is_valid() && is_range(expression) => {
                    Self::OpaqueRange
                }
                Self::Index { .. } => Self::Index {
                    expression: ExpressionHandle::invalid(),
                },
                _ => self,
            }
        }
    }

    impl Default for PlaceSegment {
        fn default() -> Self {
            Self::Field {
                symbol: SymbolHandle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct Place {
        pub root: PlaceRoot,
        pub segments: HandleSpan<PlaceSegment>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FactPlace {
        #[default]
        Unknown,
        Place(PlaceHandle),
        Symbol(SymbolHandle),
        Expression(ExpressionHandle),
        TypeReference(TypeReferenceHandle),
    }

    impl FactPlace {
        place_root_handle_accessors!(
            Place => as_place -> PlaceHandle,
            Symbol => as_symbol -> SymbolHandle,
            Expression => as_expression -> ExpressionHandle,
        );
    }

    pub fn place_bucket_hash(root: PlaceRoot, segments: &[PlaceSegment]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        root.hash(&mut hash);
        segments.hash(&mut hash);
        hash.finish()
    }

    pub struct PlaceBucketIndex {
        positions: Vec<(u64, usize)>,
    }

    impl PlaceBucketIndex {
        pub fn new(keys: impl IntoIterator<Item = u64>) -> Self {
            let mut positions: Vec<_> = keys
                .into_iter()
                .enumerate()
                .map(|(position, key)| (key, position))
                .collect();
            positions.sort_unstable();
            Self { positions }
        }

        pub fn candidates(&self, key: u64) -> impl Iterator<Item = usize> + '_ {
            let start = self
                .positions
                .partition_point(|(candidate, _)| *candidate < key);
            let end =
                start + self.positions[start..].partition_point(|(candidate, _)| *candidate == key);
            self.positions[start..end]
                .iter()
                .map(|(_, position)| *position)
        }

        pub fn group_equal_positions(&self, equal: impl Fn(usize, usize) -> bool) -> Vec<usize> {
            let mut first = vec![usize::MAX; self.positions.len()];
            let mut representatives = Vec::new();
            let mut bucket = None;
            let mut bucket_start = 0;
            for (key, position) in &self.positions {
                if bucket != Some(*key) {
                    bucket = Some(*key);
                    bucket_start = representatives.len();
                }
                let previous = representatives[bucket_start..]
                    .iter()
                    .copied()
                    .find(|previous| equal(*previous, *position));
                first[*position] = if let Some(previous) = previous {
                    previous
                } else {
                    representatives.push(*position);
                    *position
                };
            }
            first
        }
    }

    #[cfg(test)]
    mod bucket_index_tests {
        use super::PlaceBucketIndex;

        #[test]
        fn scratch_buckets_keep_collisions_and_first_encounter_positions() {
            let rows = [
                (8, "left"),
                (3, "other"),
                (8, "right"),
                (8, "left"),
                (3, "other"),
            ];
            let index = PlaceBucketIndex::new(rows.iter().map(|(key, _)| *key));
            assert_eq!(index.candidates(8).collect::<Vec<_>>(), [0, 2, 3]);
            assert!(index.candidates(99).next().is_none());
            let expected = [0, 1, 2, 0, 1];
            assert_eq!(
                index.group_equal_positions(|left, right| rows[left] == rows[right]),
                expected
            );
            let collisions = PlaceBucketIndex::new(rows.iter().map(|_| 0));
            assert_eq!(
                collisions.group_equal_positions(|left, right| rows[left] == rows[right]),
                expected
            );
        }
    }
}
