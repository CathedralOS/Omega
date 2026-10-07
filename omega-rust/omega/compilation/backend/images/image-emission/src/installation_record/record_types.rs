//! The installation record and its carriers: fingerprints, installed image
//! sections, installed functions, foreign call stacks and internal unit calls.

use crate::installation_record::codec::fingerprint::write_hex;
use crate::{ObjectBoundarySettlement, ObjectCodeAttribution, ObjectPortEffect};
use image::{CompilerTextValidationEvidence, FinalImageLayout};
use machine_code::StructuralReturnRecord;
use semantic_vocabulary::{MachineId, ProfileDecisionId, StructuralTypeId};
use std::num::NonZeroU64;
use target::NativeTarget;
use target_operations::CallSiteOwner;
use terminal_psi::TerminalPsiIdentity;

/// Non-authoritative report identity of one provider plan selected for this
/// installation. Exact selection authority remains outside the decodable
/// installation record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedProviderPlanReportIdentity(NonZeroU64);

impl SelectedProviderPlanReportIdentity {
    pub const fn new(raw: u64) -> Option<Self> {
        match NonZeroU64::new(raw) {
            Some(identity) => Some(Self(identity)),
            None => None,
        }
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Canonical report identities for one opaque component-progress acceptance.
/// The actual acceptance remains installation-owned and must be presented
/// again before publication; this projection merely prevents the terminal
/// artifact from shedding or substituting the accepted manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledComponentProgress {
    pub(crate) manifest: NonZeroU64,
    pub(crate) acceptance: NonZeroU64,
}

impl InstalledComponentProgress {
    pub const fn manifest_identity(self) -> u64 {
        self.manifest.get()
    }

    pub const fn acceptance_identity(self) -> u64 {
        self.acceptance.get()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImageFingerprint(pub(crate) [u8; 32]);

impl ImageFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for ImageFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for ImageFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstallationFingerprint(pub(crate) [u8; 32]);

impl InstallationFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for InstallationFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for InstallationFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_hex(formatter, &self.0)
    }
}

/// Canonical Omega-owned installation facts for one emitted terminal image.
///
/// This record is not executable authority and does not replace
/// `executable-installation`. It is the typed payload hashed under the
/// terminal artifact manifest's installation role: exact program, target,
/// profile decision, selected provider plans, image bytes, and the compiler
/// text-validation receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationRecord {
    pub(crate) psi: TerminalPsiIdentity,
    pub(crate) target: NativeTarget,
    pub(crate) subsystem: Option<u16>,
    pub(crate) profile_decision: ProfileDecisionId,
    pub(crate) selected_provider_plans: Vec<SelectedProviderPlanReportIdentity>,
    pub(crate) component_progress: Option<InstalledComponentProgress>,
    pub(crate) functions: Vec<InstalledFunction>,
    pub(crate) private_functions: Vec<InstalledCompilerPrivateFunction>,
    pub(crate) structural_returns: Vec<InstalledStructuralReturn>,
    pub(crate) internal_unit_calls: Vec<InstalledInternalUnitCall>,
    pub(crate) internal_unit_scalar_calls: Vec<InstalledInternalUnitScalarCall>,
    pub(crate) semantic_code_attribution: Vec<ObjectCodeAttribution>,
    pub(crate) port_effects: Vec<ObjectPortEffect>,
    pub(crate) boundary_settlements: Vec<ObjectBoundarySettlement>,
    pub(crate) boundary_opaque_applications:
        boundary_applications::BoundaryOpaqueRepresentationApplications,
    pub(crate) image: ImageFingerprint,
    pub(crate) image_sections: InstalledImageSections,
    pub(crate) compiler_text_validation: CompilerTextValidationEvidence,
}

impl InstallationRecord {
    #[cfg(any(test, feature = "test-support"))]
    pub fn functions_mut_for_test(&mut self) -> &mut Vec<InstalledFunction> {
        &mut self.functions
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn internal_unit_calls_mut_for_test(&mut self) -> &mut Vec<InstalledInternalUnitCall> {
        &mut self.internal_unit_calls
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn internal_unit_scalar_calls_mut_for_test(
        &mut self,
    ) -> &mut Vec<InstalledInternalUnitScalarCall> {
        &mut self.internal_unit_scalar_calls
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn structural_returns_mut_for_test(&mut self) -> &mut Vec<InstalledStructuralReturn> {
        &mut self.structural_returns
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn boundary_settlements_mut_for_test(&mut self) -> &mut Vec<ObjectBoundarySettlement> {
        &mut self.boundary_settlements
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn private_functions_mut_for_test(&mut self) -> &mut Vec<InstalledCompilerPrivateFunction> {
        &mut self.private_functions
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn semantic_code_attribution_mut_for_test(&mut self) -> &mut Vec<ObjectCodeAttribution> {
        &mut self.semantic_code_attribution
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn port_effects_mut_for_test(&mut self) -> &mut Vec<ObjectPortEffect> {
        &mut self.port_effects
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn psi_mut_for_test(&mut self) -> &mut TerminalPsiIdentity {
        &mut self.psi
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn target_mut_for_test(&mut self) -> &mut NativeTarget {
        &mut self.target
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn subsystem_mut_for_test(&mut self) -> &mut Option<u16> {
        &mut self.subsystem
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn profile_decision_mut_for_test(&mut self) -> &mut ProfileDecisionId {
        &mut self.profile_decision
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn selected_provider_plans_mut_for_test(
        &mut self,
    ) -> &mut Vec<SelectedProviderPlanReportIdentity> {
        &mut self.selected_provider_plans
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn component_progress_mut_for_test(&mut self) -> &mut Option<InstalledComponentProgress> {
        &mut self.component_progress
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn image_mut_for_test(&mut self) -> &mut ImageFingerprint {
        &mut self.image
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn image_sections_mut_for_test(&mut self) -> &mut InstalledImageSections {
        &mut self.image_sections
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn compiler_text_validation_mut_for_test(&mut self) -> &mut CompilerTextValidationEvidence {
        &mut self.compiler_text_validation
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn boundary_opaque_applications_mut_for_test(
        &mut self,
    ) -> &mut boundary_applications::BoundaryOpaqueRepresentationApplications {
        &mut self.boundary_opaque_applications
    }

    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn subsystem(&self) -> Option<u16> {
        self.subsystem
    }

    pub const fn profile_decision(&self) -> ProfileDecisionId {
        self.profile_decision
    }

    pub fn selected_provider_plans(&self) -> &[SelectedProviderPlanReportIdentity] {
        &self.selected_provider_plans
    }

    pub const fn component_progress(&self) -> Option<InstalledComponentProgress> {
        self.component_progress
    }

    pub fn boundary_settlements(&self) -> &[ObjectBoundarySettlement] {
        &self.boundary_settlements
    }

    /// Canonical selected-application custody this record claims for the
    /// artifact's by-value opaque boundary edges. Replay agreement with the
    /// bound artifact's coverage is checked at `bind_installed_artifact`.
    pub const fn boundary_opaque_applications(
        &self,
    ) -> &boundary_applications::BoundaryOpaqueRepresentationApplications {
        &self.boundary_opaque_applications
    }

    pub fn functions(&self) -> &[InstalledFunction] {
        &self.functions
    }

    /// Compiler-private functions remain a namespace disjoint from semantic
    /// program functions even when their artifact-local `MachineId`s happen
    /// to have the same numeric value.
    pub fn private_functions(&self) -> &[InstalledCompilerPrivateFunction] {
        &self.private_functions
    }

    pub fn structural_returns(&self) -> &[InstalledStructuralReturn] {
        &self.structural_returns
    }

    pub fn internal_unit_calls(&self) -> &[InstalledInternalUnitCall] {
        &self.internal_unit_calls
    }

    pub fn internal_unit_scalar_calls(&self) -> &[InstalledInternalUnitScalarCall] {
        &self.internal_unit_scalar_calls
    }

    pub fn semantic_code_attribution(&self) -> &[ObjectCodeAttribution] {
        &self.semantic_code_attribution
    }

    pub fn port_effects(&self) -> &[ObjectPortEffect] {
        &self.port_effects
    }

    pub const fn image(&self) -> ImageFingerprint {
        self.image
    }

    pub const fn image_sections(&self) -> InstalledImageSections {
        self.image_sections
    }

    pub const fn compiler_text_validation(&self) -> CompilerTextValidationEvidence {
        self.compiler_text_validation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InitializedDataFingerprint(pub(crate) [u8; 32]);

impl InitializedDataFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[cfg(any(test, feature = "test-support"))]
    pub const fn for_test(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledImageSections {
    pub layout: FinalImageLayout,
    /// Compiler-authored text prefix. Image-writer thunks remain in the bound
    /// image but outside this bounded installation projection.
    pub text_byte_count: usize,
    /// Compiler-authored initialized-data prefix. Image-writer binding slots
    /// remain in the bound image but outside this immutable-table projection.
    pub data_byte_count: usize,
    pub final_data_fingerprint: InitializedDataFingerprint,
    /// Complete final `.text` extent, including image-writer import thunks.
    /// `final_text_byte_count >= text_byte_count` always holds.
    pub final_text_byte_count: usize,
    /// Complete final initialized-data extent, including image-writer binding
    /// slots and alignment padding.
    pub final_data_byte_count: usize,
    /// Strong identity of the complete placed executable-region inventory:
    /// addresses, exact bytes, origin classes, and gaps for every `.text`
    /// byte. This is the sealed claim the installed artifact must replay.
    pub executable_inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    /// Strong identity of the complete placed initialized-data inventory,
    /// covering compiler data plus writer-installed binding slots.
    pub data_inventory_digest: image::PlacedDataRegionInventoryDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    /// Exact owner-designated cleanup subject, independent of attachment or call sites.
    pub nominal_cleanup_subject: Option<StructuralTypeId>,
    pub scalar_abi: Option<target_operations::ScalarFunctionAbi>,
    pub mixed_structural_scalar_abi: Option<target_operations::MixedStructuralScalarFunctionAbi>,
    pub parameter_abi: Option<machine_code::ParameterFunctionAbiRecord>,
    pub structural_call_scalar_return: Option<machine_code::StructuralCallScalarReturnEvidence>,
    pub text_offset: usize,
    pub byte_count: usize,
    /// Stack facts recomputed from exact target instructions at object
    /// construction. Retaining them here seals the emitter-derived local frame
    /// and call-edge inputs needed by later installed-root WCSU composition.
    pub unit_stack: Option<crate::ObjectUnitStack>,
    pub scalar_stack: Option<crate::ObjectScalarStack>,
    pub unit_call_stacks: Vec<crate::ObjectUnitCallStack>,
    pub scalar_call_stacks: Vec<crate::ObjectScalarCallStack>,
    /// Non-authoritative projection of exact emitted same-stack foreign leaves.
    /// Canonical installation validation rejoins every row to the retained
    /// executable image before stack composition may consume it.
    pub foreign_call_stacks: Vec<InstalledForeignCallStack>,
    pub unit_body: bool,
    pub unit_parameters: Vec<machine_code::UnitParameterRecord>,
    pub unit_parameter_homes: Vec<machine_code::UnitParameterHomeRecord>,
    pub unit_scalar_homes: Vec<machine_code::UnitScalarHomeRecord>,
    pub unit_integer_constants: Vec<machine_code::UnitIntegerConstantRecord>,
    pub unit_affine_scalar_records: Vec<machine_code::UnitAffineScalarRecordEstablishmentRecord>,
    pub unit_structural_scalar_field_stores:
        Vec<machine_code::UnitStructuralScalarFieldStoreRecord>,
    pub unit_write_only_primitive_stores: Vec<machine_code::UnitWriteOnlyPrimitiveStoreRecord>,
    pub scalar_structural_scalar_field_stores:
        Vec<machine_code::ScalarStructuralScalarFieldStoreRecord>,
    pub unit_continuations: Vec<machine_code::UnitContinuationRecord>,
    pub unit_affine_cleanup: Option<machine_code::UnitAffineCleanupRecord>,
    pub scalar_affine_cleanup: Option<machine_code::UnitAffineCleanupRecord>,
    /// Canonical true-before-false DFS cleanup leaves for the exact bounded
    /// two-decision/three-return scalar-control carrier. This remains distinct
    /// from the branch-free scalar cleanup above: each physical suffix owns its
    /// terminal-Psi return edge and exact byte interval.
    pub scalar_control_affine_cleanups: Vec<machine_code::UnitAffineCleanupRecord>,
    pub scalar_structural_parameters: Vec<machine_code::UnitParameterRecord>,
    pub scalar_structural_parameter_homes: Vec<machine_code::UnitParameterHomeRecord>,
}

/// Canonical installation custody for one compiler-private callback thunk.
///
/// The executable image remains authoritative for native bytes and symbol
/// replay. This row prevents installation serialization from shedding or
/// substituting the exact private identity, source program, ABI, or final text
/// interval already validated by object and image construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCompilerPrivateFunction {
    pub identity: function_identity::MachineFunctionIdentity,
    pub source_psi: TerminalPsiIdentity,
    pub machine: MachineId,
    pub scalar_abi: target_operations::ScalarFunctionAbi,
    pub text_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledForeignCallStack {
    pub owner: CallSiteOwner,
    pub text_offset: usize,
    pub caller_live_bytes: u32,
    pub provider_plan_report_identity: u64,
    pub contribution_report_identity: task_plans::AdmittedStackContributionReportId,
    pub contribution_commitment: task_plans::SameStackContributionCommitment,
    pub contribution_bytes: u64,
    pub contribution_alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledStructuralReturn {
    pub machine: MachineId,
    pub returned: StructuralReturnRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledInternalUnitCall {
    pub machine: MachineId,
    pub text_offset: usize,
    pub custody: machine_code::InternalUnitCallRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledInternalUnitScalarCall {
    pub machine: MachineId,
    pub text_offset: usize,
    pub custody: machine_code::InternalUnitScalarCallRecord,
}
