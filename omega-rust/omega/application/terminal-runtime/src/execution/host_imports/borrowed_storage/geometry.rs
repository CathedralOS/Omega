//! Expand fixed-array recipes and select active overlays with nominal paths.
//! The semantic snapshot authenticates the contents; these rows only place
//! them using the native compiler's already-retained layout.

use super::{encode, invalid_operands, scalar_extent};
use runtime_abi::{
    NativeBorrowedStorageDiscriminator, NativeBorrowedStorageField, NativeBorrowedStorageLayout,
    NativeStorageRepetition,
};
use semantic_vocabulary::CanonicalStructuralPathSegment as Path;
use terminal_interpreter::{
    TerminalBoundaryStorageValue, TerminalEffectRejection, TerminalStorageContents,
};

#[derive(Clone, Copy)]
pub(super) enum NativeCellKind<'layout> {
    Scalar(&'layout NativeBorrowedStorageField),
    Case(&'layout NativeBorrowedStorageDiscriminator),
}

pub(super) struct NativeCell<'layout> {
    pub path: Vec<Path>,
    pub byte_offset: usize,
    pub kind: NativeCellKind<'layout>,
}

impl NativeCell<'_> {
    pub fn byte_size(&self) -> Result<usize, TerminalEffectRejection> {
        match self.kind {
            NativeCellKind::Scalar(field) => scalar_extent(field.stored_scalar),
            NativeCellKind::Case(_) => Ok(4),
        }
    }

    pub fn encode(
        &self,
        layout: &NativeBorrowedStorageLayout,
        contents: TerminalStorageContents,
    ) -> Result<Vec<u8>, TerminalEffectRejection> {
        match (self.kind, contents) {
            (
                NativeCellKind::Scalar(field),
                TerminalStorageContents::Scalar {
                    value,
                    stored_scalar,
                },
            ) if stored_scalar == field.stored_scalar
                && value.scalar_type() == field.logical_scalar =>
            {
                Ok(encode(value, stored_scalar)?[..self.byte_size()?].to_vec())
            }
            (NativeCellKind::Case(discriminator), TerminalStorageContents::Case(case)) => {
                let mapping = layout
                    .discriminator_cases(discriminator)
                    .find(|mapping| mapping.case == case)
                    .ok_or_else(invalid_operands)?;
                Ok(mapping.native_tag.to_le_bytes().to_vec())
            }
            _ => Err(invalid_operands()),
        }
    }
}

pub(super) fn expand(
    layout: &NativeBorrowedStorageLayout,
) -> Result<Vec<NativeCell<'_>>, TerminalEffectRejection> {
    let mut cells = Vec::new();
    for field in layout.fields() {
        expand_coordinates(
            layout.field_path(field).collect(),
            field.byte_offset as usize,
            layout.field_repetitions(field),
            &mut |path, byte_offset| {
                cells.push(NativeCell {
                    path,
                    byte_offset,
                    kind: NativeCellKind::Scalar(field),
                });
                Ok(())
            },
        )?;
    }
    for discriminator in layout.discriminators() {
        expand_coordinates(
            layout.discriminator_path(discriminator).collect(),
            discriminator.byte_offset as usize,
            layout.discriminator_repetitions(discriminator),
            &mut |path, byte_offset| {
                cells.push(NativeCell {
                    path,
                    byte_offset,
                    kind: NativeCellKind::Case(discriminator),
                });
                Ok(())
            },
        )?;
    }
    Ok(cells)
}

pub(super) fn input_coverage(
    layout: &NativeBorrowedStorageLayout,
    values: &[TerminalBoundaryStorageValue],
    cells: &[NativeCell<'_>],
    byte_count: usize,
) -> Result<Vec<(usize, usize)>, TerminalEffectRejection> {
    for region in layout.opaque_regions() {
        expand_coordinates(
            layout.opaque_region_path(region).collect(),
            region.byte_offset as usize,
            layout.opaque_region_repetitions(region),
            &mut |path, offset| {
                if offset < byte_count && region.byte_size != 0 && is_active(&path, values)? {
                    return Err(TerminalEffectRejection::new(
                        "input accesses unmodeled permission or byte-descriptor storage",
                    ));
                }
                Ok(())
            },
        )?;
    }
    cells
        .iter()
        .map(|cell| {
            Ok((
                cell.byte_offset,
                cell.byte_offset
                    .checked_add(cell.byte_size()?)
                    .ok_or_else(invalid_operands)?,
            ))
        })
        .collect()
}

fn expand_coordinates(
    path: Vec<Path>,
    byte_offset: usize,
    dimensions: &[NativeStorageRepetition],
    visit: &mut impl FnMut(Vec<Path>, usize) -> Result<(), TerminalEffectRejection>,
) -> Result<(), TerminalEffectRejection> {
    let Some((dimension, remaining)) = dimensions.split_first() else {
        return visit(path, byte_offset);
    };
    for element_index in 0..dimension.length {
        let mut occurrence = path.clone();
        let coordinate = occurrence
            .get_mut(dimension.path_position)
            .ok_or_else(invalid_operands)?;
        if !matches!(coordinate, Path::FixedIndex(0)) {
            return Err(invalid_operands());
        }
        *coordinate = Path::FixedIndex(element_index);
        let displacement = element_index
            .checked_mul(u64::from(dimension.byte_stride))
            .ok_or_else(invalid_operands)?;
        let offset = byte_offset
            .checked_add(usize::try_from(displacement).map_err(|_| invalid_operands())?)
            .ok_or_else(invalid_operands)?;
        expand_coordinates(occurrence, offset, remaining, visit)?;
    }
    Ok(())
}

pub(super) fn is_active(
    path: &[Path],
    values: &[TerminalBoundaryStorageValue],
) -> Result<bool, TerminalEffectRejection> {
    for (position, segment) in path.iter().enumerate() {
        if let Path::Case(case) = segment {
            // Capture and decode retain canonical path order; decode has
            // already appended every ancestor before visiting its payload.
            let selector_position = values
                .binary_search_by(|value| value.path.as_slice().cmp(&path[..position]))
                .map_err(|_| invalid_operands())?;
            let discriminator = &values[selector_position];
            let TerminalStorageContents::Case(active) = discriminator.contents else {
                return Err(invalid_operands());
            };
            if active != *case {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
