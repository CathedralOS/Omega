//! Build adapter fixtures through the same physical and encoding producers.
use native_encoding::native_entry_bridge::{NativeEntryBridge, encode_native_entry_bridge};
use native_encoding::native_foreign_bridge::{NativeForeignBridge, encode_native_foreign_bridge};
use physical_planning::native_entry_bridge::{
    NativeEntryBridgeError, NativeEntryBridgeRequest, plan_native_entry_bridge,
};
use physical_planning::native_foreign_bridge::{
    NativeForeignBridgeError, NativeForeignBridgeRequest, plan_native_foreign_bridge,
};

pub fn emit_native_entry_bridge(
    request: NativeEntryBridgeRequest,
) -> Result<NativeEntryBridge<physical_planning::PhysicalRecipeOrigin>, NativeEntryBridgeError> {
    plan_native_entry_bridge(request).and_then(encode_native_entry_bridge)
}
pub fn emit_native_foreign_bridge(
    request: NativeForeignBridgeRequest,
) -> Result<NativeForeignBridge<physical_planning::PhysicalRecipeOrigin>, NativeForeignBridgeError>
{
    plan_native_foreign_bridge(request).and_then(encode_native_foreign_bridge)
}

/// Produce the unresolved relocation that object and runtime fixtures consume.
pub fn emit_native_foreign_bridge_object(
    request: NativeForeignBridgeRequest,
) -> Result<object_file::NativeForeignBridge, NativeForeignBridgeError> {
    let encoded = emit_native_foreign_bridge(request)?;
    let placed = function_layout::entry_placement::place_native_foreign_bridge(encoded)
        .map_err(|_| NativeForeignBridgeError::ReplayMismatch)?;
    Ok(object_file::NativeBridgeObject::from_placement(placed))
}
