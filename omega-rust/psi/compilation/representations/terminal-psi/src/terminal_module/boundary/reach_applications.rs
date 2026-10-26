//! Closed generic reach substitution is a semantic contract, not a proof that
//! the producer inferred the original source correctly. Existing template and
//! specialization commitments identify that origin; they do not authenticate
//! this projection without a checkable opening. Ordinary consumption trusts the
//! projection, then independently checks its finite union and exact call joins.
//! No generic execution, body-based narrowing, or additional selection identity
//! is introduced. Calls retain their ordinary execution and fuel semantics.

use crate::terminal_module::TerminalIdentityIndex;
use semantic_vocabulary::{MachineId, OperationId, ServiceId};

/// One concrete machine's closed substitution of an original reach dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachApplication {
    pub template_identity: TerminalIdentityIndex,
    pub template_commitment: [u8; 32],
    pub specialization_commitment: [u8; 32],
    /// Original declaration kinds, independently of the substituted arguments.
    /// Tags follow the closed reach telescope wire vocabulary.
    pub parameter_kinds: Vec<u8>,
    /// Full declaration order, including unused and non-machine parameters.
    pub telescope: Vec<ClosedReachParameter>,
    /// Conformance capture is a distinct outer substitution. Invocation tuples
    /// above retain only the method's own arguments, including unused slots.
    pub capture: Option<ClosedReachCapture>,
    pub fixed: Vec<ServiceId>,
    /// Strictly increasing zero-based positions in the full telescope.
    pub dependencies: Vec<u32>,
    /// Exact consumers, independently joined to operation-side binder markers.
    pub calls: Vec<ClosedReachCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachCapture {
    pub template_identity: TerminalIdentityIndex,
    pub template_commitment: [u8; 32],
    pub specialization_commitment: [u8; 32],
    pub parameter_kinds: Vec<u8>,
    pub telescope: Vec<ClosedReachParameter>,
}

impl ClosedReachApplication {
    /// Dependency positions are scoped: capture binders precede method-local
    /// binders. Method invocation tuples never acquire the outer capture slots.
    pub fn parameters(&self) -> impl Iterator<Item = &ClosedReachParameter> {
        self.capture
            .iter()
            .flat_map(|capture| &capture.telescope)
            .chain(&self.telescope)
    }

    pub fn parameter(&self, position: u32) -> Option<&ClosedReachParameter> {
        self.parameters().nth(position as usize)
    }
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
    /// Complete original trait telescope, including unused proposition and
    /// declaration-identity arguments. Identities may refer to scoped owner
    /// binders; the enclosing full substitution supplies those binders.
    pub requirement_parameter_kinds: Vec<u8>,
    pub requirement_arguments: Vec<TerminalIdentityIndex>,
    pub upper_bound: Vec<ServiceId>,
    pub selected_identity: TerminalIdentityIndex,
    pub selected_contract_commitment: [u8; 32],
    /// The selected public/effective contract, never inferred from emitted body
    /// operations. An inert declaration remains observable to substitution.
    pub selected_reach: Vec<ServiceId>,
    /// An unused selection need not have an emitted body. Every dependency and
    /// direct consumer must nevertheless join an actual retained callable.
    pub callee: Option<MachineId>,
    /// A generic selection with emitted applications names its original
    /// contract, not one observed body. Its consumers retain separate closed
    /// applications below. An unused selected contract needs no schema join;
    /// absence here does not assert that the selection is nongeneric.
    pub schema: Option<ClosedReachSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachSchema {
    pub template_identity: TerminalIdentityIndex,
    pub template_commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachCall {
    pub operation: OperationId,
    pub binder: u32,
    pub application: Option<ClosedReachCallApplication>,
}

/// The expected static tuple is distinct from the callee's retained tuple, so
/// changing either side alone rejects. Both remain producer-trusted source
/// projections; matching them does not authenticate original source arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedReachCallApplication {
    pub callee: MachineId,
    pub specialization_commitment: [u8; 32],
    pub arguments: Vec<ClosedReachArgument>,
}

/// Only argument selection, without duplicating requirement rows or call graphs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedReachArgument {
    Type(TerminalIdentityIndex),
    Const(TerminalIdentityIndex),
    Proposition(TerminalIdentityIndex),
    Machine {
        identity: TerminalIdentityIndex,
        contract_commitment: [u8; 32],
    },
}

impl ClosedReachArgument {
    /// Compare closed coordinates across artifact-local identity tables. The
    /// argument kind and a machine argument's contract remain independent of
    /// its readable identity; equal indices alone establish nothing.
    pub fn same_identity_as(
        &self,
        identities: &crate::TerminalIdentityTable,
        other: &Self,
        other_identities: &crate::TerminalIdentityTable,
    ) -> bool {
        let (left, right) = match (self, other) {
            (Self::Type(left), Self::Type(right))
            | (Self::Const(left), Self::Const(right))
            | (Self::Proposition(left), Self::Proposition(right)) => (*left, *right),
            (
                Self::Machine {
                    identity: left,
                    contract_commitment: left_contract,
                },
                Self::Machine {
                    identity: right,
                    contract_commitment: right_contract,
                },
            ) if left_contract == right_contract => (*left, *right),
            _ => return false,
        };
        identities
            .text(left)
            .zip(other_identities.text(right))
            .is_some_and(|(left, right)| !left.is_empty() && left == right)
    }
}
