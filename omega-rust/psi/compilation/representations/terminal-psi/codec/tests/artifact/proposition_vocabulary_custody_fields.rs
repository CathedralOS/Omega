//! Substitutable-field inventory for the canonical Terminal module's
//! proposition vocabulary rosters.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `proposition_vocabulary_custody.rs`; keeping the inventory in
//! its own file names the boundary between what is declared substitutable
//! and the driver that exercises every leg. A leg is one authored
//! substitution of a `TerminalModule` field that still canonicalizes under
//! `encode_module`: declaration, binder, parameter-type, and evidence-type
//! renames; binder-argument and argument renames; the ordinary machine
//! binder argument projected through the application's own evidence term;
//! the dropped ensures lane and the second ensures lane the retained
//! requires lane matches; the ensures output field; a dropped requires lane
//! (the one leg whose substituted module stops verifying — see
//! [`proposition_vocabulary_verification`]); and the whole vocabulary
//! cleared. Non-canonical or emptied identities, colliding names, kind and
//! classifier tag recasts, count lies, dense-index and roster-order
//! violations, double-joined interface strands, orphaned terms, and the
//! producer-side encode rejections stay authored in
//! `proposition_vocabulary_custody.rs` as decode- and encode-level
//! rejections.
//!
//! The vocabulary carries no reconstructed proof obligations, so every
//! declared leg except `RequiresLaneDropped` must keep the substituted
//! module *verifiable* under the retained bundle while the two custody
//! replays reject it — the retained-manifest join
//! (`ArtifactManifestError::ManifestMismatch`) and the sealed proof subject
//! join (`ProofSubjectMismatch`), asserted inside the matrix's checker; the
//! per-leg verification verdict is asserted by the matrix's joined replay.

use mutation_matrix::{MutationOutcome, custody_field_inventory};
use terminal_codec::ArtifactManifestError;
use terminal_psi::{
    EvidenceContractLane, EvidenceContractLaneKind, PropositionBinderKind, PropositionEvidence,
    TerminalModule,
};
use terminal_verifier::VerificationError;

use super::super::{evidence_term_id, machine_id};
use super::binder_projection;

custody_field_inventory! {
    /// One substitutable field of the canonical module's proposition
    /// vocabulary custody; each leg rejects at the retained-manifest and
    /// sealed-proof-subject joins, and its verification verdict is named by
    /// `proposition_vocabulary_verification`.
    pub enum PropositionVocabularyCustodyFieldForTest {
        // --- declaration roster ---
        FactDeclarationRenamed,
        WitnessDeclarationRenamed,
        DeclarationBinderRenamed,
        ConstBinderTypeRenamed,
        DeclarationParameterTypeRenamed,
        WitnessEvidenceTypeRenamed,
        // --- application binder arguments and arguments ---
        TypeBinderArgumentRenamed,
        ConstBinderArgumentRenamed,
        MachineBinderArgumentRenamed,
        MachineBinderArgumentProjected,
        ApplicationArgumentRenamed,
        SecondApplicationArgumentRenamed,
        // --- evidence contract lanes ---
        EnsuresLaneDropped,
        SecondEnsuresLaneMatched,
        EnsuresOutputFieldRenamed,
        RequiresLaneDropped,
        // --- whole vocabulary ---
        VocabularyCleared,
    }
}

/// Apply one declared substitution to the module.
///
/// Every leg uses a fixed alternate rather than donor values: the foreign
/// donor module's vocabulary rosters are empty, so its fields have nothing
/// to lend this identity-joined vocabulary.
pub(crate) fn substitute_proposition_vocabulary_for_test(
    record: &mut TerminalModule,
    field: PropositionVocabularyCustodyFieldForTest,
    donor: &TerminalModule,
) {
    let _ = donor;
    match field {
        PropositionVocabularyCustodyFieldForTest::FactDeclarationRenamed => {
            record.proposition_declarations[0].name = "example::Arranged".into();
        }
        PropositionVocabularyCustodyFieldForTest::WitnessDeclarationRenamed => {
            record.proposition_declarations[1].name = "example::Settled".into();
        }
        PropositionVocabularyCustodyFieldForTest::DeclarationBinderRenamed => {
            record.proposition_declarations[1].binders[0].name = "U".into();
        }
        PropositionVocabularyCustodyFieldForTest::ConstBinderTypeRenamed => {
            record.proposition_declarations[1].binders[1].kind = PropositionBinderKind::Const {
                type_identity: "example::Natural".into(),
            };
        }
        PropositionVocabularyCustodyFieldForTest::DeclarationParameterTypeRenamed => {
            record.proposition_declarations[1].parameter_types[0] = "example::Thing".into();
        }
        PropositionVocabularyCustodyFieldForTest::WitnessEvidenceTypeRenamed => {
            record.proposition_declarations[1].evidence = PropositionEvidence::Witness {
                evidence_type: "example::HeldProof".into(),
            };
        }
        PropositionVocabularyCustodyFieldForTest::TypeBinderArgumentRenamed => {
            record.proposition_applications[1].binder_arguments[0].identity = "example::Box".into();
        }
        PropositionVocabularyCustodyFieldForTest::ConstBinderArgumentRenamed => {
            record.proposition_applications[1].binder_arguments[1].identity =
                "example::four".into();
        }
        PropositionVocabularyCustodyFieldForTest::MachineBinderArgumentRenamed => {
            record.proposition_applications[1].binder_arguments[3].identity =
                "example::worker".into();
        }
        PropositionVocabularyCustodyFieldForTest::MachineBinderArgumentProjected => {
            // The ordinary machine argument projects through the same term
            // the sibling machine argument already uses: it encodes,
            // decodes, verifies, and diverges the identity.
            let argument = &mut record.proposition_applications[1].binder_arguments[3];
            argument.identity.clear();
            argument.evidence_projection = Some(binder_projection());
        }
        PropositionVocabularyCustodyFieldForTest::ApplicationArgumentRenamed => {
            record.proposition_applications[1].arguments[0] = "example::Box".into();
        }
        PropositionVocabularyCustodyFieldForTest::SecondApplicationArgumentRenamed => {
            record.proposition_applications[1].arguments[1] = "example::four".into();
        }
        PropositionVocabularyCustodyFieldForTest::EnsuresLaneDropped => {
            record.evidence_contract_lanes.remove(1);
        }
        PropositionVocabularyCustodyFieldForTest::SecondEnsuresLaneMatched => {
            // A second ensures lane on the same machine and term is matched
            // by the existing requires lane — producer provenance keys the
            // pair, not the lane — so the row stays representable.
            record.evidence_contract_lanes.push(EvidenceContractLane {
                machine: machine_id(1),
                kind: EvidenceContractLaneKind::Ensures,
                position: 1,
                term: evidence_term_id(1),
                output_field: Some("finding".into()),
            });
        }
        PropositionVocabularyCustodyFieldForTest::EnsuresOutputFieldRenamed => {
            record.evidence_contract_lanes[1].output_field = Some("finding".into());
        }
        PropositionVocabularyCustodyFieldForTest::RequiresLaneDropped => {
            record.evidence_contract_lanes.remove(0);
        }
        PropositionVocabularyCustodyFieldForTest::VocabularyCleared => {
            record.proposition_declarations.clear();
            record.proposition_applications.clear();
            record.evidence_terms.clear();
            record.evidence_contract_lanes.clear();
        }
    }
}

/// Name the verdict a declared leg must produce at the retained-manifest
/// replay: every representable substitution diverges the semantic identity
/// the manifest binds, so the join always rejects with a manifest mismatch.
/// The verification replay and the sealed proof subject join are asserted
/// beside it — the verification verdict through the joined replay, the proof
/// subject inside the matrix's checker.
pub(crate) fn proposition_vocabulary_custody_outcome(
    _field: PropositionVocabularyCustodyFieldForTest,
) -> MutationOutcome<ArtifactManifestError> {
    MutationOutcome::ExactError(ArtifactManifestError::ManifestMismatch)
}

/// Name the verdict a declared leg must produce at the verification replay:
/// a dropped requires lane leaves its ensures lane unmatched — the module
/// stays representationally sound, but independent verification reports the
/// term with no producer. Every other declared leg keeps the substituted
/// module verifiable under the retained bundle.
pub(crate) fn proposition_vocabulary_verification(
    field: PropositionVocabularyCustodyFieldForTest,
) -> Result<(), VerificationError> {
    match field {
        PropositionVocabularyCustodyFieldForTest::RequiresLaneDropped => Err(
            VerificationError::MissingEvidenceProducer(evidence_term_id(1)),
        ),
        _ => Ok(()),
    }
}
