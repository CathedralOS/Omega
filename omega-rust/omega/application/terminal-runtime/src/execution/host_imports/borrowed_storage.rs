//! Join semantic loans to native placement without inventing backing bytes.
//! Output observes only covered prefixes. Input uses private staging memory;
//! its zeroed holes are not semantic values: completion decodes a cell only
//! when every byte was supplied by the read or retained from valid entry
//! contents. This permits complete case replacement, including initially
//! inactive payloads, without publishing unread padding or manufactured data.

pub(super) mod geometry;

pub(super) use super::scalar_codec::scalar_extent;
use super::scalar_codec::{decode, encode};

use geometry::{NativeCell, NativeCellKind};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::ScalarType;

use terminal_interpreter::{
    TerminalBoundaryStorageBuffer, TerminalBoundaryStorageValue, TerminalEffectRejection,
    TerminalStorageContents,
};
use terminal_psi::StructuralAccess;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StorageTransfer {
    Observe,
    ReplacePrefix,
}

pub(super) struct CapturedStorage<'layout> {
    pub bytes: Vec<u8>,
    covered: Vec<(usize, usize)>,
    cells: Vec<NativeCell<'layout>>,
    layout: Option<&'layout NativeBorrowedStorageLayout>,
    captured_paths: Vec<Vec<semantic_vocabulary::CanonicalStructuralPathSegment>>,
    primitive_byte_root: bool,
}

/// Exact native root-word output geometry. Scalar descriptors additionally
/// retain a disjoint codec lane; address-only semantic completion authenticates
/// it against the old and new captured endpoints. This plan names only semantic paths
/// and overlap guards; the selected host pointer is never decoded as a scalar
/// or exposed as a guest address. Trailing disjoint storage remains untouched.
pub(super) struct PointerOutputPlan {
    selected: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    overlaps: Vec<Vec<semantic_vocabulary::CanonicalStructuralPathSegment>>,
}

impl PointerOutputPlan {
    pub fn prepare(
        program: &terminal_interpreter::TerminalProgram,
        layout: &NativeBorrowedStorageLayout,
        declared_path: &[terminal_psi::StructuralPathSegment],
        pointer_bytes: usize,
    ) -> Result<Self, TerminalEffectRejection> {
        if !pointer_bytes.is_power_of_two()
            || !layout.alignment.is_power_of_two()
            || usize::from(layout.alignment) < pointer_bytes
            || usize::try_from(layout.byte_size).map_err(|_| invalid_operands())? < pointer_bytes
        {
            return Err(TerminalEffectRejection::new(
                "native pointer output root has incompatible extent or alignment",
            ));
        }
        let tip = terminal_semantics::runtime_structural_path_tip(
            program.module().structural_types.iter(),
            layout.structural_type,
            declared_path,
        )
        .ok_or_else(invalid_operands)?;
        let terminal_psi::StructuralTypeShape::Reference { referent, .. } = tip.shape else {
            return Err(invalid_operands());
        };
        let descriptor_bytes = match program.structural_type(referent).map(|row| &row.shape) {
            Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar)) => {
                runtime_abi::ScalarStorageCodec::from_scalar(*scalar)
                    .ok_or_else(invalid_operands)?;
                let lanes = runtime_abi::ScalarReferenceLayout::NATIVE;
                if usize::try_from(lanes.address_byte_size).ok() != Some(pointer_bytes)
                    || lanes.address_byte_offset != 0
                    || lanes.codec_byte_offset != lanes.address_byte_size
                    || lanes.codec_byte_offset.checked_add(lanes.codec_byte_size)
                        != Some(lanes.byte_size)
                    || layout.alignment < lanes.alignment
                {
                    return Err(invalid_operands());
                }
                usize::try_from(lanes.byte_size).map_err(|_| invalid_operands())?
            }
            Some(_) => pointer_bytes,
            None => return Err(invalid_operands()),
        };
        let mut selected = None;
        let mut overlaps = Vec::new();
        for cell in geometry::expand(layout)? {
            if cell.byte_offset < pointer_bytes && cell.byte_size()? != 0 {
                overlaps.push(cell.path);
            }
        }
        for region in layout.opaque_regions() {
            geometry::expand_coordinates(
                layout.opaque_region_path(region).collect(),
                usize::try_from(region.byte_offset).map_err(|_| invalid_operands())?,
                layout.opaque_region_repetitions(region),
                &mut |path, byte_offset| {
                    let storage = terminal_semantics::canonical_structural_storage_path(
                        program.module().structural_types.iter(),
                        layout.structural_type,
                        &path,
                    )
                    .ok_or_else(invalid_operands)?;
                    if storage == declared_path {
                        if byte_offset != 0
                            || usize::try_from(region.byte_size).map_err(|_| invalid_operands())?
                                != descriptor_bytes
                            || selected.replace(path).is_some()
                        {
                            return Err(TerminalEffectRejection::new(
                                "declared reference output has incompatible native address/codec lanes",
                            ));
                        }
                    } else if byte_offset < pointer_bytes && region.byte_size != 0 {
                        overlaps.push(path);
                    }
                    Ok(())
                },
            )?;
        }
        let selected = selected.ok_or_else(|| {
            TerminalEffectRejection::new(
                "declared reference output has no exact native pointer region",
            )
        })?;
        Ok(Self { selected, overlaps })
    }

    /// Validate the actual active overlay before C writes its root word. Any
    /// other active scalar, tag or opaque region touched by that write refuses.
    pub fn validate(
        &self,
        buffer: &terminal_interpreter::TerminalBoundaryStructuralBuffer,
    ) -> Result<(), TerminalEffectRejection> {
        if !geometry::is_active(&self.selected, buffer.values())? {
            return Err(TerminalEffectRejection::new(
                "native pointer output selects an inactive reference field",
            ));
        }
        for path in &self.overlaps {
            if geometry::is_active(path, buffer.values())? {
                return Err(TerminalEffectRejection::new(
                    "native root pointer write overlaps other active output storage",
                ));
            }
        }
        Ok(())
    }
}

fn invalid_operands() -> TerminalEffectRejection {
    TerminalEffectRejection::new(
        "borrowed native storage differs from its captured semantic layout",
    )
}

impl<'layout> CapturedStorage<'layout> {
    pub fn capture(
        buffers: &[TerminalBoundaryStorageBuffer],
        access: StructuralAccess,
        layout: Option<&'layout NativeBorrowedStorageLayout>,
        byte_count: usize,
        transfer: StorageTransfer,
    ) -> Result<Self, TerminalEffectRejection> {
        let [buffer] = buffers else {
            return Err(invalid_operands());
        };
        let mut cells = match layout {
            Some(layout) => geometry::expand(layout)?,
            None => Vec::new(),
        };
        // Terminal canonical order sorts each declaration's fields by identity.
        // Lexical paths therefore place tags before descendants and mixed common
        // fields before payloads, matching semantic capture without dense IDs.
        cells.sort_by(|left, right| left.path.cmp(&right.path));
        if cells.windows(2).any(|pair| pair[0].path == pair[1].path) {
            return Err(invalid_operands());
        }
        let (argument_index, actual_access, values) = match buffer {
            TerminalBoundaryStorageBuffer::Scalar(buffer) if layout.is_none() => (
                buffer.argument_index(),
                buffer.access(),
                vec![TerminalBoundaryStorageValue {
                    path: buffer.field_path().to_vec(),
                    contents: TerminalStorageContents::Scalar {
                        value: buffer.value(),
                        stored_scalar: buffer.storage_scalar_type(),
                    },
                }],
            ),
            TerminalBoundaryStorageBuffer::Structural(buffer)
                if layout
                    .is_some_and(|layout| layout.structural_type == buffer.structural_type()) =>
            {
                (
                    buffer.argument_index(),
                    buffer.access(),
                    buffer.values().to_vec(),
                )
            }
            _ => return Err(invalid_operands()),
        };
        if argument_index != 0
            || actual_access != access
            || (transfer == StorageTransfer::ReplacePrefix
                && access != StructuralAccess::MutableBorrow)
        {
            return Err(invalid_operands());
        }
        let primitive_byte_root = layout.is_none()
            && values.len() == 1
            && values[0].path.is_empty()
            && matches!(values[0].contents, TerminalStorageContents::Scalar { value, stored_scalar: ScalarType::Integer(word) }
                if value.scalar_type() == ScalarType::Integer(word)
                    && word.bits() == 8 && !word.is_address());
        let mut encoded = Vec::with_capacity(values.len());
        for value in &values {
            let (offset, bytes) = if let Some(layout) = layout {
                let position = cells
                    .binary_search_by(|cell| cell.path.cmp(&value.path))
                    .map_err(|_| invalid_operands())?;
                let cell = &cells[position];
                (cell.byte_offset, cell.encode(layout, value.contents)?)
            } else {
                if values.len() != 1 || !value.path.is_empty() {
                    return Err(invalid_operands());
                }
                let TerminalStorageContents::Scalar {
                    value,
                    stored_scalar,
                } = value.contents
                else {
                    return Err(invalid_operands());
                };
                (
                    0,
                    encode(value, stored_scalar)?[..scalar_extent(stored_scalar)?].to_vec(),
                )
            };
            encoded.push((offset, bytes));
        }
        encoded.sort_by_key(|(offset, _)| *offset);
        let mut covered = Vec::with_capacity(encoded.len());
        let mut previous_end = 0;
        for (offset, bytes) in &encoded {
            let end = offset
                .checked_add(bytes.len())
                .ok_or_else(invalid_operands)?;
            if *offset < previous_end
                || layout.is_some_and(|layout| end > layout.byte_size as usize)
            {
                return Err(invalid_operands());
            }
            covered.push((*offset, end));
            previous_end = end;
        }
        let native_extent = layout.map_or(previous_end, |layout| layout.byte_size as usize);
        if byte_count > native_extent {
            return Err(TerminalEffectRejection::new(
                "descriptor byte count exceeds the exact borrowed storage extent",
            ));
        }
        if transfer == StorageTransfer::Observe && !covered_range(&covered, 0, byte_count) {
            return Err(TerminalEffectRejection::new(
                "borrowed storage has padding, overlap or unmodeled bytes",
            ));
        }
        if transfer == StorageTransfer::ReplacePrefix {
            let mut possible = match layout {
                Some(layout) => geometry::input_coverage(layout, &values, &cells, byte_count)?,
                None => covered.clone(),
            };
            possible.sort_unstable();
            if !covered_range(&possible, 0, byte_count) {
                return Err(TerminalEffectRejection::new(
                    "input accesses padding or bytes without a scalar or discriminator recipe",
                ));
            }
        }
        let mut bytes = vec![0; previous_end.max(byte_count)];
        for (offset, encoded) in encoded {
            bytes[offset..offset + encoded.len()].copy_from_slice(&encoded);
        }
        Ok(Self {
            bytes,
            covered,
            cells,
            layout,
            primitive_byte_root,
            captured_paths: {
                let mut paths = values
                    .iter()
                    .map(|value| value.path.clone())
                    .collect::<Vec<_>>();
                paths.sort();
                paths
            },
        })
    }

    /// A native C-string reader may see only the fully captured prefix through
    /// its first NUL. Zero-filled staging holes are not initialized terminators.
    pub fn terminated_prefix(&self) -> Result<&[u8], TerminalEffectRejection> {
        let end = self
            .bytes
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| {
                TerminalEffectRejection::new("native text input has no captured NUL terminator")
            })?
            .checked_add(1)
            .ok_or_else(invalid_operands)?;
        if !covered_range(&self.covered, 0, end) {
            return Err(TerminalEffectRejection::new(
                "native text input crosses unmodeled or uninitialized bytes",
            ));
        }
        Ok(&self.bytes[..end])
    }

    /// Translate a returned host-staging offset back to exactly one captured
    /// byte cell. This is semantic geometry, not a guest pointer conversion;
    /// padding, wide scalar interiors and inactive overlays have no endpoint.
    pub fn byte_path_at(
        &self,
        offset: usize,
    ) -> Result<Vec<semantic_vocabulary::CanonicalStructuralPathSegment>, TerminalEffectRejection>
    {
        if self.primitive_byte_root && offset == 0 {
            return Ok(Vec::new());
        }
        let mut matches = self.cells.iter().filter(|cell| {
            cell.byte_offset == offset
                && self.captured_paths.binary_search(&cell.path).is_ok()
                && matches!(cell.kind, NativeCellKind::Scalar(field)
                    if matches!(field.logical_scalar, ScalarType::Integer(word) if word.bits() == 8 && !word.is_address())
                    && field.stored_scalar == field.logical_scalar)
        });
        let path = matches.next().ok_or_else(invalid_operands)?.path.clone();
        if matches.next().is_some() {
            return Err(invalid_operands());
        }
        Ok(path)
    }

    /// The returned count grants initialization only to the supplied prefix.
    /// A short read retains valid old bytes; unread inactive bytes stay absent.
    pub fn stage(
        &self,
        buffers: &mut [TerminalBoundaryStorageBuffer],
        written: usize,
    ) -> Result<(), TerminalEffectRejection> {
        if written > self.bytes.len() {
            return Err(invalid_operands());
        }
        let [buffer] = buffers else {
            return Err(invalid_operands());
        };
        let mut covered = self.covered.clone();
        covered.push((0, written));
        covered.sort_unstable();
        if let TerminalBoundaryStorageBuffer::Scalar(buffer) = buffer {
            let size = scalar_extent(buffer.storage_scalar_type())?;
            if !covered_range(&covered, 0, size) {
                return Err(invalid_operands());
            }
            return buffer.replace(decode(
                &self.bytes[..size],
                buffer.storage_scalar_type(),
                buffer.value().scalar_type(),
            )?);
        }
        let TerminalBoundaryStorageBuffer::Structural(buffer) = buffer else {
            return Err(invalid_operands());
        };
        let layout = self.layout.ok_or_else(invalid_operands)?;
        let mut values: Vec<TerminalBoundaryStorageValue> = Vec::new();
        for cell in &self.cells {
            if !geometry::is_active(&cell.path, &values)? {
                continue;
            }
            let size = cell.byte_size()?;
            let end = cell
                .byte_offset
                .checked_add(size)
                .ok_or_else(invalid_operands)?;
            if !covered_range(&covered, cell.byte_offset, end) {
                // Non-scalar authority (e.g. shared atomic residents) is not a
                // field supplied by a native layout. Mutable snapshots capture
                // all ordinary/atomic leaves; a newly selected case must have
                // complete bytes rather than inheriting an inactive zero fill.
                return Err(TerminalEffectRejection::new(
                    "new active case has bytes neither supplied nor validly retained",
                ));
            }
            let contents = match cell.kind {
                NativeCellKind::Scalar(field) => TerminalStorageContents::Scalar {
                    value: decode(
                        self.bytes
                            .get(cell.byte_offset..end)
                            .ok_or_else(invalid_operands)?,
                        field.stored_scalar,
                        field.logical_scalar,
                    )?,
                    stored_scalar: field.stored_scalar,
                },
                NativeCellKind::Case(discriminator) => {
                    let encoded: [u8; 4] = self
                        .bytes
                        .get(cell.byte_offset..end)
                        .ok_or_else(invalid_operands)?
                        .try_into()
                        .map_err(|_| invalid_operands())?;
                    let tag = i32::from_le_bytes(encoded);
                    let case = layout
                        .discriminator_cases(discriminator)
                        .find(|mapping| mapping.native_tag == tag)
                        .ok_or_else(|| {
                            TerminalEffectRejection::new(
                                "native discriminator does not name a declared case",
                            )
                        })?;
                    TerminalStorageContents::Case(case.case)
                }
            };
            values.push(TerminalBoundaryStorageValue {
                path: cell.path.clone(),
                contents,
            });
        }
        buffer.replace(values)
    }
}

fn covered_range(covered: &[(usize, usize)], start: usize, end: usize) -> bool {
    let mut frontier = start;
    for &(begin, limit) in covered {
        if begin > frontier {
            break;
        }
        frontier = frontier.max(limit);
        if frontier >= end {
            return true;
        }
    }
    frontier >= end
}
