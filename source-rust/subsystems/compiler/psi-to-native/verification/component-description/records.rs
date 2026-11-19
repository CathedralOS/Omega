//! Canonical component description records and wire tags.
use semantic_vocabulary::{BoundaryMachineId, ServiceId};

macro_rules! wire_tagged {
    ($name:ident { $($variant:ident = $tag:literal),+ $(,)? }) => {
        impl $name {
            pub(crate) fn tag(self) -> u8 {
                match self {
                    $(Self::$variant => $tag),+
                }
            }

            pub(crate) fn from_tag(tag: u8) -> Option<Self> {
                match tag {
                    $($tag => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

pub const COMPONENT_DESCRIPTION_SCHEMA_V3: u32 = 3;

pub(crate) const MAX_COMPONENT_DESCRIPTION_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_DESCRIPTION_ROSTER: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DescriptionFrontier {
    AuthoredDeclaration,
    SelectedPlan,
    TerminalArtifactClosure,
}

wire_tagged!(DescriptionFrontier {
    AuthoredDeclaration = 1,
    SelectedPlan = 2,
    TerminalArtifactClosure = 3,
});

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImportSlot {
    pub slot: u32,
    pub requirement_identity: String,
    pub contract_identity: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExportSurface {
    pub identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentEntryKind {
    Canonical,
    SuspensionResumption,
    Startup,
    RegisteredCallback,
    Timer,
    Cleanup,
    RetainedProvider,
}

wire_tagged!(ComponentEntryKind {
    Canonical = 1,
    SuspensionResumption = 2,
    Startup = 3,
    RegisteredCallback = 4,
    Timer = 5,
    Cleanup = 6,
    RetainedProvider = 7,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryEvidence {
    ModuleDerived,
    AssumptionBound([u8; 32]),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentEntry {
    pub kind: ComponentEntryKind,
    pub identity: String,
    pub evidence: EntryEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutgoingAuthorityClass {
    BoundaryRequirement,
    PortSpaceWrite,
    RegisterAccess,
    ServiceCeiling,
}

wire_tagged!(OutgoingAuthorityClass {
    BoundaryRequirement = 1,
    PortSpaceWrite = 2,
    ServiceCeiling = 3,
    RegisterAccess = 4,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OutgoingEvidence {
    ModuleDerived,
    ProviderSealed([u8; 32]),
    PhysicalMechanism([u8; 32]),
    AssumptionBound([u8; 32]),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OutgoingAuthority {
    pub class: OutgoingAuthorityClass,
    pub identity: String,
    pub evidence: OutgoingEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstallationServiceBound {
    pub requirement_identity: String,
    pub bound: Vec<ServiceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CustodyKind {
    AccessViewInput,
    ReborrowRestoredCall,
    CompletionReceipt,
    ProgramLocalRootIntroduction,
    BoundaryContentGuarantee,
    SuspensionFrontier,
    Mapping,
    Lease,
}

wire_tagged!(CustodyKind {
    AccessViewInput = 1,
    ReborrowRestoredCall = 3,
    CompletionReceipt = 4,
    ProgramLocalRootIntroduction = 5,
    BoundaryContentGuarantee = 6,
    SuspensionFrontier = 8,
    Mapping = 9,
    Lease = 10,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CustodyEvidence {
    ModuleDerived,
    AssumptionBound([u8; 32]),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CustodyConstraint {
    pub kind: CustodyKind,
    pub identity: String,
    pub evidence: CustodyEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RetainedProvider {
    pub report_identity: u64,
    pub plan_digest: [u8; 32],
    pub plan_input: Vec<u8>,
    pub requirement_identities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObligationKind {
    StackProvision,
    ProviderOccurrence,
    ImportBinding,
    ResourceAdmission,
    Mapping,
    Lease,
}

wire_tagged!(ObligationKind {
    StackProvision = 1,
    ProviderOccurrence = 2,
    ImportBinding = 3,
    ResourceAdmission = 5,
    Mapping = 6,
    Lease = 7,
});

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstallationObligation {
    pub kind: ObligationKind,
    pub identity: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDescription {
    pub schema: u32,
    pub frontier: DescriptionFrontier,
    pub artifact_bytes: Vec<u8>,
    pub imports: Vec<ImportSlot>,
    pub exports: Vec<ExportSurface>,
    pub entries: Vec<ComponentEntry>,
    pub outgoing: Vec<OutgoingAuthority>,
    pub service_bounds: Vec<InstallationServiceBound>,
    pub custody: Vec<CustodyConstraint>,
    pub providers: Vec<RetainedProvider>,
    pub provider_closure_digest: [u8; 32],
    pub obligations: Vec<InstallationObligation>,
    pub assumptions: Vec<[u8; 32]>,
    pub realization_identity: Option<[u8; 32]>,
}

#[derive(Debug)]
pub enum DescribeError {
    ArtifactFormat(terminal_codec::CanonicalTerminalArtifactError),
    ArtifactIdentity(terminal_codec::CodecError),
    UndeclaredBoundary(BoundaryMachineId),
    BoundExceeded(&'static str),
}

impl std::fmt::Display for DescribeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArtifactFormat(error) => {
                write!(
                    formatter,
                    "component canonical artifact is invalid: {error}"
                )
            }
            Self::ArtifactIdentity(error) => {
                write!(formatter, "component subject identity failed: {error}")
            }
            Self::UndeclaredBoundary(id) => {
                write!(formatter, "called boundary machine {id} has no declaration")
            }
            Self::BoundExceeded(what) => {
                write!(
                    formatter,
                    "component description {what} exceeds the hard bound"
                )
            }
        }
    }
}

impl std::error::Error for DescribeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptionDecodeRejection {
    Corrupt(&'static str),
    InvalidMagic,
    UnsupportedTag(&'static str),
    RosterBoundExceeded(&'static str),
    IdentityInvalid(&'static str),
    NonCanonicalOrder(&'static str),
}

impl std::fmt::Display for DescriptionDecodeRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Corrupt(detail) => write!(formatter, "corrupt description: {detail}"),
            Self::InvalidMagic => write!(formatter, "not a component description"),
            Self::UnsupportedTag(what) => write!(formatter, "unsupported {what}"),
            Self::RosterBoundExceeded(what) => {
                write!(formatter, "{what} roster exceeds the hard bound")
            }
            Self::IdentityInvalid(what) => write!(formatter, "invalid identity: {what}"),
            Self::NonCanonicalOrder(what) => {
                write!(formatter, "{what} roster is not in canonical order")
            }
        }
    }
}

impl std::error::Error for DescriptionDecodeRejection {}
