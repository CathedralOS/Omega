//! Exact geometry decoding for one admitted UEFI Loaded Image occurrence.
//!
//! The caller must first obtain readable protocol bytes from the exact
//! lifecycle-scoped provider execution. This module only validates and decodes
//! those bytes through the target-owned layout; it grants no pointer, mapping,
//! `Extent`, root, or execution authority.

use std::num::NonZeroU64;

use crate::{
    TargetProfile, UefiLoadedImageNativeField, UefiLoadedImageNativeFieldKind,
    UefiLoadedImageNativeFieldLayout, ValidatedUefiLoadedImageNativeLayout,
    plan_uefi_loaded_image_native_layout,
};
use psi_diagnostics::Diagnostic;

pub const UEFI_LOADED_IMAGE_PROTOCOL_REVISION: u32 = 0x1000;

#[derive(Debug)]
#[must_use = "validated UEFI Loaded Image geometry retains exact target layout evidence"]
pub struct ValidatedUefiLoadedImageGeometry {
    layout: ValidatedUefiLoadedImageNativeLayout,
    revision: u32,
    image_base: NonZeroU64,
    image_size: NonZeroU64,
}

impl ValidatedUefiLoadedImageGeometry {
    pub const fn layout(&self) -> &ValidatedUefiLoadedImageNativeLayout {
        &self.layout
    }
    pub const fn revision(&self) -> u32 {
        self.revision
    }
    pub const fn image_base(&self) -> u64 {
        self.image_base.get()
    }
    pub const fn image_size(&self) -> u64 {
        self.image_size.get()
    }
    pub const fn image_end_exclusive(&self) -> u64 {
        self.image_base.get() + self.image_size.get()
    }
}

#[derive(Debug)]
#[must_use = "UEFI Loaded Image decoding rejection retains layout and borrowed bytes"]
pub struct UefiLoadedImageOccurrenceValidationError<'occurrence> {
    layout: ValidatedUefiLoadedImageNativeLayout,
    supplied_bytes: &'occurrence [u8],
    diagnostic: Diagnostic,
}

impl<'occurrence> UefiLoadedImageOccurrenceValidationError<'occurrence> {
    pub const fn layout(&self) -> &ValidatedUefiLoadedImageNativeLayout {
        &self.layout
    }
    pub const fn supplied_bytes(&self) -> &'occurrence [u8] {
        self.supplied_bytes
    }
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        ValidatedUefiLoadedImageNativeLayout,
        &'occurrence [u8],
        Diagnostic,
    ) {
        (self.layout, self.supplied_bytes, self.diagnostic)
    }
}

impl std::fmt::Display for UefiLoadedImageOccurrenceValidationError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiLoadedImageOccurrenceValidationError<'_> {}

pub fn validate_uefi_loaded_image_occurrence<'occurrence>(
    layout: ValidatedUefiLoadedImageNativeLayout,
    supplied_bytes: &'occurrence [u8],
) -> Result<
    ValidatedUefiLoadedImageGeometry,
    Box<UefiLoadedImageOccurrenceValidationError<'occurrence>>,
> {
    let expected = match plan_uefi_loaded_image_native_layout(TargetProfile::UefiX64) {
        Ok(expected) => expected,
        Err(error) => return reject(layout, supplied_bytes, error.into_parts().1),
    };
    if !layout.matches_exact_plan(&expected) {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error(
                "UEFI Loaded Image occurrence cannot join a drifted target-owned layout",
            ),
        );
    }
    if supplied_bytes.len() < layout.byte_size() as usize {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error(format!(
                "UEFI Loaded Image occurrence is {} bytes; expected at least {}",
                supplied_bytes.len(),
                expected.byte_size()
            )),
        );
    }
    let fields = match geometry_fields(&layout) {
        Ok(fields) => fields,
        Err(diagnostic) => return reject(layout, supplied_bytes, diagnostic),
    };
    let revision = read_u32(supplied_bytes, fields[0]);
    if revision != UEFI_LOADED_IMAGE_PROTOCOL_REVISION {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error(format!(
                "UEFI Loaded Image revision is {revision:#010x}; expected {UEFI_LOADED_IMAGE_PROTOCOL_REVISION:#010x}"
            )),
        );
    }
    let Some(image_base) = NonZeroU64::new(read_u64(supplied_bytes, fields[1])) else {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error("UEFI Loaded Image occurrence has a null ImageBase"),
        );
    };
    let Some(image_size) = NonZeroU64::new(read_u64(supplied_bytes, fields[2])) else {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error("UEFI Loaded Image occurrence has an empty ImageSize"),
        );
    };
    if image_base.get().checked_add(image_size.get()).is_none() {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error(
                "UEFI Loaded Image occurrence geometry wraps the u64 address carrier",
            ),
        );
    }
    Ok(ValidatedUefiLoadedImageGeometry {
        layout,
        revision,
        image_base,
        image_size,
    })
}

fn geometry_fields(
    layout: &ValidatedUefiLoadedImageNativeLayout,
) -> Result<[UefiLoadedImageNativeFieldLayout; 3], Diagnostic> {
    use UefiLoadedImageNativeField as Field;
    use UefiLoadedImageNativeFieldKind as Kind;
    let fields = [
        required(layout, Field::Revision)?,
        required(layout, Field::ImageBase)?,
        required(layout, Field::ImageSize)?,
    ];
    let expected = [
        (0, 4, Kind::UnsignedInteger),
        (64, 8, Kind::Pointer),
        (72, 8, Kind::UnsignedInteger),
    ];
    if fields
        .iter()
        .copied()
        .zip(expected)
        .any(|(field, (offset, size, kind))| {
            field.byte_offset() != offset || field.byte_size() != size || field.kind() != kind
        })
    {
        return Err(Diagnostic::error(
            "UEFI Loaded Image geometry fields drifted from their exact target layout",
        ));
    }
    Ok(fields)
}

fn required(
    layout: &ValidatedUefiLoadedImageNativeLayout,
    field: UefiLoadedImageNativeField,
) -> Result<UefiLoadedImageNativeFieldLayout, Diagnostic> {
    layout
        .field_layout(field)
        .ok_or_else(|| Diagnostic::error(format!("UEFI Loaded Image layout is missing {field:?}")))
}

fn reject<'occurrence>(
    layout: ValidatedUefiLoadedImageNativeLayout,
    supplied_bytes: &'occurrence [u8],
    diagnostic: Diagnostic,
) -> Result<
    ValidatedUefiLoadedImageGeometry,
    Box<UefiLoadedImageOccurrenceValidationError<'occurrence>>,
> {
    Err(Box::new(UefiLoadedImageOccurrenceValidationError {
        layout,
        supplied_bytes,
        diagnostic,
    }))
}

fn read_u32(bytes: &[u8], field: UefiLoadedImageNativeFieldLayout) -> u32 {
    let offset = field.byte_offset() as usize;
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], field: UefiLoadedImageNativeFieldLayout) -> u64 {
    let offset = field.byte_offset() as usize;
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(image_base: u64, image_size: u64) -> [u8; 96] {
        let mut bytes = [0; 96];
        bytes[0..4].copy_from_slice(&UEFI_LOADED_IMAGE_PROTOCOL_REVISION.to_le_bytes());
        bytes[64..72].copy_from_slice(&image_base.to_le_bytes());
        bytes[72..80].copy_from_slice(&image_size.to_le_bytes());
        bytes
    }

    #[test]
    fn exact_layout_decodes_nonwrapping_loaded_image_geometry() {
        let bytes = bytes(0x10_0000, 0x20_000);
        let geometry = validate_uefi_loaded_image_occurrence(
            plan_uefi_loaded_image_native_layout(TargetProfile::UefiX64).unwrap(),
            &bytes,
        )
        .unwrap();
        assert_eq!(geometry.revision(), UEFI_LOADED_IMAGE_PROTOCOL_REVISION);
        assert_eq!(geometry.image_base(), 0x10_0000);
        assert_eq!(geometry.image_size(), 0x20_000);
        assert_eq!(geometry.image_end_exclusive(), 0x12_0000);
    }

    #[test]
    fn short_revision_null_empty_and_wrapping_occurrences_reject() {
        let cases = [
            vec![0; 95],
            bytes(0x10_0000, 0x20_000)
                .into_iter()
                .enumerate()
                .map(|(index, byte)| if index == 0 { byte ^ 1 } else { byte })
                .collect(),
            bytes(0, 0x20_000).to_vec(),
            bytes(0x10_0000, 0).to_vec(),
            bytes(u64::MAX - 1, 2).to_vec(),
        ];
        for bytes in cases {
            assert!(
                validate_uefi_loaded_image_occurrence(
                    plan_uefi_loaded_image_native_layout(TargetProfile::UefiX64).unwrap(),
                    &bytes,
                )
                .is_err()
            );
        }
    }
}
