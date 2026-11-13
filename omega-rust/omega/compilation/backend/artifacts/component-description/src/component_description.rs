/*
We define the canonical `ComponentDescription` schema (`OMGCMPD\0` v3), its
module-inventory derivation (`derive_component_inventory` and
`describe_decoded_component_facts`), and its deterministic wire codec
(`encode_component_description` and `decode_component_description_with_artifact`).
We exhaustively classify every `OperationKind` when deriving outgoing
authority, custody constraints, entries, exports, and installation service
bounds from the embedded Terminal artifact, and we enforce strict roster length
limits and canonical sorting at both publication and decode time.
*/

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BoundaryMachineId, MachineId, ServiceId};
use terminal_psi::{OperationKind, TerminalIdentityIndex, TerminalModule, TerminalPsiIdentity};

const DESCRIPTION_MAGIC: &[u8; 8] = b"OMGCMPD\0";
const DESCRIPTION_IDENTITY_DOMAIN: &[u8] = b"omega-component-description-v1";
const REQUIREMENT_CONTRACT_DOMAIN: &[u8] = b"omega-component-requirement-v1";
const CANONICAL_ENTRY_CONTRACT_DOMAIN: &[u8] = b"omega-component-canonical-entry-v1";
const PORT_MECHANISM_DOMAIN: &[u8] = b"omega-component-port-mechanism-v1";
const REGISTER_MECHANISM_DOMAIN: &[u8] = b"omega-component-register-mechanism-v1";

macro_rules! wire_tagged {
    ($name:ident { $($variant:ident = $tag:literal),+ $(,)? }) => {
        impl $name {
            pub(crate) fn tag(self) -> u8 {
                match self {
                    $(Self::$variant => $tag),+
                }
            }

            fn from_tag(tag: u8) -> Option<Self> {
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

pub(crate) fn requirement_export_identity(
    requirement_identity: &str,
    provider_identity: &str,
    candidate_identity: &str,
) -> String {
    format!("export:requirement:{requirement_identity}|{provider_identity}|{candidate_identity}")
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExportContract {
    pub identity: String,
    pub contract_identity: [u8; 32],
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

pub(crate) fn component_description_identity(canonical_bytes: &[u8]) -> [u8; 32] {
    crate::digest_encoder::DomainDigestEncoder::content(
        DESCRIPTION_IDENTITY_DOMAIN,
        canonical_bytes,
    )
}

pub(crate) fn requirement_contract_identity(requirement_identity: &str) -> [u8; 32] {
    crate::digest_encoder::DomainDigestEncoder::content(
        REQUIREMENT_CONTRACT_DOMAIN,
        requirement_identity.as_bytes(),
    )
}

fn canonical_entry_label(entry: terminal_psi::TargetMachineCallable) -> String {
    match entry {
        terminal_psi::TargetMachineCallable::Machine(machine) => {
            format!("machine:{}", machine.get())
        }
        terminal_psi::TargetMachineCallable::Boundary(boundary) => {
            format!("boundary:{}", boundary.get())
        }
    }
}

pub(crate) fn identity_text(module: &TerminalModule, identity: TerminalIdentityIndex) -> &str {
    module.identities.text(identity).unwrap_or_default()
}

pub(crate) fn canonical_entry_contract_identity(
    entry: terminal_psi::TargetMachineCallable,
) -> [u8; 32] {
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(CANONICAL_ENTRY_CONTRACT_DOMAIN);
    match entry {
        terminal_psi::TargetMachineCallable::Machine(machine) => {
            digest.raw(&[1]);
            digest.raw(&machine.get().to_le_bytes());
        }
        terminal_psi::TargetMachineCallable::Boundary(boundary) => {
            digest.raw(&[2]);
            digest.raw(&boundary.get().to_le_bytes());
        }
    }
    digest.finish()
}

pub(crate) fn port_mechanism_assumption(service: ServiceId, port: u16, value: u8) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(PORT_MECHANISM_DOMAIN);
    digest.raw(&service.get().to_le_bytes());
    digest.raw(&port.to_le_bytes());
    digest.byte(value);
    digest.finish()
}

pub(crate) fn register_mechanism_assumption(
    service: ServiceId,
    operation: terminal_psi::RegisterOperation,
) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(REGISTER_MECHANISM_DOMAIN);
    digest.raw(&service.get().to_le_bytes());
    digest.raw(&operation.catalog_tag().to_le_bytes());
    digest.finish()
}

pub(crate) use crate::digest_encoder::hex;

pub(crate) struct DerivedInventory {
    pub(crate) called_requirement_identities: Vec<String>,
    pub(crate) entries: Vec<ComponentEntry>,
    pub(crate) export_contracts: Vec<ExportContract>,
    pub(crate) custody: Vec<CustodyConstraint>,
    pub(crate) port_writes: BTreeSet<(ServiceId, u16, u8)>,
    pub(crate) register_accesses: BTreeSet<(ServiceId, terminal_psi::RegisterOperation)>,
    pub(crate) concrete_service_reach: BTreeSet<ServiceId>,
    pub(crate) service_bounds: Vec<InstallationServiceBound>,
}

pub(crate) fn derive_component_inventory(
    module: &TerminalModule,
) -> Result<DerivedInventory, DescribeError> {
    let mut called_boundaries = BTreeSet::new();
    let mut port_writes = BTreeSet::new();
    let mut register_accesses = BTreeSet::new();
    let mut custody = Vec::new();

    for machine in &module.machines {
        for block in &machine.blocks {
            for operation in &block.operations {
                match &operation.kind {
                    OperationKind::EstablishNativeCodeLayout { .. }
                    | OperationKind::ReadScalarApplication { .. } => {}
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
                    OperationKind::ActivateTaskRegister { service, .. } => {
                        register_accesses.insert((
                            *service,
                            terminal_psi::RegisterOperation::TaskRegisterActivate,
                        ));
                    }
                    OperationKind::ReloadSegmentDescriptors { service, kind, .. } => {
                        register_accesses.insert((
                            *service,
                            terminal_psi::RegisterOperation::SegmentReload(*kind),
                        ));
                    }
                    OperationKind::ReadSegmentSelector { service, register } => {
                        register_accesses.insert((
                            *service,
                            terminal_psi::RegisterOperation::SegmentRead(*register),
                        ));
                    }
                    OperationKind::ReadTaskRegister { service } => {
                        register_accesses
                            .insert((*service, terminal_psi::RegisterOperation::TaskRegisterRead));
                    }
                    OperationKind::GlobalDescriptorTable {
                        service, operation, ..
                    } => {
                        register_accesses.insert((
                            *service,
                            terminal_psi::RegisterOperation::GlobalDescriptorTable(*operation),
                        ));
                    }
                    OperationKind::RegisterAccess { service, event } => {
                        if let Some(service) = service {
                            register_accesses.insert((*service, event.operation()));
                        }
                    }
                    OperationKind::Call { .. }
                    | OperationKind::CallUnit { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralScalar { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
                    | OperationKind::CallDynamic { .. } => {}
                    OperationKind::EstablishAccessReference { .. }
                    | OperationKind::EstablishReference { .. }
                    | OperationKind::ReleaseReference { .. }
                    | OperationKind::EstablishScalarArray { .. }
                    | OperationKind::EstablishStructuralArray { .. }
                    | OperationKind::EstablishPrimitiveLocal { .. }
                    | OperationKind::EstablishTrivialAffineLocal { .. }
                    | OperationKind::EstablishZeroedStructural { .. }
                    | OperationKind::EstablishStructuralLocal { .. }
                    | OperationKind::EstablishRecord { .. }
                    | OperationKind::MoveStructuralValue { .. }
                    | OperationKind::TransformContent { .. }
                    | OperationKind::EstablishScalarCase { .. }
                    | OperationKind::EstablishStructuralCase { .. }
                    | OperationKind::EstablishActivationClaim { .. }
                    | OperationKind::SplitActivationClaim { .. }
                    | OperationKind::MergeActivationClaim { .. }
                    | OperationKind::ReleaseActivationClaim { .. }
                    | OperationKind::EstablishByteSequenceLiteral { .. }
                    | OperationKind::StorageAddress { .. }
                    | OperationKind::PrimitiveScalarRead { .. }
                    | OperationKind::AtomicFence { .. }
                    | OperationKind::AtomicAccess { .. }
                    | OperationKind::StructuralScalarFieldStore { .. }
                    | OperationKind::StructuralByteSequenceFieldLength { .. }
                    | OperationKind::StructuralByteSequenceFieldByteStore { .. }
                    | OperationKind::StructuralByteSequenceFieldRead { .. }
                    | OperationKind::ByteSequencePredicate { .. }
                    | OperationKind::ByteSequenceEqual { .. }
                    | OperationKind::ByteSequenceStore { .. }
                    | OperationKind::StructuralByteSequenceFieldStore { .. }
                    | OperationKind::StructuralByteViewFieldStore { .. }
                    | OperationKind::StructuralCaseMembership { .. }
                    | terminal_psi::structural_leaf_copy_pair_pattern!()
                    | OperationKind::WriteOnlyPrimitiveStore { .. }
                    | OperationKind::ByteSequenceLength { .. }
                    | OperationKind::ByteSequenceRead { .. }
                    | OperationKind::ByteSequenceWrite { .. }
                    | OperationKind::ByteSequenceSubslice { .. }
                    | terminal_psi::element_view_establish_pair_pattern!(..)
                    | OperationKind::ElementViewLength { .. }
                    | OperationKind::ElementViewRead { .. }
                    | OperationKind::ElementViewWrite { .. }
                    | OperationKind::ElementViewSubslice { .. } => {}
                    OperationKind::IntegerConstant { .. }
                    | OperationKind::SelectImplementation { .. }
                    | OperationKind::SelectMachine { .. }
                    | OperationKind::BooleanConstant { .. }
                    | OperationKind::IeeeFloatConstant { .. }
                    | OperationKind::IeeeFloatCompare { .. }
                    | OperationKind::ScalarRepresentationView { .. }
                    | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
                    | OperationKind::BooleanStructuralField { .. }
                    | OperationKind::ScalarStructuralField { .. }
                    | OperationKind::BooleanNot { .. }
                    | OperationKind::BooleanEqual { .. }
                    | OperationKind::IntegerEqual { .. }
                    | terminal_psi::integer_less_or_equal_pair_pattern!(..)
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
                    | OperationKind::TrappingInteger { .. }
                    | OperationKind::FloatResultPolicy { .. }
                    | terminal_psi::move_or_store_structural_field_pattern!()
                    | OperationKind::EstablishByteSequenceConcatenation { .. } => {}
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
        called_requirement_identities.push(identity_text(module, declaration.identity).to_string());
        for schema in &declaration.program_local_root_introductions {
            custody.push(CustodyConstraint {
                kind: CustodyKind::ProgramLocalRootIntroduction,
                identity: format!(
                    "root-introduction:{}:{}",
                    identity_text(module, declaration.identity),
                    schema.argument_index,
                ),
                evidence: CustodyEvidence::ModuleDerived,
            });
        }
        for (index, _guarantee) in declaration.content_guarantees.iter().enumerate() {
            custody.push(CustodyConstraint {
                kind: CustodyKind::BoundaryContentGuarantee,
                identity: format!(
                    "content-guarantee:{}:{index}",
                    identity_text(module, declaration.identity)
                ),
                evidence: CustodyEvidence::ModuleDerived,
            });
        }
    }
    called_requirement_identities.sort();
    called_requirement_identities.dedup();

    for input in &module.access_view_inputs {
        custody.push(CustodyConstraint {
            kind: CustodyKind::AccessViewInput,
            identity: format!(
                "access-view:{}:{}:{}",
                input.machine.get(),
                input.position,
                identity_text(module, input.view_identity),
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

    let mut entries = Vec::new();
    if let Some(entry) = module.entry {
        entries.push(ComponentEntry {
            kind: ComponentEntryKind::Canonical,
            identity: format!("canonical-entry:{}", canonical_entry_label(entry)),
            evidence: EntryEvidence::ModuleDerived,
        });
    }
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

    let concrete_service_reach: BTreeSet<ServiceId> =
        module.root_service_reach.concrete.iter().copied().collect();
    let mut service_bounds: Vec<InstallationServiceBound> = module
        .root_service_reach
        .installation_dependencies
        .iter()
        .map(|dependency| {
            let mut bound = dependency.upper_bound.clone();
            bound.sort();
            bound.dedup();
            InstallationServiceBound {
                requirement_identity: identity_text(module, dependency.requirement_identity)
                    .to_string(),
                bound,
            }
        })
        .collect();
    service_bounds.sort();
    service_bounds.dedup();

    let mut export_contracts = Vec::new();
    if let Some(entry) = module.entry {
        export_contracts.push(ExportContract {
            identity: format!("export:canonical:{}", canonical_entry_label(entry)),
            contract_identity: canonical_entry_contract_identity(entry),
        });
    }
    for candidate in &module.provider_candidates {
        export_contracts.push(ExportContract {
            identity: requirement_export_identity(
                identity_text(module, candidate.requirement_identity),
                identity_text(module, candidate.provider_identity),
                identity_text(module, candidate.candidate_identity),
            ),
            contract_identity: requirement_contract_identity(identity_text(
                module,
                candidate.requirement_identity,
            )),
        });
    }
    export_contracts.sort();
    export_contracts.dedup();

    custody.sort();
    custody.dedup();
    entries.sort();
    entries.dedup();

    Ok(DerivedInventory {
        called_requirement_identities,
        entries,
        export_contracts,
        custody,
        port_writes,
        register_accesses,
        concrete_service_reach,
        service_bounds,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackDemandFacts {
    pub entry: MachineId,
    pub ceiling_bytes: u64,
    pub stack_alignment: u32,
}

fn component_references_plan(
    module: &TerminalModule,
    inventory: &DerivedInventory,
    plan: &effects::provider_plan::ProviderPlan,
) -> bool {
    plan.rows.iter().any(|row| {
        component_references_requirement(
            module,
            inventory,
            plan,
            plan.identities.text(row.requirement_identity),
        )
    })
}

fn component_references_requirement(
    module: &TerminalModule,
    inventory: &DerivedInventory,
    plan: &effects::provider_plan::ProviderPlan,
    requirement: &str,
) -> bool {
    inventory
        .called_requirement_identities
        .iter()
        .any(|called| called == requirement)
        || module.provider_candidates.iter().any(|candidate| {
            module
                .identities
                .text(candidate.source_requirement_identity)
                == Some(requirement)
                && module.identities.text(candidate.provider_identity)
                    == Some(plan.provider_type.as_str())
        })
}

#[derive(Debug, Clone, Copy)]
pub struct ComponentDescriptionFacts<'a> {
    pub artifact: &'a terminal_codec::CanonicalTerminalArtifact,
    pub selected_provider_plans: &'a effects::SelectedProviderPlanFacts,
    pub stack_demand: Option<StackDemandFacts>,
    pub realization_identity: Option<[u8; 32]>,
}

pub(crate) fn describe_decoded_component_facts(
    facts: ComponentDescriptionFacts<'_>,
    module: &TerminalModule,
) -> Result<ComponentDescription, DescribeError> {
    let artifact_bytes = facts.artifact.to_bytes();
    let inventory = derive_component_inventory(module)?;

    let selected = facts.selected_provider_plans;
    let providers: Vec<RetainedProvider> = selected
        .plans()
        .iter()
        .filter(|plan| component_references_plan(module, &inventory, plan))
        .map(|plan| {
            let mut requirement_identities: Vec<String> = plan
                .rows
                .iter()
                .map(|row| plan.identities.text(row.requirement_identity).to_owned())
                .filter(|requirement| {
                    component_references_requirement(module, &inventory, plan, requirement)
                })
                .collect();
            requirement_identities.sort();
            requirement_identities.dedup();
            RetainedProvider {
                report_identity: plan.report_fingerprint(),
                plan_digest: *plan.identity_digest().as_bytes(),
                plan_input: plan.identity_preimage(),
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

    for (service, operation) in &inventory.register_accesses {
        let assumption = register_mechanism_assumption(*service, *operation);
        assumptions.push(assumption);
        outgoing.push(OutgoingAuthority {
            class: OutgoingAuthorityClass::RegisterAccess,
            identity: format!(
                "register-access:{}:{}",
                service.get(),
                operation.catalog_tag()
            ),
            evidence: OutgoingEvidence::PhysicalMechanism(assumption),
        });
    }

    for service in &inventory.concrete_service_reach {
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

    assumptions.sort();
    assumptions.dedup();
    imports.sort();
    outgoing.sort();
    obligations.sort();
    obligations.dedup();

    Ok(ComponentDescription {
        schema: COMPONENT_DESCRIPTION_SCHEMA_V3,
        frontier: DescriptionFrontier::TerminalArtifactClosure,
        artifact_bytes,
        imports,
        exports: inventory
            .export_contracts
            .iter()
            .map(|contract| ExportSurface {
                identity: contract.identity.clone(),
            })
            .collect(),
        entries: inventory.entries,
        outgoing,
        service_bounds: inventory.service_bounds,
        custody: inventory.custody,
        providers,
        provider_closure_digest: *selected.identity_digest().as_bytes(),
        obligations,
        assumptions,
        realization_identity: facts.realization_identity,
    })
}

use optimization_core::cursor::{Cursor, Truncated};

type Reader<'a> = Cursor<'a>;

impl From<Truncated> for DescriptionDecodeRejection {
    fn from(_: Truncated) -> Self {
        Self::Corrupt("truncated input")
    }
}

fn roster_len(
    reader: &mut Reader<'_>,
    what: &'static str,
) -> Result<usize, DescriptionDecodeRejection> {
    let count = reader.u32()? as usize;
    if count > MAX_DESCRIPTION_ROSTER {
        return Err(DescriptionDecodeRejection::RosterBoundExceeded(what));
    }
    Ok(count)
}

fn read_identity(reader: &mut Reader<'_>) -> Result<String, DescriptionDecodeRejection> {
    let length = reader.u32()? as usize;
    String::from_utf8(reader.take(length)?.to_vec())
        .map_err(|_| DescriptionDecodeRejection::IdentityInvalid("not utf-8"))
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

pub(crate) fn check_publication_bounds(
    description: &ComponentDescription,
    encoded: &[u8],
) -> Result<(), DescribeError> {
    if encoded.len() > MAX_COMPONENT_DESCRIPTION_BYTES {
        return Err(DescribeError::BoundExceeded("byte length"));
    }
    let rosters = [
        ("import roster", description.imports.len()),
        ("export roster", description.exports.len()),
        ("entry roster", description.entries.len()),
        ("outgoing authority roster", description.outgoing.len()),
        ("service bound roster", description.service_bounds.len()),
        ("custody roster", description.custody.len()),
        ("provider roster", description.providers.len()),
        ("obligation roster", description.obligations.len()),
        ("assumption roster", description.assumptions.len()),
    ]
    .into_iter()
    .chain(
        description
            .service_bounds
            .iter()
            .map(|row| ("service bound services roster", row.bound.len())),
    )
    .chain(description.providers.iter().map(|provider| {
        (
            "provider requirement roster",
            provider.requirement_identities.len(),
        )
    }));
    for (what, count) in rosters {
        if count > MAX_DESCRIPTION_ROSTER {
            return Err(DescribeError::BoundExceeded(what));
        }
    }
    Ok(())
}

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

    let mut service_bounds = description.service_bounds.clone();
    service_bounds.sort();
    push_u32(&mut out, service_bounds.len() as u32);
    for row in &service_bounds {
        push_identity(&mut out, &row.requirement_identity);
        let mut bound = row.bound.clone();
        bound.sort();
        push_u32(&mut out, bound.len() as u32);
        for service in &bound {
            push_u64(&mut out, service.get());
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
        push_u32(&mut out, provider.plan_input.len() as u32);
        out.extend_from_slice(&provider.plan_input);
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

pub fn decode_component_description(
    bytes: &[u8],
) -> Result<ComponentDescription, DescriptionDecodeRejection> {
    decode_component_description_with_artifact(bytes).map(|(description, _artifact)| description)
}

pub fn decode_component_description_with_artifact(
    bytes: &[u8],
) -> Result<
    (
        ComponentDescription,
        terminal_codec::CanonicalTerminalArtifact,
    ),
    DescriptionDecodeRejection,
> {
    if bytes.len() > MAX_COMPONENT_DESCRIPTION_BYTES {
        return Err(DescriptionDecodeRejection::RosterBoundExceeded(
            "description byte length",
        ));
    }
    let mut reader = Cursor::new(bytes);
    if reader.take(DESCRIPTION_MAGIC.len())? != DESCRIPTION_MAGIC {
        return Err(DescriptionDecodeRejection::InvalidMagic);
    }
    let schema = reader.u32()?;
    if schema != COMPONENT_DESCRIPTION_SCHEMA_V3 {
        return Err(DescriptionDecodeRejection::UnsupportedTag(
            "description schema",
        ));
    }
    let frontier = DescriptionFrontier::from_tag(reader.byte()?)
        .ok_or(DescriptionDecodeRejection::UnsupportedTag("frontier tag"))?;

    let artifact_length = reader.u32()? as usize;
    let artifact_bytes = reader.take(artifact_length)?.to_vec();
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact_bytes)
        .map_err(|_| DescriptionDecodeRejection::Corrupt("embedded artifact did not decode"))?;

    let mut imports = Vec::new();
    let mut last_import: Option<(u32, String)> = None;
    for _ in 0..roster_len(&mut reader, "import")? {
        let slot = reader.u32()?;
        let requirement_identity = read_identity(&mut reader)?;
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
    for _ in 0..roster_len(&mut reader, "export")? {
        let identity = read_identity(&mut reader)?;
        if last_export.as_ref() >= Some(&identity) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("export"));
        }
        last_export = Some(identity.clone());
        exports.push(ExportSurface { identity });
    }

    let mut entries = Vec::new();
    let mut last_entry: Option<(u8, String)> = None;
    for _ in 0..roster_len(&mut reader, "entry")? {
        let kind = ComponentEntryKind::from_tag(reader.byte()?)
            .ok_or(DescriptionDecodeRejection::UnsupportedTag("entry kind"))?;
        let identity = read_identity(&mut reader)?;
        let evidence = match reader.byte()? {
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
    for _ in 0..roster_len(&mut reader, "outgoing authority")? {
        let class = OutgoingAuthorityClass::from_tag(reader.byte()?).ok_or(
            DescriptionDecodeRejection::UnsupportedTag("authority class"),
        )?;
        let identity = read_identity(&mut reader)?;
        let evidence = match reader.byte()? {
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

    let mut service_bounds = Vec::new();
    let mut last_bound: Option<(String, Vec<ServiceId>)> = None;
    for _ in 0..roster_len(&mut reader, "service bound")? {
        let requirement_identity = read_identity(&mut reader)?;
        let mut bound = Vec::new();
        let mut last_service: Option<ServiceId> = None;
        for _ in 0..roster_len(&mut reader, "service bound services")? {
            let service = ServiceId::new(reader.u64()?)
                .ok_or(DescriptionDecodeRejection::Corrupt("zero service identity"))?;
            if last_service >= Some(service) {
                return Err(DescriptionDecodeRejection::NonCanonicalOrder(
                    "service bound services",
                ));
            }
            last_service = Some(service);
            bound.push(service);
        }
        let key = (requirement_identity.clone(), bound.clone());
        if last_bound.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder(
                "service bound",
            ));
        }
        last_bound = Some(key);
        service_bounds.push(InstallationServiceBound {
            requirement_identity,
            bound,
        });
    }

    let mut custody = Vec::new();
    let mut last_custody: Option<(u8, String)> = None;
    for _ in 0..roster_len(&mut reader, "custody")? {
        let kind = CustodyKind::from_tag(reader.byte()?)
            .ok_or(DescriptionDecodeRejection::UnsupportedTag("custody kind"))?;
        let identity = read_identity(&mut reader)?;
        let evidence = match reader.byte()? {
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
    for _ in 0..roster_len(&mut reader, "provider")? {
        let report_identity = reader.u64()?;
        let plan_digest = reader.array::<32>()?;
        let plan_input_length = reader.u32()? as usize;
        if plan_input_length > MAX_COMPONENT_DESCRIPTION_BYTES {
            return Err(DescriptionDecodeRejection::RosterBoundExceeded(
                "provider plan input",
            ));
        }
        let plan_input = reader.take(plan_input_length)?.to_vec();
        let requirement_count = roster_len(&mut reader, "provider requirement")?;
        let mut requirement_identities = Vec::with_capacity(requirement_count);
        let mut last_requirement: Option<String> = None;
        for _ in 0..requirement_count {
            let requirement = read_identity(&mut reader)?;
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
            plan_input,
            requirement_identities,
        });
    }
    let provider_closure_digest = reader.array::<32>()?;

    let mut obligations = Vec::new();
    let mut last_obligation: Option<(u8, String)> = None;
    for _ in 0..roster_len(&mut reader, "obligation")? {
        let kind = ObligationKind::from_tag(reader.byte()?).ok_or(
            DescriptionDecodeRejection::UnsupportedTag("obligation kind"),
        )?;
        let identity = read_identity(&mut reader)?;
        let detail = read_identity(&mut reader)?;
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
    for _ in 0..roster_len(&mut reader, "assumption")? {
        let assumption = reader.array::<32>()?;
        if last_assumption >= Some(assumption) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("assumption"));
        }
        last_assumption = Some(assumption);
        assumptions.push(assumption);
    }

    let realization_identity = match reader.byte()? {
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

    Ok((
        ComponentDescription {
            schema,
            frontier,
            artifact_bytes,
            imports,
            exports,
            entries,
            outgoing,
            service_bounds,
            custody,
            providers,
            provider_closure_digest,
            obligations,
            assumptions,
            realization_identity,
        },
        artifact,
    ))
}

pub fn description_subject(
    description: &ComponentDescription,
) -> Result<TerminalPsiIdentity, DescribeError> {
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&description.artifact_bytes)
            .map_err(DescribeError::ArtifactFormat)?;
    let module = artifact.semantic_module();
    terminal_codec::terminal_psi_identity(module).map_err(DescribeError::ArtifactIdentity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_longer_than_one_kibibyte_round_trips() {
        let identity = "export:requirement:".to_owned() + &"x".repeat(4096);
        let mut encoded = Vec::new();
        push_identity(&mut encoded, &identity);
        let mut reader = Cursor::new(&encoded);
        assert_eq!(read_identity(&mut reader), Ok(identity));
    }
}
