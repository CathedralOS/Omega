//! Raw frame protocol bytes and spans. These records do not grant emission authority.

use crate::{TargetFrameLayoutIdentity, TargetFrameProtocolEncodingIdentity};
use register_model::{PhysicalRegisterModelIdentity, TargetRegisterEnvironmentIdentity};
use semantic_vocabulary::MachineId;
use target::NativeTarget;

mod identity;
pub use identity::target_frame_protocol_encoding_identity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetFrameProtocolEncodingPolicy {
    CanonicalFixedFrameV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameProtocolByteSpan {
    pub offset: u32,
    pub length: u32,
}

impl FrameProtocolByteSpan {
    pub fn bytes(self, arena: &[u8]) -> Option<&[u8]> {
        let start = usize::try_from(self.offset).ok()?;
        let end = start.checked_add(usize::try_from(self.length).ok()?)?;
        arena.get(start..end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionTargetFrameProtocolEncoding {
    pub machine: MachineId,
    pub prologue: FrameProtocolByteSpan,
    pub epilogue: FrameProtocolByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFrameProtocolEncodingPlan {
    pub frame_layout: TargetFrameLayoutIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub target: NativeTarget,
    pub policy: TargetFrameProtocolEncodingPolicy,
    pub functions: Vec<FunctionTargetFrameProtocolEncoding>,
    /// One canonical packed arena for every function's prologue and epilogue.
    pub bytes: Vec<u8>,
}
