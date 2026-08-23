use psi_checked_trees::{ContractProofFactKind, ProofFactKind};
use psi_facts::{
    ContractFactKind as SemanticContractFactKind,
    ProofObligationKind as SemanticProofObligationKind,
};

pub(crate) fn semantic_contract_fact_kind(kind: ContractProofFactKind) -> SemanticContractFactKind {
    match kind {
        ContractProofFactKind::Requires => SemanticContractFactKind::Requires,
        ContractProofFactKind::Ensures => SemanticContractFactKind::Ensures,
        ContractProofFactKind::Boundary => SemanticContractFactKind::Boundary,
    }
}

pub(crate) fn semantic_proof_obligation_kind(kind: ProofFactKind) -> SemanticProofObligationKind {
    match kind {
        ProofFactKind::BoundedAssignment => SemanticProofObligationKind::BoundedAssignment,
        ProofFactKind::BoundedCallArgument => SemanticProofObligationKind::BoundedCallArgument,
        ProofFactKind::BoundedInitializer => SemanticProofObligationKind::BoundedInitializer,
        ProofFactKind::BoundedStateReturn => SemanticProofObligationKind::BoundedStateReturn,
        ProofFactKind::BoundedValue => SemanticProofObligationKind::BoundedValue,
        ProofFactKind::BoundedTransitionArgument => {
            SemanticProofObligationKind::BoundedTransitionArgument
        }
        ProofFactKind::GuardedTransition => SemanticProofObligationKind::GuardedTransition,
    }
}
