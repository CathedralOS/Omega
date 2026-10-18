//! One relocation mechanism for image addresses; placement owners supply only
//! the referent symbol and addend, not another ISA-specific patching path.

use super::{Error, host};
use machine_code::ImageAddressFields;
use object_file::{
    ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationRecord, SectionKind,
};

pub(super) fn records(
    fields: ImageAddressFields,
    origin: RelocationOrigin,
    destination: ObjectSymbolHandle,
    addend: i64,
) -> Result<Vec<RelocationRecord>, Error> {
    let record = |offset, kind| -> Result<RelocationRecord, Error> {
        Ok(RelocationRecord {
            origin,
            section: SectionKind::Text,
            offset: host(offset)?,
            byte_width: 4,
            symbol_handle: destination,
            addend,
            kind,
        })
    };
    match fields {
        ImageAddressFields::X86_64Relative32 { patch_offset } => Ok(vec![record(
            patch_offset,
            RelocationKind::X86_64Relative32,
        )?]),
        ImageAddressFields::Aarch64PageAddress {
            page_patch_offset,
            page_offset_patch_offset,
        } => Ok(vec![
            record(page_patch_offset, RelocationKind::Aarch64Page21)?,
            record(
                page_offset_patch_offset,
                RelocationKind::Aarch64PageOffset12,
            )?,
        ]),
    }
}
