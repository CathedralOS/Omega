//! Canonical replay ledger for verifier-reconstructed Terminal obligations.
//!
//! The ledger binds the exact Terminal semantic subject and reconstruction
//! trust graph. It deliberately excludes proof routes: different valid
//! certificates may discharge the same reconstructed question.

use std::collections::BTreeSet;

use psi_core::{AdmissionSiteId, ContractId, EdgeId, EvidenceIdentity, MachineId, OperationId};
use psi_proof_admission::{AdmissionKind, AuthorizedAdmission, Obligation, ObligationClass};
use psi_terminal::{SemanticFingerprint, TerminalModule, TerminalPsiIdentity, VocabularyMarker};
use psi_terminal_verifier::{
    ReconstructedTerminalObligation, ReconstructedTerminalObligationOwner,
    reconstruct_interpretable_terminal_obligations, reconstruct_terminal_obligations,
    validate_module_for_interpretation,
};
use sha2::{Digest, Sha256};

use super::proposition_wire::{decode_proposition, encode_proposition};
use super::trust_graph::{TerminalTrustGraphIdentity, ValidatedTerminalTrustGraph};
use super::wire::{Reader, Writer};
use super::{CodecError, decode_counted, terminal_psi_identity};

const MAGIC: &[u8; 8] = b"PSIOBLG\0";
const FORMAT_MARKER: u16 = 1;
const FINGERPRINT_DOMAIN: &[u8] = b"psi-terminal-obligation-ledger-fingerprint\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObligationLedger {
    terminal_psi: TerminalPsiIdentity,
    trust_graph: TerminalTrustGraphIdentity,
    obligations: Vec<ReconstructedTerminalObligation>,
}

impl TerminalObligationLedger {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn trust_graph(&self) -> TerminalTrustGraphIdentity {
        self.trust_graph
    }

    pub fn obligations(&self) -> &[ReconstructedTerminalObligation] {
        &self.obligations
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalObligationLedgerFingerprint([u8; 32]);

impl TerminalObligationLedgerFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for TerminalObligationLedgerFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for TerminalObligationLedgerFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

pub fn build_terminal_obligation_ledger(
    module: &TerminalModule,
    trust_graph: &ValidatedTerminalTrustGraph,
) -> Result<TerminalObligationLedger, CodecError> {
    let terminal_psi = terminal_psi_identity(module)?;
    let has_ranked_scc = module
        .machines
        .iter()
        .any(|machine| machine.ranked_scc.is_some());
    let obligations = if has_ranked_scc {
        let interpreted =
            validate_module_for_interpretation(module).map_err(CodecError::InvalidModule)?;
        reconstruct_interpretable_terminal_obligations(interpreted)
            .map_err(CodecError::InvalidModule)?
            .obligations()
            .to_vec()
    } else {
        reconstruct_terminal_obligations(module)
            .map_err(CodecError::InvalidModule)?
            .obligations()
            .to_vec()
    };
    Ok(TerminalObligationLedger {
        terminal_psi,
        trust_graph: trust_graph.identity(),
        obligations,
    })
}

/// Reconstruct and compare the complete local proof question. Successful wire
/// decoding alone never establishes that a producer supplied the right ledger.
pub fn validate_terminal_obligation_ledger(
    ledger: &TerminalObligationLedger,
    module: &TerminalModule,
    trust_graph: &ValidatedTerminalTrustGraph,
) -> Result<(), CodecError> {
    let expected = build_terminal_obligation_ledger(module, trust_graph)?;
    if ledger != &expected {
        return Err(CodecError::ObligationLedgerMismatch);
    }
    Ok(())
}

pub fn encode_terminal_obligation_ledger(
    ledger: &TerminalObligationLedger,
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(FORMAT_MARKER);
    writer.u16(ledger.terminal_psi.vocabulary_marker.get());
    writer.bytes(ledger.terminal_psi.program_fingerprint.as_bytes());
    writer.bytes(ledger.trust_graph.as_bytes());
    writer.len("terminal obligations", ledger.obligations.len())?;
    for obligation in &ledger.obligations {
        encode_obligation(&mut writer, obligation)?;
    }
    Ok(writer.finish())
}

pub fn decode_terminal_obligation_ledger(
    bytes: &[u8],
) -> Result<TerminalObligationLedger, CodecError> {
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(CodecError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != FORMAT_MARKER {
        return Err(CodecError::UnsupportedFormatMarker(format_marker));
    }
    let vocabulary_raw = reader.u16()?;
    let vocabulary_marker = VocabularyMarker::new(vocabulary_raw)
        .ok_or(CodecError::UnsupportedVocabularyMarker(vocabulary_raw))?;
    let terminal_psi = TerminalPsiIdentity {
        vocabulary_marker,
        program_fingerprint: SemanticFingerprint::from_bytes(reader.array()?),
    };
    let trust_graph = TerminalTrustGraphIdentity::from_bytes(reader.array()?);
    let obligations = decode_counted(&mut reader, decode_obligation)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    let mut ids = BTreeSet::new();
    let mut owners = BTreeSet::new();
    for obligation in &obligations {
        if !ids.insert(obligation.obligation.id) {
            return Err(CodecError::NonCanonicalOrder(
                "terminal obligation identities",
            ));
        }
        if !owners.insert(obligation.owner) {
            return Err(CodecError::NonCanonicalOrder("terminal obligation owners"));
        }
    }
    let ledger = TerminalObligationLedger {
        terminal_psi,
        trust_graph,
        obligations,
    };
    if encode_terminal_obligation_ledger(&ledger)? != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(ledger)
}

pub fn terminal_obligation_ledger_fingerprint(
    ledger: &TerminalObligationLedger,
) -> Result<TerminalObligationLedgerFingerprint, CodecError> {
    let bytes = encode_terminal_obligation_ledger(ledger)?;
    let mut digest = Sha256::new();
    digest.update(FINGERPRINT_DOMAIN);
    digest.update(
        u64::try_from(bytes.len())
            .expect("terminal obligation ledger bytes fit the digest domain")
            .to_le_bytes(),
    );
    digest.update(bytes);
    Ok(TerminalObligationLedgerFingerprint(
        digest.finalize().into(),
    ))
}

fn encode_obligation(
    writer: &mut Writer,
    reconstructed: &ReconstructedTerminalObligation,
) -> Result<(), CodecError> {
    encode_owner(writer, reconstructed.owner);
    writer.id(reconstructed.obligation.id);
    encode_obligation_class(writer, reconstructed.obligation.class);
    encode_proposition(writer, &reconstructed.obligation.proposition, 0)?;
    encode_propositions(
        writer,
        "terminal obligation requirements",
        &reconstructed.requirements,
    )?;
    encode_propositions(
        writer,
        "terminal obligation axioms",
        &reconstructed.semantic_axioms,
    )?;
    writer.boolean(reconstructed.canonical_certificate);
    Ok(())
}

fn decode_obligation(
    reader: &mut Reader<'_>,
) -> Result<ReconstructedTerminalObligation, CodecError> {
    Ok(ReconstructedTerminalObligation {
        owner: decode_owner(reader)?,
        obligation: Obligation {
            id: reader.id("ObligationId")?,
            class: decode_obligation_class(reader)?,
            proposition: decode_proposition(reader, 0)?,
        },
        requirements: decode_counted(reader, |reader| decode_proposition(reader, 0))?,
        semantic_axioms: decode_counted(reader, |reader| decode_proposition(reader, 0))?,
        canonical_certificate: reader.boolean()?,
    })
}

fn encode_owner(writer: &mut Writer, owner: ReconstructedTerminalObligationOwner) {
    match owner {
        ReconstructedTerminalObligationOwner::Operation { machine, operation } => {
            writer.u8(1);
            writer.id(machine);
            writer.id(operation);
        }
        ReconstructedTerminalObligationOwner::CallRequires {
            machine,
            operation,
            requirement_position,
        } => {
            writer.u8(2);
            writer.id(machine);
            writer.id(operation);
            writer.u32(requirement_position);
        }
        ReconstructedTerminalObligationOwner::NominalCleanupRequires {
            machine,
            edge,
            cleanup_position,
            requirement_position,
        } => {
            writer.u8(3);
            writer.id(machine);
            writer.id(edge);
            writer.u32(cleanup_position);
            writer.u32(requirement_position);
        }
        ReconstructedTerminalObligationOwner::ContractEnsures {
            machine,
            contract,
            clause_position,
        } => {
            writer.u8(4);
            writer.id(machine);
            writer.id(contract);
            writer.u32(clause_position);
        }
    }
}

fn decode_owner(
    reader: &mut Reader<'_>,
) -> Result<ReconstructedTerminalObligationOwner, CodecError> {
    Ok(match reader.u8()? {
        1 => ReconstructedTerminalObligationOwner::Operation {
            machine: reader.id::<MachineId>("MachineId")?,
            operation: reader.id::<OperationId>("OperationId")?,
        },
        2 => ReconstructedTerminalObligationOwner::CallRequires {
            machine: reader.id::<MachineId>("MachineId")?,
            operation: reader.id::<OperationId>("OperationId")?,
            requirement_position: reader.u32()?,
        },
        3 => ReconstructedTerminalObligationOwner::NominalCleanupRequires {
            machine: reader.id::<MachineId>("MachineId")?,
            edge: reader.id::<EdgeId>("EdgeId")?,
            cleanup_position: reader.u32()?,
            requirement_position: reader.u32()?,
        },
        4 => ReconstructedTerminalObligationOwner::ContractEnsures {
            machine: reader.id::<MachineId>("MachineId")?,
            contract: reader.id::<ContractId>("ContractId")?,
            clause_position: reader.u32()?,
        },
        tag => return Err(CodecError::InvalidTag("TerminalObligationOwner", tag)),
    })
}

fn encode_obligation_class(writer: &mut Writer, class: ObligationClass) {
    match class {
        ObligationClass::Derivable => writer.u8(1),
        ObligationClass::AdmissionAuthorized(authorization) => {
            writer.u8(2);
            writer.id(authorization.site);
            writer.u8(match authorization.kind {
                AdmissionKind::ForeignBoundaryGuarantee => 1,
                AdmissionKind::ProviderFact => 2,
                AdmissionKind::CheckedAssemblyClaim => 3,
            });
            writer.id(authorization.authority_identity);
        }
    }
}

fn decode_obligation_class(reader: &mut Reader<'_>) -> Result<ObligationClass, CodecError> {
    Ok(match reader.u8()? {
        1 => ObligationClass::Derivable,
        2 => ObligationClass::AdmissionAuthorized(AuthorizedAdmission {
            site: reader.id::<AdmissionSiteId>("AdmissionSiteId")?,
            kind: match reader.u8()? {
                1 => AdmissionKind::ForeignBoundaryGuarantee,
                2 => AdmissionKind::ProviderFact,
                3 => AdmissionKind::CheckedAssemblyClaim,
                tag => return Err(CodecError::InvalidTag("AdmissionKind", tag)),
            },
            authority_identity: reader.id::<EvidenceIdentity>("EvidenceIdentity")?,
        }),
        tag => return Err(CodecError::InvalidTag("ObligationClass", tag)),
    })
}

fn encode_propositions(
    writer: &mut Writer,
    label: &'static str,
    propositions: &[psi_core::Proposition],
) -> Result<(), CodecError> {
    writer.len(label, propositions.len())?;
    for proposition in propositions {
        encode_proposition(writer, proposition, 0)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use psi_core::{BlockId, ContractId, EdgeId, MachineId, ObligationId, Proposition};
    use psi_terminal::{
        Block, ContractClause, MachineContract, TerminalMachine, TerminalMachineResult,
        TerminalModule, Terminator, VocabularyMarker,
    };

    use super::*;
    use crate::current_terminal_trust_graph;

    #[test]
    fn canonical_ledger_round_trips_and_rejects_a_tampered_question() {
        let module = fixture();
        let trust_graph = current_terminal_trust_graph().expect("current trust graph");
        let ledger = build_terminal_obligation_ledger(&module, &trust_graph).expect("ledger");
        assert_eq!(ledger.obligations().len(), 1);
        let bytes = encode_terminal_obligation_ledger(&ledger).expect("encode ledger");
        let decoded = decode_terminal_obligation_ledger(&bytes).expect("decode ledger");
        assert_eq!(decoded, ledger);
        validate_terminal_obligation_ledger(&decoded, &module, &trust_graph)
            .expect("decoded ledger locally reconstructs");
        assert_ne!(
            terminal_obligation_ledger_fingerprint(&decoded)
                .expect("fingerprint")
                .as_bytes(),
            &[0; 32]
        );

        let mut tampered = bytes;
        let canonical_certificate = tampered.last_mut().expect("canonical flag");
        assert_eq!(*canonical_certificate, 0);
        *canonical_certificate = 1;
        let tampered = decode_terminal_obligation_ledger(&tampered)
            .expect("structurally valid but semantically altered ledger");
        assert_eq!(
            validate_terminal_obligation_ledger(&tampered, &module, &trust_graph),
            Err(CodecError::ObligationLedgerMismatch)
        );
    }

    fn fixture() -> TerminalModule {
        TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: MachineId::new(1).unwrap(),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: MachineId::new(1).unwrap(),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(1).unwrap(),
                blocks: vec![Block {
                    id: BlockId::new(1).unwrap(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(1).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: ContractId::new(1).unwrap(),
                    crash_routes: Vec::new(),
                    requires: vec![Proposition::Truth],
                    ensures: vec![ContractClause {
                        obligation: ObligationId::new(1).unwrap(),
                        proposition: Proposition::Truth,
                    }],
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        }
    }
}
