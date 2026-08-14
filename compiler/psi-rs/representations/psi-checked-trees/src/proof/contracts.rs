use psi_arena::{Handle, HandleSpan};
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;

use crate::CheckedValueOrigin;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContractProofFactKind {
    #[default]
    Requires,
    Ensures,
    Boundary,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContractProofFactOwner {
    #[default]
    Unknown,
    Machine {
        machine_symbol: SymbolHandle,
    },
    MachineState {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    StateSignature {
        owner_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    OperatorUse {
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        operator_symbol: SymbolHandle,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BoundaryQualificationAuthorization {
    /// The boundary trait that owns the admitted requirement.
    pub requirement_symbol: SymbolHandle,
    /// The exact requirement signature whose result is qualified.
    pub signature_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContractProofFact {
    pub kind: ContractProofFactKind,
    pub owner: ContractProofFactOwner,
    pub fact: Handle<psi_typed_trees::domain::ProofFact>,
    /// Exact erased evidence term declared by a named machine contract.
    /// The arena handle is term identity; proposition identity and eventual
    /// producer provenance remain separate records.
    pub evidence_term: Option<Handle<CheckedEvidenceTerm>>,
    /// Present only for an exact `ensures result in Domain` fact published by
    /// a boundary requirement whose result carrier matches the domain target.
    pub qualification_authorization: Option<BoundaryQualificationAuthorization>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedEvidenceTerm {
    /// Public output-field name for `ensures`, local input alias for `requires`.
    pub name: String,
    pub owner: ContractProofFactOwner,
    pub kind: ContractProofFactKind,
    /// Position within the matching erased requires/ensures lane.
    pub lane_position: usize,
    /// Exact normalized proposition application inhabited by this term.
    pub proposition: crate::CheckedPropositionApplication,
    /// Canonical carrierless interface retained by the proposition endpoint.
    pub evidence_type: String,
}

impl Default for CheckedEvidenceTerm {
    fn default() -> Self {
        Self {
            name: String::new(),
            owner: ContractProofFactOwner::Unknown,
            kind: ContractProofFactKind::Requires,
            lane_position: 0,
            proposition: crate::CheckedPropositionApplication {
                declaration: SymbolHandle::invalid(),
                binder_arguments: Vec::new(),
                arguments: Vec::new(),
            },
            evidence_type: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContractProofFactRef {
    pub fact: Handle<ContractProofFact>,
}

/// One explicit erased call-lane binding. Both ends are exact checked term
/// identities; position is semantic and source names are never matched to
/// callee names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContractEvidenceArgument {
    pub source: Handle<CheckedEvidenceTerm>,
    pub parameter: Handle<CheckedEvidenceTerm>,
    pub lane_position: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractCallFact {
    pub caller_machine_symbol: SymbolHandle,
    pub caller_state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_machine_symbol: SymbolHandle,
    pub target_state_symbol: SymbolHandle,
    pub requires: HandleSpan<ContractProofFactRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
    pub evidence_arguments: HandleSpan<ContractEvidenceArgument>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractExitFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub ensures: HandleSpan<ContractProofFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractOperatorUseFact {
    pub expression: ExpressionHandle,
    pub origin: CheckedValueOrigin,
    pub operator_symbol: SymbolHandle,
    pub requires: HandleSpan<ContractProofFactRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
    pub boundary: HandleSpan<ContractProofFactRef>,
}
