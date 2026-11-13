//! Checked callable contracts and the closed machine values constructed from them.
//!
//! Contracts describe calls independently of their selected bodies. Descriptors
//! retain exact selections; this catalog does not enumerate eligible machines.

use crate::{
    BoundaryMachineResult, BoundaryParameterKind, MachineContract, StructuralParameterDeclaration,
    StructuralPathSegment, TerminalDynamicClaimPath, TerminalDynamicReferenceRegion,
    TerminalIdentityIndex, ValueDeclaration,
};
use semantic_vocabulary::{
    CallableContractId, MachineDescriptorId, MachineId, ObligationId, ServiceId, StructuralDomainId,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalCallableCatalog {
    pub contracts: Vec<TerminalCallableContract>,
    pub descriptors: Vec<TerminalMachineDescriptor>,
    pub machine_envelopes: Vec<Option<TerminalMachineEnvelope>>,
}

/// Named declarations publish their own operational contract independently of
/// the structural callable contracts to which a value may be assigned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachineEnvelope {
    pub machine: MachineId,
    pub envelope: TerminalCallableEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallableContract {
    pub id: CallableContractId,
    pub identity: TerminalIdentityIndex,
    pub scalar_parameters: Vec<ValueDeclaration>,
    pub parameter_order: Vec<BoundaryParameterKind>,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub result: BoundaryMachineResult,
    pub proof_result: crate::TerminalMachineResult,
    pub entry_claim_paths: Vec<TerminalDynamicClaimPath>,
    pub result_claim_paths: Vec<Vec<StructuralPathSegment>>,
    pub result_reference_regions: Vec<TerminalDynamicReferenceRegion>,
    pub proof: MachineContract,
    pub evidence_contract: Vec<TerminalCallableEvidenceLane>,
    pub envelope: TerminalCallableEnvelope,
}

/// The public evidence contract retains its complete checked proposition and
/// formal subjects independently of the module's supplied lane table.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TerminalCallableEvidenceLane {
    pub kind: crate::EvidenceContractLaneKind,
    pub position: u32,
    pub declaration: crate::PropositionDeclaration,
    pub application: crate::PropositionApplicationIdentity,
    pub term: crate::EvidenceTermDeclaration,
    pub argument_subjects: Vec<crate::EvidenceArgumentSubject>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachineDescriptor {
    pub id: MachineDescriptorId,
    pub contract: CallableContractId,
    pub machine: MachineId,
    pub origin: TerminalMachineDescriptorOrigin,
    pub refinement_obligations: Vec<ObligationId>,
    pub refinement_evidence: Vec<crate::CertificateEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachineDescriptorOrigin {
    pub application: TerminalIdentityIndex,
    pub conformance: Option<TerminalIdentityIndex>,
    pub requirement: Option<TerminalIdentityIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallableEnvelope {
    pub service_reach: Vec<ServiceId>,
    pub invocations: Vec<TerminalCallableInvocation>,
    pub suspends: bool,
    pub blocks: bool,
    pub termination: TerminalCallableTermination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalCallableInvocation {
    Parameter(u32),
    Service(ServiceId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalCallableTermination {
    NotGuaranteed,
    Terminates {
        premises: Vec<TerminalCallableTerminationPremise>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallableTerminationPremise {
    pub parameter: u32,
    pub path: Vec<StructuralPathSegment>,
    pub domain: StructuralDomainId,
}

impl TerminalCallableCatalog {
    pub fn validate_scalar_type(
        &self,
        scalar: semantic_vocabulary::ScalarType,
    ) -> Result<(), &'static str> {
        if let semantic_vocabulary::ScalarType::Callable(contract) = scalar
            && self.contract(contract).is_none()
        {
            return Err("callable scalar type names an absent or stale contract");
        }
        Ok(())
    }

    pub fn contract(&self, id: CallableContractId) -> Option<&TerminalCallableContract> {
        let ordinal = usize::try_from(id.get().checked_sub(1)?).ok()?;
        self.contracts
            .get(ordinal)
            .filter(|contract| contract.id == id)
    }

    pub fn machine_envelope(&self, machine: MachineId) -> Option<&TerminalCallableEnvelope> {
        let ordinal = usize::try_from(machine.get().checked_sub(1)?).ok()?;
        self.machine_envelopes
            .get(ordinal)?
            .as_ref()
            .filter(|row| row.machine == machine)
            .map(|row| &row.envelope)
    }

    pub fn descriptor(&self, id: MachineDescriptorId) -> Option<&TerminalMachineDescriptor> {
        let ordinal = usize::try_from(id.get().checked_sub(1)?).ok()?;
        self.descriptors
            .get(ordinal)
            .filter(|descriptor| descriptor.id == id)
    }
}

impl crate::ReferenceResultShape for TerminalCallableContract {
    fn result(&self) -> &BoundaryMachineResult {
        &self.result
    }

    fn reference_regions(&self) -> &[TerminalDynamicReferenceRegion] {
        &self.result_reference_regions
    }

    fn structural_parameter_count(&self) -> usize {
        self.structural_parameters.len()
    }

    fn structural_parameter(
        &self,
        ordinal: usize,
    ) -> Option<(
        semantic_vocabulary::StructuralTypeId,
        crate::StructuralAccess,
    )> {
        self.structural_parameters
            .get(ordinal)
            .map(|parameter| (parameter.structural_type, parameter.access))
    }
}
