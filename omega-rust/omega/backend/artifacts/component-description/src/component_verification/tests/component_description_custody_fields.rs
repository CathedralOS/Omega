//! Substitutable-field inventory for the canonical component-description
//! custody matrix driven by `component_verification/tests.rs`'s
//! `component_description_rejects_every_one_field_substitution`.
//!
//! The declared lanes are the `ComponentDescription` record's representable
//! custody axes: the schema and frontier envelope fields, the embedded
//! artifact bytes, every roster row and roster membership axis, the provider
//! closure digest, and the assumption roster. Keeping the inventory in its
//! own file names the boundary between what is declared substitutable and
//! the shared `run_one_field_substitution_matrix` driver that exercises
//! every leg.
//!
//! Outcomes split two ways inside the family's independent checker
//! (`verify_component`): a substitution the canonical codec refuses to carry
//! surfaces as `Corrupt` with the exact decode rejection, and a
//! substitution that stays representable is refused by independent replay
//! with the named rejection kind. `ComponentVerificationRejection` payloads
//! are evidence, not comparable outcomes, so the matrix compares the
//! classified kind. Legs that are not one-field substitutions stay authored
//! beside the matrix: the assumption-bound evidence row mutates the
//! assumption roster and the row's evidence together and needs a request
//! that accepts the digest, and the raw wire corruptions (magic, tags,
//! roster bounds, trailing bytes) are substitutions no encoder-produced
//! description can express.

use super::{describe, empty_selection, minimal_module};
use crate::component_description::{
    COMPONENT_DESCRIPTION_SCHEMA_V2, ComponentDescription, ComponentEntry, ComponentEntryKind,
    CustodyEvidence, CustodyKind, DescriptionDecodeRejection, DescriptionFrontier, EntryEvidence,
    InstallationServiceBound, MAX_IDENTITY_BYTES, ObligationKind, OutgoingAuthorityClass,
    OutgoingEvidence,
};
use crate::component_verification::ComponentVerificationRejection;
use semantic_vocabulary::ServiceId;

optimization_core::custody_field_inventory! {
    /// One substitutable lane of the canonical `ComponentDescription`;
    /// checked by the codec-plus-`verify_component` chain in
    /// `component_description_rejects_every_one_field_substitution`. Legs
    /// that move a roster key keep canonical order by re-sorting so the
    /// substitution stays representable rather than degenerating into an
    /// encoding failure; the declared `*Duplicated` and `*OverBound` legs
    /// are the representable-in-value-domain lanes that only the wire
    /// refuses.
    pub enum ComponentDescriptionFieldForTest {
        // --- envelope scalars ---
        SchemaBumped,
        FrontierEarly,
        // --- embedded artifact bytes ---
        ArtifactForeign,
        ArtifactTruncated,
        // --- import roster ---
        ImportRequirementRenamed,
        ImportContractForged,
        ImportsDropped,
        ImportDuplicated,
        // --- export roster ---
        ExportIdentityRenamed,
        ExportsDropped,
        ExportDuplicated,
        ExportIdentityOverBound,
        // --- entry roster ---
        CanonicalEntryIdentity,
        CanonicalEntryKind,
        CanonicalEntryEvidenceUnbound,
        ResumptionEntryKind,
        CanonicalEntryDropped,
        EntryInserted,
        EntryDuplicated,
        // --- outgoing authority roster ---
        SealedAuthorityEvidenceForeign,
        SealedAuthorityEvidenceModuleDerived,
        SealedAuthorityIdentity,
        SealedAuthorityClass,
        UnsealedAuthorityEvidenceSealed,
        SealedAuthorityDropped,
        ServiceCeilingDropped,
        OutgoingDuplicated,
        // --- service-bound roster ---
        ServiceBoundRequirement,
        ServiceBoundWidened,
        ServiceBoundsDropped,
        ServiceBoundInserted,
        ServiceBoundDuplicated,
        ServiceBoundServiceDuplicated,
        // --- custody roster ---
        CustodyKindSubstituted,
        CustodyIdentityForged,
        CustodyEvidenceUnbound,
        CustodyDropped,
        CustodyDuplicated,
        // --- retained provider roster ---
        ProviderPlanDigestForged,
        ProviderRequirementRetargeted,
        ProviderRequirementsDropped,
        ProvidersDropped,
        ProviderDuplicated,
        ProviderRequirementDuplicated,
        // --- provider closure digest and obligation roster ---
        ProviderClosureDigestZeroed,
        ObligationIdentityRenamed,
        ObligationKindSubstituted,
        ImportBindingObligationDropped,
        ProviderOccurrenceObligationDropped,
        ObligationDuplicated,
        // --- assumption roster ---
        AssumptionDuplicated,
    }
}

/// The `verify_component` rejection kind a substituted description's honest
/// replay reports. `ComponentVerificationRejection` is deliberately not
/// `PartialEq` — its payloads are evidence — so the matrix compares the
/// named kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptionReplayRejection {
    ArtifactInvalid,
    ProofSection,
    ModuleVerification,
    IncompatibleSchema,
    WrongSubject,
    EarlyFrontier,
    UnacceptedAssumption,
    UnboundAssumptionReference,
    MissingComponentEntry,
    UnexpectedDerivedEntry,
    MissingExport,
    UnexpectedDerivedExport,
    MissingOutgoingAuthority,
    UnexpectedDerivedAuthority,
    InvalidAuthorityEvidence,
    UnsealedProvider,
    SmuggledProviderRequirement,
    UnboundRequirement,
    SmuggledImport,
    ImportContractMismatch,
    MissingCustodyConstraint,
    UnexpectedDerivedCustody,
    MissingServiceBound,
    UnexpectedServiceBound,
    MissingObligation,
    InconsistentProviderClosure,
}

/// Classify a replay rejection into the family's named vocabulary. `Corrupt`
/// is handled by the checker's own arm — it carries the comparable decode
/// rejection, not a replay verdict.
pub fn classify_replay(rejection: &ComponentVerificationRejection) -> DescriptionReplayRejection {
    use ComponentVerificationRejection as Rejection;
    use DescriptionReplayRejection as Kind;
    match rejection {
        Rejection::ArtifactInvalid(_) => Kind::ArtifactInvalid,
        Rejection::ProofSection(_) => Kind::ProofSection,
        Rejection::ModuleVerification(_) => Kind::ModuleVerification,
        Rejection::IncompatibleSchema { .. } => Kind::IncompatibleSchema,
        Rejection::WrongSubject { .. } => Kind::WrongSubject,
        Rejection::EarlyFrontier(_) => Kind::EarlyFrontier,
        Rejection::UnacceptedAssumption(_) => Kind::UnacceptedAssumption,
        Rejection::UnboundAssumptionReference(_) => Kind::UnboundAssumptionReference,
        Rejection::MissingComponentEntry(_) => Kind::MissingComponentEntry,
        Rejection::UnexpectedDerivedEntry(_) => Kind::UnexpectedDerivedEntry,
        Rejection::MissingExport(_) => Kind::MissingExport,
        Rejection::UnexpectedDerivedExport(_) => Kind::UnexpectedDerivedExport,
        Rejection::MissingOutgoingAuthority(_) => Kind::MissingOutgoingAuthority,
        Rejection::UnexpectedDerivedAuthority(_) => Kind::UnexpectedDerivedAuthority,
        Rejection::InvalidAuthorityEvidence(_) => Kind::InvalidAuthorityEvidence,
        Rejection::UnsealedProvider(_) => Kind::UnsealedProvider,
        Rejection::SmuggledProviderRequirement(_) => Kind::SmuggledProviderRequirement,
        Rejection::UnboundRequirement(_) => Kind::UnboundRequirement,
        Rejection::SmuggledImport(_) => Kind::SmuggledImport,
        Rejection::ImportContractMismatch(_) => Kind::ImportContractMismatch,
        Rejection::MissingCustodyConstraint(_) => Kind::MissingCustodyConstraint,
        Rejection::UnexpectedDerivedCustody(_) => Kind::UnexpectedDerivedCustody,
        Rejection::MissingServiceBound(_) => Kind::MissingServiceBound,
        Rejection::UnexpectedServiceBound(_) => Kind::UnexpectedServiceBound,
        Rejection::MissingObligation(_) => Kind::MissingObligation,
        Rejection::InconsistentProviderClosure(_) => Kind::InconsistentProviderClosure,
        Rejection::Corrupt(inner) => panic!(
            "the matrix's Corrupt arm, not a replay verdict, carries the decode rejection: {inner:?}"
        ),
    }
}

/// The family's independent-checker verdict: `verify_component` either
/// refuses the canonical encoding itself (`Corrupt`, carrying the exact
/// decode rejection) or replays the decoded record and rejects it
/// semantically (`Replay`, classified by kind).
#[derive(Debug, PartialEq)]
pub enum ComponentDescriptionCustodyCheck {
    /// `verify_component` refused the canonical encoding: the exact decode
    /// rejection the description codec names.
    Corrupt(DescriptionDecodeRejection),
    /// `verify_component` decoded the record and rejected it at independent
    /// replay — the rejection's named kind.
    Replay(DescriptionReplayRejection),
}

/// An authentic foreign description: an honestly produced description of a
/// different component module, so its retained custody differs from the
/// honest record the matrix builds. Its embedded artifact supplies the
/// `ArtifactForeign` leg's real foreign payload.
pub fn foreign_description_donor() -> ComponentDescription {
    describe(&minimal_module(), &empty_selection())
}

/// The family's honest-recomputation hook: mutate exactly the declared field
/// of `description`, keeping every other axis honest and every roster in
/// canonical order so a representable substitution stays representable.
/// `donor` supplies the authentic foreign artifact the `ArtifactForeign`
/// leg substitutes; the remaining legs take fixed non-identity alternates
/// or draw the forged evidence digest from the record's own provider roster.
pub fn substitute_component_description_for_test(
    description: &mut ComponentDescription,
    field: ComponentDescriptionFieldForTest,
    donor: &ComponentDescription,
) {
    use ComponentDescriptionFieldForTest as Leg;
    match field {
        Leg::SchemaBumped => description.schema = COMPONENT_DESCRIPTION_SCHEMA_V2 + 1,
        Leg::FrontierEarly => description.frontier = DescriptionFrontier::SelectedPlan,
        Leg::ArtifactForeign => {
            description.artifact_bytes = donor.artifact_bytes.clone();
        }
        Leg::ArtifactTruncated => {
            description
                .artifact_bytes
                .truncate(description.artifact_bytes.len() / 2);
        }
        Leg::ImportRequirementRenamed => {
            description.imports[0].requirement_identity = "Smuggled::requirement".into();
        }
        Leg::ImportContractForged => description.imports[0].contract_identity = [9u8; 32],
        Leg::ImportsDropped => description.imports.clear(),
        Leg::ImportDuplicated => {
            let row = description.imports[0].clone();
            description.imports.push(row);
        }
        Leg::ExportIdentityRenamed => {
            description.exports[0].identity = "export:canonical:99".into();
        }
        Leg::ExportsDropped => description.exports.clear(),
        Leg::ExportDuplicated => {
            let row = description.exports[0].clone();
            description.exports.push(row);
        }
        Leg::ExportIdentityOverBound => {
            description.exports[0].identity = "x".repeat(MAX_IDENTITY_BYTES + 1);
        }
        Leg::CanonicalEntryIdentity => {
            description
                .entries
                .iter_mut()
                .find(|entry| entry.kind == ComponentEntryKind::Canonical)
                .expect("canonical entry")
                .identity = "canonical-entry:99".into();
        }
        Leg::CanonicalEntryKind => {
            let entry = description
                .entries
                .iter_mut()
                .find(|entry| entry.kind == ComponentEntryKind::Canonical)
                .expect("canonical entry");
            entry.kind = ComponentEntryKind::Timer;
            description.entries.sort();
        }
        Leg::CanonicalEntryEvidenceUnbound => {
            description
                .entries
                .iter_mut()
                .find(|entry| entry.kind == ComponentEntryKind::Canonical)
                .expect("canonical entry")
                .evidence = EntryEvidence::AssumptionBound([9u8; 32]);
        }
        Leg::ResumptionEntryKind => {
            let entry = description
                .entries
                .iter_mut()
                .find(|entry| entry.kind == ComponentEntryKind::SuspensionResumption)
                .expect("resumption entry");
            entry.kind = ComponentEntryKind::Canonical;
            description.entries.sort();
        }
        Leg::CanonicalEntryDropped => {
            description
                .entries
                .retain(|entry| entry.kind != ComponentEntryKind::Canonical);
        }
        Leg::EntryInserted => {
            description.entries.push(ComponentEntry {
                kind: ComponentEntryKind::RegisteredCallback,
                identity: "callback:forged".into(),
                evidence: EntryEvidence::ModuleDerived,
            });
        }
        Leg::EntryDuplicated => {
            let row = description.entries[0].clone();
            description.entries.push(row);
        }
        Leg::SealedAuthorityEvidenceForeign => {
            description
                .outgoing
                .iter_mut()
                .find(|row| row.identity == "IndexedRequirement::apply")
                .expect("sealed row")
                .evidence = OutgoingEvidence::ProviderSealed([9u8; 32]);
        }
        Leg::SealedAuthorityEvidenceModuleDerived => {
            let row = description
                .outgoing
                .iter_mut()
                .find(|row| row.identity == "IndexedRequirement::apply")
                .expect("sealed row");
            row.evidence = OutgoingEvidence::ModuleDerived;
            description.outgoing.sort();
        }
        Leg::SealedAuthorityIdentity => {
            let row = description
                .outgoing
                .iter_mut()
                .find(|row| row.identity == "IndexedRequirement::apply")
                .expect("sealed row");
            row.identity = "Sealed::renamed".into();
            description.outgoing.sort();
        }
        Leg::SealedAuthorityClass => {
            let row = description
                .outgoing
                .iter_mut()
                .find(|row| row.identity == "IndexedRequirement::apply")
                .expect("sealed row");
            row.class = OutgoingAuthorityClass::PortSpaceWrite;
            description.outgoing.sort();
        }
        Leg::UnsealedAuthorityEvidenceSealed => {
            let sealed_digest = description.providers[0].plan_digest;
            let row = description
                .outgoing
                .iter_mut()
                .find(|row| row.identity == "Unsealed::requirement")
                .expect("unsealed row");
            row.evidence = OutgoingEvidence::ProviderSealed(sealed_digest);
            description.outgoing.sort();
        }
        Leg::SealedAuthorityDropped => {
            description
                .outgoing
                .retain(|row| row.identity != "IndexedRequirement::apply");
        }
        Leg::ServiceCeilingDropped => {
            description
                .outgoing
                .retain(|row| row.class != OutgoingAuthorityClass::ServiceCeiling);
        }
        Leg::OutgoingDuplicated => {
            let row = description.outgoing[0].clone();
            description.outgoing.push(row);
        }
        Leg::ServiceBoundRequirement => {
            description.service_bounds[0].requirement_identity = "Other::requirement".into();
        }
        Leg::ServiceBoundWidened => {
            description.service_bounds[0]
                .bound
                .push(ServiceId::new(2).expect("service identity"));
        }
        Leg::ServiceBoundsDropped => description.service_bounds.clear(),
        Leg::ServiceBoundInserted => {
            description.service_bounds.push(InstallationServiceBound {
                requirement_identity: "Forged::requirement".into(),
                bound: vec![ServiceId::new(1).expect("service identity")],
            });
        }
        Leg::ServiceBoundDuplicated => {
            let row = description.service_bounds[0].clone();
            description.service_bounds.push(row);
        }
        Leg::ServiceBoundServiceDuplicated => {
            let service = description.service_bounds[0].bound[0];
            description.service_bounds[0].bound.push(service);
        }
        Leg::CustodyKindSubstituted => {
            description.custody[0].kind = CustodyKind::PlacedViewInput;
        }
        Leg::CustodyIdentityForged => description.custody[0].identity = "forged:custody".into(),
        Leg::CustodyEvidenceUnbound => {
            description.custody[0].evidence = CustodyEvidence::AssumptionBound([9u8; 32]);
        }
        Leg::CustodyDropped => description.custody.clear(),
        Leg::CustodyDuplicated => {
            let row = description.custody[0].clone();
            description.custody.push(row);
        }
        Leg::ProviderPlanDigestForged => description.providers[0].plan_digest = [9u8; 32],
        Leg::ProviderRequirementRetargeted => {
            description.providers[0].requirement_identities[0] = "Unsealed::requirement".into();
        }
        Leg::ProviderRequirementsDropped => {
            description.providers[0].requirement_identities.clear();
        }
        Leg::ProvidersDropped => description.providers.clear(),
        Leg::ProviderDuplicated => {
            let row = description.providers[0].clone();
            description.providers.push(row);
        }
        Leg::ProviderRequirementDuplicated => {
            let requirement = description.providers[0].requirement_identities[0].clone();
            description.providers[0]
                .requirement_identities
                .push(requirement);
        }
        Leg::ProviderClosureDigestZeroed => description.provider_closure_digest = [0u8; 32],
        Leg::ObligationIdentityRenamed => {
            let row = description
                .obligations
                .iter_mut()
                .find(|obligation| obligation.kind == ObligationKind::ImportBinding)
                .expect("import obligation");
            row.identity = "Other::requirement".into();
            description.obligations.sort();
        }
        Leg::ObligationKindSubstituted => {
            let row = description
                .obligations
                .iter_mut()
                .find(|obligation| obligation.kind == ObligationKind::ImportBinding)
                .expect("import obligation");
            row.kind = ObligationKind::ProgressDemand;
            description.obligations.sort();
        }
        Leg::ImportBindingObligationDropped => {
            description
                .obligations
                .retain(|obligation| obligation.kind != ObligationKind::ImportBinding);
        }
        Leg::ProviderOccurrenceObligationDropped => {
            description
                .obligations
                .retain(|obligation| obligation.kind != ObligationKind::ProviderOccurrence);
        }
        Leg::ObligationDuplicated => {
            let row = description.obligations[0].clone();
            description.obligations.push(row);
        }
        Leg::AssumptionDuplicated => {
            description.assumptions.push([7u8; 32]);
            description.assumptions.push([7u8; 32]);
        }
    }
}

/// The exact checker verdict each declared leg must produce: a canonical
/// wire violation is the codec's `Corrupt` rejection carried through
/// `verify_component`; every representable leg is the named kind of the
/// honest replay's refusal.
pub fn component_description_custody_outcome(
    field: ComponentDescriptionFieldForTest,
) -> optimization_core::MutationOutcome<ComponentDescriptionCustodyCheck> {
    use ComponentDescriptionCustodyCheck as Check;
    use ComponentDescriptionFieldForTest as Leg;
    use DescriptionDecodeRejection as Decode;
    use DescriptionReplayRejection as Replay;
    let check = match field {
        Leg::SchemaBumped => Check::Replay(Replay::IncompatibleSchema),
        Leg::FrontierEarly => Check::Replay(Replay::EarlyFrontier),
        Leg::ArtifactForeign => Check::Replay(Replay::WrongSubject),
        Leg::ArtifactTruncated => {
            Check::Corrupt(Decode::Corrupt("embedded artifact did not decode"))
        }
        // Renaming the bound import leaves the real unsealed requirement
        // unbound, which rejects first; the smuggled name would reject too.
        Leg::ImportRequirementRenamed | Leg::ImportsDropped => {
            Check::Replay(Replay::UnboundRequirement)
        }
        Leg::ImportContractForged => Check::Replay(Replay::ImportContractMismatch),
        Leg::ImportDuplicated => Check::Corrupt(Decode::NonCanonicalOrder("import")),
        Leg::ExportIdentityRenamed => Check::Replay(Replay::UnexpectedDerivedExport),
        Leg::ExportsDropped => Check::Replay(Replay::MissingExport),
        Leg::ExportDuplicated => Check::Corrupt(Decode::NonCanonicalOrder("export")),
        Leg::ExportIdentityOverBound => Check::Corrupt(Decode::IdentityInvalid("too long")),
        Leg::CanonicalEntryIdentity
        | Leg::CanonicalEntryKind
        | Leg::ResumptionEntryKind
        | Leg::EntryInserted => Check::Replay(Replay::UnexpectedDerivedEntry),
        Leg::CanonicalEntryEvidenceUnbound => Check::Replay(Replay::UnboundAssumptionReference),
        Leg::CanonicalEntryDropped => Check::Replay(Replay::MissingComponentEntry),
        Leg::EntryDuplicated => Check::Corrupt(Decode::NonCanonicalOrder("entry")),
        Leg::SealedAuthorityEvidenceForeign
        | Leg::UnsealedAuthorityEvidenceSealed
        | Leg::ProviderPlanDigestForged
        | Leg::ProviderRequirementRetargeted
        | Leg::ProviderRequirementsDropped
        | Leg::ProvidersDropped => Check::Replay(Replay::UnsealedProvider),
        Leg::SealedAuthorityEvidenceModuleDerived => Check::Replay(Replay::UnboundRequirement),
        Leg::SealedAuthorityIdentity | Leg::SealedAuthorityClass => {
            Check::Replay(Replay::UnexpectedDerivedAuthority)
        }
        Leg::SealedAuthorityDropped | Leg::ServiceCeilingDropped => {
            Check::Replay(Replay::MissingOutgoingAuthority)
        }
        Leg::OutgoingDuplicated => Check::Corrupt(Decode::NonCanonicalOrder("outgoing authority")),
        Leg::ServiceBoundRequirement | Leg::ServiceBoundWidened | Leg::ServiceBoundsDropped => {
            Check::Replay(Replay::MissingServiceBound)
        }
        Leg::ServiceBoundInserted => Check::Replay(Replay::UnexpectedServiceBound),
        Leg::ServiceBoundDuplicated => Check::Corrupt(Decode::NonCanonicalOrder("service bound")),
        Leg::ServiceBoundServiceDuplicated => {
            Check::Corrupt(Decode::NonCanonicalOrder("service bound services"))
        }
        Leg::CustodyKindSubstituted | Leg::CustodyIdentityForged => {
            Check::Replay(Replay::UnexpectedDerivedCustody)
        }
        Leg::CustodyEvidenceUnbound => Check::Replay(Replay::UnboundAssumptionReference),
        Leg::CustodyDropped => Check::Replay(Replay::MissingCustodyConstraint),
        Leg::CustodyDuplicated => Check::Corrupt(Decode::NonCanonicalOrder("custody")),
        Leg::ProviderDuplicated => Check::Corrupt(Decode::NonCanonicalOrder("provider")),
        Leg::ProviderRequirementDuplicated => {
            Check::Corrupt(Decode::NonCanonicalOrder("provider requirement"))
        }
        Leg::ProviderClosureDigestZeroed => Check::Replay(Replay::InconsistentProviderClosure),
        Leg::ObligationIdentityRenamed
        | Leg::ObligationKindSubstituted
        | Leg::ImportBindingObligationDropped
        | Leg::ProviderOccurrenceObligationDropped => Check::Replay(Replay::MissingObligation),
        Leg::ObligationDuplicated => Check::Corrupt(Decode::NonCanonicalOrder("obligation")),
        Leg::AssumptionDuplicated => Check::Corrupt(Decode::NonCanonicalOrder("assumption")),
    };
    optimization_core::MutationOutcome::ExactError(check)
}
