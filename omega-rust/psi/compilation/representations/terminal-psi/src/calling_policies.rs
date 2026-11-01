//! Checked calling declarations, independently of native policy evaluation.
//!
//! Storage and contracts belong to the referenced boundary declaration. These
//! rows retain only the source-issued telescope that
//! cannot be recovered from an erased runtime signature.

use crate::TerminalIdentityIndex;
use semantic_vocabulary::{
    BoundaryMachineId, CallbackRequirementId, CallingOperandKind, CallingSignatureId,
    NativeParameterId, StaticMachineBinderId, StructuralTypeId,
};

/// One complete declaration ABI, independently of any selected Handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingAbiDeclaration {
    pub id: CallingSignatureId,
    pub requirement_identity: TerminalIdentityIndex,
    pub declaring_application: TerminalIdentityIndex,
    pub operands: Vec<CallingStaticOperand>,
    pub formals: Vec<semantic_vocabulary::CallingGenericFormal>,
    pub default_eligible: bool,
    pub storage: Result<CallingAbiStorage, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingStaticOperand {
    pub ordinal: u32,
    pub kind: CallingOperandKind,
    pub identity: TerminalIdentityIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingAbiStorage {
    pub parameters: Vec<CallingAbiParameter>,
    pub result: Option<StructuralTypeId>,
    pub telescope: CallingSignatureTelescope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingAbiParameter {
    pub structural_type: StructuralTypeId,
    /// The direct formal carrier used to discover inline private callback
    /// slots. Stored pointers remain leaves. None is an abstract pointee,
    /// never a fabricated empty record or implicit dereference path.
    pub callback_backing: Option<StructuralTypeId>,
    pub access: crate::StructuralAccess,
}

/// A checked target-independent selected carrier. Native geometry still
/// follows that carrier's complete target-layout alternative roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOpaqueRepresentation {
    pub opaque: StructuralTypeId,
    pub carrier: StructuralTypeId,
    /// Producer-known portable storage roles, distinct from nominal data names.
    pub opaque_storage_identity: TerminalIdentityIndex,
    pub carrier_storage_identity: TerminalIdentityIndex,
    pub application: language_semantics::opaque_representations::SelectedOpaqueApplication,
}

/// Ordered explicit policy requests retain all checked-input purposes.
/// Refusals stay dormant data until this request is consumed; publication does
/// not turn a missing declaration or policy into an empty executable request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingPolicyRequest {
    pub declaration: Result<TerminalIdentityIndex, String>,
    pub display_name: String,
    pub instances: Result<Vec<CallingPolicyInstance>, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingPolicyInstance {
    pub declaring_application: TerminalIdentityIndex,
    pub policy: Result<crate::TargetMachineCallable, String>,
    pub signatures: Result<Vec<CallingSignatureId>, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingSignatureDeclaration {
    pub boundary: BoundaryMachineId,
    /// Invocation contracts remain on the boundary. This association names
    /// only its declaration ABI, or its exact deferred projection refusal.
    pub application: Result<CallingSignatureId, String>,
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
