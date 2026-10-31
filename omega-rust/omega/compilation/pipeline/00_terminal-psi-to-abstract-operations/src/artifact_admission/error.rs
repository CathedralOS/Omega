use crate::lowering::LoweringError;

#[derive(Debug)]
pub enum ArtifactLoweringError {
    TargetSelectionRequired,
    TargetFamily(String),
    SemanticDecode(terminal_codec::CodecError),
    /// A access-view roster row is bound by its call rather than by the
    /// executable's host, or a declared direct-entry row was left without a
    /// supply; the input boundary cannot silently discard either.
    AccessViewInputsRequireCustodyLowering,
    /// A supplied provider establishment answered no declared direct-entry
    /// roster row.
    AccessViewEstablishmentUnexpected {
        machine: semantic_vocabulary::MachineId,
        position: u32,
    },
    /// Two supplied provider establishments answered one roster row.
    AccessViewEstablishmentDuplicate {
        machine: semantic_vocabulary::MachineId,
        position: u32,
    },
    /// Referent domain qualifications must be strictly ordered and
    /// deduplicated, the same canonical form structural arguments carry.
    AccessViewEstablishmentQualificationsNonCanonical,
    /// The referent names a structural carrier the module never declares, so
    /// the supply cannot be the qualified backing the row seals.
    AccessViewEstablishmentBackingUndeclared(semantic_vocabulary::StructuralTypeId),
    /// The referent's path does not resolve through the declared shape
    /// graph; a stale or substituted range cannot be the lent place.
    AccessViewEstablishmentRangeUnresolved,
    /// A supplied qualification is not a structural domain the module
    /// declares.
    AccessViewEstablishmentQualificationUndeclared(semantic_vocabulary::StructuralDomainId),
    /// A declared domain accepts only its exact carrier: a qualification
    /// asserting a domain whose carrier is not the referent's declared type
    /// is a substituted qualified backing.
    AccessViewEstablishmentQualificationCarrier(semantic_vocabulary::StructuralDomainId),
    /// An exclusive-borrow referent may not overlap another established
    /// referent's place.
    AccessViewEstablishmentAliasing(u64),
    Lowering(LoweringError),
}

diagnostics::debug_display!(ArtifactLoweringError);

impl std::error::Error for ArtifactLoweringError {}
