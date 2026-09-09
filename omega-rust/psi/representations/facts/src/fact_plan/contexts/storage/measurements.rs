use std::hint::black_box;
use std::time::Instant;

use arena::HandleSpan;
use symbols::SymbolHandle;

use super::*;

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
