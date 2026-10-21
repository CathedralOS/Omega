//! Prepare one separately emitted callable for a component container.
//!
//! The ordinary body retains its Terminal replay source and ABI. An optional
//! physical entry bridge is appended as its own region, rather than changing
//! helper/recursion calling conventions or linking the body into a consumer.
//! This is image input only; no mapping or installed entry authority is created.

use diagnostics::Diagnostic;
use image::{
    FinalImage, FinalImageInput, FinalImageRelocation, FinalImageSection, FinalImageSymbol,
};
use machine_emission::native_entry_bridge::NativeEntryBridge;
use object_file::{RelocationKind, SymbolKind};

pub struct ComponentFunctionImage {
    pub(crate) body: crate::ObjectArtifact,
    pub(crate) entry_bridge: Option<NativeEntryBridge>,
    pub(crate) image: FinalImage,
    pub(crate) export_symbol: image::FinalImageSymbolHandle,
    pub(crate) slot_commitment: [u8; 32],
}

impl ComponentFunctionImage {
    pub const fn image(&self) -> &FinalImage {
        &self.image
    }
    pub const fn slot_commitment(&self) -> &[u8; 32] {
        &self.slot_commitment
    }
}

/// Project one exact emitted body and validated ABI bridge into final-image
/// coordinates. The symbolic export remains qualified by its complete slot
/// application commitment; its spelling is linker metadata, never selection.
pub fn prepare_component_function_image(
    body: crate::ObjectArtifact,
    entry_bridge: Option<NativeEntryBridge>,
    slot_commitment: [u8; 32],
) -> Result<ComponentFunctionImage, Diagnostic> {
    let invalid = |reason: &str| Diagnostic::error(format!("component function image: {reason}"));
    if slot_commitment == [0; 32]
        || body.hosted_receiver_binding().is_some()
        || body.physical_entry_binding().is_some()
        || !body.private_functions().is_empty()
    {
        return Err(invalid(
            "export lacks an exact slot or carries another entry's installation",
        ));
    }
    let mut image = image::build_final_image(FinalImageInput {
        target: body.target(),
        object: body.object(),
        relocations: body.relocations(),
        text_bytes: body.text_bytes(),
        data_bytes: body.data_bytes(),
    });
    let body_symbol = image.symbol_table.entry_symbol;
    let export_symbol = match &entry_bridge {
        None => body_symbol,
        Some(bridge) => {
            if bridge.request().target != body.target() {
                return Err(invalid("entry bridge and body targets differ"));
            }
            let ordinals = bridge
                .request()
                .parameters
                .iter()
                .map(|(_, parameter)| parameter.inbound_ordinal)
                .collect::<Vec<_>>();
            let expected = crate::callable_entry_transport::native_entry_request(
                &body,
                &bridge.request().inbound,
                &ordinals,
            )
            .map_err(|error| invalid(&error.to_string()))?;
            if expected != *bridge.request() {
                return Err(invalid(
                    "entry bridge belongs to a different ordinary body ABI",
                ));
            }
            machine_emission::native_entry_bridge::validate_native_entry_bridge(bridge)
                .map_err(|error| invalid(&error.to_string()))?;
            let offset = image
                .memory
                .text
                .len()
                .checked_add(15)
                .ok_or_else(|| invalid("entry bridge offset overflows"))?
                & !15;
            image.memory.text.resize(offset, 0);
            image.memory.text.extend_from_slice(bridge.bytes());
            let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: component_export_symbol(&slot_commitment),
                section: FinalImageSection::Text,
                offset,
                size: bridge.bytes().len(),
                kind: SymbolKind::Function,
            });
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset: offset
                        .checked_add(bridge.call_field_offset())
                        .ok_or_else(|| invalid("entry bridge call offset overflows"))?,
                    byte_width: 4,
                    symbol_handle: body_symbol,
                    addend: 0,
                    kind: match body.target().architecture {
                        target::Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
                        target::Architecture::X86_64 => RelocationKind::X86_64Relative32,
                    },
                });
            symbol
        }
    };
    Ok(ComponentFunctionImage {
        body,
        entry_bridge,
        image,
        export_symbol,
        slot_commitment,
    })
}

pub fn component_export_symbol(commitment: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut symbol = String::from("__omega_slot_");
    for byte in commitment {
        write!(&mut symbol, "{byte:02x}").expect("writing a component symbol to a String");
    }
    symbol
}
