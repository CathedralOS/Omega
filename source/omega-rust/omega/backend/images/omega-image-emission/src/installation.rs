use std::num::NonZeroU64;

use crate::{
    ExecutableImage, ObjectBoundarySettlement, ObjectCodeAttribution, ObjectPortEffect,
    boundary_results::boundary_result_is_exact,
    byte_sequence_custody::linux_write_line_custody_is_exact,
    can_emit_executable_image,
    completion_receipts::{CompletionCustodyError, validate_completion_custody},
};
use omega_calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValueClass, ValueLocation, ValuePlacement,
    ValueShape, evaluate_call_plan,
};
use omega_image::{CompilerTextValidationEvidence, FinalImageLayout};
use omega_machine_code::{SemanticCodeSite, StructuralReturnRecord};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::{BoundaryRealization, CallSiteOwner};
use psi_core::{MachineId, OperationId, PlaceId, ProfileDecisionId, StructuralTypeId};
use psi_terminal::{
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape, TerminalPsiIdentity,
};

mod boundary_result_scalar_codec;
mod boundary_settlement_codec;
mod call_site_owner_codec;
mod completion_custody_codec;
mod dynamic_conformance_codec;
mod fingerprint_codec;
mod fixed_integer_scalar_abi_codec;
mod function_affine_cleanup_codec;
mod function_codec;
mod function_parameter_codec;
mod function_stack_codec;
mod installation_header_codec;
mod installed_unit_scalar_transport;
mod internal_unit_call_codec;
mod internal_unit_scalar_call_codec;
mod port_effect_codec;
mod private_function_codec;
mod provider_execution_codec;
mod provider_plan_codec;
mod scalar_call_plan_codec;
mod semantic_code_attribution_codec;
mod structural_argument_codec;
mod structural_case_codec;
mod structural_field_codec;
mod structural_record_codec;
mod structural_return_codec;
mod structural_scalar_codec;
mod structural_signature_codec;
mod trivial_affine_local_codec;
mod unit_scalar_codec;
mod unit_structural_scalar_field_store_codec;
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
    installed_function_scalar_transport_is_canonical, validate_installed_unit_scalar_calls,
    validate_installed_unit_structural_scalar_field_stores,
};
use internal_unit_call_codec::{decode_internal_unit_calls, encode_internal_unit_calls};
use internal_unit_scalar_call_codec::{
    decode_internal_unit_scalar_calls, encode_internal_unit_scalar_calls,
};
use port_effect_codec::{decode_port_effects, encode_port_effects};
use private_function_codec::{decode_private_functions, encode_private_functions};
use provider_plan_codec::{decode_provider_plans, encode_provider_plans};
use semantic_code_attribution_codec::{
    decode_semantic_code_attributions, encode_semantic_code_attributions,
};
use structural_case_codec::{decode_structural_cases, encode_structural_cases};
use structural_record_codec::{decode_structural_fields, encode_structural_fields};
use structural_return_codec::{decode_structural_returns, encode_structural_returns};
use structural_scalar_codec::{
    decode_identity, decode_multiplicity, encode_identity, multiplicity_tag,
};
use wire_codec::{Reader, decode_boolean, push_u16, push_u32, push_u64, push_u128};

pub const INSTALLATION_FORMAT_MARKER: u16 = 52;

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
/// `omega-executable-installation`. It is the typed payload hashed under the
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
    dynamic_scalar_calls: Vec<InstalledDynamicScalarCall>,
    semantic_code_attribution: Vec<ObjectCodeAttribution>,
    port_effects: Vec<ObjectPortEffect>,
    boundary_settlements: Vec<ObjectBoundarySettlement>,
    image: ImageFingerprint,
    image_sections: InstalledImageSections,
    compiler_text_validation: CompilerTextValidationEvidence,
}

impl InstallationRecord {
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

    pub fn dynamic_scalar_calls(&self) -> &[InstalledDynamicScalarCall] {
        &self.dynamic_scalar_calls
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
    pub application_commitment: psi_terminal::ClosedConformanceApplicationCommitment,
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
pub struct InstalledDynamicScalarCall {
    pub machine: MachineId,
    pub operation: OperationId,
    pub application_commitment: psi_terminal::ClosedConformanceApplicationCommitment,
    pub initial_source: PlaceId,
    pub rebound_source: PlaceId,
    pub selected_table_byte_offset: u32,
    pub realization: MachineId,
    pub text_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub fixed_integer_scalar_abi: Option<omega_target_operations::FixedIntegerScalarFunctionAbi>,
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
    /// The exact function remains governed by the independently replayed
    /// ranked-`u32` object/image carrier. This closed body tag prevents
    /// canonical installation encoding from shedding that disjoint custody.
    pub ranked_u32_countdown: bool,
    pub unit_parameters: Vec<omega_machine_code::UnitParameterRecord>,
    pub unit_parameter_homes: Vec<omega_machine_code::UnitParameterHomeRecord>,
    pub unit_scalar_homes: Vec<omega_machine_code::UnitScalarHomeRecord>,
    pub unit_integer_constants: Vec<omega_machine_code::UnitIntegerConstantRecord>,
    pub unit_structural_scalar_field_stores:
        Vec<omega_machine_code::UnitStructuralScalarFieldStoreRecord>,
    pub unit_affine_cleanup: Option<omega_machine_code::UnitAffineCleanupRecord>,
    pub scalar_affine_cleanup: Option<omega_machine_code::UnitAffineCleanupRecord>,
    /// Canonical true-before-false DFS cleanup leaves for the exact bounded
    /// two-decision/three-return scalar-control carrier. This remains distinct
    /// from the branch-free scalar cleanup above: each physical suffix owns its
    /// terminal-Psi return edge and exact byte interval.
    pub scalar_control_affine_cleanups: Vec<omega_machine_code::UnitAffineCleanupRecord>,
    pub scalar_structural_parameters: Vec<omega_machine_code::UnitParameterRecord>,
    pub scalar_structural_parameter_homes: Vec<omega_machine_code::UnitParameterHomeRecord>,
}

/// Canonical installation custody for one compiler-private callback thunk.
///
/// The executable image remains authoritative for native bytes and symbol
/// replay. This row prevents installation serialization from shedding or
/// substituting the exact private identity, source program, ABI, or final text
/// interval already validated by object and image construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCompilerPrivateFunction {
    pub identity: omega_function_identity::MachineFunctionIdentity,
    pub source_psi: TerminalPsiIdentity,
    pub machine: MachineId,
    pub fixed_integer_scalar_abi: omega_target_operations::FixedIntegerScalarFunctionAbi,
    pub text_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledForeignCallStack {
    pub owner: CallSiteOwner,
    pub text_offset: usize,
    pub caller_live_bytes: u32,
    pub provider_plan_report_identity: u64,
    pub contribution_report_identity: omega_task_plans::AdmittedStackContributionReportId,
    pub contribution_commitment: omega_task_plans::SameStackContributionCommitment,
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
    pub custody: omega_machine_code::InternalUnitCallRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledInternalUnitScalarCall {
    pub machine: MachineId,
    pub text_offset: usize,
    pub custody: omega_machine_code::InternalUnitScalarCallRecord,
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
        std::iter::empty::<&dyn omega_installation_evidence::ProviderExecutionEvidence>(),
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
    Execution: omega_installation_evidence::ProviderExecutionEvidence + ?Sized + 'execution,
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
    component_progress: Option<
        &dyn omega_installation_evidence::ComponentProgressAcceptanceEvidence,
    >,
) -> Result<InstallationRecord, InstallationError>
where
    Execution: omega_installation_evidence::ProviderExecutionEvidence + ?Sized + 'execution,
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
    component_progress: Option<
        &dyn omega_installation_evidence::ComponentProgressAcceptanceEvidence,
    >,
) -> Result<InstallationRecord, InstallationError>
where
    Execution: omega_installation_evidence::ProviderExecutionEvidence + ?Sized + 'execution,
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
            let omega_machine_code::BoundaryExecutionRecord::AdmittedProvider(execution) =
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
                fixed_integer_scalar_abi: function.fixed_integer_scalar_abi.clone(),
                text_offset: function.text_offset,
                byte_count: function.byte_count,
                unit_stack: function.unit_stack,
                scalar_stack: function.scalar_stack,
                unit_call_stacks: function.unit_call_stacks.clone(),
                scalar_call_stacks: function.scalar_call_stacks.clone(),
                foreign_call_stacks: installed_foreign_call_stacks(image, function.machine),
                unit_body: function.unit_affine_cleanup.is_some(),
                ranked_u32_countdown: function.ranked_u32_countdown.is_some(),
                unit_parameters: function.unit_parameters.clone(),
                unit_parameter_homes: function.unit_parameter_homes.clone(),
                unit_scalar_homes: function.unit_scalar_homes.clone(),
                unit_integer_constants: function.unit_integer_constants.clone(),
                unit_structural_scalar_field_stores: function
                    .unit_structural_scalar_field_stores
                    .clone(),
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
        internal_unit_calls: image
            .functions()
            .iter()
            .flat_map(|function| {
                function.internal_unit_calls.iter().cloned().map(|custody| {
                    InstalledInternalUnitCall {
                        machine: function.machine,
                        text_offset: function.text_offset + custody.code_offset,
                        custody,
                    }
                })
            })
            .collect(),
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
        dynamic_scalar_calls: installed_dynamic_scalar_calls(image)?,
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
        omega_task_plans::AdmittedStackContributionReportId,
    >,
    admitted_contribution_commitments: &mut std::collections::BTreeSet<
        omega_task_plans::SameStackContributionCommitment,
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
        &record.dynamic_scalar_calls,
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
    let (dynamic_conformance_tables, dynamic_scalar_calls) =
        decode_dynamic_conformance_custody(&mut reader)?;
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
        dynamic_scalar_calls,
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
        || record.dynamic_scalar_calls != installed_dynamic_scalar_calls(image)?
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
        || record.internal_unit_calls
            != image
                .functions()
                .iter()
                .flat_map(|function| {
                    function.internal_unit_calls.iter().cloned().map(|custody| {
                        InstalledInternalUnitCall {
                            machine: function.machine,
                            text_offset: function.text_offset + custody.code_offset,
                            custody,
                        }
                    })
                })
                .collect::<Vec<_>>()
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
                    || installed.fixed_integer_scalar_abi != emitted.fixed_integer_scalar_abi
                    || installed.text_offset != emitted.text_offset
                    || installed.byte_count != emitted.byte_count
                    || installed.unit_stack != emitted.unit_stack
                    || installed.scalar_stack != emitted.scalar_stack
                    || installed.unit_call_stacks != emitted.unit_call_stacks
                    || installed.scalar_call_stacks != emitted.scalar_call_stacks
                    || installed.foreign_call_stacks
                        != installed_foreign_call_stacks(image, emitted.machine)
                    || installed.unit_body != emitted.unit_affine_cleanup.is_some()
                    || installed.ranked_u32_countdown != emitted.ranked_u32_countdown.is_some()
                    || installed.unit_parameters != emitted.unit_parameters
                    || installed.unit_parameter_homes != emitted.unit_parameter_homes
                    || installed.unit_scalar_homes != emitted.unit_scalar_homes
                    || installed.unit_integer_constants != emitted.unit_integer_constants
                    || installed.unit_structural_scalar_field_stores
                        != emitted.unit_structural_scalar_field_stores
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

fn installed_dynamic_scalar_calls(
    image: &ExecutableImage,
) -> Result<Vec<InstalledDynamicScalarCall>, InstallationError> {
    image
        .functions()
        .iter()
        .flat_map(|function| {
            function.dynamic_scalar_calls.iter().map(|call| {
                call.code_offset
                    .checked_add(function.text_offset)
                    .map(|text_offset| InstalledDynamicScalarCall {
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

fn installed_compiler_private_function(
    emitted: &crate::ObjectCompilerPrivateFunction,
) -> Result<InstalledCompilerPrivateFunction, InstallationError> {
    Ok(InstalledCompilerPrivateFunction {
        identity: emitted.identity,
        source_psi: emitted.source_psi,
        machine: emitted.function.machine,
        fixed_integer_scalar_abi: emitted
            .function
            .fixed_integer_scalar_abi
            .clone()
            .ok_or(InstallationError::MissingCompilerPrivateFunctionAbi)?,
        text_offset: emitted.function.text_offset,
        byte_count: emitted.function.byte_count,
    })
}

fn installed_scalar_control_cleanups_match_object(
    installed: &[omega_machine_code::UnitAffineCleanupRecord],
    emitted: &[omega_machine_code::ScalarControlAffineCleanupRecord],
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

fn validate_record_shape(record: &InstallationRecord) -> Result<(), InstallationError> {
    if !record
        .compiler_text_validation
        .has_valid_derivation_digest()
    {
        return Err(InstallationError::InvalidCompilerTextDerivationDigest);
    }
    if !can_emit_executable_image(record.target) {
        return Err(InstallationError::UnsupportedTarget(record.target));
    }
    validate_installed_dynamic_conformance(record)?;
    match record.target.object_format {
        ObjectFormat::Coff if record.subsystem.is_none() => {
            return Err(InstallationError::MissingCoffSubsystem);
        }
        ObjectFormat::Elf | ObjectFormat::MachO if record.subsystem.is_some() => {
            return Err(InstallationError::UnexpectedSubsystem);
        }
        ObjectFormat::Coff | ObjectFormat::Elf | ObjectFormat::MachO => {}
    }
    if record
        .selected_provider_plans
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(InstallationError::NonCanonicalProviderPlanOrder);
    }
    let selected = record
        .selected_provider_plans
        .iter()
        .map(|provider| provider.get())
        .collect::<std::collections::BTreeSet<_>>();
    let required = record
        .boundary_settlements
        .iter()
        .filter_map(|settlement| {
            let omega_machine_code::BoundaryExecutionRecord::AdmittedProvider(execution) =
                settlement.settlement.execution
            else {
                return None;
            };
            Some(execution.provider_plan_report_identity)
        })
        .chain(
            record
                .functions
                .iter()
                .flat_map(|function| &function.foreign_call_stacks)
                .map(|call| call.provider_plan_report_identity),
        )
        .collect::<std::collections::BTreeSet<_>>();
    if !required.is_subset(&selected) {
        return Err(InstallationError::ProviderSettlementClosureMismatch);
    }
    if record.functions.is_empty() {
        return Err(InstallationError::NoInstalledFunctions);
    }
    let mut expected_text_offset = 0_usize;
    let mut previous_function = None;
    let attachments = record
        .functions
        .iter()
        .map(|function| (function.machine, function.attachment))
        .collect::<std::collections::BTreeMap<_, _>>();
    for function in &record.functions {
        let function_unit_calls = record
            .internal_unit_calls
            .iter()
            .filter(|call| call.machine == function.machine)
            .map(|call| call.custody.clone())
            .collect::<Vec<_>>();
        let fully_consumed_affine_pair =
            crate::fully_consumed_affine_pair::exact_fully_consumed_affine_pair(
                &function.unit_parameter_homes,
                &function_unit_calls,
                function.unit_affine_cleanup.as_ref(),
            );
        let partially_consumed_affine_array =
            crate::fully_consumed_affine_pair::exact_partially_consumed_affine_array(
                &function.unit_parameter_homes,
                &function_unit_calls,
                function.unit_affine_cleanup.as_ref(),
            );
        if function.byte_count == 0
            || function.text_offset != expected_text_offset
            || previous_function.is_some_and(|previous| previous >= function.machine)
        {
            return Err(InstallationError::NonCanonicalInstalledFunctions);
        }
        let has_scalar_control_cleanup = !function.scalar_control_affine_cleanups.is_empty();
        let has_scalar_cleanup =
            function.scalar_affine_cleanup.is_some() || has_scalar_control_cleanup;
        let has_scalar_boundary_custody = record.boundary_settlements.iter().any(|settlement| {
            settlement.machine == function.machine
                && matches!(
                    settlement.settlement.realization,
                    BoundaryRealization::DirectPortReadU8(_)
                        | BoundaryRealization::LinuxExitGroupI32(_)
                )
        });
        let has_scalar_custody = has_scalar_cleanup || has_scalar_boundary_custody;
        let ranked_body_is_exclusive = !function.ranked_u32_countdown
            || (record.functions.len() == 1
                && function.attachment.is_some()
                && function.unit_stack.is_none()
                && function.scalar_stack.is_none()
                && function.unit_call_stacks.is_empty()
                && function.scalar_call_stacks.is_empty()
                && function.foreign_call_stacks.is_empty()
                && !function.unit_body
                && function.unit_parameters.is_empty()
                && function.unit_parameter_homes.is_empty()
                && function.unit_affine_cleanup.is_none()
                && function.scalar_affine_cleanup.is_none()
                && function.scalar_control_affine_cleanups.is_empty()
                && function.scalar_structural_parameters.is_empty()
                && function.scalar_structural_parameter_homes.is_empty()
                && function.fixed_integer_scalar_abi.is_none()
                && function.unit_scalar_homes.is_empty()
                && function.unit_integer_constants.is_empty()
                && function.unit_structural_scalar_field_stores.is_empty()
                && record.structural_returns.is_empty()
                && record.internal_unit_calls.is_empty()
                && record.internal_unit_scalar_calls.is_empty()
                && record.dynamic_conformance_tables.is_empty()
                && record.dynamic_scalar_calls.is_empty()
                && record.port_effects.is_empty()
                && record.boundary_settlements.is_empty()
                && record.semantic_code_attribution.len() == 9
                && record
                    .semantic_code_attribution
                    .iter()
                    .all(|row| row.machine == function.machine));
        if !installed_stack_facts_are_canonical(function, &attachments)
            || !installed_function_scalar_transport_is_canonical(function, record.target)
            || !ranked_body_is_exclusive
            || function.unit_parameters.len() != function.unit_parameter_homes.len()
            || function.unit_body != function.unit_affine_cleanup.is_some()
            || (!function.unit_body
                && !has_scalar_cleanup
                && (!function.unit_parameters.is_empty()
                    || !function.unit_parameter_homes.is_empty()))
            || function.scalar_structural_parameters.len()
                != function.scalar_structural_parameter_homes.len()
            || (!function.scalar_control_affine_cleanups.is_empty()
                && function.scalar_control_affine_cleanups.len() < 2)
            || (function.scalar_affine_cleanup.is_some() && has_scalar_control_cleanup)
            || (has_scalar_cleanup && function.unit_body)
            || function
                .scalar_structural_parameters
                .iter()
                .zip(&function.scalar_structural_parameter_homes)
                .any(|(parameter, home)| {
                    parameter.place != home.place
                        || parameter.structural_type != home.structural_type
                        || parameter.multiplicity != home.multiplicity
                        || parameter.shape != home.shape
                })
            || (!has_scalar_custody
                && (!function.scalar_structural_parameters.is_empty()
                    || !function.scalar_structural_parameter_homes.is_empty()))
            || function
                .unit_parameters
                .iter()
                .zip(&function.unit_parameter_homes)
                .any(|(parameter, home)| {
                    parameter.place != home.place
                        || parameter.structural_type != home.structural_type
                        || parameter.multiplicity != home.multiplicity
                        || parameter.shape != home.shape
                })
        {
            return Err(InstallationError::InvalidUnitAffineCleanup(
                function.machine,
            ));
        }
        if let Some(cleanup) = &function.unit_affine_cleanup {
            if !super::unit_affine_cleanup::exact_construction_prefix(cleanup) {
                return Err(InstallationError::InvalidUnitAffineCleanup(
                    function.machine,
                ));
            }
            let end = cleanup
                .code_offset
                .checked_add(cleanup.byte_count)
                .ok_or(InstallationError::FunctionOffsetNotRepresentable)?;
            let expected_local_prefix = cleanup
                .locals
                .iter()
                .rev()
                .map(|(_, place, _)| place.id)
                .collect::<Vec<_>>();
            let transferred_roots = record
                .internal_unit_calls
                .iter()
                .filter(|call| call.machine == function.machine)
                .flat_map(|call| &call.custody.arguments)
                .filter(|argument| argument.path.is_empty())
                .map(|argument| argument.place)
                .collect::<std::collections::BTreeSet<_>>();
            let dynamic_borrowed_roots = record
                .dynamic_scalar_calls
                .iter()
                .filter(|call| call.machine == function.machine)
                .flat_map(|call| [call.initial_source, call.rebound_source])
                .collect::<std::collections::BTreeSet<_>>();
            let expected_parameter_discards = function
                .unit_parameter_homes
                .iter()
                .rev()
                .filter(|home| {
                    home.multiplicity == StructuralMultiplicity::Affine
                        && !transferred_roots.contains(&home.place)
                        && !dynamic_borrowed_roots.contains(&home.place)
                        && !fully_consumed_affine_pair
                })
                .map(|home| home.place)
                .collect::<Vec<_>>();
            let discards = cleanup
                .actions
                .iter()
                .filter_map(|action| match action {
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let residual_discards = cleanup
                .actions
                .iter()
                .filter_map(|action| match action {
                    psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        Some(discard)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let nominal_cleanups = cleanup
                .actions
                .iter()
                .filter_map(|action| match action {
                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(nominal) => {
                        Some(nominal)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let exact_nominal_body = |nominal: &psi_terminal::NominalAffineCleanup| {
                if nominal.cleanup_receiver.is_some() || !nominal.requirement_obligations.is_empty()
                {
                    return None;
                }
                let target = record
                    .functions
                    .iter()
                    .find(|candidate| candidate.machine == nominal.cleanup_machine)?;
                let calls = record
                    .internal_unit_calls
                    .iter()
                    .filter(|call| call.machine == nominal.cleanup_machine)
                    .collect::<Vec<_>>();
                let owners = calls
                    .iter()
                    .map(|call| call.custody.owner)
                    .collect::<std::collections::BTreeSet<_>>();
                let targets = calls
                    .iter()
                    .map(|call| call.custody.target)
                    .collect::<std::collections::BTreeSet<_>>();
                (target.attachment == Some(nominal.structural_type)
                    && target.unit_body
                    && target.unit_parameters.is_empty()
                    && target.unit_parameter_homes.is_empty()
                    && target
                        .unit_affine_cleanup
                        .as_ref()
                        .is_some_and(|return_cleanup| {
                            return_cleanup.locals.is_empty() && return_cleanup.actions.is_empty()
                        })
                    && owners.len() == calls.len()
                    && targets.len() == calls.len()
                    && calls.iter().enumerate().all(|(ordinal, call)| {
                        matches!(call.custody.owner, CallSiteOwner::Operation(_))
                            && call.custody.operation_ordinal == ordinal
                            && call.custody.result.is_none()
                            && call.custody.structural_result.is_none()
                            && call.custody.arguments.is_empty()
                            && call.custody.claim_transfers.is_empty()
                            && record.functions.iter().any(|helper| {
                                helper.machine == call.custody.target
                                    && helper.attachment.is_some()
                                    && helper.unit_body
                                    && helper.unit_parameters.is_empty()
                                    && helper.unit_parameter_homes.is_empty()
                                    && helper.unit_affine_cleanup.as_ref().is_some_and(
                                        |return_cleanup| {
                                            return_cleanup.locals.is_empty()
                                                && return_cleanup.actions.is_empty()
                                        },
                                    )
                                    && !record
                                        .internal_unit_calls
                                        .iter()
                                        .any(|helper_call| helper_call.machine == helper.machine)
                            })
                    })
                    && calls.windows(2).all(|pair| {
                        pair[0]
                            .custody
                            .code_offset
                            .checked_add(pair[0].custody.byte_count)
                            .is_some_and(|end| end <= pair[1].custody.code_offset)
                    }))
                .then_some(!calls.is_empty())
            };
            let Some(parameter_discards) = discards.get(expected_local_prefix.len()..) else {
                return Err(InstallationError::InvalidUnitAffineCleanup(
                    function.machine,
                ));
            };
            let local_operations = cleanup
                .locals
                .iter()
                .map(|(operation, _, _)| *operation)
                .collect::<std::collections::BTreeSet<_>>();
            if cleanup.byte_count == 0
                || end != function.byte_count
                || local_operations.len() != cleanup.locals.len()
                || cleanup.locals.iter().enumerate().any(
                    |(ordinal, (_, place, structural_type))| {
                        !matches!(
                            place.kind,
                            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                                declaration_ordinal,
                                structural_type: local_type,
                                ..
                            } if usize::try_from(declaration_ordinal) == Ok(ordinal)
                                && local_type == structural_type.id
                        ) || !matches!(
                            structural_type.shape,
                            StructuralTypeShape::Record { ref fields } if fields.is_empty()
                        )
                    },
                )
                || discards.get(..expected_local_prefix.len())
                    != Some(expected_local_prefix.as_slice())
                || discards
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != discards.len()
                || discards.len() + residual_discards.len() + nominal_cleanups.len()
                    != cleanup.actions.len()
                || match (nominal_cleanups.as_slice(), residual_discards.as_slice()) {
                    ([nominal], []) => {
                        let cleanup_is_executable = exact_nominal_body(nominal);
                        let matching_cleanup_calls = record
                            .internal_unit_calls
                            .iter()
                            .filter(|call| {
                                call.machine == function.machine
                                    && call.custody.owner
                                        == CallSiteOwner::CleanupAction {
                                            edge: cleanup.psi_edge,
                                            action_ordinal: 0,
                                        }
                                    && call.custody.target == nominal.cleanup_machine
                                    && call.custody.arguments.is_empty()
                                    && call.custody.claim_transfers.is_empty()
                                    && call.custody.code_offset == cleanup.code_offset
                            })
                            .count();
                        !cleanup.locals.is_empty()
                            || !discards.is_empty()
                            || function.unit_parameter_homes.len() != 1
                            || function.unit_parameter_homes[0].place != nominal.place
                            || function.unit_parameter_homes[0].structural_type
                                != nominal.structural_type
                            || function.unit_parameter_homes[0].multiplicity
                                != StructuralMultiplicity::Affine
                            || !bounded_nominal_receiver_shape(
                                function.unit_parameter_homes[0].shape,
                            )
                            || (function.unit_parameter_homes[0].shape.byte_size == 0
                                && !function.unit_parameter_homes[0].source.locations.is_empty())
                            || (function.unit_parameter_homes[0].shape.byte_size != 0
                                && function.unit_parameter_homes[0].source.locations.is_empty())
                            || attachments.get(&nominal.cleanup_machine)
                                != Some(&Some(nominal.structural_type))
                            || cleanup_is_executable.is_none()
                            || matching_cleanup_calls
                                != usize::from(cleanup_is_executable == Some(true))
                    }
                    ([], []) => parameter_discards != expected_parameter_discards,
                    ([], residuals @ [_, ..]) => {
                        let residual_root = residuals[0].place;
                        let parameter_type = function
                            .unit_parameters
                            .iter()
                            .find(|parameter| parameter.place == residual_root)
                            .map(|parameter| parameter.structural_type);
                        let moved = record
                            .internal_unit_calls
                            .iter()
                            .filter(|call| call.machine == function.machine)
                            .flat_map(|call| &call.custody.arguments)
                            .filter(|argument| {
                                argument.place == residual_root
                                    && Some(argument.root_structural_type) == parameter_type
                            })
                            .map(|argument| (argument.path.as_slice(), argument.structural_type))
                            .collect::<Vec<_>>();
                        let parameter_is_bounded_affine_array =
                            parameter_type.is_some_and(|root_type| {
                                cleanup.structural_types.iter().any(|declaration| {
                                    declaration.id == root_type
                                        && matches!(
                                            declaration.shape,
                                            StructuralTypeShape::FixedArray { length: 3 | 4, .. }
                                        )
                                })
                            });
                        cleanup.actions.get(..discards.len()).is_none_or(|prefix| {
                            !prefix.iter().zip(&discards).all(|(action, place)| {
                                matches!(action,
                                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(actual)
                                        if actual == place)
                            })
                        })
                            || cleanup.actions.get(discards.len()..).is_none_or(|suffix| {
                                suffix.iter().zip(residuals).any(|(action, residual)| {
                                    !matches!(action,
                                        psi_terminal::TerminalAffineCleanupAction::DiscardResidual(actual)
                                            if actual == *residual)
                                })
                            })
                            || !parameter_discards.is_empty()
                            || expected_parameter_discards.as_slice() != [residual_root]
                            || residuals.iter().any(|residual| {
                                residual.place != residual_root
                                    || residual.path.is_empty()
                                    || !is_partial_cleanup_path(&residual.path)
                                    || parameter_type == Some(residual.structural_type)
                            })
                            || residuals
                                .iter()
                                .enumerate()
                                .any(|(index, residual)| {
                                    residuals[..index].iter().any(|earlier| {
                                        residual.path.starts_with(&earlier.path)
                                            || earlier.path.starts_with(&residual.path)
                                    })
                                })
                            || parameter_type.is_none()
                            || (parameter_is_bounded_affine_array
                                && !partially_consumed_affine_array)
                            || moved.is_empty()
                            || moved.iter().any(|(path, _)| {
                                path.is_empty()
                                    || !is_partial_cleanup_path(path)
                                    || residuals
                                        .iter()
                                        .any(|residual| {
                                            path.starts_with(&residual.path)
                                                || residual.path.starts_with(path)
                                        })
                            })
                            || moved
                                .iter()
                                .enumerate()
                                .any(|(index, (path, _))| {
                                    moved[..index].iter().any(|(earlier, _)| {
                                        path.starts_with(earlier)
                                            || earlier.starts_with(path)
                                    })
                                })
                            || parameter_type.is_none_or(|root_type| {
                                !super::exact_partial_cleanup_partition(
                                    &cleanup.structural_types,
                                    root_type,
                                    &moved,
                                    residuals,
                                )
                            })
                    }
                    (nominal @ [_, _, ..], []) => {
                        let bodies = nominal
                            .iter()
                            .map(|cleanup| exact_nominal_body(cleanup))
                            .collect::<Vec<_>>();
                        let executable = bodies
                            .iter()
                            .enumerate()
                            .filter_map(|(ordinal, body)| (*body == Some(true)).then_some(ordinal))
                            .collect::<Vec<_>>();
                        let caller_cleanup_calls = record
                            .internal_unit_calls
                            .iter()
                            .filter(|call| {
                                call.machine == function.machine
                                    && matches!(call.custody.owner,
                                        CallSiteOwner::CleanupAction { edge, .. }
                                            if edge == cleanup.psi_edge)
                            })
                            .collect::<Vec<_>>();
                        let ordered_executable_spans = executable
                            .iter()
                            .map(|ordinal| {
                                let action_ordinal = u32::try_from(*ordinal).ok()?;
                                let call = caller_cleanup_calls.iter().find(|call| {
                                    call.custody.owner
                                        == CallSiteOwner::CleanupAction {
                                            edge: cleanup.psi_edge,
                                            action_ordinal,
                                        }
                                        && call.custody.target == nominal[*ordinal].cleanup_machine
                                })?;
                                Some((
                                    call.custody.code_offset,
                                    call.custody
                                        .code_offset
                                        .checked_add(call.custody.byte_count)?,
                                ))
                            })
                            .collect::<Option<Vec<_>>>();
                        !cleanup.locals.is_empty()
                            || !discards.is_empty()
                            || function.unit_parameter_homes.len() != nominal.len()
                            || function.unit_parameter_homes.iter().rev().zip(nominal).any(
                                |(home, nominal)| {
                                    home.place != nominal.place
                                        || home.structural_type != nominal.structural_type
                                        || home.multiplicity != StructuralMultiplicity::Affine
                                        || !bounded_nominal_receiver_shape(home.shape)
                                        || (home.shape.byte_size == 0
                                            && !home.source.locations.is_empty())
                                        || (home.shape.byte_size != 0
                                            && home.source.locations.is_empty())
                                        || attachments.get(&nominal.cleanup_machine)
                                            != Some(&Some(nominal.structural_type))
                                },
                            )
                            || bodies.iter().any(Option::is_none)
                            || caller_cleanup_calls.len() != executable.len()
                            || ordered_executable_spans.is_none_or(|spans| {
                                spans
                                    .windows(2)
                                    .any(|pair| pair[0].0 >= pair[1].0 || pair[0].1 > pair[1].0)
                            })
                            || executable.iter().any(|ordinal| {
                                let action_ordinal = u32::try_from(*ordinal).ok();
                                action_ordinal.is_none_or(|action_ordinal| {
                                    caller_cleanup_calls
                                        .iter()
                                        .filter(|call| {
                                            call.custody.owner
                                                == CallSiteOwner::CleanupAction {
                                                    edge: cleanup.psi_edge,
                                                    action_ordinal,
                                                }
                                                && call.custody.target
                                                    == nominal[*ordinal].cleanup_machine
                                                && call.custody.arguments.is_empty()
                                                && call.custody.claim_transfers.is_empty()
                                                && call.custody.code_offset >= cleanup.code_offset
                                                && call
                                                    .custody
                                                    .code_offset
                                                    .checked_add(call.custody.byte_count)
                                                    .is_some_and(|call_end| call_end <= end)
                                        })
                                        .count()
                                        != 1
                                })
                            })
                    }
                    _ => true,
                }
            {
                return Err(InstallationError::InvalidUnitAffineCleanup(
                    function.machine,
                ));
            }
        }
        if let Some(cleanup) = &function.scalar_affine_cleanup {
            validate_scalar_affine_cleanup_shape(record, function, cleanup, true)?;
        }
        if has_scalar_control_cleanup {
            validate_scalar_control_affine_cleanup_shape(record, function)?;
        }
        expected_text_offset = expected_text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::FunctionOffsetNotRepresentable)?;
        previous_function = Some(function.machine);
    }
    if record.private_functions.len() > 1 {
        return Err(InstallationError::TooManyCompilerPrivateFunctions);
    }
    for private in &record.private_functions {
        if private.identity.callback_thunk_placement_index().is_none()
            || !private.identity.is_valid()
            || private.byte_count == 0
            || private.text_offset != expected_text_offset
            || !installed_unit_scalar_transport::installed_fixed_integer_scalar_abi_is_canonical(
                &private.fixed_integer_scalar_abi,
                record.target,
            )
        {
            return Err(InstallationError::InvalidCompilerPrivateFunction);
        }
        expected_text_offset = expected_text_offset
            .checked_add(private.byte_count)
            .ok_or(InstallationError::CompilerPrivateFunctionOffsetNotRepresentable)?;
    }
    if expected_text_offset != record.image_sections.text_byte_count {
        return Err(InstallationError::InvalidImageSectionLayout);
    }
    let function_by_machine = record
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    validate_installed_unit_scalar_calls(record, &function_by_machine)?;
    validate_installed_unit_structural_scalar_field_stores(record, &function_by_machine)?;
    let mut previous_return = None;
    for installed in &record.structural_returns {
        let function = function_by_machine.get(&installed.machine).ok_or(
            InstallationError::StructuralReturnMachineMissing(installed.machine),
        )?;
        let returned = &installed.returned;
        let expected_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(record.target),
            &CallSignature {
                parameters: returned
                    .parameter_placements
                    .iter()
                    .map(|placement| placement.shape)
                    .collect(),
                result: Some(returned.shape),
            },
        )
        .map_err(|_| InstallationError::InvalidStructuralReturn(installed.machine))?;
        let expected_result = expected_plan.result.as_ref();
        let structural_attribution = record
            .semantic_code_attribution
            .iter()
            .filter(|attribution| attribution.machine == installed.machine)
            .collect::<Vec<_>>();
        if previous_return.is_some_and(|previous| previous >= installed.machine)
            || returned.code_offset != 0
            || returned.byte_count != function.byte_count
            || returned.source.position != 0
            || returned.source.is_self
            || returned.source.multiplicity != StructuralMultiplicity::Linear
            || returned.result.multiplicity != StructuralMultiplicity::Linear
            || returned.source.structural_type != returned.result.structural_type
            || returned.source.qualifications != returned.result.qualifications
            || returned.source.place == returned.result.place
            || returned.shape.class != ValueClass::Integer
            || !((returned.shape.byte_size == 8 && returned.shape.alignment == 8)
                || (9..=16).contains(&returned.shape.byte_size))
            || returned.source_placement.shape != returned.shape
            || returned.result_placement.shape != returned.shape
            || !direct_structural_return_placement(&returned.source_placement)
            || !direct_structural_return_placement(&returned.result_placement)
            || returned.parameters.first() != Some(&returned.source)
            || returned.parameters.iter().skip(1).any(|parameter| {
                parameter.place == returned.source.place
                    || parameter.place == returned.result.place
                    || !parameter.qualifications.is_empty()
            })
            || returned
                .parameters
                .iter()
                .map(|parameter| parameter.place)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != returned.parameters.len()
            || returned.trivial_affine_locals.iter().enumerate().any(|(index, (_, local, local_type))| {
                !matches!(
                    local.kind,
                    psi_core::StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal,
                        structural_type,
                        construction: None,
                    } if usize::try_from(declaration_ordinal) == Ok(index)
                        && structural_type == local_type.id
                ) || local.id == returned.source.place
                    || local.id == returned.result.place
                    || returned.parameters.iter().any(|parameter| parameter.place == local.id)
                    || local_type.identity.is_empty()
                    || !matches!(
                        local_type.shape,
                        psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty()
                    )
            })
            || returned
                .trivial_affine_locals
                .iter()
                .map(|(_, local, _)| local.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != returned.trivial_affine_locals.len()
            || returned.parameter_placements.len() != returned.parameters.len()
            || expected_plan.parameters != returned.parameter_placements
            || returned.parameter_placements.first() != Some(&returned.source_placement)
            || returned
                .parameters
                .iter()
                .enumerate()
                .any(|(index, parameter)| {
                    parameter.is_self || usize::try_from(parameter.position) != Ok(index)
                })
            || returned.trivial_affine_discards
                != returned
                    .trivial_affine_locals
                    .iter()
                    .rev()
                    .map(|(_, local, _)| local.id)
                    .chain(
                        returned
                            .parameters
                            .iter()
                            .skip(1)
                            .rev()
                            .map(|parameter| parameter.place),
                    )
                    .collect::<Vec<_>>()
            || returned
                .parameters
                .iter()
                .skip(1)
                .any(|parameter| parameter.multiplicity != StructuralMultiplicity::Affine)
            || expected_result != Some(&returned.result_placement)
            || returned.returned_claims.len() != 1
            || structural_attribution.len() != returned.trivial_affine_locals.len() + 1
            || returned
                .trivial_affine_locals
                .iter()
                .enumerate()
                .any(|(ordinal, (operation, _, _))| {
                    structural_attribution.get(ordinal).is_none_or(|installed| {
                        installed.attribution.site
                                != SemanticCodeSite::Operation(*operation)
                            || installed.attribution.operation_ordinal != ordinal
                            || installed.attribution.code_offset != 0
                            || installed.attribution.byte_count != 0
                    })
                })
            || structural_attribution.last().is_none_or(|installed| {
                installed.attribution.site
                        != SemanticCodeSite::Edge(returned.psi_edge)
                    || installed.attribution.operation_ordinal
                        != returned.trivial_affine_locals.len()
                    || installed.attribution.code_offset != 0
                    || installed.attribution.byte_count != returned.byte_count
            })
        {
            return Err(InstallationError::InvalidStructuralReturn(
                installed.machine,
            ));
        }
        previous_return = Some(installed.machine);
    }
    let mut previous_call = None;
    for installed in &record.internal_unit_calls {
        let function = function_by_machine.get(&installed.machine).ok_or(
            InstallationError::InvalidInternalUnitCall(installed.machine),
        )?;
        let custody = &installed.custody;
        let target_returns_scalar =
            function_by_machine
                .get(&custody.target)
                .is_some_and(|target| {
                    target.scalar_stack.is_some()
                        || (target.unit_body
                            && record.internal_unit_calls.iter().any(|call| {
                                call.machine == custody.target && call.custody.result.is_some()
                            }))
                });
        let target_structural_return = record
            .structural_returns
            .iter()
            .find(|target| target.machine == custody.target)
            .map(|target| &target.returned);
        let structural_result_valid = match (&custody.structural_result, target_structural_return) {
            (None, None) => true,
            (Some(result), Some(target)) => {
                custody.result.is_none()
                    && result.operation_result.structural_type == target.result.structural_type
                    && result.operation_result.multiplicity == target.result.multiplicity
                    && result.operation_result.qualifications == target.result.qualifications
                    && result.function_result.structural_type == target.result.structural_type
                    && result.function_result.multiplicity == target.result.multiplicity
                    && result.function_result.qualifications == target.result.qualifications
                    && result.returned_claim_transfers.len() == 1
                    && target.returned_claims.as_slice()
                        == [result.returned_claim_transfers[0].callee_claim]
                    && result.operation_result.claims.len() == 1
                    && result.operation_result.claims[0].claim
                        == result.returned_claim_transfers[0].caller_claim
                    && result.returned_claims.as_slice()
                        == [result.returned_claim_transfers[0].caller_claim]
                    && result.caller_result_placement == target.result_placement
                    && result.callee_result_placement == target.result_placement
            }
            _ => false,
        };
        let expected_text_offset = function
            .text_offset
            .checked_add(custody.code_offset)
            .ok_or(InstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let end = custody
            .code_offset
            .checked_add(custody.byte_count)
            .ok_or(InstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let plan = evaluate_call_plan(
            CallingPolicy::native_for_target(record.target),
            &CallSignature {
                parameters: custody
                    .arguments
                    .iter()
                    .map(|argument| argument.shape)
                    .collect(),
                result: if let Some(result) = custody.result {
                    let bytes = match result {
                        psi_core::ScalarType::Boolean => 1,
                        psi_core::ScalarType::Integer(integer) => integer.bits().div_ceil(8),
                        psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32) => 4,
                        psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64) => 8,
                    };
                    Some(match result {
                        psi_core::ScalarType::IeeeFloat(_) => ValueShape::float(bytes),
                        _ => ValueShape::integer(bytes, bytes.next_power_of_two().min(8)),
                    })
                } else if custody.structural_result.is_some() {
                    custody.arguments.first().map(|argument| argument.shape)
                } else {
                    None
                },
            },
        )
        .map_err(|_| InstallationError::InvalidInternalUnitCall(installed.machine))?;
        let key = (
            installed.machine,
            custody.operation_ordinal,
            custody.code_offset,
        );
        let projected_argument_indexes = custody
            .arguments
            .iter()
            .enumerate()
            .filter_map(|(index, argument)| (!argument.path.is_empty()).then_some(index))
            .collect::<std::collections::BTreeSet<_>>();
        let transferred_argument_indexes = custody
            .claim_transfers
            .iter()
            .filter_map(|transfer| usize::try_from(transfer.argument_index).ok())
            .collect::<std::collections::BTreeSet<_>>();
        let control_cleanup = match custody.owner {
            CallSiteOwner::CleanupAction { edge, .. } => function
                .scalar_control_affine_cleanups
                .iter()
                .find(|cleanup| cleanup.psi_edge == edge),
            CallSiteOwner::Operation(_) => None,
        };
        let affine_cleanup = function
            .scalar_affine_cleanup
            .as_ref()
            .or(function.unit_affine_cleanup.as_ref())
            .or(control_cleanup);
        let function_unit_calls = record
            .internal_unit_calls
            .iter()
            .filter(|call| call.machine == installed.machine)
            .map(|call| call.custody.clone())
            .collect::<Vec<_>>();
        let fully_consumed_affine_pair =
            crate::fully_consumed_affine_pair::exact_fully_consumed_affine_pair(
                &function.unit_parameter_homes,
                &function_unit_calls,
                function.unit_affine_cleanup.as_ref(),
            );
        let owner_valid = match custody.owner {
            CallSiteOwner::Operation(operation) => {
                record.semantic_code_attribution.iter().any(|attribution| {
                    attribution.machine == installed.machine
                        && attribution.attribution.site
                            == SemanticCodeSite::Operation(operation)
                        && attribution.attribution.operation_ordinal == custody.operation_ordinal
                        && attribution.attribution.code_offset == custody.code_offset
                        && attribution.attribution.byte_count == custody.byte_count
                })
            }
            CallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            } => {
                custody.result.is_none()
                    && custody.structural_result.is_none()
                    && custody.arguments.is_empty()
                    && custody.claim_transfers.is_empty()
                    && affine_cleanup
                        .is_some_and(|cleanup| {
                            cleanup.psi_edge == edge
                                && usize::try_from(action_ordinal)
                                    .ok()
                                    .and_then(|ordinal| cleanup.actions.get(ordinal))
                                    .is_some_and(|action| matches!(action,
                                        psi_terminal::TerminalAffineCleanupAction::InvokeNominal(nominal)
                                            if nominal.cleanup_machine == custody.target))
                                && cleanup.code_offset <= custody.code_offset
                                && custody
                                    .code_offset
                                    .checked_add(custody.byte_count)
                                    .is_some_and(|call_end| {
                                        cleanup
                                            .code_offset
                                            .checked_add(cleanup.byte_count)
                                            .is_some_and(|cleanup_end| call_end <= cleanup_end)
                                    })
                                && record.semantic_code_attribution.iter().any(|attribution| {
                                    attribution.machine == installed.machine
                                        && attribution.attribution.site
                                            == SemanticCodeSite::Edge(edge)
                                        && attribution.attribution.operation_ordinal
                                            == custody.operation_ordinal
                                        && attribution.attribution.code_offset
                                            == cleanup.code_offset
                                        && attribution.attribution.byte_count == cleanup.byte_count
                                })
                        })
            }
        };
        if previous_call.is_some_and(|previous| previous >= key)
            || installed.text_offset != expected_text_offset
            || end > function.byte_count
            || !function_by_machine.contains_key(&custody.target)
            || custody.result.is_some() != target_returns_scalar
            || !structural_result_valid
            || (custody.structural_result.is_some() && target_returns_scalar)
            || !owner_valid
            || plan.parameters.len() != custody.arguments.len()
            || custody.arguments.windows(2).any(|pair| {
                pair[0]
                    .code_offset
                    .checked_add(pair[0].byte_count)
                    .is_none_or(|end| end > pair[1].code_offset)
            })
            || custody
                .arguments
                .iter()
                .zip(&plan.parameters)
                .any(|(argument, destination)| {
                    argument.destination != *destination
                        || argument.byte_count == 0
                        || argument.bytes.len() != argument.byte_count
                        || (!argument.path.is_empty()
                            && super::expected_projected_copy_bytes(record.target, argument)
                                .as_deref()
                                != Some(argument.bytes.as_slice()))
                        || argument.code_offset < custody.code_offset
                        || argument
                            .code_offset
                            .checked_add(argument.byte_count)
                            .is_none_or(|argument_end| argument_end > end)
                        || argument
                            .source_byte_offset
                            .checked_add(u32::from(argument.shape.byte_size))
                            .is_none_or(|end| end > u32::from(argument.source.shape.byte_size))
                        || match argument.path.as_slice() {
                            [] => {
                                argument.source_byte_offset != 0
                                    || argument.source.shape != argument.shape
                                    || argument.root_structural_type != argument.structural_type
                                    || argument.fixed_array_length.is_some()
                                    || argument.element_stride.is_some()
                            }
                            [StructuralPathSegment::FixedIndex(index)] => {
                                let expected_stride = u32::from(argument.shape.byte_size)
                                    .next_multiple_of(u32::from(argument.shape.alignment));
                                let Some(length) = argument.fixed_array_length else {
                                    return true;
                                };
                                let Some(stride) = argument.element_stride else {
                                    return true;
                                };
                                argument.root_structural_type == argument.structural_type
                                    || *index >= length
                                    || stride != expected_stride
                                    || u64::from(stride).checked_mul(*index)
                                        != Some(u64::from(argument.source_byte_offset))
                                    || u64::from(stride).checked_mul(length)
                                        != Some(u64::from(argument.source.shape.byte_size))
                                    || argument.source.shape.alignment != argument.shape.alignment
                            }
                            [
                                StructuralPathSegment::FixedIndex(outer @ (0 | 1)),
                                StructuralPathSegment::FixedIndex(
                                    inner @ (0 | 1 | 2 | 3 | 4 | 5 | 6 | 7),
                                ),
                            ] => {
                                let leaf_stride = u32::from(argument.shape.byte_size)
                                    .next_multiple_of(u32::from(argument.shape.alignment));
                                let Some(outer_stride) = argument.element_stride else {
                                    return true;
                                };
                                let Some(inner_length) = [3_u32, 4_u32, 5_u32, 6_u32, 7_u32, 8_u32]
                                    .into_iter()
                                    .find(|length| {
                                        leaf_stride.checked_mul(*length) == Some(outer_stride)
                                    })
                                else {
                                    return true;
                                };
                                let expected_offset = outer_stride
                                    .checked_mul(u32::try_from(*outer).unwrap_or(u32::MAX))
                                    .and_then(|offset| {
                                        leaf_stride
                                            .checked_mul(u32::try_from(*inner).unwrap_or(u32::MAX))
                                            .and_then(|inner| offset.checked_add(inner))
                                    });
                                argument.root_structural_type == argument.structural_type
                                    || argument.fixed_array_length != Some(2)
                                    || *inner >= u64::from(inner_length)
                                    || Some(argument.source_byte_offset) != expected_offset
                                    || outer_stride.checked_mul(2)
                                        != Some(u32::from(argument.source.shape.byte_size))
                                    || argument.source.shape.alignment != argument.shape.alignment
                            }
                            path @ [StructuralPathSegment::Field(_), ..]
                                if path.iter().all(|segment| {
                                    matches!(segment,
                                        StructuralPathSegment::Field(identity)
                                            if !identity.is_empty())
                                }) =>
                            {
                                path.is_empty()
                                    || argument.root_structural_type == argument.structural_type
                                    || argument.fixed_array_length.is_some()
                                    || argument.element_stride.is_some()
                                    || !argument
                                        .source_byte_offset
                                        .is_multiple_of(u32::from(argument.shape.alignment))
                            }
                            _ => true,
                        }
                })
            || projected_argument_indexes.iter().any(|index| {
                if transferred_argument_indexes.contains(index) {
                    return false;
                }
                let Some(argument) = custody.arguments.get(*index) else {
                    return true;
                };
                argument.path.is_empty()
                    || (!fully_consumed_affine_pair
                        && affine_cleanup.is_none_or(|cleanup| {
                            !cleanup.actions.iter().any(|action| {
                                matches!(action,
                                psi_terminal::TerminalAffineCleanupAction::DiscardResidual(residual)
                                    if residual.place == argument.place
                                        && !residual.path.is_empty()
                                        && !residual.path.starts_with(&argument.path)
                                        && !argument.path.starts_with(&residual.path)
                                        && residual.structural_type
                                            != argument.root_structural_type)
                            })
                        }))
            })
            || custody.claim_transfers.iter().any(|transfer| {
                usize::try_from(transfer.argument_index)
                    .map_or(true, |index| index >= custody.arguments.len())
            })
            || custody
                .claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != custody.claim_transfers.len()
        {
            return Err(InstallationError::InvalidInternalUnitCall(
                installed.machine,
            ));
        }
        previous_call = Some(key);
    }
    let mut previous_attribution = None;
    let mut attribution_sites = std::collections::BTreeSet::new();
    for installed in &record.semantic_code_attribution {
        let function = function_by_machine.get(&installed.machine).ok_or(
            InstallationError::SemanticCodeAttributionMachineMissing(installed.machine),
        )?;
        let expected = function
            .text_offset
            .checked_add(installed.attribution.code_offset)
            .ok_or(InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?;
        let end = installed
            .attribution
            .code_offset
            .checked_add(installed.attribution.byte_count)
            .ok_or(InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?;
        if installed.text_offset != expected || end > function.byte_count {
            return Err(InstallationError::InvalidSemanticCodeAttribution {
                machine: installed.machine,
                site: installed.attribution.site,
            });
        }
        let key = (
            installed.machine,
            installed.attribution.operation_ordinal,
            installed.text_offset,
        );
        if previous_attribution.is_some_and(|previous| previous >= key) {
            return Err(InstallationError::NonCanonicalSemanticCodeAttributionOrder);
        }
        if !attribution_sites.insert((installed.machine, installed.attribution.site)) {
            return Err(InstallationError::DuplicateSemanticCodeAttributionSite {
                machine: installed.machine,
                site: installed.attribution.site,
            });
        }
        previous_attribution = Some(key);
    }
    let mut previous_port = None;
    let mut port_operations = std::collections::BTreeSet::new();
    for installed in &record.port_effects {
        let function = function_by_machine
            .get(&installed.machine)
            .ok_or(InstallationError::EffectMachineMissing(installed.machine))?;
        let expected = function
            .text_offset
            .checked_add(installed.effect.code_offset)
            .ok_or(InstallationError::PortEffectOffsetNotRepresentable)?;
        let end = installed
            .effect
            .code_offset
            .checked_add(installed.effect.byte_count)
            .ok_or(InstallationError::PortEffectOffsetNotRepresentable)?;
        if installed.text_offset != expected
            || end > function.byte_count
            || installed.effect.byte_count
                != omega_x86_encoding::encode_immediate_port_write(
                    installed.effect.port,
                    installed.effect.value,
                )
                .len()
        {
            return Err(InstallationError::InvalidPortEffectOffset {
                machine: installed.machine,
                operation: installed.effect.psi_operation,
            });
        }
        let key = (
            installed.machine,
            installed.text_offset,
            installed.effect.operation_ordinal,
        );
        if previous_port.is_some_and(|previous| previous >= key) {
            return Err(InstallationError::NonCanonicalPortEffectOrder);
        }
        if !port_operations.insert((installed.machine, installed.effect.psi_operation)) {
            return Err(InstallationError::DuplicatePortEffectOperation {
                machine: installed.machine,
                operation: installed.effect.psi_operation,
            });
        }
        previous_port = Some(key);
    }
    let mut previous_machine = None;
    let mut previous_text_offset = 0;
    let mut previous_operation_ordinal = 0;
    let mut operations = std::collections::BTreeSet::new();
    for installed in &record.boundary_settlements {
        if let Some(machine) = previous_machine {
            if installed.machine < machine
                || (installed.machine == machine
                    && (
                        installed.text_offset,
                        installed.settlement.operation_ordinal,
                    ) <= (previous_text_offset, previous_operation_ordinal))
            {
                return Err(InstallationError::NonCanonicalBoundarySettlementOrder);
            }
        }
        if !operations.insert((installed.machine, installed.settlement.psi_operation)) {
            return Err(InstallationError::DuplicateBoundarySettlementOperation {
                machine: installed.machine,
                operation: installed.settlement.psi_operation,
            });
        }
        let function = function_by_machine
            .get(&installed.machine)
            .ok_or(InstallationError::EffectMachineMissing(installed.machine))?;
        let expected = function
            .text_offset
            .checked_add(installed.settlement.code_offset)
            .ok_or(InstallationError::SettlementOffsetNotRepresentable)?;
        if installed.text_offset != expected
            || installed
                .settlement
                .code_offset
                .checked_add(installed.settlement.byte_count)
                .is_none_or(|end| end > function.byte_count)
        {
            return Err(InstallationError::InvalidBoundarySettlementOffset {
                machine: installed.machine,
                operation: installed.settlement.psi_operation,
            });
        }
        if let Err(error) = validate_completion_custody(&installed.settlement) {
            return Err(match error {
                CompletionCustodyError::InvalidArgumentPath => {
                    InstallationError::InvalidSettlementArgumentField
                }
                CompletionCustodyError::InvalidReceiptArgumentIndex => {
                    InstallationError::InvalidCompletionReceiptArgumentIndex {
                        machine: installed.machine,
                        operation: installed.settlement.psi_operation,
                    }
                }
                CompletionCustodyError::InvalidReceiptCustody => {
                    InstallationError::InvalidCompletionReceiptCustody {
                        machine: installed.machine,
                        operation: installed.settlement.psi_operation,
                    }
                }
                CompletionCustodyError::InvalidProviderCustody => {
                    InstallationError::InvalidCompletionProviderCustody {
                        machine: installed.machine,
                        operation: installed.settlement.psi_operation,
                    }
                }
            });
        }
        let valid_realization = match installed.settlement.realization {
            BoundaryRealization::MetadataOnlyPort(realization) => {
                installed.settlement.scalar_arguments.is_empty()
                    && installed.settlement.byte_sequence_arguments.is_empty()
                    && installed.settlement.byte_count == 0
                    && record
                        .port_effects
                        .iter()
                        .filter(|effect| {
                            effect.machine == installed.machine
                                && effect.effect.psi_operation == realization.effect_operation
                                && effect.effect.service == realization.service
                                && effect.effect.port == realization.port
                                && effect.effect.value == realization.value
                                && effect.effect.operation_ordinal.checked_add(1)
                                    == Some(installed.settlement.operation_ordinal)
                                && effect
                                    .effect
                                    .code_offset
                                    .checked_add(effect.effect.byte_count)
                                    == Some(installed.settlement.code_offset)
                        })
                        .count()
                        == 1
            }
            BoundaryRealization::ClaimCompletionOnly(_) => {
                installed.settlement.scalar_arguments.is_empty()
                    && installed.settlement.byte_sequence_arguments.is_empty()
                    && installed.settlement.native_result.is_none()
                    && installed.settlement.byte_count == 0
            }
            BoundaryRealization::DirectPortReadU8(_) => {
                let exact_return_edge =
                    installed
                        .settlement
                        .native_result
                        .as_ref()
                        .is_some_and(|result| {
                            let Some(return_ordinal) =
                                installed.settlement.operation_ordinal.checked_add(1)
                            else {
                                return false;
                            };
                            let Some(return_offset) = installed
                                .settlement
                                .code_offset
                                .checked_add(installed.settlement.byte_count)
                            else {
                                return false;
                            };
                            record
                                .semantic_code_attribution
                                .iter()
                                .filter(|attribution| {
                                    attribution.machine == installed.machine
                                        && attribution.attribution.site
                                            == SemanticCodeSite::Edge(result.return_edge)
                                        && attribution.attribution.operation_ordinal
                                            == return_ordinal
                                        && attribution.attribution.code_offset == return_offset
                                        && attribution.attribution.byte_count == 1
                                })
                                .count()
                                == 1
                        });
                installed.settlement.scalar_arguments.is_empty()
                    && installed.settlement.byte_sequence_arguments.is_empty()
                    && record.target.architecture == Architecture::X86_64
                    && installed.settlement.byte_count
                        == omega_x86_encoding::IMMEDIATE_PORT_READ_U8_WIDTH
                    && function.unit_stack.is_none()
                    && function.scalar_stack.is_some()
                    && exact_return_edge
                    && installed.settlement.arguments.iter().all(|argument| {
                        argument.path.is_empty()
                            && function
                                .scalar_structural_parameters
                                .iter()
                                .any(|parameter| parameter.place == argument.place)
                    })
            }
            BoundaryRealization::LinuxWriteLine(_) => {
                linux_write_line_custody_is_exact(record.target, &installed.settlement, None)
                    && function.unit_body
                    && function.scalar_stack.is_none()
            }
            BoundaryRealization::LinuxExitGroupI32(_) => {
                let [argument] = installed.settlement.scalar_arguments.as_slice() else {
                    return Err(InstallationError::BoundaryRealizationMismatch {
                        machine: installed.machine,
                        operation: installed.settlement.psi_operation,
                    });
                };
                let i32_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
                    .expect("i32 is valid");
                let value = match (argument.scalar_type, argument.immediate) {
                    (
                        psi_core::ScalarType::Integer(actual),
                        psi_core::IntegerValue::Signed(value),
                    ) if actual == i32_type => i32::try_from(value).ok(),
                    _ => None,
                };
                let expected_destination =
                    match (record.target.object_format, record.target.architecture) {
                        (omega_target::ObjectFormat::Elf, Architecture::X86_64) => {
                            Some(omega_calling_conventions::MachineRegister::X86Rdi)
                        }
                        (omega_target::ObjectFormat::Elf, Architecture::Aarch64) => {
                            Some(omega_calling_conventions::MachineRegister::Aarch64X(0))
                        }
                        _ => None,
                    };
                let expected_byte_count = value
                    .and_then(|value| match record.target.architecture {
                        Architecture::X86_64 => {
                            Some(omega_isa_x86_64::encode_linux_exit_group_i32(value).len())
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::encode_linux_exit_group_i32(value)
                                .ok()
                                .map(|bytes| bytes.len())
                        }
                    })
                    .unwrap_or(0);
                let exact_nominal_tail = installed
                    .settlement
                    .operation_ordinal
                    .checked_add(1)
                    .is_some_and(|tail_ordinal| {
                        record
                            .semantic_code_attribution
                            .iter()
                            .filter(|attribution| {
                                attribution.machine == installed.machine
                                    && matches!(
                                        attribution.attribution.site,
                                        SemanticCodeSite::Edge(_)
                                    )
                                    && attribution.attribution.operation_ordinal == tail_ordinal
                                    && attribution.attribution.code_offset
                                        == installed
                                            .settlement
                                            .code_offset
                                            .saturating_add(installed.settlement.byte_count)
                                    && attribution
                                        .attribution
                                        .code_offset
                                        .checked_add(attribution.attribution.byte_count)
                                        == Some(function.byte_count)
                                    && (function.unit_body
                                        || attribution.attribution.byte_count == 0)
                            })
                            .count()
                            == 1
                    });
                record.target.object_format == omega_target::ObjectFormat::Elf
                    && expected_destination == Some(argument.destination)
                    && installed.settlement.byte_count == expected_byte_count
                    && expected_byte_count != 0
                    && installed.settlement.arguments.is_empty()
                    && installed.settlement.byte_sequence_arguments.is_empty()
                    && installed.settlement.native_result.is_none()
                    && function.scalar_stack.is_none()
                    && exact_nominal_tail
            }
        };
        if !valid_realization
            || !boundary_result_is_exact(
                record.target,
                installed.settlement.realization,
                installed.settlement.native_result.as_ref(),
            )
        {
            return Err(InstallationError::BoundaryRealizationMismatch {
                machine: installed.machine,
                operation: installed.settlement.psi_operation,
            });
        }
        previous_machine = Some(installed.machine);
        previous_text_offset = installed.text_offset;
        previous_operation_ordinal = installed.settlement.operation_ordinal;
    }
    Ok(())
}

fn validate_installed_dynamic_conformance(
    record: &InstallationRecord,
) -> Result<(), InstallationError> {
    let sections = record.image_sections;
    if sections.text_byte_count == 0
        || sections.layout.text_address == 0
        || sections.final_data_fingerprint.as_bytes() == &[0; 32]
        || (sections.data_byte_count == 0
            && sections.final_data_fingerprint != fingerprint_initialized_data(&[]))
    {
        return Err(InstallationError::InvalidImageSectionLayout);
    }
    if sections.data_byte_count == 0 {
        if !record.dynamic_conformance_tables.is_empty() || !record.dynamic_scalar_calls.is_empty()
        {
            return Err(InstallationError::InvalidImageSectionLayout);
        }
        return Ok(());
    }
    let text_end = sections
        .layout
        .text_address
        .checked_add(
            u64::try_from(sections.text_byte_count)
                .map_err(|_| InstallationError::InvalidImageSectionLayout)?,
        )
        .ok_or(InstallationError::InvalidImageSectionLayout)?;
    if sections.layout.data_address < text_end
        || sections.layout.data_address % 8 != 0
        || record.dynamic_conformance_tables.is_empty()
        || record.dynamic_scalar_calls.is_empty()
    {
        return Err(InstallationError::InvalidImageSectionLayout);
    }

    let functions = record
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut commitments = std::collections::BTreeSet::new();
    let mut expected_data_offset = 0usize;
    for table in &record.dynamic_conformance_tables {
        let table_byte_count = table
            .slots
            .len()
            .checked_mul(8)
            .ok_or(InstallationError::InvalidDynamicConformanceTable)?;
        if table.application_commitment.is_zero()
            || table.application_report_fingerprint == 0
            || !commitments.insert(table.application_commitment)
            || table.data_offset != expected_data_offset
            || table.byte_count != table_byte_count
            || table.slots.is_empty()
        {
            return Err(InstallationError::InvalidDynamicConformanceTable);
        }
        for (row_index, slot) in table.slots.iter().enumerate() {
            let slot_offset = table
                .data_offset
                .checked_add(
                    row_index
                        .checked_mul(8)
                        .ok_or(InstallationError::InvalidDynamicConformanceTable)?,
                )
                .ok_or(InstallationError::InvalidDynamicConformanceTable)?;
            if usize::try_from(slot.row_index) != Ok(row_index)
                || slot.data_offset != slot_offset
                || slot
                    .target
                    .is_some_and(|target| !functions.contains_key(&target))
            {
                return Err(InstallationError::InvalidDynamicConformanceTable);
            }
        }
        expected_data_offset = expected_data_offset
            .checked_add(table.byte_count)
            .ok_or(InstallationError::InvalidDynamicConformanceTable)?;
    }
    if expected_data_offset != sections.data_byte_count {
        return Err(InstallationError::InvalidDynamicConformanceTable);
    }

    let mut previous_call = None;
    let mut call_sites = std::collections::BTreeSet::new();
    let mut referenced_commitments = std::collections::BTreeSet::new();
    for call in &record.dynamic_scalar_calls {
        let function = functions
            .get(&call.machine)
            .ok_or(InstallationError::InvalidDynamicScalarCall(call.machine))?;
        let call_end = call
            .text_offset
            .checked_add(call.byte_count)
            .ok_or(InstallationError::InvalidDynamicScalarCall(call.machine))?;
        let function_end = function
            .text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::InvalidDynamicScalarCall(call.machine))?;
        let table = record
            .dynamic_conformance_tables
            .iter()
            .find(|table| table.application_commitment == call.application_commitment)
            .ok_or(InstallationError::InvalidDynamicScalarCall(call.machine))?;
        referenced_commitments.insert(call.application_commitment);
        let selected_index = usize::try_from(call.selected_table_byte_offset / 8)
            .map_err(|_| InstallationError::InvalidDynamicScalarCall(call.machine))?;
        let selected = table
            .slots
            .get(selected_index)
            .ok_or(InstallationError::InvalidDynamicScalarCall(call.machine))?;
        let order = (call.text_offset, call.machine, call.operation);
        if call.byte_count == 0
            || call.selected_table_byte_offset % 8 != 0
            || call.text_offset < function.text_offset
            || call_end > function_end
            || selected.target != Some(call.realization)
            || !function
                .unit_parameter_homes
                .iter()
                .any(|home| home.place == call.initial_source)
            || !function
                .unit_parameter_homes
                .iter()
                .any(|home| home.place == call.rebound_source)
            || previous_call.is_some_and(|previous| previous >= order)
            || !call_sites.insert((call.machine, call.operation))
        {
            return Err(InstallationError::InvalidDynamicScalarCall(call.machine));
        }
        previous_call = Some(order);
    }
    if referenced_commitments != commitments {
        return Err(InstallationError::InvalidDynamicConformanceTable);
    }
    Ok(())
}

fn is_partial_cleanup_path(path: &[StructuralPathSegment]) -> bool {
    (!path.is_empty()
        && path.iter().all(|segment| {
            matches!(segment, StructuralPathSegment::Field(identity) if !identity.is_empty())
        }))
        || matches!(
            path,
            [StructuralPathSegment::FixedIndex(0 | 1 | 2 | 3)]
                | [
                    StructuralPathSegment::FixedIndex(0 | 1),
                    StructuralPathSegment::FixedIndex(0 | 1 | 2 | 3 | 4 | 5 | 6 | 7),
                ]
        )
}

fn installed_stack_facts_are_canonical(
    function: &InstalledFunction,
    functions: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
) -> bool {
    let valid_alignment = |alignment: u32| alignment != 0 && alignment.is_power_of_two();
    if function.unit_stack.is_some() && function.scalar_stack.is_some()
        || function
            .unit_stack
            .is_some_and(|stack| !valid_alignment(stack.stack_alignment))
        || function
            .scalar_stack
            .is_some_and(|stack| !valid_alignment(stack.stack_alignment))
        || (!function.unit_call_stacks.is_empty() && function.unit_stack.is_none())
        || (!function.scalar_call_stacks.is_empty() && function.scalar_stack.is_none())
        || (!function.foreign_call_stacks.is_empty() && function.unit_stack.is_none())
    {
        return false;
    }
    let call_in_function = |target: MachineId, text_offset: usize| {
        functions.contains_key(&target)
            && text_offset >= function.text_offset
            && text_offset < function.text_offset.saturating_add(function.byte_count)
    };
    let unit_calls_valid = function.unit_call_stacks.iter().all(|call| {
        call_in_function(call.target, call.text_offset)
            && call
                .active_frame_bytes
                .checked_add(call.transient_bytes)
                .is_some_and(|sum| sum == call.caller_live_bytes)
    });
    let scalar_calls_valid = function
        .scalar_call_stacks
        .iter()
        .all(|call| call_in_function(call.target, call.text_offset));
    let foreign_calls_valid = function.foreign_call_stacks.iter().all(|call| {
        call.text_offset >= function.text_offset
            && call.text_offset < function.text_offset.saturating_add(function.byte_count)
            && call.caller_live_bytes != 0
            && call.provider_plan_report_identity != 0
            && call.contribution_report_identity.normalized_identity() != 0
            && !call.contribution_commitment.is_zero()
            && call.contribution_bytes != 0
            && call.contribution_alignment != 0
            && call.contribution_alignment.is_power_of_two()
            && function.unit_stack.is_some_and(|stack| {
                call.contribution_alignment <= u64::from(stack.stack_alignment)
            })
    });
    let unit_ordered = function.unit_call_stacks.windows(2).all(|pair| {
        (pair[0].text_offset, pair[0].owner, pair[0].target)
            < (pair[1].text_offset, pair[1].owner, pair[1].target)
    });
    let scalar_ordered = function.scalar_call_stacks.windows(2).all(|pair| {
        (pair[0].text_offset, pair[0].owner, pair[0].target)
            < (pair[1].text_offset, pair[1].owner, pair[1].target)
    });
    let foreign_ordered = function
        .foreign_call_stacks
        .windows(2)
        .all(|pair| (pair[0].text_offset, pair[0].owner) < (pair[1].text_offset, pair[1].owner));
    unit_calls_valid
        && scalar_calls_valid
        && foreign_calls_valid
        && unit_ordered
        && scalar_ordered
        && foreign_ordered
}

fn validate_scalar_affine_cleanup_shape(
    record: &InstallationRecord,
    function: &InstalledFunction,
    cleanup: &omega_machine_code::UnitAffineCleanupRecord,
    require_function_end: bool,
) -> Result<(), InstallationError> {
    let invalid = || InstallationError::InvalidUnitAffineCleanup(function.machine);
    let end = cleanup
        .code_offset
        .checked_add(cleanup.byte_count)
        .ok_or(InstallationError::FunctionOffsetNotRepresentable)?;
    if cleanup.byte_count == 0
        || end > function.byte_count
        || (require_function_end && end != function.byte_count)
        || !cleanup.locals.is_empty()
        || cleanup.actions.len() != function.scalar_structural_parameter_homes.len()
        || function
            .scalar_structural_parameter_homes
            .iter()
            .rev()
            .zip(&cleanup.actions)
            .any(|(home, action)| match action {
                psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
                    *place != home.place || home.multiplicity != StructuralMultiplicity::Affine
                }
                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(nominal) => {
                    nominal.place != home.place
                        || nominal.structural_type != home.structural_type
                        || home.multiplicity != StructuralMultiplicity::Affine
                        || nominal.cleanup_receiver.is_some()
                        || !nominal.requirement_obligations.is_empty()
                        || record.functions.iter().all(|target| {
                            target.machine != nominal.cleanup_machine
                                || target.attachment != Some(nominal.structural_type)
                                || !target.unit_body
                        })
                }
                psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => true,
            })
    {
        return Err(invalid());
    }
    for (ordinal, action) in cleanup.actions.iter().enumerate() {
        let psi_terminal::TerminalAffineCleanupAction::InvokeNominal(nominal) = action else {
            continue;
        };
        let target = record
            .functions
            .iter()
            .find(|target| target.machine == nominal.cleanup_machine)
            .ok_or_else(invalid)?;
        let executable = record
            .internal_unit_calls
            .iter()
            .any(|call| call.machine == target.machine);
        let action_ordinal = u32::try_from(ordinal).map_err(|_| invalid())?;
        let matching = record
            .internal_unit_calls
            .iter()
            .filter(|call| {
                call.machine == function.machine
                    && call.custody.owner
                        == CallSiteOwner::CleanupAction {
                            edge: cleanup.psi_edge,
                            action_ordinal,
                        }
                    && call.custody.target == nominal.cleanup_machine
                    && call.custody.arguments.is_empty()
                    && call.custody.claim_transfers.is_empty()
                    && call.custody.code_offset >= cleanup.code_offset
                    && call
                        .custody
                        .code_offset
                        .checked_add(call.custody.byte_count)
                        .is_some_and(|call_end| call_end <= end)
            })
            .count();
        if matching != usize::from(executable) {
            return Err(invalid());
        }
    }
    Ok(())
}

fn validate_scalar_control_affine_cleanup_shape(
    record: &InstallationRecord,
    function: &InstalledFunction,
) -> Result<(), InstallationError> {
    let invalid = || InstallationError::InvalidUnitAffineCleanup(function.machine);
    let cleanups = &function.scalar_control_affine_cleanups;
    if cleanups.len() < 2 {
        return Err(InstallationError::InvalidScalarControlAffineCleanupCount(
            cleanups.len(),
        ));
    }
    if !scalar_control_affine_cleanups_are_canonical(cleanups, function.byte_count) {
        return Err(invalid());
    }
    for (leaf_ordinal, cleanup) in cleanups.iter().enumerate() {
        validate_scalar_affine_cleanup_shape(record, function, cleanup, false)?;
        if record
            .semantic_code_attribution
            .iter()
            .filter(|attribution| {
                attribution.machine == function.machine
                    && attribution.attribution.site == SemanticCodeSite::Edge(cleanup.psi_edge)
                    && attribution.attribution.operation_ordinal == leaf_ordinal
                    && attribution.attribution.code_offset == cleanup.code_offset
                    && attribution.attribution.byte_count == cleanup.byte_count
            })
            .count()
            != 1
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn scalar_control_affine_cleanups_are_canonical(
    cleanups: &[omega_machine_code::UnitAffineCleanupRecord],
    function_byte_count: usize,
) -> bool {
    let Some(first) = cleanups.first() else {
        return false;
    };
    let edges = cleanups
        .iter()
        .map(|cleanup| cleanup.psi_edge)
        .collect::<std::collections::BTreeSet<_>>();
    edges.len() == cleanups.len()
        && cleanups.iter().all(|cleanup| {
            cleanup.locals.is_empty()
                && cleanup.structural_types == first.structural_types
                && cleanup.actions == first.actions
                && cleanup.byte_count == first.byte_count
        })
        && cleanups.windows(2).all(|pair| {
            pair[0]
                .code_offset
                .checked_add(pair[0].byte_count)
                .is_some_and(|end| end <= pair[1].code_offset)
        })
        && cleanups.last().is_some_and(|last| {
            last.code_offset.checked_add(last.byte_count) == Some(function_byte_count)
        })
}

fn bounded_nominal_receiver_shape(shape: ValueShape) -> bool {
    shape == ValueShape::integer(0, 1)
        || shape.class == ValueClass::Integer
            && shape.byte_size != 0
            && matches!(shape.alignment, 1 | 2 | 4 | 8)
            && shape.byte_size % shape.alignment == 0
}

fn encode_structural_types(
    bytes: &mut Vec<u8>,
    declarations: &[psi_terminal::StructuralTypeDeclaration],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(declarations.len()).map_err(|_| InstallationError::TooManyStructuralTypes)?,
    );
    for declaration in declarations {
        push_u64(bytes, declaration.id.get());
        encode_identity(bytes, &declaration.identity)?;
        match &declaration.shape {
            psi_terminal::StructuralTypeShape::PrimitiveScalar(_) => {
                return Err(InstallationError::UnsupportedStructuralPrimitiveScalar(
                    declaration.id,
                ));
            }
            psi_terminal::StructuralTypeShape::ByteSequence(carrier) => {
                bytes.extend_from_slice(&[4, 0, 0, 0]);
                match carrier {
                    psi_terminal::ByteSequenceCarrier::BorrowedView => {
                        bytes.extend_from_slice(&[1, 0, 0, 0]);
                        push_u64(bytes, 0);
                    }
                    psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity } => {
                        bytes.extend_from_slice(&[2, 0, 0, 0]);
                        push_u64(bytes, *capacity);
                    }
                }
            }
            psi_terminal::StructuralTypeShape::Record { fields } => {
                bytes.extend_from_slice(&[1, 0, 0, 0]);
                encode_structural_fields(bytes, fields)?;
            }
            psi_terminal::StructuralTypeShape::FixedArray { element, length } => {
                bytes.extend_from_slice(&[2, 0, 0, 0]);
                push_u64(bytes, element.get());
                push_u64(bytes, *length);
            }
            psi_terminal::StructuralTypeShape::Sum { cases } => {
                bytes.extend_from_slice(&[3, 0, 0, 0]);
                encode_structural_cases(bytes, cases)?;
            }
            psi_terminal::StructuralTypeShape::Mixed { fields, cases } => {
                bytes.extend_from_slice(&[5, 0, 0, 0]);
                encode_structural_fields(bytes, fields)?;
                encode_structural_cases(bytes, cases)?;
            }
        }
    }
    Ok(())
}

fn decode_structural_types(
    reader: &mut Reader<'_>,
) -> Result<Vec<psi_terminal::StructuralTypeDeclaration>, InstallationError> {
    let count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyStructuralTypes)?;
    if count > reader.remaining() {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut declarations = Vec::with_capacity(count);
    for _ in 0..count {
        let id = StructuralTypeId::new(reader.u64()?).ok_or(
            InstallationError::ZeroStructuralReturnIdentity("structural type"),
        )?;
        let identity = decode_identity(reader)?;
        let shape_tag = reader.u8()?;
        if reader.u8()? != 0 || reader.u8()? != 0 || reader.u8()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        let shape = match shape_tag {
            1 => psi_terminal::StructuralTypeShape::Record {
                fields: decode_structural_fields(reader)?,
            },
            2 => psi_terminal::StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(reader.u64()?).ok_or(
                    InstallationError::ZeroStructuralReturnIdentity("fixed-array element type"),
                )?,
                length: reader.u64()?,
            },
            3 => psi_terminal::StructuralTypeShape::Sum {
                cases: decode_structural_cases(reader)?,
            },
            4 => {
                let carrier_tag = reader.u8()?;
                if reader.u8()? != 0 || reader.u8()? != 0 || reader.u8()? != 0 {
                    return Err(InstallationError::NonzeroReservedField);
                }
                let capacity = reader.u64()?;
                psi_terminal::StructuralTypeShape::ByteSequence(match carrier_tag {
                    1 if capacity == 0 => psi_terminal::ByteSequenceCarrier::BorrowedView,
                    2 => psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity },
                    tag => {
                        return Err(InstallationError::InvalidStructuralTypeShapeTag(tag));
                    }
                })
            }
            5 => psi_terminal::StructuralTypeShape::Mixed {
                fields: decode_structural_fields(reader)?,
                cases: decode_structural_cases(reader)?,
            },
            tag => {
                return Err(InstallationError::InvalidStructuralTypeShapeTag(tag));
            }
        };
        declarations.push(psi_terminal::StructuralTypeDeclaration {
            id,
            identity,
            shape,
        });
    }
    Ok(declarations)
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
    TooManyInternalUnitCallArguments,
    TooManyInternalUnitScalarCallArguments,
    TooManyInternalUnitCallClaims,
    TooManyScalarCallPlanValues,
    TooManyScalarCallPlanRegisters,
    TooManyUnitScalarHomes,
    TooManyUnitIntegerConstants,
    TooManyUnitStructuralScalarFieldStores,
    TooManyUnitStructuralScalarFieldStoreBytes,
    TooManyStructuralReturnParameters,
    TooManyStructuralReturnClaims,
    TooManyStructuralReturnCleanups,
    TooManyStructuralTypes,
    UnsupportedStructuralPrimitiveScalar(StructuralTypeId),
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
    InvalidDynamicScalarCall(MachineId),
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

#[cfg(test)]
mod resource_tests {
    use super::*;
    use super::{
        function_affine_cleanup_codec::{
            decode_scalar_control_affine_cleanups, decode_unit_affine_cleanup,
            encode_scalar_control_affine_cleanups,
        },
        function_stack_codec::{decode_function_stack_facts, encode_function_stack_facts},
    };
    use psi_core::{EdgeId, PlaceId, StructuralCaseId, StructuralFieldId};

    fn installed_function_with_unit_call() -> InstalledFunction {
        InstalledFunction {
            machine: MachineId::new(1).expect("function"),
            attachment: None,
            fixed_integer_scalar_abi: None,
            text_offset: 24,
            byte_count: 16,
            unit_stack: Some(crate::ObjectUnitStack {
                frame_bytes: 0,
                local_peak_bytes: 16,
                stack_alignment: 16,
            }),
            scalar_stack: None,
            unit_call_stacks: vec![crate::ObjectUnitCallStack {
                owner: CallSiteOwner::Operation(OperationId::new(1).expect("call operation")),
                target: MachineId::new(2).expect("callee"),
                text_offset: 28,
                active_frame_bytes: 0,
                transient_bytes: 16,
                caller_live_bytes: 16,
            }],
            scalar_call_stacks: Vec::new(),
            foreign_call_stacks: vec![InstalledForeignCallStack {
                owner: CallSiteOwner::Operation(OperationId::new(2).expect("foreign operation")),
                text_offset: 32,
                caller_live_bytes: 16,
                provider_plan_report_identity: 7,
                contribution_report_identity:
                    omega_task_plans::AdmittedStackContributionReportId::from_normalized_identity(8)
                        .expect("contribution report"),
                contribution_commitment:
                    omega_task_plans::SameStackContributionCommitment::from_digest([9; 32]),
                contribution_bytes: 64,
                contribution_alignment: 16,
            }],
            unit_body: false,
            ranked_u32_countdown: false,
            unit_parameters: Vec::new(),
            unit_parameter_homes: Vec::new(),
            unit_scalar_homes: Vec::new(),
            unit_integer_constants: Vec::new(),
            unit_structural_scalar_field_stores: Vec::new(),
            unit_affine_cleanup: None,
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
        }
    }

    fn scalar_control_cleanup(
        edge: u64,
        code_offset: usize,
    ) -> omega_machine_code::UnitAffineCleanupRecord {
        omega_machine_code::UnitAffineCleanupRecord {
            psi_edge: EdgeId::new(edge).expect("cleanup edge"),
            structural_types: Vec::new(),
            locals: Vec::new(),
            actions: vec![psi_terminal::TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(1).expect("cleanup place"),
            )],
            code_offset,
            byte_count: 4,
        }
    }

    fn scalar_control_object_cleanup(
        edge: u64,
        code_offset: usize,
    ) -> omega_machine_code::ScalarControlAffineCleanupRecord {
        omega_machine_code::ScalarControlAffineCleanupRecord {
            cleanup: scalar_control_cleanup(edge, code_offset),
            preservation: omega_machine_code::ScalarCleanupPreservationEvidence {
                frame: omega_machine_code::StackAdjustmentPair {
                    byte_size: 16,
                    allocation_offset: code_offset,
                    allocation_byte_count: 4,
                    release_offset: code_offset + 3,
                    release_byte_count: 1,
                },
                result_byte_offset: 0,
                result_store_offset: code_offset + 1,
                result_load_offset: code_offset + 2,
                aarch64_return_link: None,
            },
        }
    }

    #[test]
    fn cleanup_decoder_rejects_impossible_capacity_before_allocation() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            decode_unit_affine_cleanup(&mut reader),
            Err(InstallationError::UnexpectedEnd)
        );
    }

    #[test]
    fn scalar_control_cleanup_codec_accepts_finite_leaf_sets() {
        for count in [0_usize, 2, 3, 4] {
            let cleanups = (0..count)
                .map(|index| scalar_control_cleanup(index as u64 + 1, index * 8))
                .collect::<Vec<_>>();
            let mut bytes = Vec::new();
            encode_scalar_control_affine_cleanups(&mut bytes, &cleanups)
                .expect("encode finite cleanup leaf set");
            let mut reader = Reader::new(&bytes);
            assert_eq!(
                decode_scalar_control_affine_cleanups(&mut reader)
                    .expect("decode finite cleanup leaf set"),
                cleanups
            );
            assert_eq!(reader.remaining(), 0);
        }

        let invalid = vec![scalar_control_cleanup(1, 0)];
        assert_eq!(
            encode_scalar_control_affine_cleanups(&mut Vec::new(), &invalid),
            Err(InstallationError::InvalidScalarControlAffineCleanupCount(1))
        );
        let mut encoded_count = Vec::new();
        push_u32(&mut encoded_count, 1);
        assert_eq!(
            decode_scalar_control_affine_cleanups(&mut Reader::new(&encoded_count)),
            Err(InstallationError::InvalidScalarControlAffineCleanupCount(1))
        );

        let mut impossible_capacity = Vec::new();
        push_u32(&mut impossible_capacity, u32::MAX);
        assert_eq!(
            decode_scalar_control_affine_cleanups(&mut Reader::new(&impossible_capacity)),
            Err(InstallationError::UnexpectedEnd)
        );
    }

    #[test]
    fn scalar_control_cleanup_canonicality_rejects_edge_and_interval_corruption() {
        let cleanups = vec![
            scalar_control_cleanup(1, 4),
            scalar_control_cleanup(2, 12),
            scalar_control_cleanup(3, 20),
            scalar_control_cleanup(4, 28),
        ];
        assert!(scalar_control_affine_cleanups_are_canonical(&cleanups, 32));

        let mut duplicate_edge = cleanups.clone();
        duplicate_edge[1].psi_edge = duplicate_edge[0].psi_edge;
        assert!(!scalar_control_affine_cleanups_are_canonical(
            &duplicate_edge,
            32
        ));

        let mut overlapping = cleanups.clone();
        overlapping[1].code_offset = 7;
        assert!(!scalar_control_affine_cleanups_are_canonical(
            &overlapping,
            32
        ));

        let mut reordered = cleanups.clone();
        reordered.swap(0, 1);
        assert!(!scalar_control_affine_cleanups_are_canonical(
            &reordered, 32
        ));

        let mut changed_actions = cleanups.clone();
        changed_actions[2].actions.clear();
        assert!(!scalar_control_affine_cleanups_are_canonical(
            &changed_actions,
            32
        ));

        assert!(!scalar_control_affine_cleanups_are_canonical(&cleanups, 33));
    }

    #[test]
    fn installed_scalar_control_cleanup_projection_binds_the_exact_object_records() {
        let emitted = vec![
            scalar_control_object_cleanup(1, 4),
            scalar_control_object_cleanup(2, 12),
            scalar_control_object_cleanup(3, 20),
        ];
        let mut installed = emitted
            .iter()
            .map(|record| record.cleanup.clone())
            .collect::<Vec<_>>();
        assert!(installed_scalar_control_cleanups_match_object(
            &installed, &emitted
        ));

        installed[1].psi_edge = EdgeId::new(9).expect("different edge");
        assert!(!installed_scalar_control_cleanups_match_object(
            &installed, &emitted
        ));
        installed[1] = emitted[1].cleanup.clone();
        installed[2].actions.clear();
        assert!(!installed_scalar_control_cleanups_match_object(
            &installed, &emitted
        ));
    }

    #[test]
    fn stack_fact_codec_round_trips_exact_emitter_evidence() {
        let function = installed_function_with_unit_call();
        let mut bytes = Vec::new();
        encode_function_stack_facts(&mut bytes, &function).expect("encode stack facts");
        let mut reader = Reader::new(&bytes);
        let (unit, scalar, unit_calls, scalar_calls, foreign_calls) =
            decode_function_stack_facts(&mut reader).expect("decode stack facts");
        assert_eq!(unit, function.unit_stack);
        assert_eq!(scalar, function.scalar_stack);
        assert_eq!(unit_calls, function.unit_call_stacks);
        assert_eq!(scalar_calls, function.scalar_call_stacks);
        assert_eq!(foreign_calls, function.foreign_call_stacks);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn installed_stack_fact_shape_rejects_nonlocal_or_forged_call_inputs() {
        let functions = [
            (MachineId::new(1).expect("caller"), None),
            (MachineId::new(2).expect("callee"), None),
        ]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
        let valid = installed_function_with_unit_call();
        assert!(installed_stack_facts_are_canonical(&valid, &functions));

        let mut nonlocal_offset = valid.clone();
        nonlocal_offset.unit_call_stacks[0].text_offset =
            nonlocal_offset.text_offset + nonlocal_offset.byte_count;
        assert!(!installed_stack_facts_are_canonical(
            &nonlocal_offset,
            &functions
        ));

        let mut forged_live_bytes = valid.clone();
        forged_live_bytes.unit_call_stacks[0].caller_live_bytes += 1;
        assert!(!installed_stack_facts_are_canonical(
            &forged_live_bytes,
            &functions
        ));

        let mut zero_provider_plan = valid.clone();
        zero_provider_plan.foreign_call_stacks[0].provider_plan_report_identity = 0;
        assert!(!installed_stack_facts_are_canonical(
            &zero_provider_plan,
            &functions
        ));

        let mut unsupported_foreign_alignment = valid.clone();
        unsupported_foreign_alignment.foreign_call_stacks[0].contribution_alignment = 32;
        assert!(!installed_stack_facts_are_canonical(
            &unsupported_foreign_alignment,
            &functions
        ));

        let mut zero_foreign_commitment = valid.clone();
        zero_foreign_commitment.foreign_call_stacks[0].contribution_commitment =
            omega_task_plans::SameStackContributionCommitment::from_digest([0; 32]);
        assert!(!installed_stack_facts_are_canonical(
            &zero_foreign_commitment,
            &functions
        ));

        let mut invalid_alignment = valid;
        invalid_alignment
            .unit_stack
            .as_mut()
            .expect("unit stack")
            .stack_alignment = 3;
        assert!(!installed_stack_facts_are_canonical(
            &invalid_alignment,
            &functions
        ));
    }

    #[test]
    fn previous_installation_marker_is_not_accepted() {
        let mut bytes = MAGIC.to_vec();
        let previous_marker = INSTALLATION_FORMAT_MARKER - 1;
        push_u16(&mut bytes, previous_marker);
        assert_eq!(
            decode_installation_record(&bytes),
            Err(InstallationError::UnsupportedFormatMarker(previous_marker))
        );
    }

    #[test]
    fn ieee_structural_and_scalar_field_formats_round_trip_in_installations() {
        let declarations = vec![psi_terminal::StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).expect("structural type"),
            identity: "Samples".into(),
            shape: psi_terminal::StructuralTypeShape::Record {
                fields: vec![
                    psi_terminal::StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).expect("f32 field"),
                        identity: "single".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::IeeeFloat(
                            psi_core::IeeeFloatFormat::Binary32,
                        ),
                    },
                    psi_terminal::StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).expect("f64 field"),
                        identity: "double".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::IeeeFloat(
                            psi_core::IeeeFloatFormat::Binary64,
                        ),
                    },
                    psi_terminal::StructuralFieldDeclaration {
                        id: StructuralFieldId::new(3).expect("scalar f32 field"),
                        identity: "scalar_single".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::Scalar(
                            psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32),
                        ),
                    },
                    psi_terminal::StructuralFieldDeclaration {
                        id: StructuralFieldId::new(4).expect("scalar f64 field"),
                        identity: "scalar_double".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::Scalar(
                            psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64),
                        ),
                    },
                ],
            },
        }];
        let mut bytes = Vec::new();
        encode_structural_types(&mut bytes, &declarations).expect("encode structural types");
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            decode_structural_types(&mut reader),
            Ok(declarations.clone())
        );
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn byte_sequence_carriers_round_trip_in_installations() {
        let declarations = vec![psi_terminal::StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).expect("structural type"),
            identity: "TextFields".into(),
            shape: psi_terminal::StructuralTypeShape::Record {
                fields: vec![
                    psi_terminal::StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).expect("borrowed field"),
                        identity: "borrowed".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::ByteSequence(
                            psi_terminal::ByteSequenceCarrier::BorrowedView,
                        ),
                    },
                    psi_terminal::StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).expect("bounded field"),
                        identity: "bounded".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::ByteSequence(
                            psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                        ),
                    },
                ],
            },
        }];
        let mut bytes = Vec::new();
        encode_structural_types(&mut bytes, &declarations).expect("encode structural types");
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_structural_types(&mut reader), Ok(declarations));
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn payload_sum_case_fields_round_trip_in_installations() {
        let declarations = vec![psi_terminal::StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).expect("structural type"),
            identity: "Mode".into(),
            shape: psi_terminal::StructuralTypeShape::Sum {
                cases: vec![
                    psi_terminal::StructuralCaseDeclaration {
                        id: StructuralCaseId::new(1).expect("off case"),
                        identity: "Off".into(),
                        fields: Vec::new(),
                    },
                    psi_terminal::StructuralCaseDeclaration {
                        id: StructuralCaseId::new(2).expect("on case"),
                        identity: "On".into(),
                        fields: vec![psi_terminal::StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).expect("payload field"),
                            identity: "value".into(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: psi_terminal::StructuralFieldType::Scalar(
                                psi_core::ScalarType::Integer(
                                    psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
                                        .expect("i32"),
                                ),
                            ),
                        }],
                    },
                ],
            },
        }];
        let mut bytes = Vec::new();
        encode_structural_types(&mut bytes, &declarations).expect("encode structural sum");
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_structural_types(&mut reader), Ok(declarations));
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn mixed_common_fields_and_cases_round_trip_in_installations() {
        let declarations = vec![psi_terminal::StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).expect("structural type"),
            identity: "Message".into(),
            shape: psi_terminal::StructuralTypeShape::Mixed {
                fields: vec![psi_terminal::StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).expect("common field"),
                    identity: "active".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: psi_terminal::StructuralFieldType::Scalar(
                        psi_core::ScalarType::Boolean,
                    ),
                }],
                cases: vec![
                    psi_terminal::StructuralCaseDeclaration {
                        id: StructuralCaseId::new(1).expect("empty case"),
                        identity: "Empty".into(),
                        fields: Vec::new(),
                    },
                    psi_terminal::StructuralCaseDeclaration {
                        id: StructuralCaseId::new(2).expect("data case"),
                        identity: "Data".into(),
                        fields: vec![psi_terminal::StructuralFieldDeclaration {
                            id: StructuralFieldId::new(2).expect("payload field"),
                            identity: "value".into(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: psi_terminal::StructuralFieldType::Scalar(
                                psi_core::ScalarType::Integer(
                                    psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
                                        .expect("i32"),
                                ),
                            ),
                        }],
                    },
                ],
            },
        }];
        let mut bytes = Vec::new();
        encode_structural_types(&mut bytes, &declarations).expect("encode mixed structural type");
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_structural_types(&mut reader), Ok(declarations));
        assert_eq!(reader.remaining(), 0);
    }
}
