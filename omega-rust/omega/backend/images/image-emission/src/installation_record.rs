//! Canonical installation record wire format: the manifest metadata sealed
//! over one emitted executable image. This root owns the record carrier
//! types, the format marker, construction from an image, encode/decode of the
//! envelope, and validation against the image; each `*_codec` child owns one
//! record family's bytes, `record_shape` owns canonical-shape validation of a
//! decoded record, and `installed_unit_scalar_transport` plus the structural
//! children replay installed call custody. The record grants no executable
//! authority.

use std::num::NonZeroU64;

use crate::object_artifact::replay::boundary::byte_sequence_custody::linux_write_line_custody_is_exact;
use crate::object_artifact::replay::boundary::completion_receipts::{
    CompletionCustodyError, validate_completion_custody,
};
use crate::object_artifact::replay::boundary::result_placement::boundary_result_is_exact;
use crate::object_artifact::replay::boundary::runtime_scalar_custody::hosted_write_byte_custody_is_exact;
use crate::{
    ExecutableImage, ObjectBoundarySettlement, ObjectCodeAttribution, ObjectPortEffect,
    can_emit_executable_image,
};
use calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValueClass, ValueLocation, ValuePlacement,
    ValueShape, evaluate_call_plan,
};
use image::{CompilerTextValidationEvidence, FinalImageLayout};
use machine_code::{SemanticCodeSite, StructuralReturnRecord};
use semantic_vocabulary::{
    MachineId, OperationId, PlaceId, ProfileDecisionId, StructuralTypeId, ValueId,
};
use target::{Architecture, NativeTarget, ObjectFormat};
use target_operations::{BoundaryRealization, CallSiteOwner};
use terminal_psi::{
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape, TerminalPsiIdentity,
};

mod borrowed_structural;
mod boundary_result_scalar_codec;
mod boundary_settlement_codec;
mod call_site_owner_codec;
mod completion_custody_codec;
mod dynamic_conformance_codec;
mod fingerprint_codec;
mod function_affine_cleanup_codec;
mod function_codec;
mod function_parameter_codec;
mod function_stack_codec;
mod graph_structural;
mod incoming_structural;
mod installation_header_codec;
mod installed_unit_scalar_transport;
mod internal_unit_call_codec;
mod internal_unit_call_source_codec;
mod internal_unit_scalar_call_codec;
mod mixed_structural_scalar_abi_codec;
mod parameter_abi_codec;
mod port_effect_codec;
mod private_function_codec;
mod provider_execution_codec;
mod provider_plan_codec;
mod record_shape;
#[cfg(test)]
mod resource_tests;
mod scalar_abi_codec;
mod scalar_call_plan_codec;
mod scalar_structural_scalar_field_store_codec;
mod semantic_code_attribution;
mod semantic_code_attribution_codec;
mod structural_argument_codec;
mod structural_case_codec;
mod structural_field_codec;
mod structural_record_codec;
mod structural_return_codec;
mod structural_scalar_codec;
mod structural_signature_codec;
mod structural_source_codec;
mod structural_type_codec;
mod trivial_affine_local_codec;
mod unit_continuation_codec;
mod unit_dynamic_descriptor_join;
mod unit_scalar_codec;
mod unit_structural_scalar_field_store_codec;
mod unit_write_only_primitive_store_codec;
mod value_placement_codec;
mod wire_codec;

use boundary_settlement_codec::{decode_boundary_settlements, encode_boundary_settlements};
use dynamic_conformance_codec::{
    decode_dynamic_conformance_custody, encode_dynamic_conformance_custody,
};
use fingerprint_codec::{
    fingerprint_image, fingerprint_initialized_data, fingerprint_record, write_hex,
};
use function_codec::{decode_functions, encode_functions};
use installation_header_codec::{
    DecodedInstallationHeader, decode_installation_header, encode_installation_header,
};
use installed_unit_scalar_transport::{
    installed_forwarded_dynamic_scalar_result_is_canonical,
    installed_function_scalar_transport_is_canonical, validate_installed_unit_scalar_calls,
    validate_installed_unit_structural_scalar_field_stores,
    validate_installed_unit_write_only_primitive_stores,
};
use internal_unit_call_codec::{decode_internal_unit_calls, encode_internal_unit_calls};
use internal_unit_scalar_call_codec::{
    decode_internal_unit_scalar_calls, encode_internal_unit_scalar_calls,
};
use port_effect_codec::{decode_port_effects, encode_port_effects};
use private_function_codec::{decode_private_functions, encode_private_functions};
use provider_plan_codec::{decode_provider_plans, encode_provider_plans};
use record_shape::validate_record_shape;
use semantic_code_attribution_codec::{
    decode_semantic_code_attributions, encode_semantic_code_attributions,
};
use structural_case_codec::{decode_structural_cases, encode_structural_cases};
use structural_record_codec::{decode_structural_fields, encode_structural_fields};
use structural_return_codec::{decode_structural_returns, encode_structural_returns};
use structural_scalar_codec::{
    decode_identity, decode_multiplicity, encode_identity, multiplicity_tag,
};
use structural_type_codec::{decode_structural_types, encode_structural_types};
use unit_dynamic_descriptor_join::validate_installed_unit_dynamic_descriptor_joins;
use wire_codec::{Reader, decode_boolean, push_u16, push_u32, push_u64, push_u128};

// The current vocabulary includes owned incoming stack pointers and AArch64's
// dedicated indirect-result register. Earlier envelopes cannot carry those roles.
pub const INSTALLATION_FORMAT_MARKER: u16 = 96;

fn direct_structural_return_placement(placement: &ValuePlacement) -> bool {
    if placement.shape.class != ValueClass::Integer
        || !((placement.shape.byte_size == 8 && placement.shape.alignment == 8)
            || (9..=16).contains(&placement.shape.byte_size))
        || !(1..=2).contains(&placement.locations.len())
    {
        return false;
    }
    let mut expected_offset = 0_u16;
    for location in &placement.locations {
        let ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        } = *location
        else {
            return false;
        };
        let expected_size = (placement.shape.byte_size - expected_offset).min(8);
        if value_byte_offset != expected_offset || byte_size != expected_size {
            return false;
        }
        let Some(next) = expected_offset.checked_add(byte_size) else {
            return false;
        };
        expected_offset = next;
    }
    expected_offset == placement.shape.byte_size
}
const MAGIC: &[u8; 8] = b"PSIINST\0";

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
    manifest: NonZeroU64,
    acceptance: NonZeroU64,
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
pub struct ImageFingerprint([u8; 32]);

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
pub struct InstallationFingerprint([u8; 32]);

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
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    subsystem: Option<u16>,
    profile_decision: ProfileDecisionId,
    selected_provider_plans: Vec<SelectedProviderPlanReportIdentity>,
    component_progress: Option<InstalledComponentProgress>,
    functions: Vec<InstalledFunction>,
    private_functions: Vec<InstalledCompilerPrivateFunction>,
    structural_returns: Vec<InstalledStructuralReturn>,
    internal_unit_calls: Vec<InstalledInternalUnitCall>,
    internal_unit_scalar_calls: Vec<InstalledInternalUnitScalarCall>,
    dynamic_conformance_tables: Vec<InstalledDynamicConformanceTable>,
    dynamic_calls: Vec<InstalledDynamicCall>,
    stored_dynamic_calls: Vec<InstalledStoredDynamicCall>,
    forwarded_dynamic_descriptor_adapters: Vec<InstalledForwardedDynamicDescriptorAdapter>,
    forwarded_dynamic_descriptor_tables: Vec<InstalledForwardedDynamicDescriptorTable>,
    forwarded_dynamic_descriptor_calls: Vec<InstalledForwardedDynamicDescriptorCall>,
    dynamic_parameter_calls: Vec<InstalledDynamicParameterCall>,
    forwarded_dynamic_parameter_calls: Vec<InstalledForwardedDynamicParameterCall>,
    semantic_code_attribution: Vec<ObjectCodeAttribution>,
    port_effects: Vec<ObjectPortEffect>,
    boundary_settlements: Vec<ObjectBoundarySettlement>,
    image: ImageFingerprint,
    image_sections: InstalledImageSections,
    compiler_text_validation: CompilerTextValidationEvidence,
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
    pub fn boundary_settlements_mut_for_test(&mut self) -> &mut Vec<ObjectBoundarySettlement> {
        &mut self.boundary_settlements
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn private_functions_mut_for_test(&mut self) -> &mut Vec<InstalledCompilerPrivateFunction> {
        &mut self.private_functions
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn dynamic_conformance_tables_mut_for_test(
        &mut self,
    ) -> &mut Vec<InstalledDynamicConformanceTable> {
        &mut self.dynamic_conformance_tables
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn dynamic_calls_mut_for_test(&mut self) -> &mut Vec<InstalledDynamicCall> {
        &mut self.dynamic_calls
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn dynamic_parameter_calls_mut_for_test(
        &mut self,
    ) -> &mut Vec<InstalledDynamicParameterCall> {
        &mut self.dynamic_parameter_calls
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn stored_dynamic_calls_mut_for_test(&mut self) -> &mut Vec<InstalledStoredDynamicCall> {
        &mut self.stored_dynamic_calls
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn forwarded_dynamic_descriptor_adapters_mut_for_test(
        &mut self,
    ) -> &mut Vec<InstalledForwardedDynamicDescriptorAdapter> {
        &mut self.forwarded_dynamic_descriptor_adapters
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn forwarded_dynamic_descriptor_tables_mut_for_test(
        &mut self,
    ) -> &mut Vec<InstalledForwardedDynamicDescriptorTable> {
        &mut self.forwarded_dynamic_descriptor_tables
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn forwarded_dynamic_descriptor_calls_mut_for_test(
        &mut self,
    ) -> &mut Vec<InstalledForwardedDynamicDescriptorCall> {
        &mut self.forwarded_dynamic_descriptor_calls
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn forwarded_dynamic_parameter_calls_mut_for_test(
        &mut self,
    ) -> &mut Vec<InstalledForwardedDynamicParameterCall> {
        &mut self.forwarded_dynamic_parameter_calls
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn semantic_code_attribution_mut_for_test(&mut self) -> &mut Vec<ObjectCodeAttribution> {
        &mut self.semantic_code_attribution
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn port_effects_mut_for_test(&mut self) -> &mut Vec<ObjectPortEffect> {
        &mut self.port_effects
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

    pub fn dynamic_conformance_tables(&self) -> &[InstalledDynamicConformanceTable] {
        &self.dynamic_conformance_tables
    }

    pub fn dynamic_calls(&self) -> &[InstalledDynamicCall] {
        &self.dynamic_calls
    }

    pub fn stored_dynamic_calls(&self) -> &[InstalledStoredDynamicCall] {
        &self.stored_dynamic_calls
    }

    pub fn forwarded_dynamic_descriptor_adapters(
        &self,
    ) -> &[InstalledForwardedDynamicDescriptorAdapter] {
        &self.forwarded_dynamic_descriptor_adapters
    }

    pub fn forwarded_dynamic_descriptor_tables(
        &self,
    ) -> &[InstalledForwardedDynamicDescriptorTable] {
        &self.forwarded_dynamic_descriptor_tables
    }

    pub fn forwarded_dynamic_descriptor_calls(&self) -> &[InstalledForwardedDynamicDescriptorCall] {
        &self.forwarded_dynamic_descriptor_calls
    }

    pub fn dynamic_parameter_calls(&self) -> &[InstalledDynamicParameterCall] {
        &self.dynamic_parameter_calls
    }

    pub fn forwarded_dynamic_parameter_calls(&self) -> &[InstalledForwardedDynamicParameterCall] {
        &self.forwarded_dynamic_parameter_calls
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
pub struct InitializedDataFingerprint([u8; 32]);

impl InitializedDataFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledDynamicConformanceTable {
    pub application_commitment: terminal_psi::ClosedConformanceApplicationCommitment,
    pub application_report_fingerprint: u64,
    pub data_offset: usize,
    pub byte_count: usize,
    pub slots: Vec<InstalledDynamicConformanceSlot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledDynamicConformanceSlot {
    pub row_index: u32,
    pub target: Option<MachineId>,
    pub data_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledDynamicCall {
    pub machine: MachineId,
    pub operation: OperationId,
    pub application_commitment: terminal_psi::ClosedConformanceApplicationCommitment,
    pub initial_source: PlaceId,
    pub rebound_source: PlaceId,
    pub selected_table_byte_offset: u32,
    pub realization: MachineId,
    pub text_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledStoredDynamicCall {
    pub machine: MachineId,
    pub establishment_operation: OperationId,
    pub operation: OperationId,
    pub descriptor_ordinal: u32,
    pub selection_ordinal: u32,
    pub application_commitment: terminal_psi::ClosedConformanceApplicationCommitment,
    pub source: PlaceId,
    pub descriptor_home_byte_offset: u32,
    pub selected_table_byte_offset: u32,
    pub realization: MachineId,
    pub establishment_text_offset: usize,
    pub establishment_byte_count: usize,
    pub text_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledForwardedDynamicDescriptorAdapter {
    pub application_commitment: terminal_psi::ClosedConformanceApplicationCommitment,
    pub row_index: u32,
    pub realization: MachineId,
    pub text_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledForwardedDynamicDescriptorTable {
    pub application_commitment: terminal_psi::ClosedConformanceApplicationCommitment,
    pub application_report_fingerprint: u64,
    pub data_offset: usize,
    pub byte_count: usize,
    pub slots: Vec<InstalledForwardedDynamicDescriptorSlot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledForwardedDynamicDescriptorSlot {
    pub row_index: u32,
    pub realization: MachineId,
    pub adapter_text_offset: usize,
    pub data_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledForwardedDynamicDescriptorCall {
    pub machine: MachineId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub application_commitment: terminal_psi::ClosedConformanceApplicationCommitment,
    pub source: terminal_psi::StructuralArgument,
    pub semantic_result: Option<abstract_operations::AbstractResult>,
    pub result: Option<machine_code::InternalUnitScalarCallResultRecord>,
    pub text_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledDynamicParameterCall {
    pub machine: MachineId,
    pub operation: OperationId,
    pub source_value: Option<ValueId>,
    pub requirement_slot: u32,
    pub text_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledForwardedDynamicParameterCall {
    pub machine: MachineId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub source_value: Option<ValueId>,
    pub scalar_type: Option<semantic_vocabulary::ScalarType>,
    pub source_parameter_ordinal: u32,
    pub target_parameter_ordinal: u32,
    pub text_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
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

/// Build the canonical installation record for an emitted image.
///
/// This convenience path succeeds only when the image has no provider-backed
/// settlements. Effectful images must use the admission-bearing constructor.
pub fn build_installation_record(
    image: &ExecutableImage,
    profile_decision: ProfileDecisionId,
) -> Result<InstallationRecord, InstallationError> {
    build_installation_record_with_provider_executions(
        image,
        profile_decision,
        std::iter::empty::<&dyn installation_evidence::ProviderExecutionEvidence>(),
    )
}

/// Build an installation record from the same ledger-owned provider
/// executions consumed by effectful terminal lowering.
///
/// The execution closure must match the image's retained settlement evidence
/// exactly. Numeric provider-plan identities are derived here and cannot be
/// supplied independently by the caller.
pub fn build_installation_record_with_provider_executions<'execution, Execution>(
    image: &ExecutableImage,
    profile_decision: ProfileDecisionId,
    provider_executions: impl IntoIterator<Item = &'execution Execution>,
) -> Result<InstallationRecord, InstallationError>
where
    Execution: installation_evidence::ProviderExecutionEvidence + ?Sized + 'execution,
{
    build_installation_record_with_evidence(image, profile_decision, provider_executions, None)
}

/// Build the terminal installation record while committing one already
/// admitted component-progress closure into artifact identity. The evidence
/// trait exposes report identities only; runtime publication must still
/// retain and replay the opaque acceptance owned by orchestration.
pub fn build_installation_record_with_evidence<'execution, Execution>(
    image: &ExecutableImage,
    profile_decision: ProfileDecisionId,
    provider_executions: impl IntoIterator<Item = &'execution Execution>,
    component_progress: Option<&dyn installation_evidence::ComponentProgressAcceptanceEvidence>,
) -> Result<InstallationRecord, InstallationError>
where
    Execution: installation_evidence::ProviderExecutionEvidence + ?Sized + 'execution,
{
    let provider_executions = provider_executions.into_iter().collect::<Vec<_>>();
    let mut selected_provider_plans = provider_executions
        .iter()
        .map(|execution| execution.provider_plan_report_identity())
        .collect::<Vec<_>>();
    selected_provider_plans.sort_unstable();
    selected_provider_plans.dedup();
    build_installation_record_with_selected_provider_plans_and_evidence(
        image,
        profile_decision,
        selected_provider_plans,
        provider_executions,
        component_progress,
    )
}

/// Build a terminal installation record from the complete selected provider
/// plan closure plus the admitted executions actually used by the image.
///
/// Selected plans need not all execute in this image, but every execution must
/// belong to the selected closure and the execution closure must still match
/// the image's retained boundary settlements exactly.
pub fn build_installation_record_with_selected_provider_plans_and_evidence<'execution, Execution>(
    image: &ExecutableImage,
    profile_decision: ProfileDecisionId,
    selected_provider_plans: impl IntoIterator<Item = u64>,
    provider_executions: impl IntoIterator<Item = &'execution Execution>,
    component_progress: Option<&dyn installation_evidence::ComponentProgressAcceptanceEvidence>,
) -> Result<InstallationRecord, InstallationError>
where
    Execution: installation_evidence::ProviderExecutionEvidence + ?Sized + 'execution,
{
    let compiler_text_validation = image
        .output()
        .compiler_text_validation
        .ok_or(InstallationError::MissingCompilerTextValidation)?;
    let mut reported_executions = std::collections::BTreeSet::new();
    let mut selected_provider_plan_set = std::collections::BTreeSet::new();
    for identity in selected_provider_plans {
        let identity = SelectedProviderPlanReportIdentity::new(identity)
            .ok_or(InstallationError::ZeroProviderPlan)?;
        if !selected_provider_plan_set.insert(identity) {
            return Err(InstallationError::DuplicateProviderPlan);
        }
    }
    for execution in provider_executions {
        if !reported_executions.insert((
            execution.provider_plan_report_identity(),
            execution.provider_execution_report_identity(),
            execution.provider_execution_report_fingerprint(),
            execution.normalized_root_report_identity(),
            execution.boundary_contract_report_fingerprint(),
        )) {
            return Err(InstallationError::DuplicateProviderExecution);
        }
        let execution_plan =
            SelectedProviderPlanReportIdentity::new(execution.provider_plan_report_identity())
                .ok_or(InstallationError::ZeroProviderPlan)?;
        if !selected_provider_plan_set.contains(&execution_plan) {
            return Err(InstallationError::ProviderExecutionOutsideSelectedClosure);
        }
    }
    let required_executions = image
        .boundary_settlements()
        .iter()
        .filter_map(|installed| {
            let machine_code::BoundaryExecutionRecord::AdmittedProvider(execution) =
                installed.settlement.execution
            else {
                return None;
            };
            Some((
                execution.provider_plan_report_identity,
                execution.provider_execution_report_identity,
                execution.provider_execution_report_fingerprint,
                execution.normalized_root_report_identity,
                execution.boundary_contract_report_fingerprint,
            ))
        })
        .chain(image.foreign_calls().iter().map(|call| {
            let execution = call.provider_execution;
            (
                execution.provider_plan_report_identity,
                execution.provider_execution_report_identity,
                execution.provider_execution_report_fingerprint,
                execution.normalized_root_report_identity,
                execution.boundary_contract_report_fingerprint,
            )
        }))
        .collect::<std::collections::BTreeSet<_>>();
    if reported_executions != required_executions {
        return Err(InstallationError::ProviderExecutionClosureMismatch);
    }
    let component_progress = component_progress
        .map(|acceptance| {
            let manifest = NonZeroU64::new(acceptance.component_progress_manifest_identity())
                .ok_or(InstallationError::ZeroComponentProgressManifestIdentity)?;
            let acceptance = NonZeroU64::new(acceptance.component_progress_acceptance_identity())
                .ok_or(InstallationError::ZeroComponentProgressAcceptanceIdentity)?;
            Ok(InstalledComponentProgress {
                manifest,
                acceptance,
            })
        })
        .transpose()?;
    let record = InstallationRecord {
        psi: image.psi(),
        target: image.target(),
        subsystem: image.subsystem(),
        profile_decision,
        selected_provider_plans: selected_provider_plan_set.into_iter().collect(),
        component_progress,
        functions: image
            .functions()
            .iter()
            .map(|function| InstalledFunction {
                machine: function.machine,
                scalar_abi: function.scalar_abi.clone(),
                mixed_structural_scalar_abi: function.mixed_structural_scalar_abi.clone(),
                parameter_abi: function.parameter_abi.clone(),
                structural_call_scalar_return: function.structural_call_scalar_return,
                text_offset: function.text_offset,
                byte_count: function.byte_count,
                unit_stack: function.unit_stack,
                scalar_stack: function.scalar_stack,
                unit_call_stacks: function.unit_call_stacks.clone(),
                scalar_call_stacks: function.scalar_call_stacks.clone(),
                foreign_call_stacks: installed_foreign_call_stacks(image, function.machine),
                unit_body: function.unit_affine_cleanup.is_some(),
                unit_parameters: function.unit_parameters.clone(),
                unit_parameter_homes: function.unit_parameter_homes.clone(),
                unit_scalar_homes: function.unit_scalar_homes.clone(),
                unit_integer_constants: function.unit_integer_constants.clone(),
                unit_affine_scalar_records: function.unit_affine_scalar_records.clone(),
                unit_structural_scalar_field_stores: function
                    .unit_structural_scalar_field_stores
                    .clone(),
                unit_write_only_primitive_stores: function.unit_write_only_primitive_stores.clone(),
                scalar_structural_scalar_field_stores: function
                    .scalar_structural_scalar_field_stores
                    .clone(),
                unit_continuations: function.unit_continuations.clone(),
                unit_affine_cleanup: function.unit_affine_cleanup.clone(),
                scalar_affine_cleanup: function.scalar_affine_cleanup.clone(),
                scalar_control_affine_cleanups: function
                    .scalar_control_affine_cleanups
                    .iter()
                    .map(|record| record.cleanup.clone())
                    .collect(),
                scalar_structural_parameters: function.scalar_structural_parameters.clone(),
                scalar_structural_parameter_homes: function
                    .scalar_structural_parameter_homes
                    .clone(),
                attachment: function.attachment,
            })
            .collect(),
        private_functions: image
            .private_functions()
            .iter()
            .map(installed_compiler_private_function)
            .collect::<Result<Vec<_>, _>>()?,
        structural_returns: image
            .functions()
            .iter()
            .filter_map(|function| {
                function
                    .structural_return
                    .clone()
                    .map(|returned| InstalledStructuralReturn {
                        machine: function.machine,
                        returned,
                    })
            })
            .collect(),
        internal_unit_calls: installed_internal_unit_calls(image),
        internal_unit_scalar_calls: image
            .functions()
            .iter()
            .flat_map(|function| {
                function
                    .internal_unit_scalar_calls
                    .iter()
                    .cloned()
                    .map(|custody| InstalledInternalUnitScalarCall {
                        machine: function.machine,
                        text_offset: function.text_offset + custody.code_offset,
                        custody,
                    })
            })
            .collect(),
        dynamic_conformance_tables: installed_dynamic_conformance_tables(image),
        dynamic_calls: installed_dynamic_calls(image)?,
        stored_dynamic_calls: installed_stored_dynamic_calls(image)?,
        forwarded_dynamic_descriptor_adapters: installed_forwarded_dynamic_descriptor_adapters(
            image,
        ),
        forwarded_dynamic_descriptor_tables: installed_forwarded_dynamic_descriptor_tables(image),
        forwarded_dynamic_descriptor_calls: installed_forwarded_dynamic_descriptor_calls(image)?,
        dynamic_parameter_calls: installed_dynamic_parameter_calls(image)?,
        forwarded_dynamic_parameter_calls: installed_forwarded_dynamic_parameter_calls(image)?,
        semantic_code_attribution: image.semantic_code_attribution().to_vec(),
        port_effects: image.port_effects().to_vec(),
        boundary_settlements: image.boundary_settlements().to_vec(),
        image: fingerprint_image(&image.output().bytes),
        image_sections: installed_image_sections(image),
        compiler_text_validation,
    };
    validate_record_shape(&record)?;
    Ok(record)
}

/// Recompose the exact internal stack closure retained by a canonical
/// installation record. The selected entry is supplied by installed-root
/// realization; external entry-adapter and interrupt-arrival demand remain
/// outside this artifact-owned closure.
pub fn derive_installation_stack_demand(
    record: &InstallationRecord,
    image: &ExecutableImage,
    entry: MachineId,
) -> Result<crate::StackDemand, InstallationStackError> {
    validate_installation_record(record, image)?;
    let functions = record
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    if !functions.contains_key(&entry) {
        return Err(crate::ObjectError::EntryFunctionMissing(entry).into());
    }
    let mut active = std::collections::BTreeSet::new();
    let mut memoized = std::collections::BTreeMap::new();
    let mut contributing_machines = std::collections::BTreeSet::new();
    let mut admitted_contribution_report_identities = std::collections::BTreeSet::new();
    let mut admitted_contribution_commitments = std::collections::BTreeSet::new();
    let ceiling_bytes = derive_installed_stack_peak(
        entry,
        &functions,
        &mut active,
        &mut memoized,
        &mut contributing_machines,
        &mut admitted_contribution_report_identities,
        &mut admitted_contribution_commitments,
    )?;
    Ok(crate::StackDemand {
        psi: record.psi,
        target: record.target,
        entry,
        ceiling_bytes,
        stack_alignment: 16,
        contributing_machines,
        admitted_contribution_report_identities,
        admitted_contribution_commitments,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallationStackError {
    Installation(InstallationError),
    Stack(crate::ObjectError),
}

impl From<InstallationError> for InstallationStackError {
    fn from(error: InstallationError) -> Self {
        Self::Installation(error)
    }
}

impl From<crate::ObjectError> for InstallationStackError {
    fn from(error: crate::ObjectError) -> Self {
        Self::Stack(error)
    }
}

impl std::fmt::Display for InstallationStackError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for InstallationStackError {}

fn derive_installed_stack_peak(
    machine: MachineId,
    functions: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
    active: &mut std::collections::BTreeSet<MachineId>,
    memoized: &mut std::collections::BTreeMap<MachineId, u64>,
    contributing_machines: &mut std::collections::BTreeSet<MachineId>,
    admitted_contribution_report_identities: &mut std::collections::BTreeSet<
        task_plans::AdmittedStackContributionReportId,
    >,
    admitted_contribution_commitments: &mut std::collections::BTreeSet<
        task_plans::SameStackContributionCommitment,
    >,
) -> Result<u64, crate::ObjectError> {
    if let Some(peak) = memoized.get(&machine) {
        contributing_machines.insert(machine);
        return Ok(*peak);
    }
    if !active.insert(machine) {
        return Err(crate::ObjectError::TerminalStackCycle(machine));
    }
    contributing_machines.insert(machine);
    let function =
        functions
            .get(&machine)
            .copied()
            .ok_or(crate::ObjectError::UnknownInternalCallTarget {
                caller: machine,
                target: machine,
            })?;
    let mut peak = match (function.unit_stack, function.scalar_stack) {
        (Some(_), Some(_)) => {
            return Err(crate::ObjectError::ConflictingTerminalStackEvidence(
                machine,
            ));
        }
        (Some(stack), None) => u64::from(stack.local_peak_bytes),
        (None, Some(stack)) => u64::from(stack.local_peak_bytes),
        (None, None) => {
            return Err(crate::ObjectError::UnaccountedTerminalStack(machine));
        }
    };
    for (owner, target, caller_live_bytes) in function
        .unit_call_stacks
        .iter()
        .map(|call| (call.owner, call.target, call.caller_live_bytes))
        .chain(
            function
                .scalar_call_stacks
                .iter()
                .map(|call| (call.owner, call.target, call.caller_live_bytes)),
        )
    {
        let callee_peak = derive_installed_stack_peak(
            target,
            functions,
            active,
            memoized,
            contributing_machines,
            admitted_contribution_report_identities,
            admitted_contribution_commitments,
        )?;
        let composed = u64::from(caller_live_bytes)
            .checked_add(callee_peak)
            .ok_or(crate::ObjectError::TerminalStackCompositionOverflow {
                caller: machine,
                owner,
            })?;
        peak = peak.max(composed);
    }
    for call in &function.foreign_call_stacks {
        let composed = u64::from(call.caller_live_bytes)
            .checked_add(call.contribution_bytes)
            .ok_or(crate::ObjectError::TerminalStackCompositionOverflow {
                caller: machine,
                owner: call.owner,
            })?;
        peak = peak.max(composed);
        admitted_contribution_report_identities.insert(call.contribution_report_identity);
        admitted_contribution_commitments.insert(call.contribution_commitment);
    }
    active.remove(&machine);
    memoized.insert(machine, peak);
    Ok(peak)
}

pub fn encode_installation_record(
    record: &InstallationRecord,
) -> Result<Vec<u8>, InstallationError> {
    validate_record_shape(record)?;
    let provider_count = u32::try_from(record.selected_provider_plans.len())
        .map_err(|_| InstallationError::TooManyProviderPlans)?;
    let settlement_count = u32::try_from(record.boundary_settlements.len())
        .map_err(|_| InstallationError::TooManyBoundarySettlements)?;
    let function_count = u32::try_from(record.functions.len())
        .map_err(|_| InstallationError::TooManyInstalledFunctions)?;
    let private_function_count = u32::try_from(record.private_functions.len())
        .map_err(|_| InstallationError::TooManyCompilerPrivateFunctions)?;
    let structural_return_count = u32::try_from(record.structural_returns.len())
        .map_err(|_| InstallationError::TooManyStructuralReturns)?;
    let internal_unit_call_count = u32::try_from(record.internal_unit_calls.len())
        .map_err(|_| InstallationError::TooManyInternalUnitCalls)?;
    let internal_unit_scalar_call_count = u32::try_from(record.internal_unit_scalar_calls.len())
        .map_err(|_| InstallationError::TooManyInternalUnitScalarCalls)?;
    let semantic_code_attribution_count = u32::try_from(record.semantic_code_attribution.len())
        .map_err(|_| InstallationError::TooManySemanticCodeAttributions)?;
    let port_effect_count = u32::try_from(record.port_effects.len())
        .map_err(|_| InstallationError::TooManyPortEffects)?;
    let text_relocation_count =
        u64::try_from(record.compiler_text_validation.text_relocation_count)
            .map_err(|_| InstallationError::CountNotRepresentable("text relocations"))?;
    let checked_instruction_validation_count = u64::try_from(
        record
            .compiler_text_validation
            .checked_instruction_validation_count,
    )
    .map_err(|_| InstallationError::CountNotRepresentable("checked instructions"))?;

    let mut bytes = Vec::with_capacity(294 + record.selected_provider_plans.len() * 8);
    encode_installation_header(
        &mut bytes,
        record,
        text_relocation_count,
        checked_instruction_validation_count,
    )?;
    encode_provider_plans(&mut bytes, provider_count, &record.selected_provider_plans);
    encode_functions(&mut bytes, function_count, &record.functions)?;
    encode_private_functions(
        &mut bytes,
        private_function_count,
        &record.private_functions,
    )?;
    encode_structural_returns(
        &mut bytes,
        structural_return_count,
        &record.structural_returns,
    )?;
    encode_internal_unit_calls(
        &mut bytes,
        internal_unit_call_count,
        &record.internal_unit_calls,
    )?;
    encode_internal_unit_scalar_calls(
        &mut bytes,
        internal_unit_scalar_call_count,
        &record.internal_unit_scalar_calls,
    )?;
    encode_dynamic_conformance_custody(
        &mut bytes,
        &record.dynamic_conformance_tables,
        &record.dynamic_calls,
        &record.stored_dynamic_calls,
        &record.forwarded_dynamic_descriptor_adapters,
        &record.forwarded_dynamic_descriptor_tables,
        &record.forwarded_dynamic_descriptor_calls,
        &record.dynamic_parameter_calls,
        &record.forwarded_dynamic_parameter_calls,
    )?;
    encode_semantic_code_attributions(
        &mut bytes,
        semantic_code_attribution_count,
        &record.semantic_code_attribution,
    )?;
    encode_port_effects(&mut bytes, port_effect_count, &record.port_effects)?;
    encode_boundary_settlements(&mut bytes, settlement_count, &record.boundary_settlements)?;
    Ok(bytes)
}

pub fn decode_installation_record(bytes: &[u8]) -> Result<InstallationRecord, InstallationError> {
    let mut reader = Reader::new(bytes);
    let DecodedInstallationHeader {
        psi,
        target,
        subsystem,
        profile_decision,
        component_progress,
        image,
        image_sections,
        compiler_text_validation,
    } = decode_installation_header(&mut reader)?;
    let selected_provider_plans = decode_provider_plans(&mut reader)?;
    let functions = decode_functions(&mut reader)?;
    let private_functions = decode_private_functions(&mut reader)?;
    let structural_returns = decode_structural_returns(&mut reader)?;
    let internal_unit_calls = decode_internal_unit_calls(&mut reader)?;
    let internal_unit_scalar_calls = decode_internal_unit_scalar_calls(&mut reader)?;
    let (
        dynamic_conformance_tables,
        dynamic_calls,
        stored_dynamic_calls,
        forwarded_dynamic_descriptor_adapters,
        forwarded_dynamic_descriptor_tables,
        forwarded_dynamic_descriptor_calls,
        dynamic_parameter_calls,
        forwarded_dynamic_parameter_calls,
    ) = decode_dynamic_conformance_custody(&mut reader)?;
    let semantic_code_attribution = decode_semantic_code_attributions(&mut reader)?;
    let port_effects = decode_port_effects(&mut reader)?;
    let boundary_settlements = decode_boundary_settlements(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(InstallationError::TrailingBytes(reader.remaining()));
    }

    let record = InstallationRecord {
        psi,
        target,
        subsystem,
        profile_decision,
        selected_provider_plans,
        component_progress,
        functions,
        private_functions,
        structural_returns,
        internal_unit_calls,
        internal_unit_scalar_calls,
        dynamic_conformance_tables,
        dynamic_calls,
        stored_dynamic_calls,
        forwarded_dynamic_descriptor_adapters,
        forwarded_dynamic_descriptor_tables,
        forwarded_dynamic_descriptor_calls,
        dynamic_parameter_calls,
        forwarded_dynamic_parameter_calls,
        semantic_code_attribution,
        port_effects,
        boundary_settlements,
        image,
        image_sections,
        compiler_text_validation,
    };
    validate_record_shape(&record)?;
    if encode_installation_record(&record)? != bytes {
        return Err(InstallationError::NonCanonicalEncoding);
    }
    Ok(record)
}

pub fn validate_installation_record(
    record: &InstallationRecord,
    image: &ExecutableImage,
) -> Result<(), InstallationError> {
    validate_record_shape(record)?;
    let expected_private_functions = image
        .private_functions()
        .iter()
        .map(installed_compiler_private_function)
        .collect::<Result<Vec<_>, _>>()?;
    if record.psi != image.psi()
        || record.target != image.target()
        || record.subsystem != image.subsystem()
        || record.image != fingerprint_image(&image.output().bytes)
        || record.image_sections != installed_image_sections(image)
        || Some(record.compiler_text_validation) != image.output().compiler_text_validation
        || record.dynamic_conformance_tables != installed_dynamic_conformance_tables(image)
        || record.dynamic_calls != installed_dynamic_calls(image)?
        || record.stored_dynamic_calls != installed_stored_dynamic_calls(image)?
        || record.forwarded_dynamic_descriptor_adapters
            != installed_forwarded_dynamic_descriptor_adapters(image)
        || record.forwarded_dynamic_descriptor_tables
            != installed_forwarded_dynamic_descriptor_tables(image)
        || record.forwarded_dynamic_descriptor_calls
            != installed_forwarded_dynamic_descriptor_calls(image)?
        || record.dynamic_parameter_calls != installed_dynamic_parameter_calls(image)?
        || record.forwarded_dynamic_parameter_calls
            != installed_forwarded_dynamic_parameter_calls(image)?
        || record.semantic_code_attribution != image.semantic_code_attribution()
        || record.port_effects != image.port_effects()
        || record.boundary_settlements != image.boundary_settlements()
        || record.private_functions != expected_private_functions
        || record.structural_returns
            != image
                .functions()
                .iter()
                .filter_map(|function| {
                    function
                        .structural_return
                        .clone()
                        .map(|returned| InstalledStructuralReturn {
                            machine: function.machine,
                            returned,
                        })
                })
                .collect::<Vec<_>>()
        || !internal_unit_calls_match_object(
            &record.internal_unit_calls,
            image.functions().iter().flat_map(|function| {
                // The record retains physical text order while the object's
                // roster follows the selected source block order.
                let mut calls: Vec<_> = function.internal_unit_calls.iter().collect();
                calls.sort_by_key(|custody| custody.code_offset);
                calls
                    .into_iter()
                    .map(move |custody| (function.machine, function.text_offset, custody))
            }),
        )
        || record.internal_unit_scalar_calls
            != image
                .functions()
                .iter()
                .flat_map(|function| {
                    function
                        .internal_unit_scalar_calls
                        .iter()
                        .cloned()
                        .map(|custody| InstalledInternalUnitScalarCall {
                            machine: function.machine,
                            text_offset: function.text_offset + custody.code_offset,
                            custody,
                        })
                })
                .collect::<Vec<_>>()
        || record.functions.len() != image.functions().len()
        || record
            .functions
            .iter()
            .zip(image.functions())
            .any(|(installed, emitted)| {
                installed.machine != emitted.machine
                    || installed.attachment != emitted.attachment
                    || installed.scalar_abi != emitted.scalar_abi
                    || installed.mixed_structural_scalar_abi != emitted.mixed_structural_scalar_abi
                    || installed.parameter_abi != emitted.parameter_abi
                    || installed.structural_call_scalar_return
                        != emitted.structural_call_scalar_return
                    || installed.text_offset != emitted.text_offset
                    || installed.byte_count != emitted.byte_count
                    || installed.unit_stack != emitted.unit_stack
                    || installed.scalar_stack != emitted.scalar_stack
                    || installed.unit_call_stacks != emitted.unit_call_stacks
                    || installed.scalar_call_stacks != emitted.scalar_call_stacks
                    || installed.foreign_call_stacks
                        != installed_foreign_call_stacks(image, emitted.machine)
                    || installed.unit_body != emitted.unit_affine_cleanup.is_some()
                    || installed.unit_parameters != emitted.unit_parameters
                    || installed.unit_parameter_homes != emitted.unit_parameter_homes
                    || installed.unit_scalar_homes != emitted.unit_scalar_homes
                    || installed.unit_integer_constants != emitted.unit_integer_constants
                    || installed.unit_affine_scalar_records != emitted.unit_affine_scalar_records
                    || installed.unit_structural_scalar_field_stores
                        != emitted.unit_structural_scalar_field_stores
                    || installed.unit_write_only_primitive_stores
                        != emitted.unit_write_only_primitive_stores
                    || installed.scalar_structural_scalar_field_stores
                        != emitted.scalar_structural_scalar_field_stores
                    || installed.unit_continuations != emitted.unit_continuations
                    || installed.unit_affine_cleanup != emitted.unit_affine_cleanup
                    || installed.scalar_affine_cleanup != emitted.scalar_affine_cleanup
                    || !installed_scalar_control_cleanups_match_object(
                        &installed.scalar_control_affine_cleanups,
                        &emitted.scalar_control_affine_cleanups,
                    )
                    || installed.scalar_structural_parameters
                        != emitted.scalar_structural_parameters
                    || installed.scalar_structural_parameter_homes
                        != emitted.scalar_structural_parameter_homes
            })
    {
        return Err(InstallationError::ImageBindingMismatch);
    }
    Ok(())
}

/// Joins every custody field, including owned copy spans and bytes, to replayed
/// object records. Candidate ABI geometry is not executable admission.
fn internal_unit_calls_match_object<'call>(
    installed: &[InstalledInternalUnitCall],
    emitted: impl IntoIterator<
        Item = (
            MachineId,
            usize,
            &'call machine_code::InternalUnitCallRecord,
        ),
    >,
) -> bool {
    let mut installed = installed.iter();
    for (machine, function_text_offset, custody) in emitted {
        let Some(actual) = installed.next() else {
            return false;
        };
        if actual.machine != machine
            || Some(actual.text_offset) != function_text_offset.checked_add(custody.code_offset)
            || actual.custody != *custody
        {
            return false;
        }
    }
    installed.next().is_none()
}

// Selected calls follow the source block roster while physical layout may
// reorder blocks; the published record retains physical call order.
fn installed_internal_unit_calls(image: &ExecutableImage) -> Vec<InstalledInternalUnitCall> {
    image
        .functions()
        .iter()
        .flat_map(|function| {
            let mut calls = function.internal_unit_calls.to_vec();
            calls.sort_by_key(|custody| custody.code_offset);
            calls.into_iter().map(|custody| InstalledInternalUnitCall {
                machine: function.machine,
                text_offset: function.text_offset + custody.code_offset,
                custody,
            })
        })
        .collect()
}

fn installed_image_sections(image: &ExecutableImage) -> InstalledImageSections {
    let text_byte_count = image
        .functions()
        .iter()
        .map(|function| {
            function
                .text_offset
                .checked_add(function.byte_count)
                .expect("validated compiler function text extent")
        })
        .chain(image.private_functions().iter().map(|private| {
            private
                .function
                .text_offset
                .checked_add(private.function.byte_count)
                .expect("validated compiler-private function text extent")
        }))
        .chain(
            image
                .forwarded_dynamic_descriptor_adapters()
                .iter()
                .map(|adapter| {
                    adapter
                        .text_offset
                        .checked_add(adapter.byte_count)
                        .expect("validated forwarded adapter text extent")
                }),
        )
        .max()
        .unwrap_or(0);
    let data_byte_count = image
        .dynamic_conformance_tables()
        .iter()
        .map(|table| {
            table
                .data_offset
                .checked_add(table.byte_count)
                .expect("validated dynamic-conformance table extent")
        })
        .chain(
            image
                .forwarded_dynamic_descriptor_tables()
                .iter()
                .map(|table| {
                    table
                        .data_offset
                        .checked_add(table.byte_count)
                        .expect("validated forwarded descriptor table extent")
                }),
        )
        .max()
        .unwrap_or(0);
    let final_compiler_data = image
        .output()
        .final_data_bytes
        .get(..data_byte_count)
        .expect("validated image retains its compiler-authored initialized-data prefix");
    InstalledImageSections {
        layout: image.output().final_image_layout,
        text_byte_count,
        data_byte_count,
        final_data_fingerprint: fingerprint_initialized_data(final_compiler_data),
    }
}

fn installed_dynamic_conformance_tables(
    image: &ExecutableImage,
) -> Vec<InstalledDynamicConformanceTable> {
    image
        .dynamic_conformance_tables()
        .iter()
        .map(|table| InstalledDynamicConformanceTable {
            application_commitment: table.application.commitment,
            application_report_fingerprint: table.application.report_fingerprint,
            data_offset: table.data_offset,
            byte_count: table.byte_count,
            slots: table
                .slots
                .iter()
                .map(|slot| InstalledDynamicConformanceSlot {
                    row_index: slot.row_index,
                    target: slot.target,
                    data_offset: slot.data_offset,
                })
                .collect(),
        })
        .collect()
}

fn installed_dynamic_calls(
    image: &ExecutableImage,
) -> Result<Vec<InstalledDynamicCall>, InstallationError> {
    image
        .functions()
        .iter()
        .flat_map(|function| {
            function.dynamic_calls.iter().map(|call| {
                call.code_offset
                    .checked_add(function.text_offset)
                    .map(|text_offset| InstalledDynamicCall {
                        machine: function.machine,
                        operation: call.psi_operation,
                        application_commitment: call.dynamic_dispatch.application.commitment,
                        initial_source: call.initial_instance.source.place,
                        rebound_source: call.rebound_instance.source.place,
                        selected_table_byte_offset: call.selected_table_byte_offset,
                        realization: call.dynamic_dispatch.dispatch.realization,
                        text_offset,
                        byte_count: call.byte_count,
                    })
                    .ok_or(InstallationError::FunctionOffsetNotRepresentable)
            })
        })
        .collect()
}

fn installed_stored_dynamic_calls(
    image: &ExecutableImage,
) -> Result<Vec<InstalledStoredDynamicCall>, InstallationError> {
    image
        .functions()
        .iter()
        .flat_map(|function| {
            function.stored_dynamic_calls.iter().map(|call| {
                let establishment = &call.establishment;
                Some(InstalledStoredDynamicCall {
                    machine: function.machine,
                    establishment_operation: establishment.psi_operation,
                    operation: call.psi_operation,
                    descriptor_ordinal: establishment.stored.descriptor.ordinal,
                    selection_ordinal: establishment.stored.selection.ordinal,
                    application_commitment: establishment.stored.application.commitment,
                    source: establishment.instance.source.place,
                    descriptor_home_byte_offset: establishment.descriptor_home_byte_offset,
                    selected_table_byte_offset: call.selected_table_byte_offset,
                    realization: call.dynamic_dispatch.dispatch.realization,
                    establishment_text_offset: establishment
                        .code_offset
                        .checked_add(function.text_offset)?,
                    establishment_byte_count: establishment.byte_count,
                    text_offset: call.code_offset.checked_add(function.text_offset)?,
                    byte_count: call.byte_count,
                })
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(InstallationError::FunctionOffsetNotRepresentable)
}

fn installed_forwarded_dynamic_descriptor_adapters(
    image: &ExecutableImage,
) -> Vec<InstalledForwardedDynamicDescriptorAdapter> {
    image
        .forwarded_dynamic_descriptor_adapters()
        .iter()
        .map(|adapter| InstalledForwardedDynamicDescriptorAdapter {
            application_commitment: adapter.record.identity.application,
            row_index: adapter.record.identity.row_index,
            realization: adapter.record.identity.realization,
            text_offset: adapter.text_offset,
            byte_count: adapter.byte_count,
        })
        .collect()
}

fn installed_forwarded_dynamic_descriptor_tables(
    image: &ExecutableImage,
) -> Vec<InstalledForwardedDynamicDescriptorTable> {
    image
        .forwarded_dynamic_descriptor_tables()
        .iter()
        .map(|table| InstalledForwardedDynamicDescriptorTable {
            application_commitment: table.application.commitment,
            application_report_fingerprint: table.application.report_fingerprint,
            data_offset: table.data_offset,
            byte_count: table.byte_count,
            slots: table
                .slots
                .iter()
                .map(|slot| {
                    let adapter = image
                        .forwarded_dynamic_descriptor_adapters()
                        .iter()
                        .find(|adapter| adapter.record.identity == slot.adapter)
                        .expect("validated forwarded descriptor slot has one adapter");
                    InstalledForwardedDynamicDescriptorSlot {
                        row_index: slot.row_index,
                        realization: slot.adapter.realization,
                        adapter_text_offset: adapter.text_offset,
                        data_offset: slot.data_offset,
                    }
                })
                .collect(),
        })
        .collect()
}

fn installed_forwarded_dynamic_descriptor_calls(
    image: &ExecutableImage,
) -> Result<Vec<InstalledForwardedDynamicDescriptorCall>, InstallationError> {
    image
        .functions()
        .iter()
        .flat_map(|function| {
            function
                .forwarded_dynamic_descriptor_calls
                .iter()
                .map(move |call| (function, call))
        })
        .map(|(function, call)| {
            let [argument] = call.dynamic_arguments.as_slice() else {
                return Err(InstallationError::InvalidForwardedDynamicDescriptorCall(
                    function.machine,
                ));
            };
            let (selection, application) = match &argument.custody.source {
                abstract_operations::AbstractDynamicDescriptorSource::Selection {
                    selection,
                    application,
                } => (selection, application),
                abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                    rebound,
                    application,
                    ..
                } => (rebound, application),
                abstract_operations::AbstractDynamicDescriptorSource::Parameter(_) => {
                    return Err(InstallationError::InvalidForwardedDynamicDescriptorCall(
                        function.machine,
                    ));
                }
            };
            Ok(InstalledForwardedDynamicDescriptorCall {
                machine: function.machine,
                operation: call.psi_operation,
                callee: call.callee,
                application_commitment: application.commitment,
                source: selection.source.clone(),
                semantic_result: call.semantic_result,
                result: call.result.clone(),
                text_offset: function
                    .text_offset
                    .checked_add(call.code_offset)
                    .ok_or(InstallationError::FunctionOffsetNotRepresentable)?,
                byte_count: call.byte_count,
            })
        })
        .collect()
}

fn installed_dynamic_parameter_calls(
    image: &ExecutableImage,
) -> Result<Vec<InstalledDynamicParameterCall>, InstallationError> {
    image
        .functions()
        .iter()
        .flat_map(|function| {
            function
                .dynamic_parameter_calls
                .iter()
                .map(move |call| (function, call))
        })
        .map(|(function, call)| {
            Ok(InstalledDynamicParameterCall {
                machine: function.machine,
                operation: call.psi_operation,
                source_value: call.source_value,
                requirement_slot: call.requirement.slot,
                text_offset: function
                    .text_offset
                    .checked_add(call.code_offset)
                    .ok_or(InstallationError::FunctionOffsetNotRepresentable)?,
                byte_count: call.byte_count,
            })
        })
        .collect()
}

fn installed_forwarded_dynamic_parameter_calls(
    image: &ExecutableImage,
) -> Result<Vec<InstalledForwardedDynamicParameterCall>, InstallationError> {
    image
        .functions()
        .iter()
        .flat_map(|function| {
            function
                .forwarded_dynamic_parameter_calls
                .iter()
                .map(move |call| (function, call))
        })
        .map(|(function, call)| {
            let abstract_operations::AbstractDynamicDescriptorSource::Parameter(source) =
                &call.argument.source
            else {
                return Err(InstallationError::InvalidForwardedDynamicParameterCall(
                    function.machine,
                ));
            };
            Ok(InstalledForwardedDynamicParameterCall {
                machine: function.machine,
                operation: call.psi_operation,
                callee: call.callee,
                source_value: call.source_value,
                scalar_type: call.scalar_type,
                source_parameter_ordinal: source.ordinal,
                target_parameter_ordinal: call.argument.target.ordinal,
                text_offset: function
                    .text_offset
                    .checked_add(call.code_offset)
                    .ok_or(InstallationError::FunctionOffsetNotRepresentable)?,
                byte_count: call.byte_count,
            })
        })
        .collect()
}

fn installed_compiler_private_function(
    emitted: &crate::ObjectCompilerPrivateFunction,
) -> Result<InstalledCompilerPrivateFunction, InstallationError> {
    Ok(InstalledCompilerPrivateFunction {
        identity: emitted.identity,
        source_psi: emitted.source_psi,
        machine: emitted.function.machine,
        scalar_abi: emitted
            .function
            .scalar_abi
            .clone()
            .ok_or(InstallationError::MissingCompilerPrivateFunctionAbi)?,
        text_offset: emitted.function.text_offset,
        byte_count: emitted.function.byte_count,
    })
}

fn installed_scalar_control_cleanups_match_object(
    installed: &[machine_code::UnitAffineCleanupRecord],
    emitted: &[machine_code::ScalarControlAffineCleanupRecord],
) -> bool {
    installed.len() == emitted.len()
        && installed
            .iter()
            .zip(emitted)
            .all(|(installed, emitted)| installed == &emitted.cleanup)
}

fn installed_foreign_call_stacks(
    image: &ExecutableImage,
    machine: MachineId,
) -> Vec<InstalledForeignCallStack> {
    image
        .foreign_calls()
        .iter()
        .filter(|call| call.machine == machine)
        .map(|call| InstalledForeignCallStack {
            owner: call.owner,
            text_offset: call.text_offset,
            caller_live_bytes: call.caller_live_bytes,
            provider_plan_report_identity: call
                .same_stack_contribution
                .provider_plan_report_identity(),
            contribution_report_identity: call.same_stack_contribution.report_identity(),
            contribution_commitment: call.same_stack_contribution.commitment(),
            contribution_bytes: call.same_stack_contribution.bytes(),
            contribution_alignment: call.same_stack_contribution.alignment(),
        })
        .collect()
}

pub fn installation_fingerprint(
    record: &InstallationRecord,
) -> Result<InstallationFingerprint, InstallationError> {
    let bytes = encode_installation_record(record)?;
    Ok(fingerprint_record(&bytes))
}

fn installed_scalar_source_is_exact(
    record: &InstallationRecord,
    function: &InstalledFunction,
    machine: MachineId,
    consumer: &machine_code::InternalUnitCallRecord,
    source: machine_code::InternalUnitScalarArgumentSourceRecord,
) -> bool {
    match source {
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary { .. }
        | machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. }
        | machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall { .. } => false,
        machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } => usize::try_from(parameter_index)
            .ok()
            .and_then(|index| {
                function
                    .parameter_abi
                    .as_ref()
                    .and_then(|abi| abi.parameters.get(index))
            })
            .is_some_and(|parameter| {
                let expected_location = function.parameter_abi.as_ref().and_then(|abi| {
                    crate::object_artifact::replay::unit::scalar_call_custody::entry_spills::parameter_location(
                        abi,
                        usize::try_from(parameter_index).ok()?,
                        source_value,
                        consumer.code_offset,
                    )
                });
                parameter.value == source_value
                    && parameter.scalar_type == scalar_type
                    && expected_location == Some(location)
            }),
        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            function
                .unit_integer_constants
                .iter()
                .filter(|constant| {
                    constant.defining_operation == defining_operation
                        && constant.source_value == source_value
                        && constant.scalar_type == scalar_type
                        && constant.value == value
                        && constant.operation_ordinal < consumer.operation_ordinal
                })
                .count()
                == 1
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            definition_ordinal < consumer.operation_ordinal
                && record
                    .semantic_code_attribution
                    .iter()
                    .filter(|attribution| {
                        attribution.machine == machine
                            && attribution.attribution.site
                                == machine_code::SemanticCodeSite::Operation(defining_operation)
                            && attribution.attribution.operation_ordinal == definition_ordinal
                            && attribution.attribution.code_offset <= consumer.code_offset
                            && attribution.attribution.byte_count == 0
                    })
                    .count()
                    == 1
                && function.unit_integer_constants.iter().all(|constant| {
                    constant.defining_operation != defining_operation
                        && constant.source_value != source_value
                })
                && function.unit_scalar_homes.iter().all(|home| {
                    home.defining_operation != defining_operation
                        && home.source_value != source_value
                })
                && installed_boolean_call_source_is_consistent(
                    record,
                    machine,
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                )
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
            function
                .unit_scalar_homes
                .iter()
                .filter(|candidate| **candidate == home)
                .count()
                == 1
                && record
                    .internal_unit_scalar_calls
                    .iter()
                    .filter(|producer| {
                        producer.machine == machine
                            && producer.custody.result.home == home
                            && producer.custody.operation_ordinal < consumer.operation_ordinal
                            && producer
                                .custody
                                .result
                                .code_offset
                                .checked_add(producer.custody.result.byte_count)
                                .is_some_and(|producer_end| producer_end <= consumer.code_offset)
                    })
                    .count()
                    == 1
        }
    }
}

fn installed_boolean_call_source_is_consistent(
    record: &InstallationRecord,
    machine: MachineId,
    defining_operation: semantic_vocabulary::OperationId,
    source_value: semantic_vocabulary::ValueId,
    value: bool,
    definition_ordinal: usize,
) -> bool {
    record
        .internal_unit_calls
        .iter()
        .filter(|call| call.machine == machine)
        .flat_map(|call| &call.custody.scalar_arguments)
        .all(|argument| match argument.source {
            machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                defining_operation: candidate_operation,
                source_value: candidate_value,
                value: candidate_literal,
                definition_ordinal: candidate_ordinal,
            } if candidate_operation == defining_operation || candidate_value == source_value => {
                candidate_operation == defining_operation
                    && candidate_value == source_value
                    && candidate_literal == value
                    && candidate_ordinal == definition_ordinal
            }
            _ => true,
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallationError {
    InvalidMagic,
    UnsupportedFormatMarker(u16),
    UnsupportedVocabularyMarker(u16),
    InvalidArchitectureTag(u8),
    InvalidObjectFormatTag(u8),
    InvalidBoolean(u8),
    InvalidPresenceFlag(u8),
    NonzeroReservedField,
    UnexpectedEnd,
    TrailingBytes(usize),
    NonCanonicalEncoding,
    NonCanonicalSubsystem,
    MissingCoffSubsystem,
    UnexpectedSubsystem,
    UnsupportedTarget(NativeTarget),
    TargetPointerFactNotRepresentable,
    ZeroProfileDecision,
    ZeroComponentProgressManifestIdentity,
    ZeroComponentProgressAcceptanceIdentity,
    ZeroProviderPlan,
    DuplicateProviderPlan,
    DuplicateProviderExecution,
    ProviderExecutionOutsideSelectedClosure,
    NonCanonicalProviderPlanOrder,
    TooManyProviderPlans,
    TooManyInstalledFunctions,
    TooManyCompilerPrivateFunctions,
    TooManyStackCallFacts,
    InvalidForeignStackContribution,
    TooManyStructuralReturns,
    TooManyInternalUnitCalls,
    TooManyInternalUnitScalarCalls,
    TooManyDynamicConformanceTables,
    TooManyDynamicConformanceSlots,
    TooManyDynamicScalarCalls,
    TooManyStoredDynamicCalls,
    TooManyForwardedDynamicDescriptorAdapters,
    TooManyForwardedDynamicDescriptorTables,
    TooManyForwardedDynamicDescriptorSlots,
    TooManyForwardedDynamicDescriptorCalls,
    TooManyDynamicParameterCalls,
    TooManyForwardedDynamicParameterCalls,
    TooManyInternalUnitCallArguments,
    TooManyInternalUnitScalarCallArguments,
    TooManyInternalUnitCallClaims,
    TooManyScalarCallPlanValues,
    TooManyScalarCallPlanRegisters,
    TooManyUnitScalarHomes,
    TooManyUnitIntegerConstants,
    TooManyUnitAffineScalarRecords,
    InvalidUnitAffineScalarRecord,
    TooManyUnitStructuralScalarFieldStores,
    TooManyUnitWriteOnlyPrimitiveStores,
    TooManyScalarStructuralScalarFieldStores,
    TooManyUnitStructuralScalarFieldStoreBytes,
    TooManyUnitWriteOnlyPrimitiveStoreBytes,
    TooManyStructuralReturnParameters,
    TooManyStructuralReturnClaims,
    TooManyStructuralReturnCleanups,
    TooManyStructuralTypes,
    TooManyStructuralFields,
    TooManyStructuralCases,
    TooManyStructuralQualifications,
    TooManySemanticCodeAttributions,
    TooManyPortEffects,
    TooManyBoundarySettlements,
    TooManySettlementScalarArguments,
    TooManySettlementArguments,
    TooManySettlementArgumentPathSegments,
    SettlementArgumentFieldTooLong,
    TooManyCompletionReceipts,
    TooManyCompletionProviderCustody,
    TooManyCompletionClaimSources,
    SettlementOffsetNotRepresentable,
    FunctionOffsetNotRepresentable,
    CompilerPrivateFunctionOffsetNotRepresentable,
    StructuralReturnOffsetNotRepresentable,
    InternalUnitCallOffsetNotRepresentable,
    InstalledScalarOffsetNotRepresentable,
    SemanticCodeAttributionOffsetNotRepresentable,
    PortEffectOffsetNotRepresentable,
    ZeroFunctionIdentity,
    InvalidCompilerPrivateFunctionIdentity,
    InvalidCompilerPrivateFunctionRoleTag(u8),
    MissingCompilerPrivateFunctionAbi,
    ZeroStructuralReturnIdentity(&'static str),
    ZeroInternalUnitCallIdentity,
    ZeroStructuralCallScalarReturnIdentity(&'static str),
    ZeroInstalledScalarIdentity,
    InvalidStructuralMultiplicity(u8),
    InvalidStructuralAccess(u8),
    UnsupportedStructuralReturnShape,
    UnsupportedStructuralReturnPlacement,
    UnsupportedInternalUnitCallPlacement,
    UnsupportedStructuralReturnRegister(MachineRegister),
    InvalidStructuralReturnRegister(u8),
    InvalidStructuralReturnLocal,
    StructuralTypeIdentityTooLong,
    InvalidStructuralTypeIdentity,
    InvalidStructuralTypeShape,
    InvalidStructuralTypeShapeTag(u8),
    InvalidStructuralFieldTypeTag(u8),
    ZeroSemanticCodeAttributionIdentity(&'static str),
    InvalidSemanticCodeSiteTag(u8),
    InvalidCallSiteOwnerTag(u8),
    InvalidStructuralSourceLocationTag(u8),
    InvalidInternalUnitCallSourceTag(u8),
    InvalidInternalUnitStructuralSourceTag(u8),
    InvalidProviderCandidateRecord(terminal_codec::CodecError),
    InvalidScalarCallingPolicyTag(u8),
    InvalidScalarEntryControlTag(u8),
    InvalidScalarCallPlanRegister {
        class: u8,
        index: u8,
    },
    UnsupportedScalarCallPlan,
    UnsupportedInstalledFixedIntegerType,
    InvalidInstalledIntegerSignTag(u8),
    InvalidInstalledIntegerValueTag(u8),
    InvalidInstalledScalarSourceTag(u8),
    InvalidIeeeFloatFormatTag(u8),
    UnsupportedInstalledScalarSource,
    InvalidBoundaryRealizationTag,
    InvalidBoundaryExecutionTag,
    InvalidCleanupActionTag(u8),
    ZeroPortEffectIdentity(&'static str),
    ZeroSettlementIdentity(&'static str),
    InvalidSettlementArgumentPathTag(u8),
    InvalidSettlementArgumentField,
    ZeroProviderExecutionEvidence,
    NoInstalledFunctions,
    NonCanonicalInstalledFunctions,
    InvalidCompilerPrivateFunction,
    StructuralReturnMachineMissing(MachineId),
    InvalidStructuralReturn(MachineId),
    InvalidInternalUnitCall(MachineId),
    InvalidInternalUnitScalarCall(MachineId),
    InvalidImageSectionLayout,
    InvalidDynamicConformanceTable,
    InvalidDynamicCall(MachineId),
    InvalidStoredDynamicCall(MachineId),
    InvalidForwardedDynamicDescriptorAdapter,
    InvalidForwardedDynamicDescriptorTable,
    InvalidForwardedDynamicDescriptorCall(MachineId),
    InvalidUnitDynamicDescriptorJoin(MachineId),
    InvalidDynamicParameterCall(MachineId),
    InvalidForwardedDynamicParameterCall(MachineId),
    InvalidUnitWriteOnlyPrimitiveStore(MachineId),
    InvalidUnitStructuralScalarFieldStore(MachineId),
    InvalidUnitAffineCleanup(MachineId),
    InvalidScalarControlAffineCleanupCount(usize),
    SemanticCodeAttributionMachineMissing(MachineId),
    NonCanonicalSemanticCodeAttributionOrder,
    DuplicateSemanticCodeAttributionSite {
        machine: MachineId,
        site: SemanticCodeSite,
    },
    InvalidSemanticCodeAttribution {
        machine: MachineId,
        site: SemanticCodeSite,
    },
    SemanticCodeAttributionClosureMismatch,
    EffectMachineMissing(MachineId),
    NonCanonicalPortEffectOrder,
    DuplicatePortEffectOperation {
        machine: MachineId,
        operation: OperationId,
    },
    InvalidPortEffectOffset {
        machine: MachineId,
        operation: OperationId,
    },
    ProviderSettlementClosureMismatch,
    ProviderExecutionClosureMismatch,
    NonCanonicalBoundarySettlementOrder,
    DuplicateBoundarySettlementOperation {
        machine: MachineId,
        operation: OperationId,
    },
    InvalidBoundarySettlementOffset {
        machine: MachineId,
        operation: OperationId,
    },
    InvalidCompletionReceiptArgumentIndex {
        machine: MachineId,
        operation: OperationId,
    },
    InvalidCompletionReceiptCustody {
        machine: MachineId,
        operation: OperationId,
    },
    InvalidCompletionProviderCustody {
        machine: MachineId,
        operation: OperationId,
    },
    InvalidCompletionClaimSource,
    InvalidBoundaryResult,
    InvalidBoundaryScalarArgument,
    BoundaryRealizationMismatch {
        machine: MachineId,
        operation: OperationId,
    },
    CountNotRepresentable(&'static str),
    MissingCompilerTextValidation,
    InvalidCompilerTextDerivationDigest,
    ImageBindingMismatch,
}

impl std::fmt::Display for InstallationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for InstallationError {}
