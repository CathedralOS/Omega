//! Encode the selected adapter with its unresolved foreign-call field.
use physical_planning::native_foreign_bridge::{
    NativeForeignBridgeError as Error, NativeForeignBridgePlan, NativeForeignBridgeRequest,
};
pub fn encode_native_foreign_bridge(
    plan: NativeForeignBridgePlan,
) -> Result<NativeForeignBridge, Error> {
    let (bytes, call_field_offset) =
        crate::adapters::encode_instructions(plan.instructions()).map_err(|_| Error::Encoding)?;
    let frame_bytes = plan.frame_bytes();
    let request = plan.into_request();
    Ok(NativeForeignBridge {
        request,
        bytes,
        frame_bytes,
        call_field_offset,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeForeignBridge {
    request: NativeForeignBridgeRequest, // Request parameters that produced this bridge.
    bytes: Vec<u8>,                      // Encoded machine bytes of the foreign bridge adapter.
    frame_bytes: u32,                    // Total stack frame size reserved by the adapter.
    call_field_offset: usize, // Byte offset of the unresolved direct-call displacement field.
}

impl NativeForeignBridge {
    pub fn request(&self) -> &NativeForeignBridgeRequest {
        &self.request
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn frame_bytes(&self) -> u32 {
        self.frame_bytes
    }

    pub fn call_field_offset(&self) -> usize {
        self.call_field_offset
    }
}
