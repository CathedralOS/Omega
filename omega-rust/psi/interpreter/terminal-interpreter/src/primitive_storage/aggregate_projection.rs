//! Scalar observations compose the original cells intersecting one byte window.
//! Geometry selects fragments; already-authenticated reference traversal supplies
//! their storage. Encoding uses the ordinary Terminal storage codec. Writes stage
//! every original endpoint before committing and preserve untouched cell bytes.
use super::*;

fn mask(bytes: u8) -> Result<u128, TerminalInterpretError> {
    if bytes == 0 || bytes > 16 {
        return Err(malformed());
    }
    Ok(u128::MAX >> (128 - usize::from(bytes) * 8))
}

fn projection_bits(
    fragments: &[PrimitiveStorageFragment],
    mut read: impl FnMut(
        &PrimitiveStorageFragment,
    ) -> Result<TerminalScalarValue, TerminalInterpretError>,
) -> Result<u128, TerminalInterpretError> {
    let mut bits = 0_u128;
    for fragment in fragments {
        let source = encoding::encode(fragment.stored, read(fragment)?)?;
        let part =
            (source >> (usize::from(fragment.source_offset) * 8)) & mask(fragment.byte_length)?;
        bits |= part << (usize::from(fragment.destination_offset) * 8);
    }
    Ok(bits)
}

impl TerminalExecution<'_> {
    pub(super) fn aggregate_projection_storage(
        &self,
        referent: &TerminalStructuralValue,
        observed: terminal_semantics::ReferenceStorageProjection,
        byte_length: u64,
    ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
        let width = terminal_semantics::reference_storage_byte_width(observed.stored_scalar)
            .ok_or_else(malformed)?;
        if u64::from(width) != byte_length {
            return Err(malformed());
        }
        let end = observed
            .byte_offset
            .checked_add(byte_length)
            .ok_or_else(malformed)?;
        let types = self.program.structural_types();
        let geometry = crate::reference::scalar_storage_fragments(
            &types,
            referent.structural_type,
            observed.byte_offset,
            byte_length,
        )
        .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
            "aggregate scalar window contains padding or noncontiguous storage",
        ))?;
        let mut fragments = Vec::with_capacity(geometry.len());
        let mut shared = false;
        for fragment in geometry {
            let canonical = crate::reference::backing_scalar_path(
                &types,
                referent.structural_type,
                &fragment.path,
            )
            .ok_or_else(malformed)?;
            let (storage, _, shared_view) = self.primitive_view_access(
                referent,
                &canonical,
                fragment.scalar.logical_scalar,
                false,
            )?;
            shared |= shared_view;
            let start = fragment.scalar.byte_offset.max(observed.byte_offset);
            let fragment_end = fragment
                .scalar
                .byte_offset
                .checked_add(fragment.byte_length)
                .ok_or_else(malformed)?
                .min(end);
            fragments.push(PrimitiveStorageFragment {
                storage,
                logical: fragment.scalar.logical_scalar,
                stored: fragment.scalar.stored_scalar,
                source_offset: u8::try_from(start - fragment.scalar.byte_offset)
                    .map_err(|_| malformed())?,
                destination_offset: u8::try_from(start - observed.byte_offset)
                    .map_err(|_| malformed())?,
                byte_length: u8::try_from(fragment_end - start).map_err(|_| malformed())?,
            });
        }
        Ok((
            PrimitiveStorage::canonical(PrimitiveStorageEndpoint::Projection {
                fragments,
                logical: observed.logical_scalar,
                stored: observed.stored_scalar,
            }),
            observed.logical_scalar,
            shared,
        ))
    }

    pub(super) fn read_storage_projection(
        &self,
        source: PlaceId,
        fragments: &[PrimitiveStorageFragment],
        stored: ScalarType,
        expected: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let bits = projection_bits(fragments, |fragment| {
            self.read_primitive_storage(
                source,
                fragment.storage.clone(),
                fragment.logical,
                fragment.logical,
                false,
            )
        })?;
        encoding::convert_scalar(encoding::decode(stored, bits)?, expected)
    }
}

impl PrimitiveStorageWrites {
    fn pending_storage_value(
        &self,
        execution: &TerminalExecution,
        destination: PlaceId,
        storage: &PrimitiveStorage,
        logical: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        if let PrimitiveStorageEndpoint::Projection {
            fragments, stored, ..
        } = &storage.endpoint
        {
            let bits = projection_bits(fragments, |fragment| {
                self.pending_storage_value(
                    execution,
                    destination,
                    &fragment.storage,
                    fragment.logical,
                )
            })?;
            return encoding::convert_scalar(encoding::decode(*stored, bits)?, logical);
        }
        if let Some(write) = self
            .remaining_writes
            .iter()
            .rev()
            .chain(self.first_write.iter())
            .find(|write| write.endpoint == storage.endpoint)
        {
            return if let Some(codec) = storage.encoding {
                let bits = encoding::encode(codec.source, write.value)?;
                encoding::convert_scalar(encoding::decode(codec.view, bits)?, logical)
            } else {
                encoding::convert_scalar(write.value, logical)
            };
        }
        execution.read_primitive_storage(destination, storage.clone(), logical, logical, false)
    }

    pub(super) fn stage_projection(
        &mut self,
        execution: &TerminalExecution,
        destination: PlaceId,
        fragments: &[PrimitiveStorageFragment],
        stored: ScalarType,
        value: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        let bits = encoding::encode(stored, value)?;
        for fragment in fragments {
            let width = terminal_semantics::reference_storage_byte_width(fragment.stored)
                .ok_or_else(malformed)?;
            let source_shift = usize::from(fragment.source_offset) * 8;
            let selected_mask = mask(fragment.byte_length)? << source_shift;
            let old_bits = if fragment.source_offset == 0 && fragment.byte_length == width {
                0
            } else {
                encoding::encode(
                    fragment.stored,
                    self.pending_storage_value(
                        execution,
                        destination,
                        &fragment.storage,
                        fragment.logical,
                    )?,
                )?
            };
            let replacement = (old_bits & !selected_mask)
                | (((bits >> (usize::from(fragment.destination_offset) * 8))
                    & mask(fragment.byte_length)?)
                    << source_shift);
            let replacement = encoding::convert_scalar(
                encoding::decode(fragment.stored, replacement)?,
                fragment.logical,
            )?;
            self.stage(
                execution,
                destination,
                fragment.storage.clone(),
                fragment.logical,
                replacement,
            )?;
        }
        Ok(())
    }
}
