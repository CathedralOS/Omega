use crate::ClosedConformanceApplicationCommitment;
use crate::terminal_module::TerminalIdentityIndex;
use semantic_vocabulary::{EvidenceTermId, MachineId, PropositionId, ScalarType};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutputCall {
    pub caller: MachineId,
    pub ordinal: u32,
    pub target_identity: TerminalIdentityIndex,
    pub static_requirement_dispatch: Option<StaticRequirementDispatch>,
    pub runtime_result: Option<ProofOutputRuntimeResult>,
    pub runtime_call: Option<ProofOutputRuntimeCall>,
    pub evidence_arguments: Vec<ProofOutputEvidenceArgument>,
    pub outputs: Vec<ProofOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StaticRequirementDispatch {
    pub conformance_application_report_fingerprint: u64,
    pub conformance_application_commitment: ClosedConformanceApplicationCommitment,
    pub public_requirement_identity: TerminalIdentityIndex,
    pub declaring_trait_identity: TerminalIdentityIndex,
    pub requirement_identity: TerminalIdentityIndex,
    pub realization_identity: TerminalIdentityIndex,
    pub realization_callable_identity: TerminalIdentityIndex,
    pub realization: MachineId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutputEvidenceArgument {
    pub input_position: u32,
    pub callee_proposition: PropositionId,
    pub source: EvidenceTermId,
    pub instantiated_proposition: PropositionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofOutputRuntimeResult {
    Unit,
    Scalar(ScalarType),
    Structural(semantic_vocabulary::StructuralTypeId),
    NoNormal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceCallableTarget {
    Machine(MachineId),
    Runtime {
        contract: semantic_vocabulary::CallableContractId,
        callee: semantic_vocabulary::ValueId,
    },
}

impl EvidenceCallableTarget {
    pub const fn owner(self) -> super::declarations::EvidenceCallableOwner {
        match self {
            Self::Machine(machine) => {
                super::declarations::EvidenceCallableOwner::Machine(machine)
            }
            Self::Runtime { contract, .. } => {
                super::declarations::EvidenceCallableOwner::Runtime(contract)
            }
        }
    }
    pub const fn machine(self) -> Option<MachineId> {
        match self {
            Self::Machine(machine) => Some(machine),
            Self::Runtime { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutputRuntimeCall {
    pub site: semantic_vocabulary::InvocationSite,
    pub target: EvidenceCallableTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutput {
    pub output_position: u32,
    pub output_field: String,
    pub callee_proposition: PropositionId,
    pub callee_output: Option<EvidenceTermId>,
    pub instantiated_proposition: PropositionId,
    pub forwarded_input_position: Option<u32>,
    pub output: Option<EvidenceTermId>,
}
