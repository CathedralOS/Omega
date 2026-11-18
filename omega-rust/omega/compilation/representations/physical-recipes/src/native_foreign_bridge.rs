//! The call request and operations selected for a foreign adapter.
use post_allocation_machine::adapters::AdapterInstructions;
pub type NativeForeignBridgePlan<Origin> = crate::PhysicalRecipe<Origin, NativeForeignInstructions>;
pub use calling_conventions::NativeForeignBridgeRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeForeignInstructions {
    pub request: NativeForeignBridgeRequest,
    pub instructions: AdapterInstructions,
    pub frame_bytes: u32,
}

impl<Origin> NativeForeignBridgePlan<Origin> {
    pub fn instructions(&self) -> &AdapterInstructions {
        &self.instructions.instructions
    }
    pub fn into_request(self) -> NativeForeignBridgeRequest {
        self.instructions.request
    }

    pub fn frame_bytes(&self) -> u32 {
        self.instructions.frame_bytes
    }
    pub fn request(&self) -> &NativeForeignBridgeRequest {
        &self.instructions.request
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeForeignBridgeError {
    IncompatiblePlan,
    ScalarParameterMismatch,
    UnsupportedValueTransport,
    FrameOverflow,
    Encoding,
    ReplayMismatch,
}
diagnostics::failed_display!(NativeForeignBridgeError, "native foreign bridge failed");
impl std::error::Error for NativeForeignBridgeError {}

/// Bytes and relocation metadata for inspection. Admission is carried by the owning encoded or placed bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeForeignBridgeCode {
    request: NativeForeignBridgeRequest, // Request parameters that produced this bridge.
    bytes: Vec<u8>,                      // Encoded machine bytes of the foreign bridge adapter.
    frame_bytes: u32,                    // Total stack frame size reserved by the adapter.
    call_field_offset: usize, // Byte offset of the unresolved direct-call displacement field.
}

impl NativeForeignBridgeCode {
    pub fn from_recipe<Origin>(
        plan: NativeForeignBridgePlan<Origin>,
        bytes: Vec<u8>,
        call_field_offset: usize,
    ) -> Self {
        let frame_bytes = plan.frame_bytes();
        Self {
            request: plan.into_request(),
            bytes,
            frame_bytes,
            call_field_offset,
        }
    }

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
