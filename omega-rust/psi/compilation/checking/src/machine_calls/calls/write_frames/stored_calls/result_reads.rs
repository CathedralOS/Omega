//! Public result reads use completed descriptor suppliers, then current loads.
//! Return geometry alone never supplies the storage behind a reference field.

use super::super as frames;
use super::*;
use frames::path_instantiation::aggregate_arguments::ReferenceSource;
use frames::reference_subjects::{read_backing, read_projection};
use frames::stored_origins::StorageProjectionCursor;

pub(in crate::machine_calls::calls::write_frames) fn completed_result_read_backing(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    expression: ExpressionHandle,
    path: &[PlaceSegment],
    prefix: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<frames::demand::CapturedStorageReadBacking> {
    let (captured, after) = expression_effects::completed_call_value(
        program,
        machine,
        state,
        statement,
        prefix,
        symbols,
        &mut FrameInference::for_state(state.symbol),
        expression,
    )?;
    let reference = crate::machine_calls::calls::call_result_type(program, expression)?;
    if crate::reference_shell(program, reference).is_some() {
        return None;
    }
    let mut cursor = StorageProjectionCursor::new(program, reference);
    let mut first = None;
    for position in 0..=path.len() {
        if let Some(access) = cursor.reference_access() {
            if !access.is_readable() {
                return None;
            }
            first = Some(position);
            break;
        }
        if let Some(segment) = path.get(position) {
            cursor.advance(segment)?;
        }
    }
    let first = first?;
    let returned =
        captured_inputs::completed_result(program, machine, &captured, reference, symbols)?;
    let mut reads = Vec::new();
    let mut selected = false;
    for leaf in returned.references {
        if leaf.local_segments != path[..first] {
            continue;
        }
        selected = true;
        if leaf.may_select
            || !leaf.lifetime_dependencies.is_empty()
            || !matches!(leaf.kind, stored_origins::StoredOriginKind::Reference(access) if access.is_readable())
        {
            return None;
        }
        let source = match leaf.origin? {
            ReferenceSource::Captured(observed) => observed.source,
            ReferenceSource::Symbolic(origin) if origin.source.is_static() => SlotSource {
                carrier: origin.source.clone(),
                carrier_backing: None,
                origin,
                lifetime_access: None,
            },
            ReferenceSource::Symbolic(_) => return None,
        };
        // Recreate only declaration geometry for each alternative. This does
        // not choose an origin or recapture an already completed descriptor.
        let mut cursor = StorageProjectionCursor::new(program, reference);
        for segment in &path[..first] {
            cursor.advance(segment)?;
        }
        cursor.enter_referee()?;
        reads.extend(read_projection::complete_contents(
            program,
            source,
            cursor,
            &path[first..],
            &after.stored,
        )?);
    }
    if !selected {
        return None;
    }
    read_backing::from_reads(reads, Vec::new())
}
