//! Native physical evidence: for each operation that survives optimization,
//! the machine, object and final-image byte spans that realize it, bound to
//! the operator or boundary occurrence it realizes.
//!
//! This file defines the evidence records: the surviving occurrence roster
//! (`NativeOptimizationProjection`), the validated scope that admits it
//! (`ValidatedOptimizedNativePhysicalEvidenceScope`), each physical child
//! with its parent, spans, byte digests and relocation disposition
//! (`NativePhysicalChild`), boundary settlements, normalized foreign call
//! relocations and import fields, and the gap that
//! names where a derivation stopped (`NativePhysicalEvidenceGap`). Their
//! fields are private. The constructor functions at the end of this file are
//! used only by the children below; code outside `physical` reads the
//! records, and several public records can be rebuilt from replayed parts.
//!
//! One entry is called from `native_artifact`:
//!
//! - `derive_physical_evidence` (`derivation`) derives the evidence for an
//!   emitted artifact within its scope: one child per boundary occurrence,
//!   from its installed settlement, its normalized foreign call, or an
//!   installed provider realized as a resolved internal call; a check that
//!   every retained privileged port effect was consumed by a settlement; then
//!   one child per operator occurrence, whose spans `operator_applications`
//!   derives. It returns `Complete` evidence,
//!   `Blocked` with a gap naming the first occurrence or retained record it
//!   could not bind, or `Unavailable` when the scope admits no derivation.
//!   The native artifact types call it when built from emitted parts, when
//!   rebuilt from replayed parts and when validated.
//!
//! Two further scope entries have no caller at this revision:
//! `derive_validated_optimization_scope` (`projection`) and
//! `derive_fragment_publication_scope` (`fragment_publication`). The latter is
//! referenced only by
//! `NativePhysicalEvidenceScope::from_validated_fragment_publication`, which
//! nothing calls, and the former is reached only through it; so
//! `ValidatedOptimizedProjection` has no constructor. They are unwired, not
//! superseded: the retained fragment-publication custody that only they build
//! is what the surviving `UnoptimizedCompleteBoundaryEvidence` scope lacks for
//! three boundary lanes — an installed-provider internal call, a structural
//! boundary result, and a kernel entry without a symbolic import, each of
//! which rejoins selected-plan rows no other input carries. Wiring that
//! custody to the surviving route is what lets the optimized scope and
//! `projection.rs` be deleted; deleting them first removes those lanes
//! instead.

mod derivation;
mod fragment_publication;
mod identity_bytes;
mod operator_applications;
mod projection;

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

    /// The gap-report spelling of this occurrence's four identity fields —
    /// every boundary-occurrence gap names its site the same way before its
    /// own reason clause.
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

/// Exact optimized-operation survivor roster used to derive D32 children.
/// Identity and non-identity optimization lanes share this representation.
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

/// Opaque native custody for one independently validated optimized abstract
/// projection and its exact surviving D29/D41 occurrence set.
///
/// Construction is owned by [`crate::NativePhysicalEvidenceScope`]. Keeping
/// every field private prevents replay parts from manufacturing optimizer
/// authority; callers may only retain or move a value issued by that
/// constructor.
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

    /// Foreign-call custody projected at publication time. Empty for scopes
    /// that never replayed a fragment publication source.
    pub(crate) fn foreign_call_custody(&self) -> &[image_emission::ObjectForeignCall] {
        self.publication
            .as_ref()
            .map_or(&[], |publication| publication.foreign_call_custody())
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

/// Scalar-argument custody one hosted builtin settlement retained. The two
/// shapes cover the argument rosters the settlement record can fill: a
/// compile-time scalar row or one emitted runtime scalar source record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompilerBuiltinScalarArgument {
    /// One compile-time scalar argument: the exact value, type, immediate,
    /// and ABI destination the settlement declared.
    Immediate(BoundaryScalarArgument),
    /// One runtime scalar argument retained by its emitted source record.
    RuntimeScalar(machine_code::ForeignCallScalarArgumentRecord),
}

/// Result custody one hosted builtin settlement retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompilerBuiltinResult {
    /// The builtin settles with a unit result and retains no result record.
    Unit,
    /// The builtin writes one exact structural result into its caller home.
    Structural(machine_code::BoundaryStructuralResultRecord),
}

/// Complete role-specific D41 custody. Installed provider authority and the
/// consuming lowerer's builtin catalog remain disjoint and cannot substitute
/// for one another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoundaryTraitSettlementRole {
    /// One hosted builtin settlement bound by its declared scalar-argument and
    /// result shapes. A further hosted builtin needs no new role variant: the
    /// custody kinds below already cover the argument and result shapes the
    /// settlement record can carry.
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
    /// Exact installed provider settlement realized in place by a supported
    /// target mechanism: a privileged metadata-only port effect, a direct
    /// port read, a Linux write-line byte custody, or a claim completion.
    /// The complete retained settlement row stays with the role so replay
    /// cannot substitute its realization, argument, completion, result, or
    /// source coordinates; a joined privileged port effect is retained beside
    /// it rather than widening the occurrence roster.
    AdmittedProviderSettlement {
        execution: ProviderExecutionBinding,
        settlement: machine_code::BoundarySettlementRecord,
        /// Exact privileged port effect joined to a `MetadataOnlyPort`
        /// settlement; `None` for every other admitted realization.
        port_effect: Option<machine_code::PortEffectRecord>,
    },
    /// One installed provider boundary call realized as an ordinary resolved
    /// internal call into its selected checked-machine candidate. The
    /// retained object call-stack row names the owner operation and the
    /// callee machine; the selected Terminal conformance row rejoins the
    /// exact choosing declaration, provider and candidate identities. The
    /// physical parent commits that declaration even when code is shared.
    /// An in-module provider call
    /// crosses no external boundary and carries no external execution
    /// binding, so this role retains none.
    InstalledProviderInternalCall {
        boundary: BoundaryMachineId,
        provider_declaration: String,
        provider_identity: String,
        candidate_identity: String,
        call: BoundaryInternalCallCustody,
    },
}

/// Retained call custody for a selected boundary callee realized as
/// an ordinary internal call. The variants mirror the caller's retained
/// object call-stack record family: an aggregate-frame Unit caller keeps its
/// frame terms, a scalar caller keeps caller-live bytes only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoundaryInternalCallCustody {
    /// Call-site custody of the `ObjectUnitCallStack` family.
    Unit {
        owner: CallSiteOwner,
        target: MachineId,
        /// Absolute object `.text` offset of the call's mutable field: the
        /// x86-64 rel32 displacement or the Aarch64 BL immediate bits.
        call_field_offset: usize,
        active_frame_bytes: u32,
        transient_bytes: u32,
        caller_live_bytes: u32,
    },
    /// Call-site custody of the `ObjectScalarCallStack` family.
    Scalar {
        owner: CallSiteOwner,
        target: MachineId,
        /// Absolute object `.text` offset of the call's mutable field.
        call_field_offset: usize,
        caller_live_bytes: u32,
    },
}

impl BoundaryTraitSettlement {
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
}

/// Physical custody for a checked target-family member. This parent records
/// selection from the immutable canonical roster, not provider installation
/// or external execution authority.
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

    /// The semantic occurrence class is independent from the physical parent
    /// kind encoded in its digest. Both family selection and provider settlement
    /// retain an ordinary boundary occurrence, with distinct selection custody.
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
    /// Fragment-publication custody: the call's import field lives in the
    /// retained relocation-free object plan, which owns the unresolved field
    /// row and declared import symbol in place of object relocation records.
    UnresolvedNormalizedForeignCallImportField(NormalizedForeignCallImportField),
}

/// Exact unresolved import relocation retained by one normalized-foreign D41
/// child. The parent owns the complete locator and boundary contract; their
/// strong identities are repeated here to bind that semantic parent to this
/// object symbol and final-image relocation.
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

/// Exact private-function relocation custody for the one direct callback
/// shape admitted by normalized foreign calls. The closed variants mirror the
/// two architecture-native encodings; no open-ended mutable relocation list
/// can be smuggled into D32 evidence.
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

/// One exact object relocation targeting a compiler-private callback
/// function. Construction remains derivation-owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedForeignCallbackRelocation {
    object_symbol: object_file::ObjectSymbolHandle,
    origin: object_file::RelocationOrigin,
    offset: usize,
    byte_width: usize,
    addend: i64,
    kind: object_file::RelocationKind,
}

impl NormalizedForeignCallbackRelocation {
    pub const fn object_symbol(&self) -> object_file::ObjectSymbolHandle {
        self.object_symbol
    }

    pub const fn origin(&self) -> object_file::RelocationOrigin {
        self.origin
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
}

impl NormalizedForeignCallbackRelocations {}

/// Exact unresolved import-field custody retained by one normalized-foreign
/// D41 child realized through fragment publication. The relocation-free
/// object plan binds the `{caller, operation}` field owner and the declared
/// import symbol directly; the parent locator and boundary-contract
/// identities are repeated here so the field record cannot be rebound to a
/// different call or contract.
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

    pub const fn object_symbol(&self) -> object_file::ObjectSymbolHandle {
        self.object_symbol
    }

    pub const fn origin(&self) -> object_file::RelocationOrigin {
        self.origin
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

/// One occurrence's exact ordered covered union. Allocation may insert
/// compiler-only instructions between its ABI transport and result capture;
/// those gaps are excluded rather than absorbed into a semantic span hull.
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

    /// Ordered exact covered intervals; compiler-only gaps are not source work.
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
    identity: [u8; 32],
}

impl NativePhysicalEvidence {
    pub const fn projection(&self) -> &NativeOptimizationProjection {
        &self.projection
    }

    pub fn children(&self) -> &[NativePhysicalChild] {
        &self.children
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub fn into_parts(self) -> NativePhysicalEvidenceParts {
        NativePhysicalEvidenceParts {
            projection: self.projection,
            children: self.children,
            identity: self.identity,
        }
    }

    pub fn from_replayed_parts(parts: NativePhysicalEvidenceParts) -> Self {
        Self {
            projection: parts.projection,
            children: parts.children,
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
    pub identity: [u8; 32],
}

/// The exact subject that stopped a scoped physical-evidence derivation
/// before it could bind every surviving occurrence to a physical child.
///
/// Scoped evidence is all-or-nothing: the first occurrence or retained record
/// that cannot be bound blocks the whole derivation, and the gap names that
/// subject rather than exposing a bare absence. Every variant carries enough
/// coordinate custody to rejoin the exact Terminal operation or retained
/// object record independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePhysicalEvidenceGapSubject {
    /// A normalized foreign call is owned by a cleanup action rather than an
    /// operation, so no surviving boundary occurrence can claim it.
    ForeignCallSiteOwner {
        machine: MachineId,
        owner: CallSiteOwner,
    },
    /// A surviving boundary occurrence's installed settlement carries an
    /// execution and realization pair no supported child arm admits.
    UnsupportedSettlementRealization {
        occurrence: OptimizedBoundaryOccurrence,
    },
    /// A surviving boundary occurrence's normalized foreign call declined the
    /// normalized-foreign child derivation.
    UnsupportedNormalizedForeignCall {
        occurrence: OptimizedBoundaryOccurrence,
    },
    /// A surviving boundary occurrence retained no supported realization:
    /// no installed settlement, no normalized foreign call, and no internal
    /// call custody a supported arm can bind.
    UnrealizedBoundaryOccurrence {
        occurrence: OptimizedBoundaryOccurrence,
    },
    /// A surviving operator occurrence's realization declined every physical
    /// span arm.
    UnsupportedOperatorSpan {
        occurrence: OptimizedOperatorOccurrence,
    },
    /// A retained privileged port effect was consumed by no exact
    /// `MetadataOnlyPort` settlement join.
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

/// Canonical name of the subject that stopped one scoped physical-evidence
/// derivation. The identity binds the complete subject, so an artifact's
/// identity commits to exactly where its evidence stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativePhysicalEvidenceGap {
    subject: NativePhysicalEvidenceGapSubject,
    identity: [u8; 32],
}

impl NativePhysicalEvidenceGap {
    pub const fn subject(&self) -> NativePhysicalEvidenceGapSubject {
        self.subject
    }

    /// The surviving occurrence when the blocking subject is one; `None` for
    /// retained-record and machine-level subjects.
    pub const fn occurrence(&self) -> Option<NativePhysicalOccurrence> {
        match self.subject {
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
            NativePhysicalEvidenceGapSubject::ForeignCallSiteOwner { machine, owner } => {
                match owner {
                    CallSiteOwner::Operation(operation) => write!(
                        formatter,
                        "normalized foreign call on machine {machine} is owned by operation {operation}"
                    ),
                    CallSiteOwner::ExternalCompletion(edge) => write!(
                        formatter,
                        "normalized foreign call on machine {machine} is owned by completion edge {edge}"
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
        identity,
    }
}

/// Construction stays derivation-owned: the identity must be the derivation's
/// canonical encoding of `subject`, never a caller-asserted digest.
pub(crate) fn native_physical_evidence_gap(
    subject: NativePhysicalEvidenceGapSubject,
    identity: [u8; 32],
) -> NativePhysicalEvidenceGap {
    NativePhysicalEvidenceGap { subject, identity }
}
