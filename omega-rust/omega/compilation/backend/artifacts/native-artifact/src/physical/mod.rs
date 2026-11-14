/*
We define the native physical evidence records and constructors that bind each
surviving optimized operator or boundary occurrence to its exact machine,
object, and final-image byte spans, byte digests, and relocation disposition.
Every record field stays private to this module so replay paths cannot
manufacture optimizer or derivation authority outside our scoped constructors.
*/

pub(crate) mod derivation {
    pub(crate) mod evidence;
    pub(super) mod foreign_calls;
    pub(super) mod fragment_custody;
    pub(crate) mod hashing;
    pub(super) mod hosted_builtins;
    pub(super) mod internal_calls;
    pub(super) mod provider_custody;
    pub(super) mod provider_settlements;
    pub(super) mod settlement_identity;
    #[cfg(test)]
    mod tests;

    pub(crate) use evidence::{NativePhysicalEvidenceDerivation, derive_physical_evidence};
}
mod fragment_publication;
pub(crate) mod identity_bytes {
    use super::canonical_usize;

    pub(crate) fn terminal_identity_bytes(terminal: terminal_psi::TerminalPsiIdentity) -> Vec<u8> {
        let mut canonical = Vec::with_capacity(40);
        crate::codec_bytes::push_encoded(
            &mut canonical,
            terminal.vocabulary_marker.get().to_le_bytes(),
        );
        canonical.extend_from_slice(terminal.program_fingerprint.as_bytes());
        canonical
    }

    pub(crate) fn append_operator_occurrence_identity(
        canonical: &mut Vec<u8>,
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        operation_ordinal: usize,
    ) {
        crate::codec_bytes::push_encoded(canonical, machine.get().to_le_bytes());
        crate::codec_bytes::push_encoded(canonical, operation.get().to_le_bytes());
        canonical.extend_from_slice(&canonical_usize(operation_ordinal));
    }

    pub(crate) fn append_boundary_occurrence_identity(
        canonical: &mut Vec<u8>,
        machine: semantic_vocabulary::MachineId,
        site: impl Into<semantic_vocabulary::InvocationSite>,
        boundary: semantic_vocabulary::BoundaryMachineId,
        operation_ordinal: usize,
    ) {
        crate::codec_bytes::push_encoded(canonical, machine.get().to_le_bytes());
        match site.into() {
            semantic_vocabulary::InvocationSite::Operation(operation) => {
                canonical.push(0);
                crate::codec_bytes::push_encoded(canonical, operation.get().to_le_bytes());
            }
            semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                canonical.push(1);
                crate::codec_bytes::push_encoded(canonical, edge.get().to_le_bytes());
            }
            semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => {
                canonical.push(2);
                crate::codec_bytes::push_encoded(canonical, edge.get().to_le_bytes());
            }
        }
        crate::codec_bytes::push_encoded(canonical, boundary.get().to_le_bytes());
        canonical.extend_from_slice(&canonical_usize(operation_ordinal));
    }
}
mod operator_applications;
mod projection;
mod replay;
pub(crate) use replay::PhysicalEvidencePass;

use crate::NativeSelectedProviderPlanDigest;
use boundary_applications::OperatorApplicationCoverageRef;
pub(crate) use derivation::hashing::{canonical_usize, hash_bytes, hash_call_site_owner};
pub(crate) use derivation::{NativePhysicalEvidenceDerivation, derive_physical_evidence};
pub(crate) use fragment_publication::derive_scope as derive_fragment_publication_scope;
use optimization_core::{
    NativeOptimizationProjectionIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId, ServiceId};
use target::NativeTarget;
use target_operations::{
    BoundaryRealization, BoundaryScalarArgument, CallSiteOwner, NormalizedForeignCallBinding,
    ProviderExecutionBinding,
};
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeByteSpan {
    offset: usize,
    byte_count: usize,
}

impl NativeByteSpan {
    pub const fn from_replayed_parts(offset: usize, byte_count: usize) -> Self {
        Self { offset, byte_count }
    }

    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn byte_count(self) -> usize {
        self.byte_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeCompilerBuiltinCatalogIdentity {
    HostedV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedBoundaryOccurrence {
    terminal: TerminalPsiIdentity,
    machine: MachineId,
    invocation_site: semantic_vocabulary::InvocationSite,
    boundary: BoundaryMachineId,
    operation_ordinal: usize,
    identity: OptimizedBoundaryOccurrenceIdentity,
}

impl OptimizedBoundaryOccurrence {
    pub const fn terminal(&self) -> TerminalPsiIdentity {
        self.terminal
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn operation(&self) -> Option<OperationId> {
        self.invocation_site.operation()
    }

    pub const fn invocation_site(&self) -> semantic_vocabulary::InvocationSite {
        self.invocation_site
    }

    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }

    pub const fn operation_ordinal(&self) -> usize {
        self.operation_ordinal
    }

    pub const fn identity(&self) -> OptimizedBoundaryOccurrenceIdentity {
        self.identity
    }

    pub fn label(&self) -> String {
        format!(
            "boundary occurrence on machine {} site {} boundary {} ordinal {}",
            self.machine, self.invocation_site, self.boundary, self.operation_ordinal
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedOperatorOccurrence {
    terminal: TerminalPsiIdentity,
    machine: MachineId,
    operation: OperationId,
    operation_ordinal: usize,
    identity: OptimizedOperatorOccurrenceIdentity,
}

impl OptimizedOperatorOccurrence {
    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn operation(&self) -> OperationId {
        self.operation
    }

    pub const fn operation_ordinal(&self) -> usize {
        self.operation_ordinal
    }

    pub const fn identity(&self) -> OptimizedOperatorOccurrenceIdentity {
        self.identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOptimizationProjection {
    terminal: TerminalPsiIdentity,
    operator_occurrences: Vec<OptimizedOperatorOccurrence>,
    boundary_occurrences: Vec<OptimizedBoundaryOccurrence>,
    identity: NativeOptimizationProjectionIdentity,
}

impl NativeOptimizationProjection {
    pub const fn terminal(&self) -> TerminalPsiIdentity {
        self.terminal
    }

    pub(crate) fn boundary_occurrences(&self) -> &[OptimizedBoundaryOccurrence] {
        &self.boundary_occurrences
    }

    pub(crate) fn operator_occurrences(&self) -> &[OptimizedOperatorOccurrence] {
        &self.operator_occurrences
    }

    pub const fn identity(&self) -> NativeOptimizationProjectionIdentity {
        self.identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizedNativePhysicalEvidenceScope {
    validation: OptimizedAbstractPlanProjectionIdentity,
    final_unit: OptimizationUnitIdentity,
    boundary_application_coverage: [u8; 32],
    projection: NativeOptimizationProjection,
    publication: Option<self::fragment_publication::FragmentPublicationBinding>,
    identity: [u8; 32],
}

impl ValidatedOptimizedNativePhysicalEvidenceScope {
    pub const fn projection(&self) -> &NativeOptimizationProjection {
        &self.projection
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    const fn boundary_application_coverage(&self) -> &[u8; 32] {
        &self.boundary_application_coverage
    }

    const fn fragment_publication(
        &self,
    ) -> Option<&self::fragment_publication::FragmentPublicationBinding> {
        self.publication.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryTraitSettlement {
    occurrence: OptimizedBoundaryOccurrence,
    requirement_identity: String,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    role: BoundaryTraitSettlementRole,
    identity: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompilerBuiltinScalarArgument {
    Immediate(BoundaryScalarArgument),
    RuntimeScalar(machine_code::ForeignCallScalarArgumentRecord),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompilerBuiltinResult {
    Unit,
    Structural(machine_code::BoundaryStructuralResultRecord),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoundaryTraitSettlementRole {
    CompilerBuiltin {
        catalog: NativeCompilerBuiltinCatalogIdentity,
        execution: target_operations::CompilerBuiltinExecution,
        realization: BoundaryRealization,
        scalar_argument: Option<CompilerBuiltinScalarArgument>,
        result: CompilerBuiltinResult,
    },
    AdmittedProvider {
        execution: ProviderExecutionBinding,
        realization: NormalizedForeignCallBinding,
    },
    AdmittedProviderSettlement {
        execution: ProviderExecutionBinding,
        settlement: machine_code::BoundarySettlementRecord,
        port_effect: Option<machine_code::PortEffectRecord>,
    },
    InstalledProviderInternalCall {
        boundary: BoundaryMachineId,
        provider_declaration: String,
        provider_identity: String,
        candidate_identity: String,
        call: BoundaryInternalCallCustody,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoundaryInternalCallCustody {
    Unit {
        owner: CallSiteOwner,
        target: MachineId,
        call_field_offset: usize,
        active_frame_bytes: u32,
        transient_bytes: u32,
        caller_live_bytes: u32,
    },
    Scalar {
        owner: CallSiteOwner,
        target: MachineId,
        call_field_offset: usize,
        caller_live_bytes: u32,
    },
}

impl BoundaryTraitSettlement {
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFamilyInternalCall {
    terminal: TerminalPsiIdentity,
    occurrence: OptimizedBoundaryOccurrence,
    head_identity: String,
    head_contract: [u8; 32],
    member: terminal_psi::TargetMachineFamilyMember,
    target: NativeTarget,
    completion_claim_sources: Vec<target_operations::CompletionClaimSource>,
    completion_receipts: Vec<terminal_psi::CompletionReceipt>,
    call: BoundaryInternalCallCustody,
    identity: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalChildParent {
    OperatorApplicationCoverage(OperatorApplicationCoverageRef),
    BoundaryTraitSettlement(BoundaryTraitSettlement),
    TargetFamilyInternalCall(TargetFamilyInternalCall),
}

impl PhysicalChildParent {
    pub const fn identity(&self) -> [u8; 32] {
        match self {
            Self::OperatorApplicationCoverage(reference) => *reference.coverage().as_bytes(),
            Self::BoundaryTraitSettlement(settlement) => *settlement.identity(),
            Self::TargetFamilyInternalCall(call) => call.identity,
        }
    }

    pub const fn role_tag(&self) -> u8 {
        match self {
            Self::OperatorApplicationCoverage(_) => 1,
            Self::BoundaryTraitSettlement(_) => 2,
            Self::TargetFamilyInternalCall(_) => 3,
        }
    }

    pub(crate) const fn occurrence_role_tag(&self) -> u8 {
        match self {
            Self::OperatorApplicationCoverage(_) => 1,
            Self::BoundaryTraitSettlement(_) | Self::TargetFamilyInternalCall(_) => 2,
        }
    }

    pub(crate) fn owns_occurrence(
        &self,
        terminal: TerminalPsiIdentity,
        occurrence: NativePhysicalOccurrence,
    ) -> bool {
        match self {
            Self::OperatorApplicationCoverage(_) => {
                matches!(occurrence, NativePhysicalOccurrence::Operator(_))
            }
            Self::BoundaryTraitSettlement(settlement) => {
                settlement.occurrence.terminal() == terminal
                    && occurrence
                        == NativePhysicalOccurrence::Boundary(settlement.occurrence.identity())
            }
            Self::TargetFamilyInternalCall(call) => {
                call.terminal == terminal
                    && call.occurrence.terminal() == terminal
                    && occurrence == NativePhysicalOccurrence::Boundary(call.occurrence.identity())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalRelocationDisposition {
    DirectInstructionBytes,
    ResolvedInternalCall,
    UnresolvedNormalizedForeignCall(NormalizedForeignCallRelocation),
    UnresolvedNormalizedForeignCallImportField(NormalizedForeignCallImportField),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedForeignCallRelocation {
    locator_identity: [u8; 32],
    boundary_plan_identity: [u8; 32],
    object_symbol: object_file::ObjectSymbolHandle,
    origin: object_file::RelocationOrigin,
    offset: usize,
    byte_width: usize,
    addend: i64,
    kind: object_file::RelocationKind,
    callbacks: Vec<NormalizedForeignCallbackRelocations>,
    final_image_symbol_identity: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedForeignCallbackRelocations {
    X86_64Relative32 {
        callback_function: function_identity::MachineFunctionIdentity,
        relocation: NormalizedForeignCallbackRelocation,
    },
    Aarch64PageAddress {
        callback_function: function_identity::MachineFunctionIdentity,
        page: NormalizedForeignCallbackRelocation,
        page_offset: NormalizedForeignCallbackRelocation,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedForeignCallbackRelocation {
    object_symbol: object_file::ObjectSymbolHandle,
    origin: object_file::RelocationOrigin,
    offset: usize,
    byte_width: usize,
    addend: i64,
    kind: object_file::RelocationKind,
}

macro_rules! copied_accessors {
    ($($name:ident : $ty:ty),* $(,)?) => {
        $(
            pub const fn $name(&self) -> $ty {
                self.$name
            }
        )*
    };
}

impl NormalizedForeignCallbackRelocation {
    copied_accessors! {
        object_symbol: object_file::ObjectSymbolHandle,
        origin: object_file::RelocationOrigin,
        offset: usize,
        byte_width: usize,
        addend: i64,
        kind: object_file::RelocationKind,
    }
}

impl NormalizedForeignCallbackRelocations {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedForeignCallImportField {
    locator_identity: [u8; 32],
    boundary_plan_identity: [u8; 32],
    caller: MachineId,
    invocation_site: semantic_vocabulary::InvocationSite,
    boundary: BoundaryMachineId,
    ordinal: u32,
    object_symbol: object_file::ObjectLocalSymbolId,
    offset: usize,
    byte_width: usize,
    addend: i64,
    kind: object_file::RelocationKind,
    final_image_symbol_identity: [u8; 32],
}

impl NormalizedForeignCallImportField {
    pub const fn locator_identity(&self) -> &[u8; 32] {
        &self.locator_identity
    }

    pub const fn boundary_plan_identity(&self) -> &[u8; 32] {
        &self.boundary_plan_identity
    }

    pub const fn caller(&self) -> MachineId {
        self.caller
    }

    pub const fn invocation_site(&self) -> semantic_vocabulary::InvocationSite {
        self.invocation_site
    }

    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub const fn object_symbol(&self) -> object_file::ObjectLocalSymbolId {
        self.object_symbol
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub const fn byte_width(&self) -> usize {
        self.byte_width
    }

    pub const fn addend(&self) -> i64 {
        self.addend
    }

    pub const fn kind(&self) -> object_file::RelocationKind {
        self.kind
    }

    pub const fn final_image_symbol_identity(&self) -> &[u8; 32] {
        &self.final_image_symbol_identity
    }
}

impl NormalizedForeignCallRelocation {
    pub const fn locator_identity(&self) -> &[u8; 32] {
        &self.locator_identity
    }

    pub const fn boundary_plan_identity(&self) -> &[u8; 32] {
        &self.boundary_plan_identity
    }

    copied_accessors! {
        object_symbol: object_file::ObjectSymbolHandle,
        origin: object_file::RelocationOrigin,
        offset: usize,
        byte_width: usize,
        addend: i64,
        kind: object_file::RelocationKind,
    }

    pub fn callbacks(&self) -> &[NormalizedForeignCallbackRelocations] {
        &self.callbacks
    }

    pub const fn final_image_symbol_identity(&self) -> &[u8; 32] {
        &self.final_image_symbol_identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NativePhysicalOccurrence {
    Operator(OptimizedOperatorOccurrenceIdentity),
    Boundary(OptimizedBoundaryOccurrenceIdentity),
}

impl NativePhysicalOccurrence {
    pub const fn identity(self) -> [u8; 32] {
        match self {
            Self::Operator(identity) => identity.bytes(),
            Self::Boundary(identity) => identity.bytes(),
        }
    }

    pub const fn role_tag(self) -> u8 {
        match self {
            Self::Operator(_) => 1,
            Self::Boundary(_) => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePhysicalChild {
    parent: PhysicalChildParent,
    projection: NativeOptimizationProjectionIdentity,
    occurrence: NativePhysicalOccurrence,
    machine_spans: Vec<NativeByteSpan>,
    object_spans: Vec<NativeByteSpan>,
    final_image_spans: Vec<NativeByteSpan>,
    machine_bytes_digest: [u8; 32],
    object_bytes_digest: [u8; 32],
    final_image_bytes_digest: [u8; 32],
    relocation: PhysicalRelocationDisposition,
    identity: [u8; 32],
}

impl NativePhysicalChild {
    pub const fn parent(&self) -> &PhysicalChildParent {
        &self.parent
    }

    pub const fn projection(&self) -> NativeOptimizationProjectionIdentity {
        self.projection
    }

    pub const fn occurrence(&self) -> NativePhysicalOccurrence {
        self.occurrence
    }

    pub(crate) fn machine_spans(&self) -> &[NativeByteSpan] {
        &self.machine_spans
    }

    pub(crate) fn object_spans(&self) -> &[NativeByteSpan] {
        &self.object_spans
    }

    pub(crate) fn final_image_spans(&self) -> &[NativeByteSpan] {
        &self.final_image_spans
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub fn into_parts(self) -> NativePhysicalChildParts {
        NativePhysicalChildParts {
            parent: self.parent,
            projection: self.projection,
            occurrence: self.occurrence,
            machine_spans: self.machine_spans,
            object_spans: self.object_spans,
            final_image_spans: self.final_image_spans,
            machine_bytes_digest: self.machine_bytes_digest,
            object_bytes_digest: self.object_bytes_digest,
            final_image_bytes_digest: self.final_image_bytes_digest,
            relocation: self.relocation,
            identity: self.identity,
        }
    }

    pub fn from_replayed_parts(parts: NativePhysicalChildParts) -> Self {
        parts.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePhysicalEvidence {
    projection: NativeOptimizationProjection,
    children: Vec<NativePhysicalChild>,
    private_functions: Vec<PrivatePhysicalEvidence>,
    identity: [u8; 32],
}

/// A separately emitted namespace linked into this image. Machine and object
/// spans name that body's storage; final-image spans name the containing image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivatePhysicalEvidence {
    function: function_identity::MachineFunctionIdentity,
    text_offset: usize,
    evidence: NativePhysicalEvidence,
}

impl PrivatePhysicalEvidence {
    pub const fn function(&self) -> function_identity::MachineFunctionIdentity {
        self.function
    }
    pub const fn text_offset(&self) -> usize {
        self.text_offset
    }
    pub const fn evidence(&self) -> &NativePhysicalEvidence {
        &self.evidence
    }
}

impl NativePhysicalEvidence {
    pub const fn projection(&self) -> &NativeOptimizationProjection {
        &self.projection
    }

    pub fn children(&self) -> &[NativePhysicalChild] {
        &self.children
    }

    pub fn private_functions(&self) -> &[PrivatePhysicalEvidence] {
        &self.private_functions
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub fn into_parts(self) -> NativePhysicalEvidenceParts {
        NativePhysicalEvidenceParts {
            projection: self.projection,
            children: self.children,
            private_functions: self.private_functions,
            identity: self.identity,
        }
    }

    pub fn from_replayed_parts(parts: NativePhysicalEvidenceParts) -> Self {
        Self {
            projection: parts.projection,
            children: parts.children,
            private_functions: parts.private_functions,
            identity: parts.identity,
        }
    }
}

#[derive(Debug)]
pub(crate) struct BoundaryTraitSettlementParts {
    pub occurrence: OptimizedBoundaryOccurrence,
    pub requirement_identity: String,
    pub selected_plan_digest: NativeSelectedProviderPlanDigest,
    pub target: NativeTarget,
    pub role: BoundaryTraitSettlementRole,
    pub identity: [u8; 32],
}

impl From<BoundaryTraitSettlementParts> for BoundaryTraitSettlement {
    fn from(parts: BoundaryTraitSettlementParts) -> Self {
        Self {
            occurrence: parts.occurrence,
            requirement_identity: parts.requirement_identity,
            selected_plan_digest: parts.selected_plan_digest,
            target: parts.target,
            role: parts.role,
            identity: parts.identity,
        }
    }
}

#[derive(Debug)]
pub struct NativePhysicalChildParts {
    pub parent: PhysicalChildParent,
    pub projection: NativeOptimizationProjectionIdentity,
    pub occurrence: NativePhysicalOccurrence,
    pub machine_spans: Vec<NativeByteSpan>,
    pub object_spans: Vec<NativeByteSpan>,
    pub final_image_spans: Vec<NativeByteSpan>,
    pub machine_bytes_digest: [u8; 32],
    pub object_bytes_digest: [u8; 32],
    pub final_image_bytes_digest: [u8; 32],
    pub relocation: PhysicalRelocationDisposition,
    pub identity: [u8; 32],
}

#[derive(Debug)]
pub struct NativePhysicalEvidenceParts {
    pub projection: NativeOptimizationProjection,
    pub children: Vec<NativePhysicalChild>,
    pub private_functions: Vec<PrivatePhysicalEvidence>,
    pub identity: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePhysicalEvidenceGapSubject {
    UncoveredPrivateFunction {
        function: function_identity::MachineFunctionIdentity,
    },
    ForeignCallSiteOwner {
        machine: MachineId,
        owner: CallSiteOwner,
    },
    UnsupportedSettlementRealization {
        occurrence: OptimizedBoundaryOccurrence,
    },
    UnsupportedNormalizedForeignCall {
        occurrence: OptimizedBoundaryOccurrence,
    },
    UnrealizedBoundaryOccurrence {
        occurrence: OptimizedBoundaryOccurrence,
    },
    UnsupportedOperatorSpan {
        occurrence: OptimizedOperatorOccurrence,
    },
    UnownedPortEffect {
        machine: MachineId,
        psi_operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
        operation_ordinal: usize,
        code_offset: usize,
        byte_count: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativePhysicalEvidenceGap {
    subject: NativePhysicalEvidenceGapSubject,
    identity: [u8; 32],
}

impl NativePhysicalEvidenceGap {
    pub const fn subject(&self) -> NativePhysicalEvidenceGapSubject {
        self.subject
    }

    pub const fn occurrence(&self) -> Option<NativePhysicalOccurrence> {
        match self.subject {
            NativePhysicalEvidenceGapSubject::UncoveredPrivateFunction { .. } => None,
            NativePhysicalEvidenceGapSubject::ForeignCallSiteOwner { .. }
            | NativePhysicalEvidenceGapSubject::UnownedPortEffect { .. } => None,
            NativePhysicalEvidenceGapSubject::UnsupportedSettlementRealization { occurrence }
            | NativePhysicalEvidenceGapSubject::UnsupportedNormalizedForeignCall { occurrence }
            | NativePhysicalEvidenceGapSubject::UnrealizedBoundaryOccurrence { occurrence } => {
                Some(NativePhysicalOccurrence::Boundary(occurrence.identity()))
            }
            NativePhysicalEvidenceGapSubject::UnsupportedOperatorSpan { occurrence } => {
                Some(NativePhysicalOccurrence::Operator(occurrence.identity()))
            }
        }
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
}

impl std::fmt::Display for NativePhysicalEvidenceGap {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.subject {
            NativePhysicalEvidenceGapSubject::UncoveredPrivateFunction { function } => {
                write!(
                    formatter,
                    "private function {function:?} requires its own physical occurrence projection"
                )
            }
            NativePhysicalEvidenceGapSubject::ForeignCallSiteOwner { machine, owner } => {
                match owner {
                    CallSiteOwner::Operation(operation) => write!(
                        formatter,
                        "normalized foreign call on machine {machine} is owned by operation {operation}"
                    ),
                    CallSiteOwner::InvokeNoNormal(edge) => write!(
                        formatter,
                        "normalized foreign call on machine {machine} incorrectly names module invocation edge {edge}"
                    ),
                    CallSiteOwner::ExternalCompletion(edge) => write!(
                        formatter,
                        "normalized foreign call on machine {machine} is owned by completion edge {edge}"
                    ),
                    CallSiteOwner::Installation { operation, role } => write!(
                        formatter,
                        "normalized foreign call on machine {machine} incorrectly names installation helper {role:?} on operation {operation}"
                    ),
                    CallSiteOwner::CleanupAction {
                        edge,
                        action_ordinal,
                    } => write!(
                        formatter,
                        "normalized foreign call on machine {machine} is owned by cleanup action {action_ordinal} on edge {edge}"
                    ),
                }
            }
            NativePhysicalEvidenceGapSubject::UnsupportedSettlementRealization { occurrence } => {
                write!(
                    formatter,
                    "{} has no supported settlement realization",
                    occurrence.label()
                )
            }
            NativePhysicalEvidenceGapSubject::UnsupportedNormalizedForeignCall { occurrence } => {
                write!(
                    formatter,
                    "{} declined the normalized-foreign child derivation",
                    occurrence.label()
                )
            }
            NativePhysicalEvidenceGapSubject::UnrealizedBoundaryOccurrence { occurrence } => {
                write!(
                    formatter,
                    "{} retained no supported boundary realization",
                    occurrence.label()
                )
            }
            NativePhysicalEvidenceGapSubject::UnsupportedOperatorSpan { occurrence } => {
                write!(
                    formatter,
                    "operator occurrence on machine {} operation {} ordinal {} has no supported physical span",
                    occurrence.machine(),
                    occurrence.operation(),
                    occurrence.operation_ordinal(),
                )
            }
            NativePhysicalEvidenceGapSubject::UnownedPortEffect {
                machine,
                psi_operation,
                service,
                port,
                value,
                operation_ordinal,
                ..
            } => write!(
                formatter,
                "privileged port effect on machine {machine} operation {psi_operation} service {service} port {port} value {value} ordinal {operation_ordinal} was consumed by no MetadataOnlyPort settlement"
            ),
        }
    }
}

impl From<NativePhysicalChildParts> for NativePhysicalChild {
    fn from(parts: NativePhysicalChildParts) -> Self {
        Self {
            parent: parts.parent,
            projection: parts.projection,
            occurrence: parts.occurrence,
            machine_spans: parts.machine_spans,
            object_spans: parts.object_spans,
            final_image_spans: parts.final_image_spans,
            machine_bytes_digest: parts.machine_bytes_digest,
            object_bytes_digest: parts.object_bytes_digest,
            final_image_bytes_digest: parts.final_image_bytes_digest,
            relocation: parts.relocation,
            identity: parts.identity,
        }
    }
}

pub(crate) fn native_byte_span(offset: usize, byte_count: usize) -> NativeByteSpan {
    NativeByteSpan::from_replayed_parts(offset, byte_count)
}

pub(crate) fn sole_terminal_operation(
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
    operation: OperationId,
) -> Option<&terminal_psi::Operation> {
    let mut matching = module
        .machine(machine)?
        .operations()
        .filter(|row| row.id == operation);
    let (Some(operation), None) = (matching.next(), matching.next()) else {
        return None;
    };
    Some(operation)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn normalized_foreign_call_relocation(
    locator_identity: [u8; 32],
    boundary_plan_identity: [u8; 32],
    object_symbol: object_file::ObjectSymbolHandle,
    origin: object_file::RelocationOrigin,
    offset: usize,
    byte_width: usize,
    addend: i64,
    kind: object_file::RelocationKind,
    callbacks: Vec<NormalizedForeignCallbackRelocations>,
    final_image_symbol_identity: [u8; 32],
) -> NormalizedForeignCallRelocation {
    NormalizedForeignCallRelocation {
        locator_identity,
        boundary_plan_identity,
        object_symbol,
        origin,
        offset,
        byte_width,
        addend,
        kind,
        callbacks,
        final_image_symbol_identity,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn normalized_foreign_call_import_field(
    locator_identity: [u8; 32],
    boundary_plan_identity: [u8; 32],
    caller: MachineId,
    invocation_site: semantic_vocabulary::InvocationSite,
    boundary: BoundaryMachineId,
    ordinal: u32,
    object_symbol: object_file::ObjectLocalSymbolId,
    offset: usize,
    byte_width: usize,
    addend: i64,
    kind: object_file::RelocationKind,
    final_image_symbol_identity: [u8; 32],
) -> NormalizedForeignCallImportField {
    NormalizedForeignCallImportField {
        locator_identity,
        boundary_plan_identity,
        caller,
        invocation_site,
        boundary,
        ordinal,
        object_symbol,
        offset,
        byte_width,
        addend,
        kind,
        final_image_symbol_identity,
    }
}

fn normalized_foreign_callback_relocation(
    object_symbol: object_file::ObjectSymbolHandle,
    origin: object_file::RelocationOrigin,
    offset: usize,
    byte_width: usize,
    addend: i64,
    kind: object_file::RelocationKind,
) -> NormalizedForeignCallbackRelocation {
    NormalizedForeignCallbackRelocation {
        object_symbol,
        origin,
        offset,
        byte_width,
        addend,
        kind,
    }
}

fn optimized_boundary_occurrence(
    terminal: TerminalPsiIdentity,
    machine: MachineId,
    invocation_site: impl Into<semantic_vocabulary::InvocationSite>,
    boundary: BoundaryMachineId,
    operation_ordinal: usize,
    identity: OptimizedBoundaryOccurrenceIdentity,
) -> OptimizedBoundaryOccurrence {
    OptimizedBoundaryOccurrence {
        terminal,
        machine,
        invocation_site: invocation_site.into(),
        boundary,
        operation_ordinal,
        identity,
    }
}

fn optimized_operator_occurrence(
    terminal: TerminalPsiIdentity,
    machine: MachineId,
    operation: OperationId,
    operation_ordinal: usize,
    identity: OptimizedOperatorOccurrenceIdentity,
) -> OptimizedOperatorOccurrence {
    OptimizedOperatorOccurrence {
        terminal,
        machine,
        operation,
        operation_ordinal,
        identity,
    }
}

fn native_optimization_projection(
    terminal: TerminalPsiIdentity,
    operator_occurrences: Vec<OptimizedOperatorOccurrence>,
    boundary_occurrences: Vec<OptimizedBoundaryOccurrence>,
    identity: NativeOptimizationProjectionIdentity,
) -> NativeOptimizationProjection {
    NativeOptimizationProjection {
        terminal,
        operator_occurrences,
        boundary_occurrences,
        identity,
    }
}

fn validated_optimized_native_physical_evidence_scope(
    validation: OptimizedAbstractPlanProjectionIdentity,
    final_unit: OptimizationUnitIdentity,
    boundary_application_coverage: [u8; 32],
    projection: NativeOptimizationProjection,
    identity: [u8; 32],
) -> ValidatedOptimizedNativePhysicalEvidenceScope {
    ValidatedOptimizedNativePhysicalEvidenceScope {
        validation,
        final_unit,
        boundary_application_coverage,
        projection,
        publication: None,
        identity,
    }
}

fn validated_fragment_native_physical_evidence_scope(
    mut scope: ValidatedOptimizedNativePhysicalEvidenceScope,
    publication: self::fragment_publication::FragmentPublicationBinding,
    identity: [u8; 32],
) -> ValidatedOptimizedNativePhysicalEvidenceScope {
    scope.publication = Some(publication);
    scope.identity = identity;
    scope
}

fn native_physical_evidence(
    projection: NativeOptimizationProjection,
    children: Vec<NativePhysicalChild>,
    identity: [u8; 32],
) -> NativePhysicalEvidence {
    NativePhysicalEvidence {
        projection,
        children,
        private_functions: Vec::new(),
        identity,
    }
}

pub(crate) fn native_physical_evidence_gap(
    subject: NativePhysicalEvidenceGapSubject,
    identity: [u8; 32],
) -> NativePhysicalEvidenceGap {
    NativePhysicalEvidenceGap { subject, identity }
}
