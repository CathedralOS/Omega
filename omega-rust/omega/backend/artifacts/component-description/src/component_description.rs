//! Canonical component descriptions: the bounded, byte-replayed carrier that
//! publishes a checked component's exact semantic subject, entry roster,
//! outgoing authority, custody constraints, retained providers, and
//! installation obligations.
//!
//! A description is self-contained evidence, not authority: it embeds the
//! canonical Terminal artifact (semantic module and sealed proof section) and
//! every claim a verifier can independently replay from those bytes. Reading
//! a description grants no callable authority, no installed custody, and no
//! resource admission; installation-dependent facts are emitted only as
//! obligations that a later occurrence must satisfy under a fresh profile.
//!
//! The producer [`describe_component_facts`] publishes this source-free
//! carrier from independently supplied facts; `component-candidate` fills
//! those facts from a realized `ComponentCandidate`. The independent consumer
//! lives in `component_verification::verify_component`, which re-decodes the
//! embedded artifact and re-derives every description fact rather than
//! trusting the producer's rows.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BoundaryMachineId, MachineId, ServiceId};
use sha2::{Digest, Sha256};
use terminal_psi::{
    OperationKind, SemanticFingerprint, TerminalModule, TerminalPsiIdentity, VocabularyMarker,
};

/// Wire magic for the canonical component-description encoding.
const DESCRIPTION_MAGIC: &[u8; 8] = b"OMGCMPD\0";
/// Domain prefix for the description's own collision-resistant identity.
const DESCRIPTION_IDENTITY_DOMAIN: &[u8] = b"omega-component-description-v1";
/// Domain prefix for one import slot's requirement contract identity.
const REQUIREMENT_CONTRACT_DOMAIN: &[u8] = b"omega-component-requirement-v1";
/// Domain prefix for a derived port-space mechanism assumption.
const PORT_MECHANISM_DOMAIN: &[u8] = b"omega-component-port-mechanism-v1";

/// Current and only published description schema.
pub const COMPONENT_DESCRIPTION_SCHEMA_V1: u32 = 1;

/// Hard bound on one encoded description, including the embedded artifact.
pub const MAX_COMPONENT_DESCRIPTION_BYTES: usize = 8 * 1024 * 1024;
/// Hard bound on any single roster inside a description.
pub const MAX_DESCRIPTION_ROSTER: usize = 4096;
/// Hard bound on one declared or derived identity string.
pub const MAX_IDENTITY_BYTES: usize = 1024;

/// How far the checked component evidence has advanced.
///
/// Only [`DescriptionFrontier::TerminalArtifactClosure`] is a complete
/// description: the embedded artifact closes the semantic module, and every
/// roster is replayable from it plus the retained selected-provider facts.
/// Earlier frontiers describe a component before its closure exists and are
/// rejected by `verify_component` rather than treated as complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DescriptionFrontier {
    /// The component was declared in source but no checked closure exists.
    AuthoredDeclaration,
    /// A provider plan was selected but the artifact closure was not bound.
    SelectedPlan,
    /// The canonical Terminal artifact closes the component.
    TerminalArtifactClosure,
}

impl DescriptionFrontier {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::AuthoredDeclaration => 1,
            Self::SelectedPlan => 2,
            Self::TerminalArtifactClosure => 3,
        }
    }

    fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::AuthoredDeclaration),
            2 => Some(Self::SelectedPlan),
            3 => Some(Self::TerminalArtifactClosure),
            _ => None,
        }
    }
}

/// One import slot: an outgoing requirement the component demands but does
/// not itself seal to a retained provider. Installation must bind each slot
/// to an exact provider occurrence under a fresh profile.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImportSlot {
    pub slot: u32,
    /// Canonical identity of the boundary requirement left unsealed.
    pub requirement_identity: String,
    /// Collision-resistant contract identity for the requirement spelling.
    pub contract_identity: [u8; 32],
}

/// One exported callable surface offered to the component's callers.
///
/// Two export families exist, both re-derived from the embedded artifact:
/// the canonical entry (`export:canonical:{machine}`) and one row per checked
/// provider candidate the module retains
/// ([`requirement_export_identity`]). The second family is what an
/// independently selected provider component offers to a consumer's
/// `Independent` selection: the exact requirement it realizes, the provider
/// type that realizes it, and the checked candidate machine identity the
/// selected `CheckedAdapter` row names. A description cannot invent or omit
/// either family; the verifier compares the roster row for row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExportSurface {
    /// Canonical export identity.
    pub identity: String,
}

/// Canonical export identity of one checked provider realization retained by
/// the module's provider-candidate catalog. The three identities are the
/// exact join coordinates the selected-plan join uses; the spelling is a
/// string identity compared whole, never parsed.
pub fn requirement_export_identity(
    requirement_identity: &str,
    provider_identity: &str,
    candidate_identity: &str,
) -> String {
    format!("export:requirement:{requirement_identity}|{provider_identity}|{candidate_identity}")
}

/// Every possible way execution may enter the described component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentEntryKind {
    /// The module's canonical selected entry machine.
    Canonical,
    /// A suspension call site whose stored frontier re-enters on resume.
    SuspensionResumption,
    /// Startup behavior the component expects installation to run.
    Startup,
    /// A callback registration the component performs.
    RegisteredCallback,
    /// A timer registration the component performs.
    Timer,
    /// A cleanup route the component requires on removal.
    Cleanup,
    /// Entry through a retained provider owned by this component.
    RetainedProvider,
}

impl ComponentEntryKind {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::Canonical => 1,
            Self::SuspensionResumption => 2,
            Self::Startup => 3,
            Self::RegisteredCallback => 4,
            Self::Timer => 5,
            Self::Cleanup => 6,
            Self::RetainedProvider => 7,
        }
    }

    fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Canonical),
            2 => Some(Self::SuspensionResumption),
            3 => Some(Self::Startup),
            4 => Some(Self::RegisteredCallback),
            5 => Some(Self::Timer),
            6 => Some(Self::Cleanup),
            7 => Some(Self::RetainedProvider),
            _ => None,
        }
    }
}

/// Evidence binding one entry row to independently checkable fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryEvidence {
    /// The entry is re-derived from the embedded artifact itself.
    ModuleDerived,
    /// The entry is declared by the producer and inseparable from the named
    /// assumption digest, which the verifier's profile must accept.
    AssumptionBound([u8; 32]),
}

/// One row of the component's complete entry roster.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentEntry {
    pub kind: ComponentEntryKind,
    pub identity: String,
    pub evidence: EntryEvidence,
}

/// Classes of authority that can leave the component's own closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutgoingAuthorityClass {
    /// A bodyless boundary machine invoked by a `BoundaryCall` operation.
    BoundaryRequirement,
    /// An immediate port-space write naming its exact service identity.
    PortSpaceWrite,
    /// The published service ceiling of the selected entry's reach.
    ServiceCeiling,
}

impl OutgoingAuthorityClass {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::BoundaryRequirement => 1,
            Self::PortSpaceWrite => 2,
            Self::ServiceCeiling => 3,
        }
    }

    fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::BoundaryRequirement),
            2 => Some(Self::PortSpaceWrite),
            3 => Some(Self::ServiceCeiling),
            _ => None,
        }
    }
}

/// Evidence binding one outgoing-authority row to checkable fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OutgoingEvidence {
    /// The row is re-derived from the embedded artifact and remains an
    /// unsealed demand: installation must bind it per occurrence.
    ModuleDerived,
    /// The row is sealed by the retained provider plan with this exact
    /// strong plan digest. The digest must name a provider in the
    /// description's retained-provider roster.
    ProviderSealed([u8; 32]),
    /// The row is module-derived and additionally requires a hardware- or
    /// environment-mediating assumption with this exact digest.
    PhysicalMechanism([u8; 32]),
    /// The row is declared and inseparable from the named assumption digest.
    AssumptionBound([u8; 32]),
}

/// One row of the component's complete outgoing-authority roster.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OutgoingAuthority {
    pub class: OutgoingAuthorityClass,
    pub identity: String,
    pub evidence: OutgoingEvidence,
}

/// Ownership, custody, and capability constraints the component carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CustodyKind {
    /// A closure-wide direct entry input bound to one exact placed view.
    PlacedViewInput,
    /// An exact direct-root custody handoff over a finite reborrow lineage.
    ReborrowRootHandoff,
    /// A whole-parent mutating call reactivating a direct mutable root.
    ReborrowRestoredCall,
    /// A live caller claim consumed by a successful boundary invocation.
    CompletionReceipt,
    /// A per-occurrence capacity schema authorized by a called boundary.
    ProgramLocalRootIntroduction,
    /// A provider assumption retained by a called boundary's content
    /// guarantee.
    BoundaryContentGuarantee,
    /// Source-free local dynamic selection or dispatch custody.
    DynamicDescriptorCustody,
    /// The exact live frontier carried across a possibly-suspending call.
    SuspensionFrontier,
}

impl CustodyKind {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::PlacedViewInput => 1,
            Self::ReborrowRootHandoff => 2,
            Self::ReborrowRestoredCall => 3,
            Self::CompletionReceipt => 4,
            Self::ProgramLocalRootIntroduction => 5,
            Self::BoundaryContentGuarantee => 6,
            Self::DynamicDescriptorCustody => 7,
            Self::SuspensionFrontier => 8,
        }
    }

    fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::PlacedViewInput),
            2 => Some(Self::ReborrowRootHandoff),
            3 => Some(Self::ReborrowRestoredCall),
            4 => Some(Self::CompletionReceipt),
            5 => Some(Self::ProgramLocalRootIntroduction),
            6 => Some(Self::BoundaryContentGuarantee),
            7 => Some(Self::DynamicDescriptorCustody),
            8 => Some(Self::SuspensionFrontier),
            _ => None,
        }
    }
}

/// Evidence binding one custody row to checkable fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CustodyEvidence {
    /// The row is re-derived from the embedded artifact.
    ModuleDerived,
    /// The row is declared and inseparable from the named assumption digest.
    AssumptionBound([u8; 32]),
}

/// One row of the component's custody roster. These constraints preserve
/// custody; they grant no cleanup, transfer, or discharge authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CustodyConstraint {
    pub kind: CustodyKind,
    pub identity: String,
    pub evidence: CustodyEvidence,
}

/// One provider retained by the selected component closure. The strong
/// `plan_digest` is the only authoritative plan identity; `report_identity`
/// is a non-authoritative compact coordinate for reports and cache joins.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RetainedProvider {
    pub report_identity: u64,
    pub plan_digest: [u8; 32],
    /// Exact requirement identities this provider seals for the component.
    pub requirement_identities: Vec<String>,
}

/// Installation-dependent facts the description can demand but never
/// satisfy. Each kind requires fresh per-occurrence resource and profile
/// admission at installation; compilation or provider code alone cannot
/// discharge them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObligationKind {
    /// Provision the exact selected-entry stack closure demand.
    StackProvision,
    /// Admit one retained provider occurrence under a fresh profile.
    ProviderOccurrence,
    /// Bind one import slot to an exact provider at installation.
    ImportBinding,
    /// Satisfy one build-bound progress demand retained by the manifest.
    ProgressDemand,
    /// Admit per-occurrence resource capacity named by a boundary schema.
    ResourceAdmission,
}

impl ObligationKind {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::StackProvision => 1,
            Self::ProviderOccurrence => 2,
            Self::ImportBinding => 3,
            Self::ProgressDemand => 4,
            Self::ResourceAdmission => 5,
        }
    }

    fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::StackProvision),
            2 => Some(Self::ProviderOccurrence),
            3 => Some(Self::ImportBinding),
            4 => Some(Self::ProgressDemand),
            5 => Some(Self::ResourceAdmission),
            _ => None,
        }
    }
}

/// One installation obligation. The `identity` is the canonical obligation
/// coordinate; `detail` carries bounded human-readable context only.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstallationObligation {
    pub kind: ObligationKind,
    pub identity: String,
    pub detail: String,
}

/// The complete canonical component description.
///
/// Every roster is kept in canonical order by the codec; construction outside
/// `describe_component` is possible but the verifier re-derives and compares
/// every module-derived row, so a hand-authored description cannot claim
/// facts the embedded artifact does not contain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDescription {
    pub schema: u32,
    pub frontier: DescriptionFrontier,
    /// Canonical bytes of the sealed Terminal artifact this description
    /// publishes. The codec validates them with
    /// `CanonicalTerminalArtifact::from_bytes` at every boundary.
    pub artifact_bytes: Vec<u8>,
    /// Exact unsealed imports installation must still bind.
    pub imports: Vec<ImportSlot>,
    /// Exact exported callable surfaces.
    pub exports: Vec<ExportSurface>,
    /// Every possible entry into the component.
    pub entries: Vec<ComponentEntry>,
    /// Every authority path leaving the component closure.
    pub outgoing: Vec<OutgoingAuthority>,
    /// Every custody constraint the component carries.
    pub custody: Vec<CustodyConstraint>,
    /// The retained selected-provider roster.
    pub providers: Vec<RetainedProvider>,
    /// Strong digest of the complete selected provider closure facts this
    /// roster was drawn from. `ComponentCandidate::checked` already binds it
    /// to the artifact; the verifier checks internal consistency here.
    pub provider_closure_digest: [u8; 32],
    /// Installation-dependent demands, never satisfied by the description.
    pub obligations: Vec<InstallationObligation>,
    /// Inseparable assumptions the consumer's profile must accept before
    /// any declared entry, authority, or custody row bound to them verifies.
    pub assumptions: Vec<[u8; 32]>,
    /// Strong identity of the native realization, when one is bound.
    /// Artifact evidence only; never installation or invocation authority.
    pub realization_identity: Option<[u8; 32]>,
}

/// Producer-side failure to describe a candidate.
#[derive(Debug)]
pub enum DescribeError {
    /// The embedded semantic bytes did not decode to a canonical module.
    ArtifactDecode(terminal_codec::CodecError),
    /// The embedded canonical artifact bytes failed artifact validation.
    ArtifactFormat(terminal_codec::CanonicalTerminalArtifactError),
    /// The artifact identity could not be reconstructed.
    ArtifactIdentity(terminal_codec::CodecError),
    /// A called boundary machine had no declaration in the module.
    UndeclaredBoundary(BoundaryMachineId),
}

impl std::fmt::Display for DescribeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArtifactDecode(error) => {
                write!(
                    formatter,
                    "component semantic artifact did not decode: {error}"
                )
            }
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
        }
    }
}

impl std::error::Error for DescribeError {}

/// Decode failures for the canonical description wire format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptionDecodeRejection {
    /// Input is empty, truncated, or has trailing bytes.
    Corrupt(&'static str),
    /// The wire magic did not match.
    InvalidMagic,
    /// An unknown schema, frontier, kind, class, or evidence tag was read.
    UnsupportedTag(&'static str),
    /// A roster exceeded its hard bound.
    RosterBoundExceeded(&'static str),
    /// An identity string exceeded its bound or was not UTF-8.
    IdentityInvalid(&'static str),
    /// A roster was not in canonical order or contained a duplicate.
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

/// Collision-resistant identity of one exact encoded description.
pub fn component_description_identity(canonical_bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(DESCRIPTION_IDENTITY_DOMAIN);
    digest.update(
        u64::try_from(canonical_bytes.len())
            .unwrap_or(0)
            .to_le_bytes(),
    );
    digest.update(canonical_bytes);
    digest.finalize().into()
}

/// Collision-resistant contract identity of one requirement spelling.
pub fn requirement_contract_identity(requirement_identity: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(REQUIREMENT_CONTRACT_DOMAIN);
    digest.update(
        u64::try_from(requirement_identity.len())
            .unwrap_or(0)
            .to_le_bytes(),
    );
    digest.update(requirement_identity.as_bytes());
    digest.finalize().into()
}

/// Derived assumption digest for one immediate port-space mechanism.
pub fn port_mechanism_assumption(service: ServiceId, port: u16, value: u8) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(PORT_MECHANISM_DOMAIN);
    digest.update(service.get().to_le_bytes());
    digest.update(port.to_le_bytes());
    digest.update([value]);
    digest.finalize().into()
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Every fact `verify_component` can re-derive from the embedded artifact
/// alone. The producer emits exactly these rows (plus declared rows bound to
/// assumptions and selected-provider seals); the verifier rejects any
/// module-derived row that is missing or does not match.
pub(crate) struct DerivedInventory {
    /// Requirement identities of every called boundary, canonical order.
    pub(crate) called_requirement_identities: Vec<String>,
    /// Module-derived entry rows.
    pub(crate) entries: Vec<ComponentEntry>,
    /// Module-derived export rows.
    pub(crate) exports: Vec<ExportSurface>,
    /// Module-derived custody rows.
    pub(crate) custody: Vec<CustodyConstraint>,
    /// Exact port-space writes as `(service, port, value)`.
    pub(crate) port_writes: BTreeSet<(ServiceId, u16, u8)>,
    /// Exact published service ceiling of the selected entry reach.
    pub(crate) service_ceiling: BTreeSet<ServiceId>,
}

/// Re-derive the complete module-evident inventory of a decoded artifact.
///
/// This is the shared producer/verifier derivation: the producer uses it to
/// emit rows and the verifier re-runs it on its own decode so no producer row
/// can be trusted rather than replayed.
pub(crate) fn derive_component_inventory(
    module: &TerminalModule,
) -> Result<DerivedInventory, DescribeError> {
    let mut called_boundaries = BTreeSet::new();
    let mut port_writes = BTreeSet::new();
    let mut custody = Vec::new();

    for machine in &module.machines {
        for block in &machine.blocks {
            for operation in &block.operations {
                // Every operation kind must be placed in an explicit group:
                // an authority-bearing kind added to `OperationKind` without a
                // decision here stops compiling rather than silently reading
                // as empty outgoing authority. Operations are the complete
                // authority surface: block terminators are closure-internal
                // control flow, suspension resumptions are the module-level
                // call-site table counted below, and in-module calls resolve
                // through the verified module's own machine and
                // dynamic-dispatch tables, so their callee's operations are
                // scanned by this same enumeration and they carry no
                // authority past the component closure.
                match &operation.kind {
                    OperationKind::BoundaryCall {
                        boundary,
                        completion_receipts,
                        ..
                    } => {
                        called_boundaries.insert(*boundary);
                        for receipt in completion_receipts {
                            custody.push(CustodyConstraint {
                                kind: CustodyKind::CompletionReceipt,
                                identity: format!(
                                    "completion-receipt:{}:{}:{}:{}",
                                    boundary.get(),
                                    operation.id.get(),
                                    receipt.claim.get(),
                                    receipt.argument_index,
                                ),
                                evidence: CustodyEvidence::ModuleDerived,
                            });
                        }
                    }
                    OperationKind::PortWrite {
                        service,
                        port,
                        value,
                    } => {
                        port_writes.insert((*service, *port, *value));
                    }
                    OperationKind::Call { .. }
                    | OperationKind::CallUnit { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralScalar { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
                    | OperationKind::CallDynamicScalar { .. }
                    | OperationKind::CallDynamicParameterScalar { .. }
                    | OperationKind::CallDynamicUnit { .. }
                    | OperationKind::CallDynamicParameterUnit { .. }
                    | OperationKind::StoreDynamicDescriptor { .. } => {}
                    OperationKind::EstablishReference { .. }
                    | OperationKind::ReleaseReference { .. }
                    | OperationKind::EstablishScalarArray { .. }
                    | OperationKind::EstablishPrimitiveLocal { .. }
                    | OperationKind::EstablishTrivialAffineLocal { .. }
                    | OperationKind::EstablishRecord { .. }
                    | OperationKind::EstablishScalarCase { .. }
                    | OperationKind::EstablishByteSequenceLiteral { .. }
                    | OperationKind::PrimitiveScalarRead { .. }
                    | OperationKind::StructuralScalarFieldStore { .. }
                    | OperationKind::StructuralByteSequenceFieldLength { .. }
                    | OperationKind::StructuralByteSequenceFieldByteStore { .. }
                    | OperationKind::StructuralByteSequenceFieldStore { .. }
                    | OperationKind::StructuralCaseMembership { .. }
                    | OperationKind::WriteOnlyPrimitiveStore { .. }
                    | OperationKind::WriteOnlyIndexedPrimitiveStore { .. }
                    | OperationKind::ByteSequenceLength { .. }
                    | OperationKind::ByteSequenceRead { .. }
                    | OperationKind::ByteSequenceWrite { .. }
                    | OperationKind::ByteSequenceSubslice { .. } => {}
                    OperationKind::IntegerConstant { .. }
                    | OperationKind::BooleanConstant { .. }
                    | OperationKind::IeeeFloatConstant { .. }
                    | OperationKind::IeeeFloatCompare { .. }
                    | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
                    | OperationKind::BooleanStructuralField { .. }
                    | OperationKind::IntegerStructuralField { .. }
                    | OperationKind::BooleanNot { .. }
                    | OperationKind::BooleanEqual { .. }
                    | OperationKind::IntegerEqual { .. }
                    | OperationKind::IntegerLessThan { .. }
                    | OperationKind::IntegerLessOrEqual { .. }
                    | OperationKind::IntegerBitwiseNot { .. }
                    | OperationKind::IntegerBitwiseAnd { .. }
                    | OperationKind::IntegerBitwiseOr { .. }
                    | OperationKind::IntegerBitwiseXor { .. }
                    | OperationKind::IntegerWiden { .. }
                    | OperationKind::IntegerExactCast { .. }
                    | OperationKind::WrappingIntegerShiftLeft { .. }
                    | OperationKind::WrappingIntegerShiftRight { .. }
                    | OperationKind::ExactIntegerShiftLeft { .. }
                    | OperationKind::ExactIntegerShiftRight { .. }
                    | OperationKind::WrappingIntegerAdd { .. }
                    | OperationKind::SaturatingIntegerAdd { .. }
                    | OperationKind::ExactIntegerAdd { .. }
                    | OperationKind::WrappingIntegerSubtract { .. }
                    | OperationKind::SaturatingIntegerSubtract { .. }
                    | OperationKind::ExactIntegerSubtract { .. }
                    | OperationKind::WrappingIntegerMultiply { .. }
                    | OperationKind::SaturatingIntegerMultiply { .. }
                    | OperationKind::ExactIntegerMultiply { .. }
                    | OperationKind::WrappingIntegerDivide { .. }
                    | OperationKind::SaturatingIntegerDivide { .. }
                    | OperationKind::ExactIntegerDivide { .. }
                    | OperationKind::WrappingIntegerRemainder { .. }
                    | OperationKind::SaturatingIntegerRemainder { .. }
                    | OperationKind::ExactIntegerRemainder { .. }
                    | OperationKind::MoveStructuralField { .. }
                    | OperationKind::StoreStructuralField { .. } => {}
                }
            }
        }
    }

    let declarations: BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration> =
        module
            .boundary_machines
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect();

    let mut called_requirement_identities = Vec::new();
    for boundary in &called_boundaries {
        let declaration = declarations
            .get(boundary)
            .ok_or(DescribeError::UndeclaredBoundary(*boundary))?;
        called_requirement_identities.push(declaration.identity.clone());
        for schema in &declaration.program_local_root_introductions {
            custody.push(CustodyConstraint {
                kind: CustodyKind::ProgramLocalRootIntroduction,
                identity: format!(
                    "root-introduction:{}:{}",
                    declaration.identity, schema.argument_index,
                ),
                evidence: CustodyEvidence::ModuleDerived,
            });
        }
        for (index, _guarantee) in declaration.content_guarantees.iter().enumerate() {
            custody.push(CustodyConstraint {
                kind: CustodyKind::BoundaryContentGuarantee,
                identity: format!("content-guarantee:{}:{index}", declaration.identity),
                evidence: CustodyEvidence::ModuleDerived,
            });
        }
    }
    called_requirement_identities.sort();
    called_requirement_identities.dedup();

    for input in &module.placed_view_inputs {
        custody.push(CustodyConstraint {
            kind: CustodyKind::PlacedViewInput,
            identity: format!(
                "placed-view:{}:{}:{}",
                input.machine.get(),
                input.position,
                input.view_identity,
            ),
            evidence: CustodyEvidence::ModuleDerived,
        });
    }
    for handoff in &module.reborrow_root_handoffs {
        custody.push(CustodyConstraint {
            kind: CustodyKind::ReborrowRootHandoff,
            identity: format!(
                "reborrow-handoff:{}:{}",
                handoff.machine.get(),
                handoff.direct_root_lifetime_identity,
            ),
            evidence: CustodyEvidence::ModuleDerived,
        });
    }
    for restored in &module.reborrow_restored_call_uses {
        custody.push(CustodyConstraint {
            kind: CustodyKind::ReborrowRestoredCall,
            identity: format!(
                "reborrow-call:{}:{}",
                restored.machine.get(),
                restored.operation.get(),
            ),
            evidence: CustodyEvidence::ModuleDerived,
        });
    }

    let dispatch = &module.dynamic_dispatch;
    let lanes = [
        ("parameters", dispatch.parameters.len()),
        ("arguments", dispatch.arguments.len()),
        ("selections", dispatch.selections.len()),
        ("rebound-descriptors", dispatch.rebound_descriptors.len()),
        ("stored-descriptors", dispatch.stored_descriptors.len()),
        ("direct-dispatches", dispatch.direct_dispatches.len()),
        ("indirect-dispatches", dispatch.indirect_dispatches.len()),
        ("stored-dispatches", dispatch.stored_dispatches.len()),
        ("parameter-dispatches", dispatch.parameter_dispatches.len()),
    ];
    for (lane, count) in lanes {
        if count > 0 {
            custody.push(CustodyConstraint {
                kind: CustodyKind::DynamicDescriptorCustody,
                identity: format!("dynamic:{lane}:{count}"),
                evidence: CustodyEvidence::ModuleDerived,
            });
        }
    }

    let mut entries = Vec::new();
    entries.push(ComponentEntry {
        kind: ComponentEntryKind::Canonical,
        identity: format!("canonical-entry:{}", module.entry.get()),
        evidence: EntryEvidence::ModuleDerived,
    });
    for site in &module.suspension_call_sites {
        entries.push(ComponentEntry {
            kind: ComponentEntryKind::SuspensionResumption,
            identity: format!(
                "suspension-resumption:{}:{}",
                site.operation.get(),
                site.crossing.get(),
            ),
            evidence: EntryEvidence::ModuleDerived,
        });
        custody.push(CustodyConstraint {
            kind: CustodyKind::SuspensionFrontier,
            identity: format!(
                "suspension-frontier:{}:{}:{}",
                site.operation.get(),
                site.crossing.get(),
                hex(&site.frontier_commitment),
            ),
            evidence: CustodyEvidence::ModuleDerived,
        });
    }

    let mut service_ceiling = BTreeSet::new();
    service_ceiling.extend(module.root_service_reach.concrete.iter().copied());
    for dependency in &module.root_service_reach.installation_dependencies {
        service_ceiling.extend(dependency.upper_bound.iter().copied());
    }

    let mut exports = vec![ExportSurface {
        identity: format!("export:canonical:{}", module.entry.get()),
    }];
    // Every checked provider candidate the module retains is an exported
    // realization: a consumer's `Independent` selection joins its selected
    // plan rows to exactly these coordinates. The catalog is semantic, not a
    // selection, so exporting it grants nothing; installation still binds an
    // occurrence per selected row.
    for candidate in &module.provider_candidates {
        exports.push(ExportSurface {
            identity: requirement_export_identity(
                &candidate.requirement_identity,
                &candidate.provider_identity,
                &candidate.candidate_identity,
            ),
        });
    }
    exports.sort();
    exports.dedup();

    custody.sort();
    custody.dedup();
    entries.sort();
    entries.dedup();

    Ok(DerivedInventory {
        called_requirement_identities,
        entries,
        exports,
        custody,
        port_writes,
        service_ceiling,
    })
}

/// The selected entry's internal stack demand reduced to the three facts the
/// description publishes as its `StackProvision` obligation.
///
/// The emitter derives the complete demand (with its target, contributing
/// machines, and admitted contributions) beside the native artifact; the
/// description keeps only the entry, ceiling, and alignment that installation
/// must provision, so this crate never imports image emission. The row is
/// declared evidence: it names a demand and grants no provision, lease, or
/// installed-root admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackDemandFacts {
    /// The selected component entry machine the demand closes over.
    pub entry: MachineId,
    /// Exact internal call-graph stack ceiling in bytes.
    pub ceiling_bytes: u64,
    /// Required stack alignment in bytes.
    pub stack_alignment: u32,
}

/// The exact publication facts a component description is built from.
///
/// `stack_demand` and `realization_identity` are optional so the same
/// producer covers a Psi-only component capsule (no native realization yet)
/// and a fully realized `ComponentCandidate`; `component-candidate`'s
/// `describe_component` supplies both.
#[derive(Debug, Clone, Copy)]
pub struct ComponentDescriptionFacts<'a> {
    /// The sealed canonical Terminal artifact to embed.
    pub artifact: &'a terminal_codec::CanonicalTerminalArtifact,
    /// The complete selected provider-plan facts for this component.
    pub selected_provider_plans: &'a effects::SelectedProviderPlanFacts,
    /// Any retained build-bound progress manifest.
    pub component_progress: Option<&'a effects::ComponentProgressManifest>,
    /// Emitter-derived internal stack demand for the selected entry, when a
    /// native realization exists.
    pub stack_demand: Option<StackDemandFacts>,
    /// Strong identity of the bound native realization, when one exists.
    pub realization_identity: Option<[u8; 32]>,
}

/// Publish one canonical description from independently supplied facts.
///
/// The produced carrier embeds the sealed canonical artifact and emits the
/// module-derived inventory plus the retained selected-provider facts. This
/// is publication evidence only: it cannot satisfy its own installation
/// obligations or grant callable authority to any reader.
pub fn describe_component_facts(
    facts: ComponentDescriptionFacts<'_>,
) -> Result<ComponentDescription, DescribeError> {
    let artifact_bytes = facts.artifact.to_bytes();
    let module = terminal_codec::decode_module(facts.artifact.semantic_bytes())
        .map_err(DescribeError::ArtifactDecode)?;
    let inventory = derive_component_inventory(&module)?;

    let selected = facts.selected_provider_plans;
    let providers: Vec<RetainedProvider> = selected
        .plans()
        .iter()
        .map(|plan| {
            let mut requirement_identities: Vec<String> = plan
                .rows
                .iter()
                .map(|row| row.requirement_identity.clone())
                .collect();
            requirement_identities.sort();
            requirement_identities.dedup();
            RetainedProvider {
                report_identity: plan.report_fingerprint(),
                plan_digest: *plan.identity_digest().as_bytes(),
                requirement_identities,
            }
        })
        .collect();

    let mut sealed: BTreeMap<&str, [u8; 32]> = BTreeMap::new();
    for provider in &providers {
        for requirement in &provider.requirement_identities {
            sealed
                .entry(requirement.as_str())
                .or_insert(provider.plan_digest);
        }
    }

    let mut imports = Vec::new();
    let mut outgoing = Vec::new();
    let mut assumptions = Vec::new();
    let mut obligations = Vec::new();

    for requirement_identity in &inventory.called_requirement_identities {
        let evidence = match sealed.get(requirement_identity.as_str()) {
            Some(plan_digest) => OutgoingEvidence::ProviderSealed(*plan_digest),
            None => OutgoingEvidence::ModuleDerived,
        };
        outgoing.push(OutgoingAuthority {
            class: OutgoingAuthorityClass::BoundaryRequirement,
            identity: requirement_identity.clone(),
            evidence,
        });
        if !sealed.contains_key(requirement_identity.as_str()) {
            let slot = u32::try_from(imports.len()).unwrap_or(u32::MAX);
            imports.push(ImportSlot {
                slot,
                requirement_identity: requirement_identity.clone(),
                contract_identity: requirement_contract_identity(requirement_identity),
            });
        }
    }

    for (service, port, value) in &inventory.port_writes {
        let assumption = port_mechanism_assumption(*service, *port, *value);
        assumptions.push(assumption);
        outgoing.push(OutgoingAuthority {
            class: OutgoingAuthorityClass::PortSpaceWrite,
            identity: format!("port-write:{}:{port}:{value}", service.get()),
            evidence: OutgoingEvidence::PhysicalMechanism(assumption),
        });
    }

    for service in &inventory.service_ceiling {
        outgoing.push(OutgoingAuthority {
            class: OutgoingAuthorityClass::ServiceCeiling,
            identity: format!("service-ceiling:{}", service.get()),
            evidence: OutgoingEvidence::ModuleDerived,
        });
    }

    if let Some(stack) = facts.stack_demand {
        obligations.push(InstallationObligation {
            kind: ObligationKind::StackProvision,
            identity: format!(
                "stack-provision:{}:{}:{}",
                stack.entry.get(),
                stack.ceiling_bytes,
                stack.stack_alignment,
            ),
            detail: "provision the exact selected-entry stack closure under a fresh profile"
                .to_string(),
        });
    }
    for import in &imports {
        obligations.push(InstallationObligation {
            kind: ObligationKind::ImportBinding,
            identity: import.requirement_identity.clone(),
            detail: "bind one exact provider occurrence for this import".to_string(),
        });
    }
    for provider in &providers {
        obligations.push(InstallationObligation {
            kind: ObligationKind::ProviderOccurrence,
            identity: format!("provider-occurrence:{}", hex(&provider.plan_digest)),
            detail: "admit one retained provider occurrence under a fresh profile".to_string(),
        });
    }
    for row in &inventory.custody {
        if row.kind == CustodyKind::ProgramLocalRootIntroduction {
            obligations.push(InstallationObligation {
                kind: ObligationKind::ResourceAdmission,
                identity: row.identity.clone(),
                detail: "admit the per-occurrence resource capacity of this schema".to_string(),
            });
        }
    }
    if let Some(manifest) = facts.component_progress {
        for demand in manifest.pending() {
            obligations.push(InstallationObligation {
                kind: ObligationKind::ProgressDemand,
                identity: format!(
                    "progress-demand:{}:{}:{}",
                    demand.requirement_identity,
                    demand.profile_identity,
                    hex(demand.provider_plan_digest.as_bytes()),
                ),
                detail: "satisfy the retained build-bound progress demand".to_string(),
            });
        }
    }

    assumptions.sort();
    assumptions.dedup();
    imports.sort();
    outgoing.sort();
    obligations.sort();
    obligations.dedup();

    Ok(ComponentDescription {
        schema: COMPONENT_DESCRIPTION_SCHEMA_V1,
        frontier: DescriptionFrontier::TerminalArtifactClosure,
        artifact_bytes,
        imports,
        exports: inventory.exports,
        entries: inventory.entries,
        outgoing,
        custody: inventory.custody,
        providers,
        provider_closure_digest: *selected.identity_digest().as_bytes(),
        obligations,
        assumptions,
        realization_identity: facts.realization_identity,
    })
}

// ---------------------------------------------------------------------------
// Canonical wire codec
// ---------------------------------------------------------------------------

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], DescriptionDecodeRejection> {
        if self.remaining() < count {
            return Err(DescriptionDecodeRejection::Corrupt("truncated input"));
        }
        let slice = &self.bytes[self.offset..self.offset + count];
        self.offset += count;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, DescriptionDecodeRejection> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, DescriptionDecodeRejection> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| DescriptionDecodeRejection::Corrupt("truncated u32"))?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, DescriptionDecodeRejection> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .map_err(|_| DescriptionDecodeRejection::Corrupt("truncated u64"))?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DescriptionDecodeRejection> {
        self.take(N)?
            .try_into()
            .map_err(|_| DescriptionDecodeRejection::Corrupt("truncated fixed bytes"))
    }

    fn roster_len(&mut self, what: &'static str) -> Result<usize, DescriptionDecodeRejection> {
        let count = self.u32()? as usize;
        if count > MAX_DESCRIPTION_ROSTER {
            return Err(DescriptionDecodeRejection::RosterBoundExceeded(what));
        }
        Ok(count)
    }

    fn identity(&mut self) -> Result<String, DescriptionDecodeRejection> {
        let length = self.u32()? as usize;
        if length > MAX_IDENTITY_BYTES {
            return Err(DescriptionDecodeRejection::IdentityInvalid("too long"));
        }
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|_| DescriptionDecodeRejection::IdentityInvalid("not utf-8"))
    }
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_identity(out: &mut Vec<u8>, identity: &str) {
    push_u32(out, identity.len() as u32);
    out.extend_from_slice(identity.as_bytes());
}

/// Encode one description to its canonical wire form.
///
/// The encoder emits every roster in sorted order; decoding any other order
/// is rejected as non-canonical, so the encoding is a bijection between
/// semantically equal descriptions and their byte form.
pub fn encode_component_description(description: &ComponentDescription) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(DESCRIPTION_MAGIC);
    push_u32(&mut out, description.schema);
    out.push(description.frontier.tag());

    push_u32(&mut out, description.artifact_bytes.len() as u32);
    out.extend_from_slice(&description.artifact_bytes);

    let mut imports = description.imports.clone();
    imports.sort();
    push_u32(&mut out, imports.len() as u32);
    for slot in &imports {
        push_u32(&mut out, slot.slot);
        push_identity(&mut out, &slot.requirement_identity);
        out.extend_from_slice(&slot.contract_identity);
    }

    let mut exports = description.exports.clone();
    exports.sort();
    push_u32(&mut out, exports.len() as u32);
    for export in &exports {
        push_identity(&mut out, &export.identity);
    }

    let mut entries = description.entries.clone();
    entries.sort();
    push_u32(&mut out, entries.len() as u32);
    for entry in &entries {
        out.push(entry.kind.tag());
        push_identity(&mut out, &entry.identity);
        match entry.evidence {
            EntryEvidence::ModuleDerived => out.push(0),
            EntryEvidence::AssumptionBound(digest) => {
                out.push(1);
                out.extend_from_slice(&digest);
            }
        }
    }

    let mut outgoing = description.outgoing.clone();
    outgoing.sort();
    push_u32(&mut out, outgoing.len() as u32);
    for row in &outgoing {
        out.push(row.class.tag());
        push_identity(&mut out, &row.identity);
        match row.evidence {
            OutgoingEvidence::ModuleDerived => out.push(0),
            OutgoingEvidence::ProviderSealed(digest)
            | OutgoingEvidence::PhysicalMechanism(digest)
            | OutgoingEvidence::AssumptionBound(digest) => {
                out.push(match row.evidence {
                    OutgoingEvidence::ProviderSealed(_) => 1,
                    OutgoingEvidence::PhysicalMechanism(_) => 2,
                    OutgoingEvidence::AssumptionBound(_) => 3,
                    OutgoingEvidence::ModuleDerived => unreachable!(),
                });
                out.extend_from_slice(&digest);
            }
        }
    }

    let mut custody = description.custody.clone();
    custody.sort();
    push_u32(&mut out, custody.len() as u32);
    for row in &custody {
        out.push(row.kind.tag());
        push_identity(&mut out, &row.identity);
        match row.evidence {
            CustodyEvidence::ModuleDerived => out.push(0),
            CustodyEvidence::AssumptionBound(digest) => {
                out.push(1);
                out.extend_from_slice(&digest);
            }
        }
    }

    let mut providers = description.providers.clone();
    providers.sort();
    push_u32(&mut out, providers.len() as u32);
    for provider in &providers {
        push_u64(&mut out, provider.report_identity);
        out.extend_from_slice(&provider.plan_digest);
        push_u32(&mut out, provider.requirement_identities.len() as u32);
        for requirement in &provider.requirement_identities {
            push_identity(&mut out, requirement);
        }
    }
    out.extend_from_slice(&description.provider_closure_digest);

    let mut obligations = description.obligations.clone();
    obligations.sort();
    push_u32(&mut out, obligations.len() as u32);
    for obligation in &obligations {
        out.push(obligation.kind.tag());
        push_identity(&mut out, &obligation.identity);
        push_identity(&mut out, &obligation.detail);
    }

    let mut assumptions = description.assumptions.clone();
    assumptions.sort();
    push_u32(&mut out, assumptions.len() as u32);
    for assumption in &assumptions {
        out.extend_from_slice(assumption);
    }

    match description.realization_identity {
        Some(identity) => {
            out.push(1);
            out.extend_from_slice(&identity);
        }
        None => out.push(0),
    }
    out
}

/// Decode a canonical description, enforcing every bound and roster order.
pub fn decode_component_description(
    bytes: &[u8],
) -> Result<ComponentDescription, DescriptionDecodeRejection> {
    if bytes.len() > MAX_COMPONENT_DESCRIPTION_BYTES {
        return Err(DescriptionDecodeRejection::RosterBoundExceeded(
            "description byte length",
        ));
    }
    let mut reader = Reader::new(bytes);
    if reader.take(DESCRIPTION_MAGIC.len())? != DESCRIPTION_MAGIC {
        return Err(DescriptionDecodeRejection::InvalidMagic);
    }
    let schema = reader.u32()?;
    let frontier = DescriptionFrontier::from_tag(reader.u8()?)
        .ok_or(DescriptionDecodeRejection::UnsupportedTag("frontier tag"))?;

    let artifact_length = reader.u32()? as usize;
    let artifact_bytes = reader.take(artifact_length)?.to_vec();
    let _artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact_bytes)
        .map_err(|_| DescriptionDecodeRejection::Corrupt("embedded artifact did not decode"))?;

    let mut imports = Vec::new();
    let mut last_import: Option<(u32, String)> = None;
    for _ in 0..reader.roster_len("import")? {
        let slot = reader.u32()?;
        let requirement_identity = reader.identity()?;
        let contract_identity = reader.array::<32>()?;
        let key = (slot, requirement_identity.clone());
        if last_import.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("import"));
        }
        last_import = Some(key);
        imports.push(ImportSlot {
            slot,
            requirement_identity,
            contract_identity,
        });
    }

    let mut exports = Vec::new();
    let mut last_export: Option<String> = None;
    for _ in 0..reader.roster_len("export")? {
        let identity = reader.identity()?;
        if last_export.as_ref() >= Some(&identity) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("export"));
        }
        last_export = Some(identity.clone());
        exports.push(ExportSurface { identity });
    }

    let mut entries = Vec::new();
    let mut last_entry: Option<(u8, String)> = None;
    for _ in 0..reader.roster_len("entry")? {
        let kind = ComponentEntryKind::from_tag(reader.u8()?)
            .ok_or(DescriptionDecodeRejection::UnsupportedTag("entry kind"))?;
        let identity = reader.identity()?;
        let evidence = match reader.u8()? {
            0 => EntryEvidence::ModuleDerived,
            1 => EntryEvidence::AssumptionBound(reader.array::<32>()?),
            _ => return Err(DescriptionDecodeRejection::UnsupportedTag("entry evidence")),
        };
        let key = (kind.tag(), identity.clone());
        if last_entry.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("entry"));
        }
        last_entry = Some(key);
        entries.push(ComponentEntry {
            kind,
            identity,
            evidence,
        });
    }

    let mut outgoing = Vec::new();
    let mut last_outgoing: Option<(u8, String)> = None;
    for _ in 0..reader.roster_len("outgoing authority")? {
        let class = OutgoingAuthorityClass::from_tag(reader.u8()?).ok_or(
            DescriptionDecodeRejection::UnsupportedTag("authority class"),
        )?;
        let identity = reader.identity()?;
        let evidence = match reader.u8()? {
            0 => OutgoingEvidence::ModuleDerived,
            1 => OutgoingEvidence::ProviderSealed(reader.array::<32>()?),
            2 => OutgoingEvidence::PhysicalMechanism(reader.array::<32>()?),
            3 => OutgoingEvidence::AssumptionBound(reader.array::<32>()?),
            _ => {
                return Err(DescriptionDecodeRejection::UnsupportedTag(
                    "authority evidence",
                ));
            }
        };
        let key = (class.tag(), identity.clone());
        if last_outgoing.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder(
                "outgoing authority",
            ));
        }
        last_outgoing = Some(key);
        outgoing.push(OutgoingAuthority {
            class,
            identity,
            evidence,
        });
    }

    let mut custody = Vec::new();
    let mut last_custody: Option<(u8, String)> = None;
    for _ in 0..reader.roster_len("custody")? {
        let kind = CustodyKind::from_tag(reader.u8()?)
            .ok_or(DescriptionDecodeRejection::UnsupportedTag("custody kind"))?;
        let identity = reader.identity()?;
        let evidence = match reader.u8()? {
            0 => CustodyEvidence::ModuleDerived,
            1 => CustodyEvidence::AssumptionBound(reader.array::<32>()?),
            _ => {
                return Err(DescriptionDecodeRejection::UnsupportedTag(
                    "custody evidence",
                ));
            }
        };
        let key = (kind.tag(), identity.clone());
        if last_custody.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("custody"));
        }
        last_custody = Some(key);
        custody.push(CustodyConstraint {
            kind,
            identity,
            evidence,
        });
    }

    let mut providers = Vec::new();
    let mut last_provider: Option<u64> = None;
    for _ in 0..reader.roster_len("provider")? {
        let report_identity = reader.u64()?;
        let plan_digest = reader.array::<32>()?;
        let requirement_count = reader.roster_len("provider requirement")?;
        let mut requirement_identities = Vec::with_capacity(requirement_count);
        let mut last_requirement: Option<String> = None;
        for _ in 0..requirement_count {
            let requirement = reader.identity()?;
            if last_requirement.as_ref() >= Some(&requirement) {
                return Err(DescriptionDecodeRejection::NonCanonicalOrder(
                    "provider requirement",
                ));
            }
            last_requirement = Some(requirement.clone());
            requirement_identities.push(requirement);
        }
        if last_provider >= Some(report_identity) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("provider"));
        }
        last_provider = Some(report_identity);
        providers.push(RetainedProvider {
            report_identity,
            plan_digest,
            requirement_identities,
        });
    }
    let provider_closure_digest = reader.array::<32>()?;

    let mut obligations = Vec::new();
    let mut last_obligation: Option<(u8, String)> = None;
    for _ in 0..reader.roster_len("obligation")? {
        let kind = ObligationKind::from_tag(reader.u8()?).ok_or(
            DescriptionDecodeRejection::UnsupportedTag("obligation kind"),
        )?;
        let identity = reader.identity()?;
        let detail = reader.identity()?;
        let key = (kind.tag(), identity.clone());
        if last_obligation.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("obligation"));
        }
        last_obligation = Some(key);
        obligations.push(InstallationObligation {
            kind,
            identity,
            detail,
        });
    }

    let mut assumptions = Vec::new();
    let mut last_assumption: Option<[u8; 32]> = None;
    for _ in 0..reader.roster_len("assumption")? {
        let assumption = reader.array::<32>()?;
        if last_assumption >= Some(assumption) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("assumption"));
        }
        last_assumption = Some(assumption);
        assumptions.push(assumption);
    }

    let realization_identity = match reader.u8()? {
        0 => None,
        1 => Some(reader.array::<32>()?),
        _ => {
            return Err(DescriptionDecodeRejection::UnsupportedTag(
                "realization presence",
            ));
        }
    };

    if reader.remaining() != 0 {
        return Err(DescriptionDecodeRejection::Corrupt("trailing bytes"));
    }

    Ok(ComponentDescription {
        schema,
        frontier,
        artifact_bytes,
        imports,
        exports,
        entries,
        outgoing,
        custody,
        providers,
        provider_closure_digest,
        obligations,
        assumptions,
        realization_identity,
    })
}

/// Reconstruct the description's semantic subject from the embedded artifact.
///
/// The subject is never read from a description field: it is the
/// `TerminalPsiIdentity` reconstructed from the artifact's own decoded
/// semantic module, so no producer spelling can rename the component.
pub fn description_subject(
    description: &ComponentDescription,
) -> Result<TerminalPsiIdentity, DescribeError> {
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&description.artifact_bytes)
            .map_err(DescribeError::ArtifactFormat)?;
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .map_err(DescribeError::ArtifactDecode)?;
    terminal_codec::terminal_psi_identity(&module).map_err(DescribeError::ArtifactIdentity)
}

/// Subject reconstruction helper for callers that only hold the raw parts.
pub fn component_subject_from_semantic_bytes(
    semantic_bytes: &[u8],
) -> Result<TerminalPsiIdentity, DescribeError> {
    let module =
        terminal_codec::decode_module(semantic_bytes).map_err(DescribeError::ArtifactDecode)?;
    terminal_codec::terminal_psi_identity(&module).map_err(DescribeError::ArtifactIdentity)
}

/// Convenience: vocabulary marker + fingerprint of the embedded artifact.
pub fn description_vocabulary_marker(
    description: &ComponentDescription,
) -> Result<VocabularyMarker, DescribeError> {
    Ok(description_subject(description)?.vocabulary_marker)
}

/// Convenience: semantic fingerprint of the embedded artifact.
pub fn description_semantic_fingerprint(
    description: &ComponentDescription,
) -> Result<SemanticFingerprint, DescribeError> {
    Ok(description_subject(description)?.program_fingerprint)
}
