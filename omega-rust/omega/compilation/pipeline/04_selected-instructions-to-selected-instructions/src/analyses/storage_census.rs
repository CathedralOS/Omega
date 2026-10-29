//! Profiling-only storage counts. Borrow facts; never clone their payloads.
use register_homes::FunctionAllocationLegality;
use selected_instructions::{FunctionLiveRanges, FunctionLiveness, SelectedFunction};
use std::mem::size_of;
use std::sync::Arc;

#[derive(Default, Debug)]
pub(super) struct Storage {
    vector_len_bytes: usize,
    vector_capacity_bytes: usize,
    // Arena exposes live storage, not reservation capacity. Keep it separate.
    arena_payload_bytes: usize,
    interference_len: usize,
    interference_capacity: usize,
}

impl Storage {
    fn vector<T>(&mut self, value: &Vec<T>) {
        self.vector_len_bytes = self
            .vector_len_bytes
            .saturating_add(value.len().saturating_mul(size_of::<T>()));
        self.vector_capacity_bytes = self
            .vector_capacity_bytes
            .saturating_add(value.capacity().saturating_mul(size_of::<T>()));
    }

    fn add(&mut self, other: Self) {
        self.vector_len_bytes = self.vector_len_bytes.saturating_add(other.vector_len_bytes);
        self.vector_capacity_bytes = self
            .vector_capacity_bytes
            .saturating_add(other.vector_capacity_bytes);
        self.arena_payload_bytes = self
            .arena_payload_bytes
            .saturating_add(other.arena_payload_bytes);
        self.interference_len = self.interference_len.saturating_add(other.interference_len);
        self.interference_capacity = self
            .interference_capacity
            .saturating_add(other.interference_capacity);
    }
}

pub(super) fn liveness(value: &FunctionLiveness) -> Storage {
    let mut out = Storage::default();
    out.vector(&value.entry_definitions);
    out.vector(&value.blocks);
    out.arena_payload_bytes = std::mem::size_of_val(value.operand_positions.storage_slice());
    for block in &value.blocks {
        out.vector(&block.virtual_live_in);
        out.vector(&block.virtual_live_out);
        out.vector(&block.unit_live_in);
        out.vector(&block.unit_live_out);
        out.vector(&block.instructions);
        out.vector(&block.successors);
        for instruction in &block.instructions {
            out.vector(&instruction.virtual_uses);
            out.vector(&instruction.virtual_defs);
            out.vector(&instruction.virtual_live_in);
            out.vector(&instruction.virtual_live_out);
            out.vector(&instruction.unit_uses);
            out.vector(&instruction.unit_defs);
            out.vector(&instruction.unit_clobbers);
            out.vector(&instruction.unit_live_in);
            out.vector(&instruction.unit_live_out);
        }
        for edge in &block.successors {
            out.vector(&edge.virtual_live);
            out.vector(&edge.unit_live);
        }
    }
    out
}

pub(super) fn ranges(value: &FunctionLiveRanges) -> Storage {
    let mut out = Storage::default();
    out.vector(&value.block_domains);
    out.vector(&value.virtual_registers);
    out.vector(&value.tied_pairs);
    out.vector(&value.edge_transfers);
    out.vector(&value.copy_affinities);
    out.vector(&value.early_clobbers);
    out.vector(&value.architectural_units);
    out.vector(&value.interference);
    out.interference_len = value.interference.len();
    out.interference_capacity = value.interference.capacity();
    for register in &value.virtual_registers {
        out.vector(&register.occurrences);
        out.vector(&register.fixed_constraints);
        out.vector(&register.fragments);
        out.vector(&register.edge_connectors);
    }
    for early in &value.early_clobbers {
        out.vector(&early.uses);
    }
    for unit in &value.architectural_units {
        out.vector(&unit.actions);
        out.vector(&unit.fragments);
        out.vector(&unit.edge_connectors);
    }
    out
}

pub(super) fn legality(value: &FunctionAllocationLegality) -> Storage {
    let mut out = Storage::default();
    out.vector(&value.virtual_registers);
    for register in &value.virtual_registers {
        out.vector(&register.spans);
        out.vector(&register.early_clobber_points);
        out.vector(&register.entry_transitions);
        for span in &register.spans {
            out.vector(&span.candidates);
        }
        for point in &register.early_clobber_points {
            out.vector(&point.candidates);
        }
    }
    out
}

pub(super) fn replacement(
    phase: &str,
    function: usize,
    before: Storage,
    after: Storage,
    prior_arc_count: usize,
) {
    // Only verbosity is filtered. No computation/storage decision reads this.
    if phase_profile::enabled()
        && before
            .vector_capacity_bytes
            .saturating_add(after.vector_capacity_bytes)
            >= 1024 * 1024
    {
        eprintln!(
            "storage-census replacement phase={phase} function={function} prior_arc_count={prior_arc_count} before={before:?} after={after:?}"
        );
    }
}

pub(super) fn function(function: usize, selected: &SelectedFunction) {
    if !phase_profile::enabled() {
        return;
    }
    let max_id = selected.virtual_registers.iter().map(|r| r.id.0).max();
    eprintln!(
        "storage-census function index={function} machine={:?} blocks={} registers={} max_declared_vreg_id={max_id:?}",
        selected.machine,
        selected.blocks.len(),
        selected.virtual_registers.len()
    );
}

pub(super) fn roster(
    phase: &str,
    liveness_rows: &[Arc<FunctionLiveness>],
    range_rows: &[Arc<FunctionLiveRanges>],
    legality_rows: &[Arc<FunctionAllocationLegality>],
) {
    if !phase_profile::enabled() {
        return;
    }
    let mut live = Storage::default();
    let mut range = Storage::default();
    let mut legal = Storage::default();
    for row in liveness_rows {
        live.add(liveness(row));
    }
    for row in range_rows {
        range.add(ranges(row));
    }
    for row in legality_rows {
        legal.add(legality(row));
    }
    // These are references, not deduplicated allocation totals across rosters.
    let live_refs: usize = liveness_rows.iter().fold(0usize, |sum, row| {
        sum.saturating_add(Arc::strong_count(row))
    });
    let range_refs: usize = range_rows.iter().fold(0usize, |sum, row| {
        sum.saturating_add(Arc::strong_count(row))
    });
    let legal_refs: usize = legality_rows.iter().fold(0usize, |sum, row| {
        sum.saturating_add(Arc::strong_count(row))
    });
    eprintln!(
        "storage-census roster phase={phase} functions={} live_arc_refs={live_refs} range_arc_refs={range_refs} legality_arc_refs={legal_refs} liveness={live:?} ranges={range:?} legality={legal:?}",
        liveness_rows.len()
    );
}

pub(super) fn sharing<T>(phase: &str, original: &[Arc<T>], current: &[Arc<T>]) {
    if !phase_profile::enabled() {
        return;
    }
    let shared = original
        .iter()
        .zip(current)
        .filter(|(a, b)| Arc::ptr_eq(a, b))
        .count();
    eprintln!(
        "storage-census sharing phase={phase} original_rows={} current_rows={} same_index_shared_rows={shared} replaced_rows={}",
        original.len(),
        current.len(),
        current.len().saturating_sub(shared)
    );
}

/// Visit only existing memo allocations. Profile gating avoids all default-off
/// walks; no `lookup()`/`slot_summary()` builder is called by this observation.
pub(super) fn selected_caches(phase: &str, plan: &selected_instructions::SelectedInstructionPlan) {
    if !phase_profile::enabled() {
        return;
    }
    let mut total = selected_instructions::InitializedBlockCacheStorage::default();
    for (index, function) in plan.functions.iter().enumerate() {
        let row = function.blocks.initialized_cache_storage();
        if row
            .lookup_vector_capacity_bytes
            .saturating_add(row.slot_vector_capacity_bytes)
            >= 1024 * 1024
        {
            eprintln!(
                "storage-census initialized-cache-function phase={phase} function={index} machine={:?} storage={row:?}",
                function.machine
            );
        }
        total.accumulate(row);
    }
    let holes = total
        .span_len
        .saturating_sub(total.span_used)
        .saturating_mul(8);
    let slack = total
        .span_capacity
        .saturating_sub(total.span_len)
        .saturating_mul(8);
    eprintln!(
        "storage-census initialized-cache-roster phase={phase} functions={} storage={total:?} span_hole_payload_bytes={holes} span_capacity_slack_bytes={slack} scope=referenced-capacity-not-unique-RSS",
        plan.functions.len()
    );
}
