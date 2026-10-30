//! Completed values retain descriptor suppliers, then observe current loads.
//! A copied reference freezes that descriptor, never the slots subsequently
//! loaded through it. Calls, moves and construction consume one ordered value
//! capture; geometry alone supplies no backing, access or load authority.

use super::super as frames;
use super::*;
use frames::path_instantiation::aggregate_arguments::ReferenceSource;
use frames::reference_subjects::{read_backing, read_projection};
use frames::stored_origins::StorageProjectionCursor;
use typed_trees::types::TypeReferenceHandle;

pub(in crate::machine_calls::calls::write_frames) struct CompletedValueReads {
    returned: frames::path_instantiation::aggregate_arguments::AggregateOrigins,
    after: CollectedStatementPrefix,
    reference: TypeReferenceHandle,
}

pub(in crate::machine_calls::calls::write_frames) fn completed_value_reads(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    prefix: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<CompletedValueReads> {
    let (captured, after) = expression_effects::completed_value(
        program,
        machine,
        state,
        statement,
        prefix,
        symbols,
        &mut FrameInference::for_state(state.symbol),
        expression,
    )?;
    let returned =
        captured_inputs::completed_result(program, machine, &captured, reference, symbols)?;
    Some(CompletedValueReads {
        returned,
        after,
        reference,
    })
}

impl CompletedValueReads {
    pub(in crate::machine_calls::calls::write_frames) fn read_backing(
        &self,
        program: &dyn crate::CheckingInput,
        path: &[PlaceSegment],
    ) -> Option<frames::demand::CapturedStorageReadBacking> {
        let reference = self.reference;
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
        let mut reads = Vec::new();
        let mut selected = false;
        for leaf in &self.returned.references {
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
            let source = match leaf.origin.as_ref()? {
                ReferenceSource::Captured(observed) => observed.source.clone(),
                ReferenceSource::Symbolic(origin) if origin.source.is_static() => SlotSource {
                    carrier: origin.source.clone(),
                    carrier_backing: None,
                    origin: origin.clone(),
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
                &self.after.stored,
            )?);
        }
        if !selected {
            return None;
        }
        read_backing::from_reads(reads, Vec::new())
    }
}
