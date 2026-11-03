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
    from_reads(
        program,
        reads,
        vec![super::super::frame_coordinate_path(program, root, path)?],
    )
}

pub(in crate::machine_calls::calls::write_frames) fn from_reads(
    program: &dyn crate::CheckingInput,
    reads: Vec<stored_origins::CapturedReferenceRead>,
    mut values: Vec<String>,
) -> Option<CapturedStorageReadBacking> {
    if reads.is_empty() {
        return None;
    }
    let mut bindings = Vec::new();
    let mut sources = Vec::new();
    for read in reads {
        sources.push(read.source.origin.clone().into());
        if !values.contains(&read.source.origin.path) {
            values.push(read.source.origin.path);
        }
        for load in read.slots {
            if !load.access.is_readable() {
                return None;
            }
            let slot = load.place;
            if !super::read_projection::resolved_read_coordinate(program, &slot)
                || !slot.source.root.is_valid()
                || slot.source.output_identity().is_some()
            {
                return None;
            }
            let slot: super::super::caller_aliases::LocalWriteSource = slot.into();
            if !bindings.contains(&slot) {
                bindings.push(slot);
            }
        }
    }
    Some(CapturedStorageReadBacking {
        sources,
        values,
        bindings,
    })
}
