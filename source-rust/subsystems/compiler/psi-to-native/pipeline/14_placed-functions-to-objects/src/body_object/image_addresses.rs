use super::{Error, host};
use crate::{ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
use machine_code::ImageAddressFields;
pub fn records(
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
