//! Preserve the load slots before spatial normalization discards them.
use super::*;
use crate::machine_calls::calls::CapturedStorageReadBacking;

pub(in crate::machine_calls::calls::write_frames) fn capture(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    index: usize,
    root: SymbolHandle,
    path: &[PlaceSegment],
    prefix: &CollectedStatementPrefix,
) -> Option<CapturedStorageReadBacking> {
    let source = FrameSourcePlace {
        root,
        segments: path.to_vec(),
        builtin_coordinates: true,
        ..FrameSourcePlace::default()
    };
    let reads = super::coordinate_reads_in_prefix(program, machine, state, index, &source, prefix)?;
    if reads.is_empty() {
        return None;
    }
    let mut values = vec![super::super::frame_coordinate_path(program, root, path)?];
    let mut bindings = Vec::new();
    for read in reads {
        if !values.contains(&read.source.origin.path) {
            values.push(read.source.origin.path);
        }
        for slot in read.slots {
            if slot.precision != FramePathPrecision::Exact
                || !slot.source.root.is_valid()
                || slot.source.output_identity().is_some()
            {
                return None;
            }
            if !bindings.contains(&slot.path) {
                bindings.push(slot.path);
            }
        }
    }
    Some(CapturedStorageReadBacking { values, bindings })
}
