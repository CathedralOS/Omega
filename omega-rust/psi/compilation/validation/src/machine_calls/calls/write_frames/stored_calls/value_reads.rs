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
    let completed = expression_effects::completed_value(
        program,
        machine,
        state,
        statement,
        prefix,
        symbols,
        &mut FrameInference::for_state(state.symbol),
        expression,
    );
    let Some((captured, after)) = completed else {
        if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT completed-value-read-refused machine={:?} expression={expression:?} reference={reference:?} phase=ordered-value",
                machine.symbol
            );
        }
        return None;
    };
    let returned =
        captured_inputs::completed_result(program, machine, &captured, reference, symbols);
    let Some(returned) = returned else {
        if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT completed-value-read-refused machine={:?} expression={expression:?} reference={reference:?} phase=suppliers",
                machine.symbol
            );
        }
        return None;
    };
    Some(CompletedValueReads {
        returned,
        after,
        reference,
    })
}

impl CompletedValueReads {
    /// Complete a scalar read without claiming to know its contents. The
    /// descriptor is evaluated at this cut, every supplier remains readable,
    /// and the complete scalar span has an established interpretation. Exact
    /// content equations still require `read_backing` and scalar translation.
    pub(in crate::machine_calls::calls::write_frames) fn scalar_read_completed(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
    ) -> Result<(), &'static str> {
        let shell = crate::reference_shell(program, self.reference)
            .filter(|shell| shell.access.is_readable())
            .ok_or("completed scalar read has no readable reference")?;
        crate::value_custody::recasts::unrestricted_scalar_read_projection(program, shell.referee)
            .ok_or("completed scalar read has no established unrestricted scalar interpretation")?;
        let (boundary, sources) = self
            .sources_at_boundary(program, &[])
            .ok_or("completed scalar read has an incomplete descriptor supplier frontier")?;
        if boundary != 0 {
            return Err("completed scalar read crosses an uncaptured descriptor");
        }
        for source in sources {
            let origin = &source.origin.source;
            if origin.recasts.is_empty() {
                read_projection::complete_contents(
                    program,
                    source,
                    self.referee_cursor(program, &[])
                        .ok_or("completed scalar read has no referee geometry")?,
                    &[],
                    &self.after.stored,
                )
                .ok_or("completed scalar read has no resolved ordinary supplier")?;
                continue;
            }
            if !origin.root.is_valid()
                || !origin.builtin_coordinates
                || origin.output_identity().is_some()
                || origin.result_home().is_some()
                || source.lifetime_access.is_some()
            {
                return Err("completed scalar view has no admitted declaration supplier");
            }
            let (mut carrier, path) = origin
                .representation
                .as_ref()
                .map(|(carrier, path)| (*carrier, path))
                .ok_or("completed scalar view has no retained interpretation")?;
            let leaf = crate::project_type_reference_from_segments(program, carrier, path)
                .ok_or("completed scalar view has no declared leaf")?;
            if !crate::declared_carriers_match(program, leaf, shell.referee) {
                return Err("completed scalar view does not match its read carrier");
            }
            for recast in origin.recasts.iter().rev() {
                crate::value_custody::recasts::established_reference_formation(
                    program,
                    recast.formation.expression(),
                )?;
                let (predecessor, path) = frames::place_paths::retained_view_predecessor(
                    program, state, recast, carrier,
                )?;
                crate::project_type_reference_from_segments(program, predecessor, &path)
                    .ok_or("completed scalar view has no declared predecessor projection")?;
                carrier = predecessor;
            }
            // The descriptor owns its captured selector and complete view span.
            // Keep its physical dependencies without equating a multi-byte read
            // with one source element or reevaluating the original selector.
            read_projection::contents_invalidation_paths(
                program,
                source,
                self.referee_cursor(program, &[])
                    .ok_or("completed scalar read has no referee geometry")?,
                &[],
                &self.after.stored,
            )
            .ok_or("completed scalar view has no complete read footprint")?;
        }
        Ok(())
    }

    /// Publish every supplier of the first captured descriptor. A coarse write
    /// footprint does not discard its structured supplier coordinates. Consumers
    /// requiring one exact referent must still validate those coordinates and
    /// agree across the complete frontier. This inventory cannot supply a later
    /// pointer load through the descriptor's referent.
    pub(in crate::machine_calls::calls::write_frames) fn reference_sources(
        &self,
        program: &dyn crate::CheckingInput,
    ) -> Option<Vec<frames::LocalWriteSource>> {
        let addressed = self.sources_at_boundary(program, &[]);
        if addressed.is_none() && std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT completed-address-refused reference={:?} leaves={:?}",
                program.type_reference_table.type_reference(self.reference),
                self.returned
                    .references
                    .iter()
                    .map(|leaf| (&leaf.local_segments, &leaf.origin))
                    .collect::<Vec<_>>()
            );
        }
        let (boundary, sources) = addressed?;
        if boundary != 0 {
            return None;
        }
        sources
            .into_iter()
            .map(|source| {
                (source.lifetime_access.is_none()
                    && source.origin.source.output_identity().is_none())
                .then(|| source.origin.into())
            })
            .collect()
    }

    /// Possible interpretation issuers of the completed descriptor. Unlike a
    /// storage read, phase tracking must retain coarse coordinates and lifetime
    /// suppliers too: either may carry an unestablished recast. This grants no
    /// primitive address, value equality, read permission or reference validity.
    pub(in crate::machine_calls::calls::write_frames) fn interpretation_sources(
        &self,
        program: &dyn crate::CheckingInput,
    ) -> Result<Vec<frames::LocalWriteSource>, &'static str> {
        crate::reference_shell(program, self.reference)
            .ok_or("interpretation query has no reference descriptor type")?;
        let mut sources = Vec::new();
        for leaf in &self.returned.references {
            if !leaf.local_segments.is_empty() {
                continue;
            }
            if !matches!(leaf.kind, stored_origins::StoredOriginKind::Reference(_)) {
                return Err("completed descriptor has a non-reference interpretation leaf");
            }
            // Selection and lifetime debts cannot authorize a storage read.
            // Interpretation tracking instead retains every possible issuer;
            // the ordinary loan and access owners still check those debts.
            let origin = match leaf
                .origin
                .as_ref()
                .ok_or("completed reference has an unavailable interpretation issuer")?
            {
                ReferenceSource::Captured(observed) => observed.source.origin.clone(),
                ReferenceSource::Symbolic(origin) => origin.clone(),
            };
            let source: frames::LocalWriteSource = origin.into();
            if !source.root.is_valid()
                && source.invocation_output.is_none()
                && !source.is_image_storage
                && source.recasts.is_empty()
            {
                return Err("completed reference retains an unknown interpretation supplier");
            }
            sources.push(source);
        }
        if sources.is_empty() {
            return Err("completed reference has no interpretation leaves");
        }
        Ok(sources)
    }

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
        read_backing::from_reads(program, reads, Vec::new())
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
                || leaf
                    .lifetime_dependencies
                    .as_ref()
                    .is_none_or(|dependencies| !dependencies.is_empty())
                || !matches!(leaf.kind, stored_origins::StoredOriginKind::Reference(access) if access.is_readable())
            {
                if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                    eprintln!(
                        "STRUCT completed-value-read-refused path={path:?} phase=leaf may_select={} lifetime_debts={:?} source={:?}",
                        leaf.may_select,
                        leaf.lifetime_dependencies.as_ref().map(Vec::len),
                        leaf.origin
                    );
                }
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
                ReferenceSource::Symbolic(origin) => {
                    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                        eprintln!(
                            "STRUCT completed-value-read-refused path={path:?} phase=symbolic origin={origin:?}"
                        );
                    }
                    return None;
                }
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
