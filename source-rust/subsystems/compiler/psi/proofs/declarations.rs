use crate::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramProofs {
    pub theorem_parameters: Vec<TheoremParameter>,
    pub theorem_applications: Vec<TheoremApplication>,
    pub theorem_citations: Vec<TheoremCitation>,
    pub indexed_theorems: Vec<TerminalIndexedTheorem>,
    pub proposition_declarations: Vec<PropositionDeclaration>,
    pub proposition_applications: Vec<PropositionApplicationIdentity>,
    pub evidence_terms: Vec<EvidenceTermDeclaration>,
    pub evidence_contract_lanes: Vec<EvidenceContractLane>,
    pub proof_output_calls: Vec<ProofOutputCall>,
    pub proof_recursive_components: Vec<TerminalProofRecursiveComponent>,
    pub scalar_block_invariants: Vec<ScalarBlockInvariant>,
    pub operation_crash_contracts: Vec<TerminalOperationCrashContract>,
    pub closed_conformance_applications: Vec<ClosedConformanceApplication>,
}
