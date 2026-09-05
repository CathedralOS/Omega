use boundary_applications::OperatorApplicationCoverageRef;
use optimization_core::{
    NativeOptimizationProjectionIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use target::NativeTarget;
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundaryScalarArgument,
    NormalizedForeignCallBinding, ProviderExecutionBinding,
};
use terminal_psi::TerminalPsiIdentity;

use crate::NativeSelectedProviderPlanDigest;

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
pub enum NativeCompilerBuiltinCatalogIdentity {
    LinuxElfV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedBoundaryOccurrence {
    terminal: TerminalPsiIdentity,
    machine: MachineId,
    operation: OperationId,
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

    pub const fn operation(&self) -> OperationId {
        self.operation
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
    pub const fn terminal(&self) -> TerminalPsiIdentity {
        self.terminal
    }

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

    pub fn boundary_occurrences(&self) -> &[OptimizedBoundaryOccurrence] {
        &self.boundary_occurrences
    }

    pub fn operator_occurrences(&self) -> &[OptimizedOperatorOccurrence] {
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
    selected_lowering_publication:
        Option<super::selected_lowering::SelectedLoweringNativePublicationBinding>,
    identity: [u8; 32],
}

impl ValidatedOptimizedNativePhysicalEvidenceScope {
    pub const fn validation(&self) -> OptimizedAbstractPlanProjectionIdentity {
        self.validation
    }

    pub const fn final_unit(&self) -> OptimizationUnitIdentity {
        self.final_unit
    }

    pub const fn projection(&self) -> &NativeOptimizationProjection {
        &self.projection
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub(super) const fn boundary_application_coverage(&self) -> &[u8; 32] {
        &self.boundary_application_coverage
    }

    pub(super) const fn selected_lowering_publication(
        &self,
    ) -> Option<&super::selected_lowering::SelectedLoweringNativePublicationBinding> {
        self.selected_lowering_publication.as_ref()
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

/// Complete role-specific D41 custody. Installed provider authority and the
/// consuming lowerer's builtin catalog remain disjoint and cannot substitute
/// for one another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryTraitSettlementRole {
    CompilerBuiltin {
        catalog: NativeCompilerBuiltinCatalogIdentity,
        execution: target_operations::CompilerBuiltinExecution,
        realization: BoundaryRealization,
        scalar_argument: BoundaryScalarArgument,
    },
    CompilerBuiltinRuntimeScalar {
        catalog: NativeCompilerBuiltinCatalogIdentity,
        execution: target_operations::CompilerBuiltinExecution,
        realization: BoundaryRealization,
        scalar_argument: machine_code::ForeignCallScalarArgumentRecord,
    },
    CompilerBuiltinStructural {
        catalog: NativeCompilerBuiltinCatalogIdentity,
        execution: target_operations::CompilerBuiltinExecution,
        realization: BoundaryRealization,
        result: machine_code::BoundaryStructuralResultRecord,
    },
    AdmittedProvider {
        execution: ProviderExecutionBinding,
        realization: NormalizedForeignCallBinding,
    },
}

impl BoundaryTraitSettlementRole {
    pub const fn execution(&self) -> BoundaryExecutionBinding {
        match self {
            Self::CompilerBuiltin { execution, .. } => {
                BoundaryExecutionBinding::CompilerBuiltin(*execution)
            }
            Self::CompilerBuiltinRuntimeScalar { execution, .. } => {
                BoundaryExecutionBinding::CompilerBuiltin(*execution)
            }
            Self::CompilerBuiltinStructural { execution, .. } => {
                BoundaryExecutionBinding::CompilerBuiltin(*execution)
            }
            Self::AdmittedProvider { execution, .. } => {
                BoundaryExecutionBinding::AdmittedProvider(*execution)
            }
        }
    }
}

impl BoundaryTraitSettlement {
    pub const fn occurrence(&self) -> &OptimizedBoundaryOccurrence {
        &self.occurrence
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn selected_plan_digest(&self) -> NativeSelectedProviderPlanDigest {
        self.selected_plan_digest
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn execution(&self) -> BoundaryExecutionBinding {
        self.role.execution()
    }

    pub const fn role(&self) -> &BoundaryTraitSettlementRole {
        &self.role
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub fn into_parts(self) -> BoundaryTraitSettlementParts {
        BoundaryTraitSettlementParts {
            occurrence: self.occurrence,
            requirement_identity: self.requirement_identity,
            selected_plan_digest: self.selected_plan_digest,
            target: self.target,
            role: self.role,
            identity: self.identity,
        }
    }

    pub fn from_replayed_parts(parts: BoundaryTraitSettlementParts) -> Self {
        parts.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalChildParent {
    OperatorApplicationCoverage(OperatorApplicationCoverageRef),
    BoundaryTraitSettlement(BoundaryTraitSettlement),
}

impl PhysicalChildParent {
    pub const fn identity(&self) -> [u8; 32] {
        match self {
            Self::OperatorApplicationCoverage(reference) => *reference.coverage().as_bytes(),
            Self::BoundaryTraitSettlement(settlement) => *settlement.identity(),
        }
    }

    pub const fn role_tag(&self) -> u8 {
        match self {
            Self::OperatorApplicationCoverage(_) => 1,
            Self::BoundaryTraitSettlement(_) => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalRelocationDisposition {
    DirectInstructionBytes,
    ResolvedInternalCall,
    UnresolvedNormalizedForeignCall(NormalizedForeignCallRelocation),
}

/// Exact unresolved import relocation retained by one normalized-foreign D41
/// child. The parent owns the complete locator and boundary contract; their
/// strong identities are repeated here to bind that semantic parent to this
/// object symbol and final-image relocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedForeignCallRelocation {
    locator_identity: [u8; 32],
    boundary_plan_identity: [u8; 32],
    object_symbol: object_file::ObjectSymbolHandle,
    origin: object_file::RelocationOrigin,
    offset: usize,
    byte_width: usize,
    addend: i64,
    kind: object_file::RelocationKind,
    callback: Option<NormalizedForeignCallbackRelocations>,
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

impl NormalizedForeignCallbackRelocations {
    pub const fn callback_function(&self) -> function_identity::MachineFunctionIdentity {
        match self {
            Self::X86_64Relative32 {
                callback_function, ..
            }
            | Self::Aarch64PageAddress {
                callback_function, ..
            } => *callback_function,
        }
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

    pub const fn callback(&self) -> Option<NormalizedForeignCallbackRelocations> {
        self.callback
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
    machine_span: NativeByteSpan,
    object_span: NativeByteSpan,
    final_image_span: NativeByteSpan,
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

    pub const fn machine_span(&self) -> NativeByteSpan {
        self.machine_span
    }

    pub const fn object_span(&self) -> NativeByteSpan {
        self.object_span
    }

    pub const fn final_image_span(&self) -> NativeByteSpan {
        self.final_image_span
    }

    pub const fn relocation(&self) -> PhysicalRelocationDisposition {
        self.relocation
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub fn into_parts(self) -> NativePhysicalChildParts {
        NativePhysicalChildParts {
            parent: self.parent,
            projection: self.projection,
            occurrence: self.occurrence,
            machine_span: self.machine_span,
            object_span: self.object_span,
            final_image_span: self.final_image_span,
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
pub struct BoundaryTraitSettlementParts {
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
    pub machine_span: NativeByteSpan,
    pub object_span: NativeByteSpan,
    pub final_image_span: NativeByteSpan,
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

impl From<NativePhysicalChildParts> for NativePhysicalChild {
    fn from(parts: NativePhysicalChildParts) -> Self {
        Self {
            parent: parts.parent,
            projection: parts.projection,
            occurrence: parts.occurrence,
            machine_span: parts.machine_span,
            object_span: parts.object_span,
            final_image_span: parts.final_image_span,
            machine_bytes_digest: parts.machine_bytes_digest,
            object_bytes_digest: parts.object_bytes_digest,
            final_image_bytes_digest: parts.final_image_bytes_digest,
            relocation: parts.relocation,
            identity: parts.identity,
        }
    }
}

pub(super) fn native_byte_span(offset: usize, byte_count: usize) -> NativeByteSpan {
    NativeByteSpan::from_replayed_parts(offset, byte_count)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn normalized_foreign_call_relocation(
    locator_identity: [u8; 32],
    boundary_plan_identity: [u8; 32],
    object_symbol: object_file::ObjectSymbolHandle,
    origin: object_file::RelocationOrigin,
    offset: usize,
    byte_width: usize,
    addend: i64,
    kind: object_file::RelocationKind,
    callback: Option<NormalizedForeignCallbackRelocations>,
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
        callback,
        final_image_symbol_identity,
    }
}

pub(super) fn normalized_foreign_callback_relocation(
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

pub(super) fn optimized_boundary_occurrence(
    terminal: TerminalPsiIdentity,
    machine: MachineId,
    operation: OperationId,
    boundary: BoundaryMachineId,
    operation_ordinal: usize,
    identity: OptimizedBoundaryOccurrenceIdentity,
) -> OptimizedBoundaryOccurrence {
    OptimizedBoundaryOccurrence {
        terminal,
        machine,
        operation,
        boundary,
        operation_ordinal,
        identity,
    }
}

pub(super) fn optimized_operator_occurrence(
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

pub(super) fn native_optimization_projection(
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

pub(super) fn validated_optimized_native_physical_evidence_scope(
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
        selected_lowering_publication: None,
        identity,
    }
}

pub(super) fn validated_selected_lowering_native_physical_evidence_scope(
    mut scope: ValidatedOptimizedNativePhysicalEvidenceScope,
    publication: super::selected_lowering::SelectedLoweringNativePublicationBinding,
    identity: [u8; 32],
) -> ValidatedOptimizedNativePhysicalEvidenceScope {
    scope.selected_lowering_publication = Some(publication);
    scope.identity = identity;
    scope
}

pub(super) fn native_physical_evidence(
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
