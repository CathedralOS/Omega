//! Compare the descriptor consumed at runtime against final loaded image
//! addresses and the complete finalized instruction attribution.
use crate::private_object_composition::ObjectNamespace;
use diagnostics::Diagnostic;
use image::EmittedImageOutput;
use machine_code::ImageAddressFields;
use selected_instructions::SelectedImageAddressReference;

pub(super) fn validate_targets(
    namespace: &ObjectNamespace<'_>,
    output: &EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let Some(pool) = &namespace.artifact.native_code_layout_pool else {
        return Ok(());
    };
    let data_address = |offset: usize| -> Result<u64, Diagnostic> {
        output
            .final_image_layout
            .data_address
            .checked_add(
                u64::try_from(namespace.data_offset(offset)?)
                    .map_err(|_| Diagnostic::error("native metadata data offset overflows"))?,
            )
            .ok_or_else(|| Diagnostic::error("native metadata data address overflows"))
    };
    let text_address = |offset: u64| -> Result<u64, Diagnostic> {
        output
            .final_image_layout
            .text_address
            .checked_add(
                u64::try_from(namespace.text_offset)
                    .map_err(|_| Diagnostic::error("native metadata text offset overflows"))?,
            )
            .and_then(|base| base.checked_add(offset))
            .ok_or_else(|| Diagnostic::error("native metadata text address overflows"))
    };
    let word = |offset: usize| -> Result<u64, Diagnostic> {
        let offset = namespace.data_offset(offset)?;
        let end = offset
            .checked_add(8)
            .ok_or_else(|| Diagnostic::error("native metadata word end overflows"))?;
        output
            .final_data_bytes
            .get(offset..end)
            .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
            .map(u64::from_le_bytes)
            .ok_or_else(|| Diagnostic::error("native metadata word is truncated"))
    };
    for descriptor in &pool.layouts {
        if word(descriptor.data_offset)? != text_address(descriptor.layout.entry_section_offset)?
            || word(descriptor.data_offset + 8)? != data_address(descriptor.data_offset + 24)?
            || word(descriptor.data_offset + 16)?
                != u64::try_from(descriptor.layout.regions.len())
                    .map_err(|_| Diagnostic::error("native metadata region count overflows"))?
        {
            return Err(Diagnostic::error(
                "final native descriptor differs from its exact selected entry or finite region sequence",
            ));
        }
        for (ordinal, region) in descriptor.layout.regions.iter().enumerate() {
            let offset = descriptor.data_offset + 24 + ordinal * 16;
            if word(offset)? != text_address(region.section_offset)?
                || word(offset + 8)? != region.length
            {
                return Err(Diagnostic::error(
                    "final native region differs from finalized instruction attribution",
                ));
            }
        }
    }
    for address in &pool.addresses {
        let SelectedImageAddressReference::NativeCodeLayout { machine } = address.reference else {
            return Err(Diagnostic::error(
                "native metadata address has another image object kind",
            ));
        };
        let descriptor = pool
            .layouts
            .iter()
            .find(|row| row.layout.machine == machine)
            .ok_or_else(|| Diagnostic::error("final native descriptor is absent"))?;
        let field = |offset: u64| -> Result<usize, Diagnostic> {
            namespace.text_offset(
                usize::try_from(offset)
                    .map_err(|_| Diagnostic::error("native metadata relocation field overflows"))?,
            )
        };
        let target = match address.fields {
            ImageAddressFields::X86_64Relative32 { patch_offset } => {
                super::decode_x86_relative_target(output, field(patch_offset)?)?
            }
            ImageAddressFields::Aarch64PageAddress {
                page_patch_offset,
                page_offset_patch_offset,
            } => super::decode_aarch64_page_target(
                output,
                field(page_patch_offset)?,
                field(page_offset_patch_offset)?,
            )?,
        };
        if target != data_address(descriptor.data_offset)? {
            return Err(Diagnostic::error(
                "final native query address does not resolve to its exact descriptor",
            ));
        }
    }
    Ok(())
}
