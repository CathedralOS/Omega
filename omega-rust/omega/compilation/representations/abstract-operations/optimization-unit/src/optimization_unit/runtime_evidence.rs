//! Verified declaration custody for replay after executable transformations.
//! This catalog is not an acceptance receipt: current operand snapshots,
//! completion and validity are independently reconstructed on every replay.
use semantic_vocabulary::MachineId;
use terminal_psi::TerminalPsiIdentity;
use terminal_psi::{
    CheckedMachineDeclaration, EvidenceContractLane, EvidenceTermDeclaration, ProofOutputCall,
    PropositionApplicationIdentity, TerminalModule,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEvidenceCatalog {
    psi: TerminalPsiIdentity,
    declarations: Vec<(MachineId, Option<CheckedMachineDeclaration>)>,
    applications: Vec<PropositionApplicationIdentity>,
    terms: Vec<EvidenceTermDeclaration>,
    lanes: Vec<EvidenceContractLane>,
    calls: Vec<ProofOutputCall>,
}
impl RuntimeEvidenceCatalog {
    pub(crate) fn from_verified_module(psi: TerminalPsiIdentity, module: &TerminalModule) -> Self {
        Self {
            psi,
            declarations: module
                .machines
                .iter()
                .map(|machine| (machine.id, machine.declaration.clone()))
                .collect(),
            applications: module.proposition_applications.clone(),
            terms: module.evidence_terms.clone(),
            lanes: module.evidence_contract_lanes.clone(),
            calls: module.proof_output_calls.clone(),
        }
    }
    pub fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }
    pub fn declarations(&self) -> &[(MachineId, Option<CheckedMachineDeclaration>)] {
        &self.declarations
    }
    pub fn applications(&self) -> &[PropositionApplicationIdentity] {
        &self.applications
    }
    pub fn terms(&self) -> &[EvidenceTermDeclaration] {
        &self.terms
    }
    pub fn lanes(&self) -> &[EvidenceContractLane] {
        &self.lanes
    }
    pub fn calls(&self) -> &[ProofOutputCall] {
        &self.calls
    }
}

/// Compare retained custody with the independently owned verified input.
/// Identity hashing alone cannot authorize deleting or replacing this catalog.
/// The optimizer session invokes this for initial, transformed and identity
/// paths using its immutable VerifiedPsiOptimizationInput, not caller labels.
/// Bare reconstruction seeds have no such authorization and correctly refuse.
pub fn validate_runtime_evidence_context(
    unit: &super::PsiOptimizationUnit,
    expected: TerminalPsiIdentity,
    module: &TerminalModule,
) -> Result<(), &'static str> {
    if unit.psi != expected || unit.identities != module.identities {
        return Err("optimizer runtime evidence context belongs to another verified input");
    }
    let catalog = unit
        .runtime_evidence
        .as_ref()
        .ok_or("verified optimizer runtime evidence catalog is absent")?;
    if catalog.psi != expected
        || !catalog
            .declarations
            .iter()
            .map(|(machine, declaration)| (*machine, declaration))
            .eq(module
                .machines
                .iter()
                .map(|machine| (machine.id, &machine.declaration)))
        || catalog.applications != module.proposition_applications
        || catalog.terms != module.evidence_terms
        || catalog.lanes != module.evidence_contract_lanes
        || catalog.calls != module.proof_output_calls
    {
        return Err("optimizer runtime evidence catalog differs from admitted Terminal source");
    }
    for function in &unit.functions {
        let source = module
            .machine(function.machine)
            .ok_or("runtime evidence function has no verified source declaration")?;
        if function.verified_contract.as_ref() != Some(&source.contract)
            || !function.evidence_contract_lanes.iter().eq(module
                .evidence_contract_lanes
                .iter()
                .filter(|lane| lane.machine == function.machine))
        {
            return Err("optimizer changed verified evidence contract custody");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
