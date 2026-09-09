use std::hint::black_box;
use std::time::Instant;

use arena::HandleSpan;
use symbols::SymbolHandle;

use super::*;

/// Compares the former three-lookups-per-entry route with retained declaration
/// groups, including preparation and repeated context-store clones. Other flow
/// checking work is deliberately not simulated or included in these timings.
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
                    });
                }
                // Keep unrelated per-call groups in the same store, not only
                // the entry groups selected by this benchmark.
                for call_ordinal in 0..8 {
                    baseline.append(FactContext {
                        point: ProgramPoint::CallEnsures {
                            machine_symbol,
                            state_symbol,
                            statement_index: 1,
                            call_ordinal,
                        },
                        facts: HandleSpan::empty(),
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
        let new_lookups = 1 + machine_count + passes * machine_count * states_per_machine;
        println!(
            "machines={machine_count} states_per_machine={states_per_machine} declared={declared} passes={passes} contexts={} old={old_elapsed:?} prepared_including_setup={new_elapsed:?} setup={preparation:?} old_lookups={old_lookups} new_lookups={new_lookups} selector_capacity_bytes={} selected={new_count}",
            baseline.len(),
            (groups.capacity() + 1) * size_of::<FactContextGroup>()
        );
    }
}

/// Same-process lookup baseline; timings are reported, never asserted.
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
            });
            for state_index in 0..4 {
                let state_symbol =
                    SymbolHandle::from_arena_index(machine_count + index * 4 + state_index);
                let point = ProgramPoint::State {
                    machine_symbol,
                    state_symbol,
                };
                queries.extend([ProgramPoint::Global, machine_point, point]);
                for _ in 0..4 {
                    source.push(FactContext {
                        point,
                        facts: HandleSpan::empty(),
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
        // Logical retained index payload, excluding allocator slack and arena/hash
        // bookkeeping. Report capacity separately, not as a peak-memory claim.
        let link_payload = indexed.links.len() * size_of::<ContextLink>();
        let group_payload = indexed.groups.len() * size_of::<(ProgramPoint, ContextGroup)>();
        println!(
            "machines={machine_count} contexts={} queries={} baseline_setup={baseline_setup:?} indexed_setup={indexed_setup:?} baseline_lookup={baseline_lookup:?} indexed_lookup={indexed_lookup:?} scanned={} selected={indexed_selected} link_payload={link_payload} group_payload={group_payload} group_capacity={}",
            indexed.len(),
            queries.len(),
            baseline.len() * queries.len(),
            indexed.groups.capacity()
        );
    }
}
