//! Included only by capture.patch, never by the ordinary compiler.

use crate::TypedTrees;
use selected_body_allocation_trace as trace;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

mod snapshot_limits {
    include!("snapshot_limits.rs");
}

struct Snapshot {
    ordinal: usize,
    site: &'static str,
    machines: usize,
    states: usize,
    expressions: usize,
    statements: usize,
    types: usize,
    shared: bool,
    limits: compiler_storage::RecordLimits,
}

fn capture(typed: &TypedTrees, site: &'static str, shared: bool) -> Option<Snapshot> {
    trace::directory()?;
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let ordinal = NEXT.fetch_add(1, Ordering::Relaxed);
    static LIMITS: std::sync::OnceLock<compiler_storage::RecordLimits> = std::sync::OnceLock::new();
    let limits = *LIMITS.get_or_init(|| {
        let configuration = match std::env::var(snapshot_limits::ENVIRONMENT) {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => snapshot_limits::DEFAULT.into(),
            Err(std::env::VarError::NotUnicode(_)) => {
                panic!("private snapshot limits are not UTF-8")
            }
        };
        let [bytes, allocation, elements, depth] =
            snapshot_limits::parse(&configuration).expect("validated private snapshot limits");
        compiler_storage::RecordLimits {
            bytes,
            allocation,
            elements,
            depth,
        }
    });
    Some(Snapshot {
        ordinal,
        site,
        machines: typed.machines().len(),
        states: typed.machine_states.len(),
        expressions: typed.expression_table.expression_count(),
        statements: typed.statement_table.statement_count(),
        types: typed.type_reference_table.type_reference_count(),
        shared,
        limits,
    })
}

impl Snapshot {
    fn finish(self, typed: &TypedTrees, counts: trace::Counts) {
        // Preserve the measured operation even if the diagnostic export fails.
        trace::log(format_args!(
            "SELECTED_BODY_CLONE ordinal={} context={} site={} shared={} machines={} states={} expressions={} statements={} types={} allocations={} requested_bytes={} live_delta={} peak_delta={} ns={} arena_clones={} arena_slots={} arena_payload_bytes={}",
            self.ordinal,
            trace::current_context(),
            self.site,
            self.shared,
            self.machines,
            self.states,
            self.expressions,
            self.statements,
            self.types,
            counts.allocations,
            counts.requested_bytes,
            counts.live_delta,
            counts.peak_delta,
            counts.nanoseconds,
            counts.arena_clones,
            counts.arena_slots,
            counts.arena_payload_bytes,
        ));
        // Export the returned clone before its caller mutates it. Reading the
        // payload beforehand would warm every table before the measured copy.
        if self.shared {
            let bytes = compiler_storage::encode(typed, self.limits)
                .expect("bounded real typed-program snapshot encodes");
            let length = bytes.len();
            std::fs::write(
                trace::directory()
                    .unwrap()
                    .join(format!("typed-{:04}.bin", self.ordinal)),
                bytes,
            )
            .expect("profile directory exists and snapshot is new");
            trace::log(format_args!(
                "SELECTED_BODY_SNAPSHOT ordinal={} encoded_bytes={length} limit_bytes={} limit_allocation={} limit_elements={} limit_depth={}",
                self.ordinal,
                self.limits.bytes,
                self.limits.allocation,
                self.limits.elements,
                self.limits.depth,
            ));
        }
    }
}

pub fn make_mut<'a>(typed: &'a mut Arc<TypedTrees>, site: &'static str) -> &'a mut TypedTrees {
    let snapshot = capture(typed, site, Arc::strong_count(typed) > 1);
    let (value, counts) = trace::measure(|| Arc::make_mut(typed));
    if let Some(snapshot) = snapshot {
        snapshot.finish(value, counts);
    }
    value
}

pub fn unwrap_or_clone(typed: Arc<TypedTrees>, site: &'static str) -> TypedTrees {
    let snapshot = capture(&typed, site, Arc::strong_count(&typed) > 1);
    let (value, counts) = trace::measure(|| Arc::unwrap_or_clone(typed));
    if let Some(snapshot) = snapshot {
        snapshot.finish(&value, counts);
    }
    value
}

pub fn clone_owned(typed: &TypedTrees, site: &'static str) -> TypedTrees {
    let snapshot = capture(typed, site, true);
    let (value, counts) = trace::measure(|| typed.clone());
    if let Some(snapshot) = snapshot {
        snapshot.finish(&value, counts);
    }
    value
}
