//! Build adapter fixtures through the same physical and encoding producers.
use crate::native_entry_bridge::{NativeEntryBridge, encode_native_entry_bridge};
use crate::native_foreign_bridge::{NativeForeignBridge, encode_native_foreign_bridge};
use physical_planning::native_entry_bridge::{
    NativeEntryBridgeError, NativeEntryBridgeRequest, plan_native_entry_bridge,
};
use physical_planning::native_foreign_bridge::{
    NativeForeignBridgeError, NativeForeignBridgeRequest, plan_native_foreign_bridge,
};

pub fn emit_native_entry_bridge(
    request: NativeEntryBridgeRequest,
) -> Result<NativeEntryBridge, NativeEntryBridgeError> {
    plan_native_entry_bridge(request).and_then(encode_native_entry_bridge)
}
pub fn emit_native_foreign_bridge(
    request: NativeForeignBridgeRequest,
) -> Result<NativeForeignBridge, NativeForeignBridgeError> {
    plan_native_foreign_bridge(request).and_then(encode_native_foreign_bridge)
}
