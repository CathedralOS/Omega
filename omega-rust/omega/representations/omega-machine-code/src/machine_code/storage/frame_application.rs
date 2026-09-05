//! Applied stack-frame bytes and their per-return placement records.
//!
//! These are current program data, not source or protocol admission.

mod identity;
pub use identity::function_fragment_frame_application_identity;

use crate::{FunctionFragmentEmissionPlan, TargetFrameProtocolEncodingIdentity};
use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
};
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId};
use psi_core::{EdgeId, MachineId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionFragmentFrameApplicationIdentity([u8; 32]);

impl FunctionFragmentFrameApplicationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionAppliedFrameEpilogue {
    pub block: SelectedBlockId,
    pub return_instruction: SelectedInstructionId,
    pub psi_return_edge: EdgeId,
    pub function_offset: u64,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAppliedFrameProtocol {
    pub machine: MachineId,
    pub prologue_function_offset: u64,
    pub prologue_byte_count: u64,
    pub epilogues: Vec<FunctionAppliedFrameEpilogue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentFrameApplication {
    pub identity: FunctionFragmentFrameApplicationIdentity,
    pub source_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    pub source_fragments: FunctionFragmentEmissionIdentity,
    pub frame_protocol: TargetFrameProtocolEncodingIdentity,
    pub functions: Vec<FunctionAppliedFrameProtocol>,
    pub fragments: FunctionFragmentEmissionPlan,
}

impl FunctionFragmentFrameApplication {
    pub fn recomputed_identity(&self) -> FunctionFragmentFrameApplicationIdentity {
        function_fragment_frame_application_identity(self)
    }
}
