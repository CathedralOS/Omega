/*
We maintain verifier-admitted proof custody for an optimization unit:
`AcceptedObligationFact`, `ProofQuestion`, `RuntimeEvidenceCatalog`, and the
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

use super::{
    AcceptedObligationFactIdentity, AdmissionSiteId, BlockId, ContractId, EdgeId, EvidenceIdentity,
    ObligationId, OperationId, ProofQuestionIdentity,
};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofQuestionOwner {
    ScalarBlockInvariant {
        machine: MachineId,
        header: BlockId,
        edge: EdgeId,
    },
    Operation {
        machine: MachineId,
        operation: OperationId,
    },
    CallRequires {
        machine: MachineId,
        operation: OperationId,
        requirement_position: u32,
    },
    NominalCleanupRequires {
        machine: MachineId,
        edge: EdgeId,
        cleanup_position: u32,
        requirement_position: u32,
    },
    ContractEnsures {
        machine: MachineId,
        contract: ContractId,
        clause_position: u32,
    },
}

impl ProofQuestionOwner {
    pub const fn machine(self) -> MachineId {
        match self {
            Self::ScalarBlockInvariant { machine, .. }
            | Self::Operation { machine, .. }
            | Self::CallRequires { machine, .. }
            | Self::NominalCleanupRequires { machine, .. }
            | Self::ContractEnsures { machine, .. } => machine,
        }
    }
}

pub type ProofQuestionAdmissionKind = terminal_psi::AdmissionKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofQuestionClass {
    Derivable,
    AdmissionAuthorized {
        site: AdmissionSiteId,
        kind: ProofQuestionAdmissionKind,
        authority_identity: EvidenceIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofQuestion {
    pub identity: ProofQuestionIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub proof_bundle_fingerprint: [u8; 32],
    pub owner: ProofQuestionOwner,
    pub obligation: ObligationId,
    pub class: ProofQuestionClass,
    pub proposition: Vec<u8>,
    pub requirements: Vec<Vec<u8>>,
    pub semantic_axioms: Vec<Vec<u8>>,
    pub canonical_certificate: bool,
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

impl ProofQuestion {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        terminal_psi: TerminalPsiIdentity,
        proof_bundle_fingerprint: [u8; 32],
        owner: ProofQuestionOwner,
        obligation: ObligationId,
        class: ProofQuestionClass,
        proposition: Vec<u8>,
        requirements: Vec<Vec<u8>>,
        semantic_axioms: Vec<Vec<u8>>,
        canonical_certificate: bool,
    ) -> Self {
        let identity = proof_question_identity(
            terminal_psi,
            proof_bundle_fingerprint,
            owner,
            obligation,
            class,
            &proposition,
            &requirements,
            &semantic_axioms,
            canonical_certificate,
        );
        Self {
            identity,
            terminal_psi,
            proof_bundle_fingerprint,
            owner,
            obligation,
            class,
            proposition,
            requirements,
            semantic_axioms,
            canonical_certificate,
        }
    }

    pub fn has_canonical_identity(&self) -> bool {
        self.identity
            == proof_question_identity(
                self.terminal_psi,
                self.proof_bundle_fingerprint,
                self.owner,
                self.obligation,
                self.class,
                &self.proposition,
                &self.requirements,
                &self.semantic_axioms,
                self.canonical_certificate,
            )
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn proof_question_identity(
    terminal_psi: TerminalPsiIdentity,
    proof_bundle_fingerprint: [u8; 32],
    owner: ProofQuestionOwner,
    obligation: ObligationId,
    class: ProofQuestionClass,
    proposition: &[u8],
    requirements: &[Vec<u8>],
    semantic_axioms: &[Vec<u8>],
    canonical_certificate: bool,
) -> ProofQuestionIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-proof-question.v2\0");
    canonical.extend_from_slice(terminal_psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(
        &mut canonical,
        terminal_psi.vocabulary_marker.get().to_le_bytes(),
    );
    canonical.extend_from_slice(&proof_bundle_fingerprint);
    encode_proof_question_owner(&mut canonical, owner);
    crate::codec_bytes::push_encoded(&mut canonical, obligation.get().to_le_bytes());
    encode_proof_question_class(&mut canonical, class);
    encode_proof_question_bytes(&mut canonical, proposition);
    encode_proof_question_byte_rows(&mut canonical, requirements);
    encode_proof_question_byte_rows(&mut canonical, semantic_axioms);
    canonical.push(u8::from(canonical_certificate));
    ProofQuestionIdentity::from_canonical_bytes(&canonical)
}

fn encode_proof_question_owner(bytes: &mut Vec<u8>, owner: ProofQuestionOwner) {
    match owner {
        ProofQuestionOwner::ScalarBlockInvariant {
            machine,
            header,
            edge,
        } => {
            bytes.push(5);
            crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, header.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
        }
        ProofQuestionOwner::Operation { machine, operation } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
        }
        ProofQuestionOwner::CallRequires {
            machine,
            operation,
            requirement_position,
        } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, requirement_position.to_le_bytes());
        }
        ProofQuestionOwner::NominalCleanupRequires {
            machine,
            edge,
            cleanup_position,
            requirement_position,
        } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, cleanup_position.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, requirement_position.to_le_bytes());
        }
        ProofQuestionOwner::ContractEnsures {
            machine,
            contract,
            clause_position,
        } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, contract.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, clause_position.to_le_bytes());
        }
    }
}

fn encode_proof_question_class(bytes: &mut Vec<u8>, class: ProofQuestionClass) {
    match class {
        ProofQuestionClass::Derivable => bytes.push(1),
        ProofQuestionClass::AdmissionAuthorized {
            site,
            kind,
            authority_identity,
        } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, site.get().to_le_bytes());
            bytes.push(match kind {
                ProofQuestionAdmissionKind::ForeignBoundaryGuarantee => 1,
                ProofQuestionAdmissionKind::ProviderFact => 2,
                ProofQuestionAdmissionKind::CheckedAssemblyClaim => 3,
            });
            crate::codec_bytes::push_encoded(bytes, authority_identity.get().to_le_bytes());
        }
    }
}

fn encode_proof_question_byte_rows(bytes: &mut Vec<u8>, rows: &[Vec<u8>]) {
    bytes.extend_from_slice(
        &u64::try_from(rows.len())
            .expect("canonical proof-question row count fits u64")
            .to_le_bytes(),
    );
    for row in rows {
        encode_proof_question_bytes(bytes, row);
    }
}

fn encode_proof_question_bytes(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(
        &u64::try_from(value.len())
            .expect("canonical proof-question byte length fits u64")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(value);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofQuestionIndexError {
    AlreadyAttached,
    TerminalIdentityMismatch,
    InvalidQuestionIdentity,
    DuplicateQuestion,
}

optimization_core::invalid_error_display!(ProofQuestionIndexError, "proof-question index");
