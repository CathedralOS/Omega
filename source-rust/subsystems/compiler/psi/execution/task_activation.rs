use crate::TerminalIdentityIndex;
use semantic_vocabulary::{ScalarType, StructuralTypeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTaskStartOperation {
    Start,
    TryStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTaskValueType {
    Unit,
    Scalar(ScalarType),
    Structural(StructuralTypeId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskSignatureValue {
    pub value_type: TerminalTaskValueType,
    pub normalized_identity: TerminalIdentityIndex,
    pub is_mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTaskCarryStorage {
    Persistent,
    Parameter,
    Local,
    CallArgument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskLiveCarry {
    pub place: u64,
    pub value_type: u64,
    pub storage: TerminalTaskCarryStorage,
    pub claims: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskSuspensionCrossing {
    pub identity: u64,
    pub live_carry: Vec<TerminalTaskLiveCarry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskResidentValue {
    pub value_type: TerminalTaskValueType,
    pub normalized_identity: TerminalIdentityIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskFrameState {
    pub state: TerminalIdentityIndex,
    pub resident: Vec<TerminalTaskResidentValue>,
    pub crossing_arguments: Vec<Vec<TerminalTaskResidentValue>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskFrameCall {
    pub callee: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTaskUnresolvedCallKind {
    UnresolvedTarget,
    NonCheckedSupply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskUnresolvedCall {
    pub state: TerminalIdentityIndex,
    pub statement_index: u32,
    pub call_ordinal: u32,
    pub kind: TerminalTaskUnresolvedCallKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskFrame {
    pub identity: u64,
    pub machine_callable_identity: TerminalIdentityIndex,
    pub entry: TerminalIdentityIndex,
    pub states: Vec<TerminalTaskFrameState>,
    pub calls: Vec<TerminalTaskFrameCall>,
    pub unresolved_calls: Vec<TerminalTaskUnresolvedCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskActivationDemand {
    pub operation: TerminalTaskStartOperation,
    pub report_fingerprint: u64,
    pub requirement_callable_identity: TerminalIdentityIndex,
    pub requirement_owner_path: TerminalIdentityIndex,
    pub requirement_owner_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub requirement_in_scope: bool,
    pub authored_requirement_identity: TerminalIdentityIndex,
    pub machine_callable_identity: TerminalIdentityIndex,
    pub entry: TerminalIdentityIndex,
    pub entry_report_fingerprint: u64,
    pub contract_report_fingerprint: u64,
    pub specialization_commitment: [u8; 32],
    pub specialization_report_fingerprint: u64,
    pub may_suspend: bool,
    pub may_block: bool,
    pub cancellation_required: bool,
    pub signature_arguments: Vec<TerminalTaskSignatureValue>,
    pub signature_outcome: TerminalTaskSignatureValue,
    pub root_frame: u64,
    pub frames: Vec<TerminalTaskFrame>,
    pub crossings: Vec<TerminalTaskSuspensionCrossing>,
}
