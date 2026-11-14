/*
We maintain verifier-admitted proof custody for an optimization unit:
`AcceptedObligationFact`, `RuntimeEvidenceCatalog`, and the
one-time attachment routines that bind verified Terminal module context and
accepted obligation facts into `PsiOptimizationUnit`. Equal propositions at
distinct source sites keep distinct positional owners so optimizer rewrites
cannot treat site-specific proof authority as interchangeable.
*/

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

use super::{AcceptedObligationFactIdentity, ObligationId};
use super::{MachineId, PsiProvenance, TerminalPsiIdentity};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedObligationFact {
    pub identity: AcceptedObligationFactIdentity,
    pub psi: TerminalPsiIdentity,
    pub proof_bundle_fingerprint: [u8; 32],
    pub machine: MachineId,
    pub owner: PsiProvenance,
    pub obligation: ObligationId,
    pub proposition: Vec<u8>,
    pub scalar_receiving: Option<SourceCheckedScalarReceiving>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCheckedScalarReceiving {
    pub value: semantic_vocabulary::ValueId,
    pub restriction: terminal_psi::StructuralFieldType,
}

impl super::PsiOptimizationUnit {
    pub fn accepted_obligation_fact(
        &self,
        machine: MachineId,
        owner: PsiProvenance,
        obligation: Option<ObligationId>,
    ) -> Option<&AcceptedObligationFact> {
        self.accepted_obligation_facts.iter().find(|fact| {
            fact.machine == machine && fact.owner == owner && Some(fact.obligation) == obligation
        })
    }
}

impl AcceptedObligationFact {
    pub fn new(
        psi: TerminalPsiIdentity,
        proof_bundle_fingerprint: [u8; 32],
        machine: MachineId,
        owner: PsiProvenance,
        obligation: ObligationId,
        proposition: Vec<u8>,
    ) -> Self {
        let identity = accepted_obligation_fact_identity(
            psi,
            proof_bundle_fingerprint,
            machine,
            owner,
            obligation,
            &proposition,
            None,
        );
        Self {
            identity,
            psi,
            proof_bundle_fingerprint,
            machine,
            owner,
            obligation,
            proposition,
            scalar_receiving: None,
        }
    }

    pub fn with_scalar_receiving(mut self, receiving: SourceCheckedScalarReceiving) -> Self {
        self.scalar_receiving = Some(receiving);
        self.identity = accepted_obligation_fact_identity(
            self.psi,
            self.proof_bundle_fingerprint,
            self.machine,
            self.owner,
            self.obligation,
            &self.proposition,
            self.scalar_receiving.as_ref(),
        );
        self
    }

    pub fn has_canonical_identity(&self) -> bool {
        self.identity
            == accepted_obligation_fact_identity(
                self.psi,
                self.proof_bundle_fingerprint,
                self.machine,
                self.owner,
                self.obligation,
                &self.proposition,
                self.scalar_receiving.as_ref(),
            )
    }
}

pub(crate) fn accepted_obligation_fact_identity(
    psi: TerminalPsiIdentity,
    proof_bundle_fingerprint: [u8; 32],
    machine: MachineId,
    owner: PsiProvenance,
    obligation: ObligationId,
    proposition: &[u8],
    scalar_receiving: Option<&SourceCheckedScalarReceiving>,
) -> AcceptedObligationFactIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-accepted-obligation-fact.v2\0");
    canonical.extend_from_slice(psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut canonical, psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(&proof_bundle_fingerprint);
    crate::codec_bytes::push_encoded(&mut canonical, machine.get().to_le_bytes());
    match owner {
        PsiProvenance::Operation(operation) => {
            canonical.push(0);
            crate::codec_bytes::push_encoded(&mut canonical, operation.get().to_le_bytes());
        }
        PsiProvenance::Edge(edge) => {
            canonical.push(1);
            crate::codec_bytes::push_encoded(&mut canonical, edge.get().to_le_bytes());
        }
    }
    crate::codec_bytes::push_encoded(&mut canonical, obligation.get().to_le_bytes());
    canonical.extend_from_slice(
        &u64::try_from(proposition.len())
            .expect("canonical proposition length fits u64")
            .to_le_bytes(),
    );
    canonical.extend_from_slice(proposition);
    if let Some(receiving) = scalar_receiving {
        canonical.extend_from_slice(b"source-checked-scalar-receiving.v1\0");
        canonical.extend_from_slice(&super::identity::scalar_receiving_identity_bytes(receiving));
    }
    AcceptedObligationFactIdentity::from_canonical_bytes(&canonical)
}

pub fn attach_verified_module_context(
    unit: &mut super::PsiOptimizationUnit,
    module: &terminal_psi::TerminalModule,
) -> Result<(), MachineId> {
    unit.runtime_evidence = Some(std::sync::Arc::new(
        RuntimeEvidenceCatalog::from_verified_module(unit.psi, module),
    ));
    unit.structural_domains = module.structural_domains.clone().into();
    unit.services = module.services.clone().into();
    unit.root_service_reach = module.root_service_reach.clone();
    for function in &mut unit.functions {
        let source = module.machine(function.machine).ok_or(function.machine)?;
        function.structural_places = source.structural_places.clone();
        function.content_entry_claims = source.content_entry_claims.clone();
        function.verified_contract = Some(source.contract.clone());
        function.evidence_contract_lanes = module
            .evidence_contract_lanes
            .iter()
            .filter(|lane| lane.machine == function.machine)
            .cloned()
            .collect();
        function.declared_service_reach = source.declared_service_reach.clone().unwrap_or_default();
    }
    unit.identity = super::recompute_psi_optimization_unit_identity(unit);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptedObligationFactIndexError {
    AlreadyAttached,
    TerminalIdentityMismatch,
    InvalidFactIdentity,
    DuplicateOwner,
}

optimization_core::invalid_error_display!(
    AcceptedObligationFactIndexError,
    "accepted obligation fact index"
);

pub fn attach_accepted_obligation_facts(
    mut unit: super::PsiOptimizationUnit,
    mut facts: Vec<AcceptedObligationFact>,
) -> Result<super::PsiOptimizationUnit, AcceptedObligationFactIndexError> {
    if !unit.accepted_obligation_facts.is_empty() {
        return Err(AcceptedObligationFactIndexError::AlreadyAttached);
    }
    if facts.iter().any(|fact| fact.psi != unit.psi) {
        return Err(AcceptedObligationFactIndexError::TerminalIdentityMismatch);
    }
    if facts.iter().any(|fact| !fact.has_canonical_identity()) {
        return Err(AcceptedObligationFactIndexError::InvalidFactIdentity);
    }
    facts.sort_by_key(|fact| (fact.machine, fact.owner, fact.obligation));
    if facts.windows(2).any(|pair| {
        (pair[0].machine, pair[0].owner, pair[0].obligation)
            == (pair[1].machine, pair[1].owner, pair[1].obligation)
    }) {
        return Err(AcceptedObligationFactIndexError::DuplicateOwner);
    }
    unit.accepted_obligation_facts = facts;
    unit.identity = super::recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}
