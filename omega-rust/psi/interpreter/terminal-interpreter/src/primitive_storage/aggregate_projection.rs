//! Scalar observations compose the original cells intersecting one byte window.
//! Geometry selects fragments; already-authenticated reference traversal supplies
//! their storage. Encoding uses the ordinary Terminal storage codec. Writes stage
//! every original endpoint before committing and preserve untouched cell bytes.
use super::*;

fn mask(bits: u8) -> Result<u128, TerminalInterpretError> {
    if bits == 0 || bits > 128 {
        return Err(malformed());
    }
    Ok(u128::MAX >> (128 - usize::from(bits)))
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
        let part = (source >> usize::from(fragment.source_bit_offset)) & mask(fragment.bit_length)?;
        bits |= part << usize::from(fragment.destination_bit_offset);
    }
    Ok(bits)
}

impl TerminalExecution<'_> {
    pub(super) fn aggregate_projection_storage(
        &self,
        referent: &TerminalStructuralValue,
        observed: terminal_semantics::ReferenceStorageProjection,
        byte_length: u64,
        home: Option<&crate::element_views::ElementViewHome>,
        writing: bool,
    ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
        let width = terminal_semantics::reference_storage_byte_width(observed.stored_scalar)
            .ok_or_else(malformed)?;
        if u64::from(width) != byte_length {
            return Err(malformed());
        }
        let first = observed.byte_offset.checked_mul(8).ok_or_else(malformed)?;
        let end = observed
            .byte_offset
            .checked_add(byte_length)
            .and_then(|end| end.checked_mul(8))
            .ok_or_else(malformed)?;
        let types = self.structural_types();
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
            let (storage, actual, shared_view) = self.original_view_primitive_access(
                referent,
                home,
                &canonical,
                fragment.scalar.logical_scalar,
                writing,
            )?;
            if actual != fragment.scalar.logical_scalar {
                return Err(malformed());
            }
            shared |= shared_view;
            let start = fragment.bit_offset.max(first);
            let fragment_end = fragment
                .bit_offset
                .checked_add(fragment.bit_length)
                .ok_or_else(malformed)?
                .min(end);
            let source_bit_offset = fragment
                .source_bit_offset
                .checked_add(start - fragment.bit_offset)
                .ok_or_else(malformed)?;
            let selected_length = fragment_end - start;
            let stored_bits = u64::from(
                terminal_semantics::reference_storage_byte_width(fragment.scalar.stored_scalar)
                    .ok_or_else(malformed)?,
            )
            .checked_mul(8)
            .ok_or_else(malformed)?;
            if selected_length == 0
                || source_bit_offset
                    .checked_add(selected_length)
                    .is_none_or(|end| end > stored_bits)
            {
                return Err(malformed());
            }
            fragments.push(PrimitiveStorageFragment {
                storage,
                logical: fragment.scalar.logical_scalar,
                stored: fragment.scalar.stored_scalar,
                source_bit_offset: u8::try_from(source_bit_offset).map_err(|_| malformed())?,
                destination_bit_offset: u8::try_from(start - first).map_err(|_| malformed())?,
                bit_length: u8::try_from(selected_length).map_err(|_| malformed())?,
                addressable_cell: fragment.addressable_cell,
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
            let source_shift = usize::from(fragment.source_bit_offset);
            let selected_mask = mask(fragment.bit_length)? << source_shift;
            let old_bits = if fragment.source_bit_offset == 0
                && u16::from(fragment.bit_length) == u16::from(width) * 8
            {
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
                | (((bits >> usize::from(fragment.destination_bit_offset))
                    & mask(fragment.bit_length)?)
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
