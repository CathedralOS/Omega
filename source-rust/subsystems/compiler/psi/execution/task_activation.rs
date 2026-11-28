use crate::TerminalIdentityIndex;
use semantic_vocabulary::{ScalarType, StructuralTypeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalTaskStartOperation {
    Start,
    TryStart,
}

/// Who owns execution is separate from the frames that must survive a park.
/// Only scheduled tasks select a TaskRuntime provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalActivationOrigin {
    TaskRuntime(TerminalTaskRuntimeSelection),
    Caller(TerminalPreparedActivationEntry),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTaskRuntimeSelection {
    pub operation: TerminalTaskStartOperation,
    pub requirement_callable_identity: TerminalIdentityIndex,
    pub requirement_owner_path: TerminalIdentityIndex,
    pub requirement_owner_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub requirement_in_scope: bool,
    pub authored_requirement_identity: TerminalIdentityIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalPreparedActivationEntry {
    pub preparation: semantic_vocabulary::BoundaryMachineId,
    pub entry_machine: semantic_vocabulary::MachineId,
    pub storage: TerminalActivationStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalActivationStorage {
    Fixed,
    Split,
}

impl TerminalPreparedActivationEntry {
    /// The checked prepare application, not a service name or the program's
    /// root entry, determines which child this code ID is allowed to start.
    /// This checks the link only; it neither admits a plan nor grants backing.
    pub fn selected_body(self, module: &crate::TerminalModule) -> Option<&crate::TerminalMachine> {
        let binding = module.boundary_machine(self.preparation)?.declaration.as_ref()?;
        if binding.execution != crate::CheckedBoundaryExecution::OwnedActivation(
            language_core::sealed_intrinsics::ActivationOperation::Prepare
        ) {
            return None;
        }
        let [crate::ClosedReachArgument::Type(_), crate::ClosedReachArgument::Type(_),
            entry @ crate::ClosedReachArgument::Machine { .. }, crate::ClosedReachArgument::Const(_)] =
            binding.method_arguments.as_slice() else { return None };
        entry.checked_body(module, self.entry_machine)
    }
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
    pub origin: TerminalActivationOrigin,
    pub report_fingerprint: u64,
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
