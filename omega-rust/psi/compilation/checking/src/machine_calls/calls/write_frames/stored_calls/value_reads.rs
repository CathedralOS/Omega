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
        let (first, sources) = self.sources_at_boundary(program, path)?;
        let mut reads = Vec::new();
        for source in sources {
            let cursor = self.referee_cursor(program, &path[..first])?;
            reads.extend(read_projection::complete_contents(
                program,
                source,
                cursor,
                &path[first..],
                &self.after.stored,
            )?);
        }
        read_backing::from_reads(reads, Vec::new())
    }

    /// Existing scalar observations need a complete invalidation footprint,
    /// not a new exact read identity. An unknown element in the final owned
    /// span may widen that footprint; it cannot supply a later pointer load.
    pub(in crate::machine_calls::calls::write_frames) fn invalidation_paths(
        &self,
        program: &dyn crate::CheckingInput,
        path: &[PlaceSegment],
    ) -> Option<Vec<String>> {
        let (first, sources) = self.sources_at_boundary(program, path)?;
        let mut paths = Vec::new();
        for source in sources {
            let cursor = self.referee_cursor(program, &path[..first])?;
            for dependency in read_projection::contents_invalidation_paths(
                program,
                source,
                cursor,
                &path[first..],
                &self.after.stored,
            )? {
                if !paths.contains(&dependency) {
                    paths.push(dependency);
                }
            }
        }
        Some(paths)
    }

    fn sources_at_boundary(
        &self,
        program: &dyn crate::CheckingInput,
        path: &[PlaceSegment],
    ) -> Option<(usize, Vec<SlotSource>)> {
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
        let mut sources = Vec::new();
        for leaf in &self.returned.references {
            if leaf.local_segments != path[..first] {
                continue;
            }
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
            sources.push(source);
        }
        (!sources.is_empty()).then_some((first, sources))
    }

    fn referee_cursor<'program>(
        &self,
        program: &'program dyn crate::CheckingInput,
        prefix: &[PlaceSegment],
    ) -> Option<StorageProjectionCursor<'program>> {
        // Recreate declaration geometry, never the completed descriptor value.
        let mut cursor = StorageProjectionCursor::new(program, self.reference);
        for segment in prefix {
            cursor.advance(segment)?;
        }
        cursor.enter_referee()?;
        Some(cursor)
    }
}
