//! Build adapter fixtures through the same physical and encoding producers.
use entry_planning::native_entry_bridge::{
    NativeEntryBridgeError, NativeEntryBridgeRequest, plan_native_entry_bridge,
};
use entry_planning::native_foreign_bridge::{
    NativeForeignBridgeError, NativeForeignBridgeRequest, plan_native_foreign_bridge,
};
use native_encoding::native_entry_bridge::{NativeEntryBridge, encode_native_entry_bridge};
use native_encoding::native_foreign_bridge::{NativeForeignBridge, encode_native_foreign_bridge};

pub fn emit_native_entry_bridge(
    request: NativeEntryBridgeRequest,
) -> Result<NativeEntryBridge<entry_planning::PhysicalRecipeOrigin>, NativeEntryBridgeError> {
    plan_native_entry_bridge(request).and_then(encode_native_entry_bridge)
}
pub fn emit_native_foreign_bridge(
    request: NativeForeignBridgeRequest,
) -> Result<NativeForeignBridge<entry_planning::PhysicalRecipeOrigin>, NativeForeignBridgeError> {
    plan_native_foreign_bridge(request).and_then(encode_native_foreign_bridge)
}

/// Exercise the production stages that bind the import body to a foreign adapter.
pub fn prepare_foreign_adapter_object(
    request: NativeForeignBridgeRequest,
    locator: target::NormalizedForeignLocator,
    application: [u8; 32],
) -> Result<object_file::ForeignAdapterObject, String> {
    let plan =
        entry_planning::native_foreign_bridge::plan_foreign_adapter(request, locator, application)?;
    let encoded = native_encoding::native_foreign_bridge::encode_foreign_adapter(
        installation_binding::entries::bind_foreign_adapter(plan),
    )
    .map_err(|error| error.to_string())?;
    let layout = placement::entry_layout::resolve_foreign_adapter(encoded)
        .map_err(|error| error.to_string())?;
    let placed = function_layout::place_fixed_entry(relaxation::retain_fixed_entry(layout));
    object_file::ForeignAdapterObject::from_placement(placed)
}
