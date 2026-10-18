//! Bind relocated selector addresses and callable rows to their final targets.
//!
//! Relocation envelopes deliberately mask immediate fields. Exact metadata
//! therefore does not establish where patched instructions or pointers lead.
//! Validate both sides of the table on every image format; Mach-O loader-fixup
//! checking is additional evidence, not a substitute on ELF or PE.

use diagnostics::Diagnostic;
use image::EmittedImageOutput;
use machine_code::ImageAddressFields;

use crate::private_object_composition::ObjectNamespace;

pub(super) fn validate_targets(
    namespace: &ObjectNamespace<'_>,
    output: &EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let artifact = namespace.artifact;
    let Some(pool) = &artifact.implementation_table_pool else {
        return Ok(());
    };
    for address in &pool.addresses {
        let table = pool
            .tables
            .iter()
            .find(|table| Some(table.table) == address.implementation_table())
            .ok_or_else(|| Diagnostic::error("final implementation address has no table"))?;
        let expected_target = section_address(
            output.final_image_layout.data_address,
            namespace.data_offset(table.data_offset)?,
        )?;
        let field_offset = |offset| {
            usize::try_from(offset)
                .ok()
                .and_then(|offset| offset.checked_add(namespace.text_offset))
                .ok_or_else(|| Diagnostic::error("implementation address field offset overflows"))
        };
        let actual_target = match address.fields {
            ImageAddressFields::X86_64Relative32 { patch_offset } => {
                super::decode_x86_relative_target(output, field_offset(patch_offset)?)?
            }
            ImageAddressFields::Aarch64PageAddress {
                page_patch_offset,
                page_offset_patch_offset,
            } => super::decode_aarch64_page_target(
                output,
                field_offset(page_patch_offset)?,
                field_offset(page_offset_patch_offset)?,
            )?,
        };
        if actual_target != expected_target {
            return Err(Diagnostic::error(
                "final implementation address does not resolve to its exact table",
            ));
        }
    }
    for table in &pool.tables {
        let declaration = pool.catalog.table(table.table).ok_or_else(|| {
            Diagnostic::error("final implementation table has no source declaration")
        })?;
        for (slot, machine) in declaration.rows.iter().enumerate() {
            let expected_target = if let Some(machine) = machine {
                let function = artifact
                    .function_on_machine(*machine)
                    .ok_or_else(|| Diagnostic::error("final implementation row has no function"))?;
                section_address(
                    output.final_image_layout.text_address,
                    namespace.text_offset(function.text_offset)?,
                )?
            } else {
                0
            };
            let row_offset = slot
                .checked_mul(8)
                .and_then(|offset| table.data_offset.checked_add(offset))
                .and_then(|offset| offset.checked_add(namespace.data_offset))
                .ok_or_else(|| Diagnostic::error("final implementation row offset overflows"))?;
            let row_end = row_offset
                .checked_add(8)
                .ok_or_else(|| Diagnostic::error("final implementation row end overflows"))?;
            let actual_target = output
                .final_data_bytes
                .get(row_offset..row_end)
                .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
                .map(u64::from_le_bytes)
                .ok_or_else(|| Diagnostic::error("final implementation row is truncated"))?;
            if actual_target != expected_target {
                return Err(Diagnostic::error(
                    "final implementation row does not resolve to its exact function or trap",
                ));
            }
        }
    }
    Ok(())
}

fn section_address(section_address: u64, offset: usize) -> Result<u64, Diagnostic> {
    u64::try_from(offset)
        .ok()
        .and_then(|offset| section_address.checked_add(offset))
        .ok_or_else(|| Diagnostic::error("final implementation target address overflows"))
}
