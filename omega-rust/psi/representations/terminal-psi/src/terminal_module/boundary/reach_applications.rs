//! Closed generic reach substitution is a semantic contract, not a proof that
//! the producer inferred the original source correctly. Existing template and
//! specialization commitments identify that origin; they do not authenticate
//! this projection without a checkable opening. Ordinary consumption trusts the
//! projection, then independently checks its finite union and exact call joins.
//! No generic execution, body-based narrowing, or additional selection identity
//! is introduced. Calls retain their ordinary execution and fuel semantics.

use semantic_vocabulary::{MachineId, OperationId, ServiceId};

/// One concrete machine's closed substitution of an original reach dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachApplication {
    pub template_identity: String,
    pub template_commitment: [u8; 32],
    pub specialization_commitment: [u8; 32],
    /// Full declaration order, including unused and non-machine parameters.
    pub telescope: Vec<ClosedReachParameter>,
    pub fixed: Vec<ServiceId>,
    /// Strictly increasing zero-based positions in the full telescope.
    pub dependencies: Vec<u32>,
    /// Exact consumers, independently joined to operation-side binder markers.
    pub calls: Vec<ClosedReachCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedReachParameter {
    Type { argument: String },
    Const { argument: String },
    Proposition { argument: String },
    Machine(ClosedReachMachineBinding),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachMachineBinding {
    /// None denotes a structural contract with a fixed, non-variable row.
    pub nominal_requirement: Option<String>,
    pub upper_bound: Vec<ServiceId>,
    pub selected_identity: String,
    pub selected_contract_commitment: [u8; 32],
    /// The selected public/effective contract, never inferred from emitted body
    /// operations. An inert declaration remains observable to substitution.
    pub selected_reach: Vec<ServiceId>,
    /// An unused selection need not have an emitted body. Every dependency and
    /// direct consumer must nevertheless join an actual retained callable.
    pub callee: Option<MachineId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachCall {
    pub operation: OperationId,
    pub binder: u32,
}
