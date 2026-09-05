//! Accepted-obligation and proof-question custody with canonical identities.

use super::*;

/// An admitted proof fact projected from the immutable verifier context.
///
/// The row binds both semantic artifact identities and the exact operation
/// owner. It remains attached after a rewrite removes that operation so the
/// transformation ledger and manifest can retain proof custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedObligationFact {
    pub identity: AcceptedObligationFactIdentity,
    pub psi: TerminalPsiIdentity,
    pub proof_bundle_fingerprint: [u8; 32],
    pub machine: MachineId,
    pub operation: OperationId,
    pub obligation: ObligationId,
    pub proposition: Vec<u8>,
}

/// Exact verifier owner of one retained proof question. Positional coordinates
/// are semantic: they prevent equal propositions at distinct source sites from
/// becoming interchangeable optimizer authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofQuestionOwner {
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
            Self::Operation { machine, .. }
            | Self::CallRequires { machine, .. }
            | Self::NominalCleanupRequires { machine, .. }
            | Self::ContractEnsures { machine, .. } => machine,
        }
    }
}

/// Source-independent mirror of the proof-admission classification retained
/// at optimizer admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofQuestionAdmissionKind {
    ForeignBoundaryGuarantee,
    ProviderFact,
    CheckedAssemblyClaim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofQuestionClass {
    Derivable,
    AdmissionAuthorized {
        site: AdmissionSiteId,
        kind: ProofQuestionAdmissionKind,
        authority_identity: EvidenceIdentity,
    },
}

/// Immutable, complete proof question projected one-for-one from Terminal
/// verification. Canonical proposition bytes retain exact ordered premises and
/// axioms without coupling this target-neutral representation to a prover.
/// Rewrites preserve the entire catalog, including rows owned by pruned code.
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
        operation: OperationId,
        obligation: ObligationId,
        proposition: Vec<u8>,
    ) -> Self {
        let identity = accepted_obligation_fact_identity(
            psi,
            proof_bundle_fingerprint,
            machine,
            operation,
            obligation,
            &proposition,
        );
        Self {
            identity,
            psi,
            proof_bundle_fingerprint,
            machine,
            operation,
            obligation,
            proposition,
        }
    }

    pub fn has_canonical_identity(&self) -> bool {
        self.identity
            == accepted_obligation_fact_identity(
                self.psi,
                self.proof_bundle_fingerprint,
                self.machine,
                self.operation,
                self.obligation,
                &self.proposition,
            )
    }
}

pub fn accepted_obligation_fact_identity(
    psi: TerminalPsiIdentity,
    proof_bundle_fingerprint: [u8; 32],
    machine: MachineId,
    operation: OperationId,
    obligation: ObligationId,
    proposition: &[u8],
) -> AcceptedObligationFactIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-accepted-obligation-fact.v1\0");
    canonical.extend_from_slice(psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(&proof_bundle_fingerprint);
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&operation.get().to_le_bytes());
    canonical.extend_from_slice(&obligation.get().to_le_bytes());
    canonical.extend_from_slice(
        &u64::try_from(proposition.len())
            .expect("canonical proposition length fits u64")
            .to_le_bytes(),
    );
    canonical.extend_from_slice(proposition);
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
pub fn proof_question_identity(
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
    canonical.extend_from_slice(b"omega.psi-proof-question.v1\0");
    canonical.extend_from_slice(terminal_psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&terminal_psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(&proof_bundle_fingerprint);
    encode_proof_question_owner(&mut canonical, owner);
    canonical.extend_from_slice(&obligation.get().to_le_bytes());
    encode_proof_question_class(&mut canonical, class);
    encode_proof_question_bytes(&mut canonical, proposition);
    encode_proof_question_byte_rows(&mut canonical, requirements);
    encode_proof_question_byte_rows(&mut canonical, semantic_axioms);
    canonical.push(u8::from(canonical_certificate));
    ProofQuestionIdentity::from_canonical_bytes(&canonical)
}

fn encode_proof_question_owner(bytes: &mut Vec<u8>, owner: ProofQuestionOwner) {
    match owner {
        ProofQuestionOwner::Operation { machine, operation } => {
            bytes.push(1);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&operation.get().to_le_bytes());
        }
        ProofQuestionOwner::CallRequires {
            machine,
            operation,
            requirement_position,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&operation.get().to_le_bytes());
            bytes.extend_from_slice(&requirement_position.to_le_bytes());
        }
        ProofQuestionOwner::NominalCleanupRequires {
            machine,
            edge,
            cleanup_position,
            requirement_position,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&edge.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup_position.to_le_bytes());
            bytes.extend_from_slice(&requirement_position.to_le_bytes());
        }
        ProofQuestionOwner::ContractEnsures {
            machine,
            contract,
            clause_position,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&contract.get().to_le_bytes());
            bytes.extend_from_slice(&clause_position.to_le_bytes());
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
            bytes.extend_from_slice(&site.get().to_le_bytes());
            bytes.push(match kind {
                ProofQuestionAdmissionKind::ForeignBoundaryGuarantee => 1,
                ProofQuestionAdmissionKind::ProviderFact => 2,
                ProofQuestionAdmissionKind::CheckedAssemblyClaim => 3,
            });
            bytes.extend_from_slice(&authority_identity.get().to_le_bytes());
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
