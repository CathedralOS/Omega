//! Checked calling declarations, independently of native policy evaluation.
//!
//! Storage and contracts belong to the referenced boundary declaration. These
//! rows retain only the source-issued telescope that
//! cannot be recovered from an erased runtime signature.

use crate::TerminalIdentityIndex;
use semantic_vocabulary::{
    BoundaryMachineId, CallbackRequirementId, NativeParameterId, StaticMachineBinderId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingSignatureDeclaration {
    pub boundary: BoundaryMachineId,
    pub requirement_identity: TerminalIdentityIndex,
    /// Unsupported source projections remain deferred until this exact
    /// declaration is requested. Inventory publication cannot reject an unused
    /// declaration merely because its native calling surface is unsupported.
    pub telescope: Result<CallingSignatureTelescope, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingSignatureTelescope {
    pub parameters: Vec<CallingRuntimeFormal>,
    pub binders: Vec<CallingCallbackBinder>,
    pub direct_callbacks: Vec<CallingDirectCallback>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallingRuntimeFormal {
    pub identity: NativeParameterId,
    pub native_ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingCallbackBinder {
    pub binder: StaticMachineBinderId,
    pub requirement: CallbackRequirementId,
    pub requirement_identity: TerminalIdentityIndex,
    pub static_machine_ordinal: u32,
    pub invokes_synchronously: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingDirectCallback {
    pub identity: NativeParameterId,
    pub native_ordinal: u32,
    pub binder: StaticMachineBinderId,
    pub requirement: CallbackRequirementId,
}
