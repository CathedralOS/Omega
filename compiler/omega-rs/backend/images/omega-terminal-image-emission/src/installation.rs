use std::num::NonZeroU64;

use omega_calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValueClass, ValueLocation, ValuePlacement,
    ValueShape, evaluate_call_plan,
};
use omega_image::CompilerTextValidationEvidence;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_machine_code::{
    TerminalBoundarySettlementRecord, TerminalNativeFuelAttribution, TerminalNativeFuelSite,
    TerminalPortEffectRecord, TerminalProviderExecutionRecord, TerminalStructuralReturnRecord,
};
use omega_terminal_target_operations::TerminalMetadataOnlyPortRealization;
use psi_core::{
    BoundaryMachineId, ClaimId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId,
    ProfileDecisionId, ServiceId, StructuralDomainId, StructuralTypeId,
};
use psi_terminal::{
    CompletionReceipt, SemanticFingerprint, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeShape, TerminalPsiIdentity, VocabularyMarker,
};
use psi_terminal_fuel::TerminalFuelSchedule;
use sha2::{Digest, Sha256};

use crate::{
    TerminalExecutableImage, TerminalObjectBoundarySettlement, TerminalObjectFuelAttribution,
    TerminalObjectPortEffect, can_emit_terminal_executable_image,
};

pub const TERMINAL_INSTALLATION_FORMAT_MARKER: u16 = 11;
const MAGIC: &[u8; 8] = b"PSIINST\0";
const IMAGE_DOMAIN: &[u8] = b"omega-terminal-installed-image\0";
const RECORD_DOMAIN: &[u8] = b"omega-terminal-installation-record\0";

/// Exact normalized identity of one provider plan selected for this
/// installation. The current scalar canaries have an empty provider closure;
/// later call/boundary slices populate this set from their selected plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedProviderPlanIdentity(NonZeroU64);

impl SelectedProviderPlanIdentity {
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalImageFingerprint([u8; 32]);

impl TerminalImageFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for TerminalImageFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for TerminalImageFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalInstallationFingerprint([u8; 32]);

impl TerminalInstallationFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for TerminalInstallationFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for TerminalInstallationFingerprint {
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
pub struct TerminalInstallationRecord {
    terminal_psi: TerminalPsiIdentity,
    target: NativeTarget,
    subsystem: Option<u16>,
    profile_decision: ProfileDecisionId,
    selected_provider_plans: Vec<SelectedProviderPlanIdentity>,
    functions: Vec<TerminalInstalledFunction>,
    structural_returns: Vec<TerminalInstalledStructuralReturn>,
    internal_unit_calls: Vec<TerminalInstalledInternalUnitCall>,
    fuel_attribution: Vec<TerminalObjectFuelAttribution>,
    port_effects: Vec<TerminalObjectPortEffect>,
    boundary_settlements: Vec<TerminalObjectBoundarySettlement>,
    image: TerminalImageFingerprint,
    compiler_text_validation: CompilerTextValidationEvidence,
}

impl TerminalInstallationRecord {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
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

    pub fn selected_provider_plans(&self) -> &[SelectedProviderPlanIdentity] {
        &self.selected_provider_plans
    }

    pub fn boundary_settlements(&self) -> &[TerminalObjectBoundarySettlement] {
        &self.boundary_settlements
    }

    pub fn functions(&self) -> &[TerminalInstalledFunction] {
        &self.functions
    }

    pub fn structural_returns(&self) -> &[TerminalInstalledStructuralReturn] {
        &self.structural_returns
    }

    pub fn internal_unit_calls(&self) -> &[TerminalInstalledInternalUnitCall] {
        &self.internal_unit_calls
    }

    pub fn fuel_attribution(&self) -> &[TerminalObjectFuelAttribution] {
        &self.fuel_attribution
    }

    pub fn port_effects(&self) -> &[TerminalObjectPortEffect] {
        &self.port_effects
    }

    pub const fn image(&self) -> TerminalImageFingerprint {
        self.image
    }

    pub const fn compiler_text_validation(&self) -> CompilerTextValidationEvidence {
        self.compiler_text_validation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInstalledFunction {
    pub machine: MachineId,
    pub text_offset: usize,
    pub byte_count: usize,
    pub unit_body: bool,
    pub unit_parameters: Vec<omega_terminal_machine_code::TerminalUnitParameterRecord>,
    pub unit_parameter_homes: Vec<omega_terminal_machine_code::TerminalUnitParameterHomeRecord>,
    pub unit_affine_cleanup: Option<omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInstalledStructuralReturn {
    pub machine: MachineId,
    pub returned: TerminalStructuralReturnRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInstalledInternalUnitCall {
    pub machine: MachineId,
    pub text_offset: usize,
    pub custody: omega_terminal_machine_code::TerminalInternalUnitCallRecord,
}

/// Build the canonical installation record for an emitted image.
///
/// This convenience path succeeds only when the image has no provider-backed
/// settlements. Effectful images must use the admission-bearing constructor.
pub fn build_terminal_installation_record(
    image: &TerminalExecutableImage,
    profile_decision: ProfileDecisionId,
) -> Result<TerminalInstallationRecord, TerminalInstallationError> {
    build_terminal_installation_record_with_provider_executions(image, profile_decision, [])
}

/// Build an installation record from the same ledger-owned provider
/// executions consumed by effectful terminal lowering.
///
/// The execution closure must match the image's retained settlement evidence
/// exactly. Numeric provider-plan identities are derived here and cannot be
/// supplied independently by the caller.
pub fn build_terminal_installation_record_with_provider_executions<'execution>(
    image: &TerminalExecutableImage,
    profile_decision: ProfileDecisionId,
    provider_executions: impl IntoIterator<Item = &'execution omega_external_roots::ProviderExecution>,
) -> Result<TerminalInstallationRecord, TerminalInstallationError> {
    let compiler_text_validation = image
        .output()
        .compiler_text_validation
        .ok_or(TerminalInstallationError::MissingCompilerTextValidation)?;
    let mut admitted_executions = std::collections::BTreeSet::new();
    let mut selected_provider_plans = std::collections::BTreeSet::new();
    for execution in provider_executions {
        let admitted = execution.terminal_binding();
        if !admitted_executions.insert((
            admitted.provider_plan(),
            admitted.provider_execution_identity(),
            admitted.provider_execution_fingerprint(),
            admitted.normalized_root_identity(),
            admitted.boundary_contract_fingerprint(),
        )) {
            return Err(TerminalInstallationError::DuplicateProviderExecution);
        }
        selected_provider_plans.insert(
            SelectedProviderPlanIdentity::new(admitted.provider_plan())
                .ok_or(TerminalInstallationError::ZeroProviderPlan)?,
        );
    }
    let required_executions = image
        .boundary_settlements()
        .iter()
        .map(|installed| {
            let execution = installed.settlement.provider_execution;
            (
                execution.provider_plan,
                execution.provider_execution_identity,
                execution.provider_execution_fingerprint,
                execution.normalized_root_identity,
                execution.boundary_contract_fingerprint,
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    if admitted_executions != required_executions {
        return Err(TerminalInstallationError::ProviderExecutionClosureMismatch);
    }
    let record = TerminalInstallationRecord {
        terminal_psi: image.terminal_psi(),
        target: image.target(),
        subsystem: image.subsystem(),
        profile_decision,
        selected_provider_plans: selected_provider_plans.into_iter().collect(),
        functions: image
            .functions()
            .iter()
            .map(|function| TerminalInstalledFunction {
                machine: function.machine,
                text_offset: function.text_offset,
                byte_count: function.byte_count,
                unit_body: function.unit_stack.is_some(),
                unit_parameters: function.unit_parameters.clone(),
                unit_parameter_homes: function.unit_parameter_homes.clone(),
                unit_affine_cleanup: function.unit_affine_cleanup.clone(),
            })
            .collect(),
        structural_returns: image
            .functions()
            .iter()
            .filter_map(|function| {
                function.structural_return.clone().map(|returned| {
                    TerminalInstalledStructuralReturn {
                        machine: function.machine,
                        returned,
                    }
                })
            })
            .collect(),
        internal_unit_calls: image
            .functions()
            .iter()
            .flat_map(|function| {
                function.internal_unit_calls.iter().cloned().map(|custody| {
                    TerminalInstalledInternalUnitCall {
                        machine: function.machine,
                        text_offset: function.text_offset + custody.code_offset,
                        custody,
                    }
                })
            })
            .collect(),
        fuel_attribution: image.fuel_attribution().to_vec(),
        port_effects: image.port_effects().to_vec(),
        boundary_settlements: image.boundary_settlements().to_vec(),
        image: fingerprint_image(&image.output().bytes),
        compiler_text_validation,
    };
    validate_record_shape(&record)?;
    Ok(record)
}

pub fn encode_terminal_installation_record(
    record: &TerminalInstallationRecord,
) -> Result<Vec<u8>, TerminalInstallationError> {
    validate_record_shape(record)?;
    let provider_count = u32::try_from(record.selected_provider_plans.len())
        .map_err(|_| TerminalInstallationError::TooManyProviderPlans)?;
    let settlement_count = u32::try_from(record.boundary_settlements.len())
        .map_err(|_| TerminalInstallationError::TooManyBoundarySettlements)?;
    let function_count = u32::try_from(record.functions.len())
        .map_err(|_| TerminalInstallationError::TooManyInstalledFunctions)?;
    let structural_return_count = u32::try_from(record.structural_returns.len())
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturns)?;
    let internal_unit_call_count = u32::try_from(record.internal_unit_calls.len())
        .map_err(|_| TerminalInstallationError::TooManyInternalUnitCalls)?;
    let fuel_attribution_count = u32::try_from(record.fuel_attribution.len())
        .map_err(|_| TerminalInstallationError::TooManyFuelAttributions)?;
    let port_effect_count = u32::try_from(record.port_effects.len())
        .map_err(|_| TerminalInstallationError::TooManyPortEffects)?;
    let text_relocation_count =
        u64::try_from(record.compiler_text_validation.text_relocation_count)
            .map_err(|_| TerminalInstallationError::CountNotRepresentable("text relocations"))?;
    let checked_instruction_validation_count = u64::try_from(
        record
            .compiler_text_validation
            .checked_instruction_validation_count,
    )
    .map_err(|_| TerminalInstallationError::CountNotRepresentable("checked instructions"))?;

    let mut bytes = Vec::with_capacity(166 + record.selected_provider_plans.len() * 8);
    bytes.extend_from_slice(MAGIC);
    push_u16(&mut bytes, TERMINAL_INSTALLATION_FORMAT_MARKER);
    push_u16(&mut bytes, record.terminal_psi.vocabulary_marker.get());
    bytes.extend_from_slice(record.terminal_psi.program_fingerprint.as_bytes());
    bytes.push(architecture_tag(record.target.architecture));
    bytes.push(object_format_tag(record.target.object_format));
    bytes.push(u8::from(record.subsystem.is_some()));
    bytes.push(0);
    push_u64(
        &mut bytes,
        u64::try_from(record.target.pointer_size)
            .map_err(|_| TerminalInstallationError::TargetPointerFactNotRepresentable)?,
    );
    push_u64(
        &mut bytes,
        u64::try_from(record.target.pointer_alignment)
            .map_err(|_| TerminalInstallationError::TargetPointerFactNotRepresentable)?,
    );
    push_u16(&mut bytes, record.subsystem.unwrap_or(0));
    push_u16(&mut bytes, 0);
    push_u64(&mut bytes, record.profile_decision.get());
    bytes.extend_from_slice(record.image.as_bytes());
    push_u64(
        &mut bytes,
        record.compiler_text_validation.encoded_text_fingerprint,
    );
    push_u64(
        &mut bytes,
        record
            .compiler_text_validation
            .final_compiler_text_fingerprint,
    );
    push_u64(
        &mut bytes,
        record
            .compiler_text_validation
            .relocation_envelope_fingerprint,
    );
    push_u64(
        &mut bytes,
        record
            .compiler_text_validation
            .checked_instruction_validation_fingerprint,
    );
    push_u64(
        &mut bytes,
        record
            .compiler_text_validation
            .checked_instruction_footprint_fingerprint,
    );
    push_u64(
        &mut bytes,
        record.compiler_text_validation.derivation_fingerprint,
    );
    push_u64(&mut bytes, text_relocation_count);
    push_u64(&mut bytes, checked_instruction_validation_count);
    push_u32(&mut bytes, provider_count);
    for provider in &record.selected_provider_plans {
        push_u64(&mut bytes, provider.get());
    }
    push_u32(&mut bytes, function_count);
    for function in &record.functions {
        push_u64(&mut bytes, function.machine.get());
        push_u64(
            &mut bytes,
            u64::try_from(function.text_offset)
                .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?,
        );
        push_u64(
            &mut bytes,
            u64::try_from(function.byte_count)
                .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?,
        );
        bytes.push(u8::from(function.unit_body));
        bytes.extend_from_slice(&[0; 3]);
        push_u32(
            &mut bytes,
            u32::try_from(function.unit_parameters.len())
                .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?,
        );
        for parameter in &function.unit_parameters {
            push_u64(&mut bytes, parameter.place.get());
            push_u64(&mut bytes, parameter.structural_type.get());
            bytes.push(multiplicity_tag(parameter.multiplicity));
            bytes.extend_from_slice(&[0; 3]);
            encode_shape(&mut bytes, parameter.shape)?;
        }
        push_u32(
            &mut bytes,
            u32::try_from(function.unit_parameter_homes.len())
                .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?,
        );
        for home in &function.unit_parameter_homes {
            push_u64(&mut bytes, home.place.get());
            push_u64(&mut bytes, home.structural_type.get());
            bytes.push(multiplicity_tag(home.multiplicity));
            bytes.extend_from_slice(&[0; 3]);
            encode_shape(&mut bytes, home.shape)?;
            encode_direct_placement(&mut bytes, &home.source)?;
            push_u32(&mut bytes, home.byte_offset);
            bytes.push(u8::from(home.indirect));
            bytes.extend_from_slice(&[0; 3]);
        }
        match &function.unit_affine_cleanup {
            Some(cleanup) => {
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                encode_unit_affine_cleanup(&mut bytes, cleanup)?;
            }
            None => {
                bytes.push(0);
                bytes.extend_from_slice(&[0; 3]);
            }
        }
    }
    push_u32(&mut bytes, structural_return_count);
    for installed in &record.structural_returns {
        encode_structural_return(&mut bytes, installed)?;
    }
    push_u32(&mut bytes, internal_unit_call_count);
    for installed in &record.internal_unit_calls {
        encode_internal_unit_call(&mut bytes, installed)?;
    }
    push_u32(&mut bytes, fuel_attribution_count);
    for installed in &record.fuel_attribution {
        let attribution = &installed.attribution;
        push_u64(&mut bytes, installed.machine.get());
        push_u32(&mut bytes, attribution.schedule.marker());
        match attribution.site {
            TerminalNativeFuelSite::Operation(operation) => {
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(&mut bytes, operation.get());
            }
            TerminalNativeFuelSite::Edge(edge) => {
                bytes.push(2);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(&mut bytes, edge.get());
            }
        }
        push_u64(&mut bytes, attribution.units);
        push_u64(
            &mut bytes,
            u64::try_from(attribution.operation_ordinal)
                .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            &mut bytes,
            u64::try_from(installed.text_offset)
                .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            &mut bytes,
            u64::try_from(attribution.code_offset)
                .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
        push_u64(
            &mut bytes,
            u64::try_from(attribution.byte_count)
                .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?,
        );
    }
    push_u32(&mut bytes, port_effect_count);
    for installed in &record.port_effects {
        let effect = &installed.effect;
        push_u64(&mut bytes, installed.machine.get());
        push_u64(&mut bytes, effect.psi_operation.get());
        push_u64(&mut bytes, effect.service.get());
        push_u16(&mut bytes, effect.port);
        bytes.push(effect.value);
        bytes.push(0);
        push_u64(
            &mut bytes,
            u64::try_from(effect.operation_ordinal)
                .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?,
        );
        push_u64(
            &mut bytes,
            u64::try_from(installed.text_offset)
                .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?,
        );
        push_u64(
            &mut bytes,
            u64::try_from(effect.code_offset)
                .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?,
        );
        push_u64(
            &mut bytes,
            u64::try_from(effect.byte_count)
                .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?,
        );
    }
    push_u32(&mut bytes, settlement_count);
    for installed in &record.boundary_settlements {
        let settlement = &installed.settlement;
        push_u64(&mut bytes, installed.machine.get());
        push_u64(&mut bytes, settlement.psi_operation.get());
        push_u64(&mut bytes, settlement.boundary.get());
        let execution = settlement.provider_execution;
        push_u64(&mut bytes, execution.provider_plan);
        push_u64(&mut bytes, execution.provider_execution_identity);
        push_u64(&mut bytes, execution.provider_execution_fingerprint);
        push_u64(&mut bytes, execution.normalized_root_identity);
        push_u64(&mut bytes, execution.boundary_contract_fingerprint);
        push_u64(&mut bytes, settlement.realization.effect_operation.get());
        push_u64(&mut bytes, settlement.realization.service.get());
        push_u16(&mut bytes, settlement.realization.port);
        bytes.push(settlement.realization.value);
        bytes.push(0);
        push_u64(
            &mut bytes,
            u64::try_from(settlement.operation_ordinal)
                .map_err(|_| TerminalInstallationError::SettlementOffsetNotRepresentable)?,
        );
        push_u64(
            &mut bytes,
            u64::try_from(installed.text_offset)
                .map_err(|_| TerminalInstallationError::SettlementOffsetNotRepresentable)?,
        );
        push_u64(
            &mut bytes,
            u64::try_from(settlement.code_offset)
                .map_err(|_| TerminalInstallationError::SettlementOffsetNotRepresentable)?,
        );
        push_u32(
            &mut bytes,
            u32::try_from(settlement.arguments.len())
                .map_err(|_| TerminalInstallationError::TooManySettlementArguments)?,
        );
        for argument in &settlement.arguments {
            encode_structural_argument(&mut bytes, argument)?;
        }
        push_u32(
            &mut bytes,
            u32::try_from(settlement.completion_receipts.len())
                .map_err(|_| TerminalInstallationError::TooManyCompletionReceipts)?,
        );
        for claim in &settlement.completion_receipts {
            push_u64(&mut bytes, claim.claim.get());
            push_u32(&mut bytes, claim.argument_index);
        }
    }
    Ok(bytes)
}

fn encode_structural_argument(
    bytes: &mut Vec<u8>,
    argument: &StructuralArgument,
) -> Result<(), TerminalInstallationError> {
    push_u64(bytes, argument.place.get());
    push_u32(
        bytes,
        u32::try_from(argument.path.len())
            .map_err(|_| TerminalInstallationError::TooManySettlementArgumentPathSegments)?,
    );
    for segment in &argument.path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                if identity.is_empty() {
                    return Err(TerminalInstallationError::InvalidSettlementArgumentField);
                }
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                push_u32(
                    bytes,
                    u32::try_from(identity.len())
                        .map_err(|_| TerminalInstallationError::SettlementArgumentFieldTooLong)?,
                );
                bytes.extend_from_slice(identity.as_bytes());
            }
            StructuralPathSegment::FixedIndex(index) => {
                bytes.push(2);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(bytes, *index);
            }
        }
    }
    Ok(())
}

pub fn decode_terminal_installation_record(
    bytes: &[u8],
) -> Result<TerminalInstallationRecord, TerminalInstallationError> {
    let mut reader = Reader::new(bytes);
    if reader.array::<8>()? != *MAGIC {
        return Err(TerminalInstallationError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != TERMINAL_INSTALLATION_FORMAT_MARKER {
        return Err(TerminalInstallationError::UnsupportedFormatMarker(
            format_marker,
        ));
    }
    let vocabulary_marker_raw = reader.u16()?;
    let vocabulary_marker = VocabularyMarker::new(vocabulary_marker_raw).ok_or(
        TerminalInstallationError::UnsupportedVocabularyMarker(vocabulary_marker_raw),
    )?;
    let program_fingerprint = SemanticFingerprint::from_bytes(reader.array()?);
    let architecture = decode_architecture(reader.u8()?)?;
    let object_format = decode_object_format(reader.u8()?)?;
    let subsystem_present = decode_boolean(reader.u8()?)?;
    if reader.u8()? != 0 {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    let pointer_size = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::TargetPointerFactNotRepresentable)?;
    let pointer_alignment = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::TargetPointerFactNotRepresentable)?;
    let subsystem_raw = reader.u16()?;
    if reader.u16()? != 0 {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    let subsystem = if subsystem_present {
        Some(subsystem_raw)
    } else {
        if subsystem_raw != 0 {
            return Err(TerminalInstallationError::NonCanonicalSubsystem);
        }
        None
    };
    let profile_decision = ProfileDecisionId::new(reader.u64()?)
        .ok_or(TerminalInstallationError::ZeroProfileDecision)?;
    let image = TerminalImageFingerprint(reader.array()?);
    let encoded_text_fingerprint = reader.u64()?;
    let final_compiler_text_fingerprint = reader.u64()?;
    let relocation_envelope_fingerprint = reader.u64()?;
    let checked_instruction_validation_fingerprint = reader.u64()?;
    let checked_instruction_footprint_fingerprint = reader.u64()?;
    let derivation_fingerprint = reader.u64()?;
    let text_relocation_count = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::CountNotRepresentable("text relocations"))?;
    let checked_instruction_validation_count = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::CountNotRepresentable("checked instructions"))?;
    let provider_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyProviderPlans)?;
    if provider_count > reader.remaining() / 8 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut selected_provider_plans = Vec::with_capacity(provider_count);
    for _ in 0..provider_count {
        let provider = SelectedProviderPlanIdentity::new(reader.u64()?)
            .ok_or(TerminalInstallationError::ZeroProviderPlan)?;
        if let Some(previous) = selected_provider_plans.last().copied()
            && previous >= provider
        {
            return Err(TerminalInstallationError::NonCanonicalProviderPlanOrder);
        }
        selected_provider_plans.push(provider);
    }
    let function_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyInstalledFunctions)?;
    if function_count > reader.remaining() / 24 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut functions = Vec::with_capacity(function_count);
    for _ in 0..function_count {
        let machine =
            MachineId::new(reader.u64()?).ok_or(TerminalInstallationError::ZeroFunctionIdentity)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?;
        let unit_body = decode_boolean(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        let parameter_count = usize::try_from(reader.u32()?)
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?;
        let mut unit_parameters = Vec::with_capacity(parameter_count);
        for _ in 0..parameter_count {
            let place = PlaceId::new(reader.u64()?).ok_or(
                TerminalInstallationError::ZeroStructuralReturnIdentity("Unit parameter place"),
            )?;
            let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
                TerminalInstallationError::ZeroStructuralReturnIdentity("Unit parameter type"),
            )?;
            let multiplicity = decode_multiplicity(reader.u8()?)?;
            if reader.take(3)? != [0; 3] {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            unit_parameters.push(omega_terminal_machine_code::TerminalUnitParameterRecord {
                place,
                structural_type,
                multiplicity,
                shape: decode_shape(&mut reader)?,
            });
        }
        let home_count = usize::try_from(reader.u32()?)
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?;
        let mut unit_parameter_homes = Vec::with_capacity(home_count);
        for _ in 0..home_count {
            let place = PlaceId::new(reader.u64()?).ok_or(
                TerminalInstallationError::ZeroStructuralReturnIdentity("Unit home place"),
            )?;
            let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
                TerminalInstallationError::ZeroStructuralReturnIdentity("Unit home type"),
            )?;
            let multiplicity = decode_multiplicity(reader.u8()?)?;
            if reader.take(3)? != [0; 3] {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            let shape = decode_shape(&mut reader)?;
            let source = decode_direct_placement(&mut reader)?;
            let byte_offset = reader.u32()?;
            let indirect = decode_boolean(reader.u8()?)?;
            if reader.take(3)? != [0; 3] {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            unit_parameter_homes.push(
                omega_terminal_machine_code::TerminalUnitParameterHomeRecord {
                    place,
                    structural_type,
                    multiplicity,
                    shape,
                    source,
                    byte_offset,
                    indirect,
                },
            );
        }
        functions.push(TerminalInstalledFunction {
            machine,
            text_offset,
            byte_count,
            unit_body,
            unit_parameters,
            unit_parameter_homes,
            unit_affine_cleanup: match reader.u8()? {
                0 => {
                    if reader.take(3)? != [0; 3] {
                        return Err(TerminalInstallationError::NonzeroReservedField);
                    }
                    None
                }
                1 => {
                    if reader.take(3)? != [0; 3] {
                        return Err(TerminalInstallationError::NonzeroReservedField);
                    }
                    Some(decode_unit_affine_cleanup(&mut reader)?)
                }
                tag => return Err(TerminalInstallationError::InvalidBoolean(tag)),
            },
        });
    }
    let structural_return_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturns)?;
    let mut structural_returns = Vec::with_capacity(structural_return_count);
    for _ in 0..structural_return_count {
        structural_returns.push(decode_structural_return(&mut reader)?);
    }
    let internal_unit_call_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyInternalUnitCalls)?;
    if internal_unit_call_count > reader.remaining() / 60 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut internal_unit_calls = Vec::with_capacity(internal_unit_call_count);
    for _ in 0..internal_unit_call_count {
        internal_unit_calls.push(decode_internal_unit_call(&mut reader)?);
    }
    let fuel_attribution_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyFuelAttributions)?;
    if fuel_attribution_count > reader.remaining() / 64 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut fuel_attribution = Vec::with_capacity(fuel_attribution_count);
    for _ in 0..fuel_attribution_count {
        let machine = MachineId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroFuelAttributionIdentity("MachineId"),
        )?;
        let schedule = FuelScheduleIdentity::new(reader.u32()?)
            .ok_or(TerminalInstallationError::ZeroFuelScheduleIdentity)?;
        let site_tag = reader.u8()?;
        if reader.take(3)? != [0; 3] {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        let site_identity = reader.u64()?;
        let site = match site_tag {
            1 => TerminalNativeFuelSite::Operation(OperationId::new(site_identity).ok_or(
                TerminalInstallationError::ZeroFuelAttributionIdentity("OperationId"),
            )?),
            2 => TerminalNativeFuelSite::Edge(EdgeId::new(site_identity).ok_or(
                TerminalInstallationError::ZeroFuelAttributionIdentity("EdgeId"),
            )?),
            _ => return Err(TerminalInstallationError::InvalidFuelSiteTag(site_tag)),
        };
        let units = reader.u64()?;
        let operation_ordinal = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?;
        fuel_attribution.push(TerminalObjectFuelAttribution {
            machine,
            attribution: TerminalNativeFuelAttribution {
                schedule,
                site,
                units,
                operation_ordinal,
                code_offset,
                byte_count,
            },
            text_offset,
        });
    }
    let port_effect_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyPortEffects)?;
    if port_effect_count > reader.remaining() / 60 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut port_effects = Vec::with_capacity(port_effect_count);
    for _ in 0..port_effect_count {
        let machine = MachineId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroPortEffectIdentity("MachineId"),
        )?;
        let psi_operation = OperationId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroPortEffectIdentity("OperationId"),
        )?;
        let service = ServiceId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroPortEffectIdentity("ServiceId"),
        )?;
        let port = reader.u16()?;
        let value = reader.u8()?;
        if reader.u8()? != 0 {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        let operation_ordinal = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::PortEffectOffsetNotRepresentable)?;
        port_effects.push(TerminalObjectPortEffect {
            machine,
            effect: TerminalPortEffectRecord {
                psi_operation,
                service,
                port,
                value,
                operation_ordinal,
                code_offset,
                byte_count,
            },
            text_offset,
        });
    }
    let settlement_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyBoundarySettlements)?;
    let mut boundary_settlements = Vec::with_capacity(settlement_count);
    for _ in 0..settlement_count {
        let machine = MachineId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroSettlementIdentity("MachineId"),
        )?;
        let psi_operation = OperationId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroSettlementIdentity("OperationId"),
        )?;
        let boundary = BoundaryMachineId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroSettlementIdentity("BoundaryMachineId"),
        )?;
        let provider_execution = TerminalProviderExecutionRecord::new(
            reader.u64()?,
            reader.u64()?,
            reader.u64()?,
            reader.u64()?,
            reader.u64()?,
        )
        .ok_or(TerminalInstallationError::ZeroProviderExecutionEvidence)?;
        let realization = TerminalMetadataOnlyPortRealization {
            effect_operation: OperationId::new(reader.u64()?).ok_or(
                TerminalInstallationError::ZeroSettlementIdentity("realization OperationId"),
            )?,
            service: ServiceId::new(reader.u64()?).ok_or(
                TerminalInstallationError::ZeroSettlementIdentity("realization ServiceId"),
            )?,
            port: reader.u16()?,
            value: reader.u8()?,
        };
        if reader.u8()? != 0 {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        let operation_ordinal = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::SettlementOffsetNotRepresentable)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::SettlementOffsetNotRepresentable)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::SettlementOffsetNotRepresentable)?;
        let argument_count = usize::try_from(reader.u32()?)
            .map_err(|_| TerminalInstallationError::TooManySettlementArguments)?;
        if argument_count > reader.remaining() / 12 {
            return Err(TerminalInstallationError::UnexpectedEnd);
        }
        let mut arguments = Vec::with_capacity(argument_count);
        for _ in 0..argument_count {
            arguments.push(decode_structural_argument(&mut reader)?);
        }
        let claim_count = usize::try_from(reader.u32()?)
            .map_err(|_| TerminalInstallationError::TooManyCompletionReceipts)?;
        if claim_count > reader.remaining() / 12 {
            return Err(TerminalInstallationError::UnexpectedEnd);
        }
        let mut completion_receipts = Vec::with_capacity(claim_count);
        for _ in 0..claim_count {
            completion_receipts.push(CompletionReceipt {
                claim: ClaimId::new(reader.u64()?)
                    .ok_or(TerminalInstallationError::ZeroSettlementIdentity("ClaimId"))?,
                argument_index: reader.u32()?,
            });
        }
        boundary_settlements.push(TerminalObjectBoundarySettlement {
            machine,
            settlement: TerminalBoundarySettlementRecord {
                psi_operation,
                boundary,
                provider_execution,
                realization,
                arguments,
                completion_receipts,
                operation_ordinal,
                code_offset,
            },
            text_offset,
        });
    }
    if reader.remaining() != 0 {
        return Err(TerminalInstallationError::TrailingBytes(reader.remaining()));
    }

    let record = TerminalInstallationRecord {
        terminal_psi: TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint,
        },
        target: NativeTarget {
            architecture,
            object_format,
            pointer_size,
            pointer_alignment,
        },
        subsystem,
        profile_decision,
        selected_provider_plans,
        functions,
        structural_returns,
        internal_unit_calls,
        fuel_attribution,
        port_effects,
        boundary_settlements,
        image,
        compiler_text_validation: CompilerTextValidationEvidence {
            encoded_text_fingerprint,
            final_compiler_text_fingerprint,
            relocation_envelope_fingerprint,
            checked_instruction_validation_fingerprint,
            checked_instruction_footprint_fingerprint,
            derivation_fingerprint,
            text_relocation_count,
            checked_instruction_validation_count,
        },
    };
    validate_record_shape(&record)?;
    if encode_terminal_installation_record(&record)? != bytes {
        return Err(TerminalInstallationError::NonCanonicalEncoding);
    }
    Ok(record)
}

fn decode_structural_argument(
    reader: &mut Reader<'_>,
) -> Result<StructuralArgument, TerminalInstallationError> {
    let place = PlaceId::new(reader.u64()?)
        .ok_or(TerminalInstallationError::ZeroSettlementIdentity("PlaceId"))?;
    let path_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManySettlementArgumentPathSegments)?;
    if path_count > reader.remaining() / 8 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut path = Vec::with_capacity(path_count);
    for _ in 0..path_count {
        let tag = reader.u8()?;
        if reader.take(3)? != [0; 3] {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        path.push(match tag {
            1 => {
                let identity_len = usize::try_from(reader.u32()?)
                    .map_err(|_| TerminalInstallationError::SettlementArgumentFieldTooLong)?;
                let identity = std::str::from_utf8(reader.take(identity_len)?)
                    .map_err(|_| TerminalInstallationError::InvalidSettlementArgumentField)?
                    .to_owned();
                if identity.is_empty() {
                    return Err(TerminalInstallationError::InvalidSettlementArgumentField);
                }
                StructuralPathSegment::Field(identity)
            }
            2 => StructuralPathSegment::FixedIndex(reader.u64()?),
            _ => {
                return Err(TerminalInstallationError::InvalidSettlementArgumentPathTag(
                    tag,
                ));
            }
        });
    }
    Ok(StructuralArgument { place, path })
}

pub fn validate_terminal_installation_record(
    record: &TerminalInstallationRecord,
    image: &TerminalExecutableImage,
) -> Result<(), TerminalInstallationError> {
    validate_record_shape(record)?;
    if record.terminal_psi != image.terminal_psi()
        || record.target != image.target()
        || record.subsystem != image.subsystem()
        || record.image != fingerprint_image(&image.output().bytes)
        || Some(record.compiler_text_validation) != image.output().compiler_text_validation
        || record.fuel_attribution != image.fuel_attribution()
        || record.port_effects != image.port_effects()
        || record.boundary_settlements != image.boundary_settlements()
        || record.structural_returns
            != image
                .functions()
                .iter()
                .filter_map(|function| {
                    function.structural_return.clone().map(|returned| {
                        TerminalInstalledStructuralReturn {
                            machine: function.machine,
                            returned,
                        }
                    })
                })
                .collect::<Vec<_>>()
        || record.internal_unit_calls
            != image
                .functions()
                .iter()
                .flat_map(|function| {
                    function.internal_unit_calls.iter().cloned().map(|custody| {
                        TerminalInstalledInternalUnitCall {
                            machine: function.machine,
                            text_offset: function.text_offset + custody.code_offset,
                            custody,
                        }
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
                    || installed.text_offset != emitted.text_offset
                    || installed.byte_count != emitted.byte_count
                    || installed.unit_body != emitted.unit_stack.is_some()
                    || installed.unit_parameters != emitted.unit_parameters
                    || installed.unit_parameter_homes != emitted.unit_parameter_homes
                    || installed.unit_affine_cleanup != emitted.unit_affine_cleanup
            })
    {
        return Err(TerminalInstallationError::ImageBindingMismatch);
    }
    Ok(())
}

pub fn terminal_installation_fingerprint(
    record: &TerminalInstallationRecord,
) -> Result<TerminalInstallationFingerprint, TerminalInstallationError> {
    let bytes = encode_terminal_installation_record(record)?;
    Ok(TerminalInstallationFingerprint(hash(RECORD_DOMAIN, &bytes)))
}

fn validate_record_shape(
    record: &TerminalInstallationRecord,
) -> Result<(), TerminalInstallationError> {
    if !can_emit_terminal_executable_image(record.target) {
        return Err(TerminalInstallationError::UnsupportedTarget(record.target));
    }
    match record.target.object_format {
        ObjectFormat::Coff if record.subsystem.is_none() => {
            return Err(TerminalInstallationError::MissingCoffSubsystem);
        }
        ObjectFormat::Elf | ObjectFormat::MachO if record.subsystem.is_some() => {
            return Err(TerminalInstallationError::UnexpectedSubsystem);
        }
        ObjectFormat::Coff | ObjectFormat::Elf | ObjectFormat::MachO => {}
    }
    if record
        .selected_provider_plans
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(TerminalInstallationError::NonCanonicalProviderPlanOrder);
    }
    let selected = record
        .selected_provider_plans
        .iter()
        .map(|provider| provider.get())
        .collect::<std::collections::BTreeSet<_>>();
    let required = record
        .boundary_settlements
        .iter()
        .map(|settlement| settlement.settlement.provider_execution.provider_plan)
        .collect::<std::collections::BTreeSet<_>>();
    if required != selected {
        return Err(TerminalInstallationError::ProviderSettlementClosureMismatch);
    }
    if record.functions.is_empty() {
        return Err(TerminalInstallationError::NoInstalledFunctions);
    }
    let mut expected_text_offset = 0_usize;
    let mut previous_function = None;
    for function in &record.functions {
        if function.byte_count == 0
            || function.text_offset != expected_text_offset
            || previous_function.is_some_and(|previous| previous >= function.machine)
        {
            return Err(TerminalInstallationError::NonCanonicalInstalledFunctions);
        }
        if function.unit_parameters.len() != function.unit_parameter_homes.len()
            || function.unit_body != function.unit_affine_cleanup.is_some()
            || (!function.unit_body
                && (!function.unit_parameters.is_empty()
                    || !function.unit_parameter_homes.is_empty()))
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
            return Err(TerminalInstallationError::InvalidUnitAffineCleanup(
                function.machine,
            ));
        }
        if let Some(cleanup) = &function.unit_affine_cleanup {
            let end = cleanup
                .code_offset
                .checked_add(cleanup.byte_count)
                .ok_or(TerminalInstallationError::FunctionOffsetNotRepresentable)?;
            let expected_local_prefix = cleanup
                .locals
                .iter()
                .rev()
                .map(|(_, place, _)| place.id)
                .collect::<Vec<_>>();
            let expected_parameter_discards = function
                .unit_parameter_homes
                .iter()
                .rev()
                .filter(|home| home.multiplicity == StructuralMultiplicity::Affine)
                .map(|home| home.place)
                .collect::<Vec<_>>();
            let Some(parameter_discards) = cleanup.discards.get(expected_local_prefix.len()..)
            else {
                return Err(TerminalInstallationError::InvalidUnitAffineCleanup(
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
                            } if usize::try_from(declaration_ordinal) == Ok(ordinal)
                                && local_type == structural_type.id
                        ) || !matches!(
                            structural_type.shape,
                            StructuralTypeShape::Record { ref fields } if fields.is_empty()
                        )
                    },
                )
                || cleanup.discards.get(..expected_local_prefix.len())
                    != Some(expected_local_prefix.as_slice())
                || cleanup
                    .discards
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != cleanup.discards.len()
                || parameter_discards != expected_parameter_discards
            {
                return Err(TerminalInstallationError::InvalidUnitAffineCleanup(
                    function.machine,
                ));
            }
        }
        expected_text_offset = expected_text_offset
            .checked_add(function.byte_count)
            .ok_or(TerminalInstallationError::FunctionOffsetNotRepresentable)?;
        previous_function = Some(function.machine);
    }
    let function_by_machine = record
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut previous_return = None;
    for installed in &record.structural_returns {
        let function = function_by_machine.get(&installed.machine).ok_or(
            TerminalInstallationError::StructuralReturnMachineMissing(installed.machine),
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
        .map_err(|_| TerminalInstallationError::InvalidStructuralReturn(installed.machine))?;
        let expected_result = expected_plan.result.as_ref();
        let structural_fuel = record
            .fuel_attribution
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
            || returned.shape != ValueShape::integer(8, 8)
            || returned.source_placement.shape != returned.shape
            || returned.result_placement.shape != returned.shape
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
                        structural_type
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
            || structural_fuel.len() != returned.trivial_affine_locals.len() + 1
            || returned
                .trivial_affine_locals
                .iter()
                .enumerate()
                .any(|(ordinal, (operation, _, _))| {
                    structural_fuel.get(ordinal).is_none_or(|installed| {
                        installed.attribution.schedule
                            != TerminalFuelSchedule::CURRENT.identity()
                            || installed.attribution.site
                                != TerminalNativeFuelSite::Operation(*operation)
                            || installed.attribution.units != 1
                            || installed.attribution.operation_ordinal != ordinal
                            || installed.attribution.code_offset != 0
                            || installed.attribution.byte_count != 0
                    })
                })
            || structural_fuel.last().is_none_or(|installed| {
                installed.attribution.schedule != TerminalFuelSchedule::CURRENT.identity()
                    || installed.attribution.site
                        != TerminalNativeFuelSite::Edge(returned.psi_edge)
                    || installed.attribution.units != 1
                    || installed.attribution.operation_ordinal
                        != returned.trivial_affine_locals.len()
                    || installed.attribution.code_offset != 0
                    || installed.attribution.byte_count != returned.byte_count
            })
        {
            return Err(TerminalInstallationError::InvalidStructuralReturn(
                installed.machine,
            ));
        }
        previous_return = Some(installed.machine);
    }
    let mut previous_call = None;
    for installed in &record.internal_unit_calls {
        let function = function_by_machine.get(&installed.machine).ok_or(
            TerminalInstallationError::InvalidInternalUnitCall(installed.machine),
        )?;
        let custody = &installed.custody;
        let expected_text_offset = function
            .text_offset
            .checked_add(custody.code_offset)
            .ok_or(TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let end = custody
            .code_offset
            .checked_add(custody.byte_count)
            .ok_or(TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let plan = evaluate_call_plan(
            CallingPolicy::native_for_target(record.target),
            &CallSignature {
                parameters: custody
                    .arguments
                    .iter()
                    .map(|argument| argument.shape)
                    .collect(),
                result: None,
            },
        )
        .map_err(|_| TerminalInstallationError::InvalidInternalUnitCall(installed.machine))?;
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
        if previous_call.is_some_and(|previous| previous >= key)
            || installed.text_offset != expected_text_offset
            || end > function.byte_count
            || !function_by_machine.contains_key(&custody.target)
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
                            _ => true,
                        }
                })
            || !projected_argument_indexes.is_subset(&transferred_argument_indexes)
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
            return Err(TerminalInstallationError::InvalidInternalUnitCall(
                installed.machine,
            ));
        }
        previous_call = Some(key);
    }
    let mut previous_fuel = None;
    let mut fuel_sites = std::collections::BTreeSet::new();
    for installed in &record.fuel_attribution {
        let function = function_by_machine.get(&installed.machine).ok_or(
            TerminalInstallationError::FuelAttributionMachineMissing(installed.machine),
        )?;
        let expected = function
            .text_offset
            .checked_add(installed.attribution.code_offset)
            .ok_or(TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?;
        let end = installed
            .attribution
            .code_offset
            .checked_add(installed.attribution.byte_count)
            .ok_or(TerminalInstallationError::FuelAttributionOffsetNotRepresentable)?;
        if installed.attribution.schedule != TerminalFuelSchedule::CURRENT.identity()
            || installed.attribution.units == 0
            || installed.text_offset != expected
            || end > function.byte_count
        {
            return Err(TerminalInstallationError::InvalidFuelAttribution {
                machine: installed.machine,
                site: installed.attribution.site,
            });
        }
        let key = (
            installed.machine,
            installed.attribution.operation_ordinal,
            installed.text_offset,
        );
        if previous_fuel.is_some_and(|previous| previous >= key) {
            return Err(TerminalInstallationError::NonCanonicalFuelAttributionOrder);
        }
        if !fuel_sites.insert((installed.machine, installed.attribution.site)) {
            return Err(TerminalInstallationError::DuplicateFuelAttributionSite {
                machine: installed.machine,
                site: installed.attribution.site,
            });
        }
        previous_fuel = Some(key);
    }
    let mut previous_port = None;
    let mut port_operations = std::collections::BTreeSet::new();
    for installed in &record.port_effects {
        let function = function_by_machine.get(&installed.machine).ok_or(
            TerminalInstallationError::EffectMachineMissing(installed.machine),
        )?;
        let expected = function
            .text_offset
            .checked_add(installed.effect.code_offset)
            .ok_or(TerminalInstallationError::PortEffectOffsetNotRepresentable)?;
        let end = installed
            .effect
            .code_offset
            .checked_add(installed.effect.byte_count)
            .ok_or(TerminalInstallationError::PortEffectOffsetNotRepresentable)?;
        if installed.text_offset != expected
            || end > function.byte_count
            || installed.effect.byte_count
                != omega_x86_encoding::encode_immediate_port_write(
                    installed.effect.port,
                    installed.effect.value,
                )
                .len()
        {
            return Err(TerminalInstallationError::InvalidPortEffectOffset {
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
            return Err(TerminalInstallationError::NonCanonicalPortEffectOrder);
        }
        if !port_operations.insert((installed.machine, installed.effect.psi_operation)) {
            return Err(TerminalInstallationError::DuplicatePortEffectOperation {
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
                return Err(TerminalInstallationError::NonCanonicalBoundarySettlementOrder);
            }
        }
        if !operations.insert((installed.machine, installed.settlement.psi_operation)) {
            return Err(
                TerminalInstallationError::DuplicateBoundarySettlementOperation {
                    machine: installed.machine,
                    operation: installed.settlement.psi_operation,
                },
            );
        }
        let function = function_by_machine.get(&installed.machine).ok_or(
            TerminalInstallationError::EffectMachineMissing(installed.machine),
        )?;
        let expected = function
            .text_offset
            .checked_add(installed.settlement.code_offset)
            .ok_or(TerminalInstallationError::SettlementOffsetNotRepresentable)?;
        if installed.text_offset != expected
            || installed.settlement.code_offset > function.byte_count
        {
            return Err(TerminalInstallationError::InvalidBoundarySettlementOffset {
                machine: installed.machine,
                operation: installed.settlement.psi_operation,
            });
        }
        if installed.settlement.arguments.iter().any(|argument| {
            argument.path.iter().any(
                |segment| matches!(segment, StructuralPathSegment::Field(identity) if identity.is_empty()),
            )
        }) {
            return Err(TerminalInstallationError::InvalidSettlementArgumentField);
        }
        if installed
            .settlement
            .completion_receipts
            .iter()
            .any(|receipt| {
                usize::try_from(receipt.argument_index)
                    .map_or(true, |index| index >= installed.settlement.arguments.len())
            })
        {
            return Err(
                TerminalInstallationError::InvalidCompletionReceiptArgumentIndex {
                    machine: installed.machine,
                    operation: installed.settlement.psi_operation,
                },
            );
        }
        let realization = installed.settlement.realization;
        let matching_effects = record
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
            .count();
        if matching_effects != 1 {
            return Err(TerminalInstallationError::BoundaryRealizationMismatch {
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

fn fingerprint_image(bytes: &[u8]) -> TerminalImageFingerprint {
    TerminalImageFingerprint(hash(IMAGE_DOMAIN, bytes))
}

fn encode_structural_return(
    bytes: &mut Vec<u8>,
    installed: &TerminalInstalledStructuralReturn,
) -> Result<(), TerminalInstallationError> {
    let returned = &installed.returned;
    push_u64(bytes, installed.machine.get());
    push_u64(bytes, returned.psi_edge.get());
    push_u32(
        bytes,
        u32::try_from(returned.parameters.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?,
    );
    for parameter in &returned.parameters {
        encode_structural_parameter(bytes, parameter)?;
    }
    push_u32(
        bytes,
        u32::try_from(returned.parameter_placements.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?,
    );
    for placement in &returned.parameter_placements {
        encode_placement(bytes, placement)?;
    }
    encode_structural_parameter(bytes, &returned.source)?;
    encode_structural_result(bytes, &returned.result)?;
    encode_shape(bytes, returned.shape)?;
    encode_placement(bytes, &returned.source_placement)?;
    encode_placement(bytes, &returned.result_placement)?;
    push_u32(
        bytes,
        u32::try_from(returned.returned_claims.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnClaims)?,
    );
    for claim in &returned.returned_claims {
        push_u64(bytes, claim.get());
    }
    push_u32(
        bytes,
        u32::try_from(returned.trivial_affine_locals.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?,
    );
    for (operation, local, local_type) in &returned.trivial_affine_locals {
        push_u64(bytes, operation.get());
        encode_trivial_affine_local(bytes, local)?;
        encode_trivial_affine_local_type(bytes, local_type)?;
    }
    push_u32(
        bytes,
        u32::try_from(returned.trivial_affine_discards.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?,
    );
    for place in &returned.trivial_affine_discards {
        push_u64(bytes, place.get());
    }
    push_u64(
        bytes,
        u64::try_from(returned.code_offset)
            .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(returned.byte_count)
            .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
    );
    Ok(())
}

fn encode_unit_affine_cleanup(
    bytes: &mut Vec<u8>,
    cleanup: &omega_terminal_machine_code::TerminalUnitAffineCleanupRecord,
) -> Result<(), TerminalInstallationError> {
    push_u64(bytes, cleanup.psi_edge.get());
    push_u32(
        bytes,
        u32::try_from(cleanup.locals.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?,
    );
    for (operation, local, local_type) in &cleanup.locals {
        push_u64(bytes, operation.get());
        encode_trivial_affine_local(bytes, local)?;
        encode_trivial_affine_local_type(bytes, local_type)?;
    }
    push_u32(
        bytes,
        u32::try_from(cleanup.discards.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?,
    );
    for place in &cleanup.discards {
        push_u64(bytes, place.get());
    }
    push_u64(
        bytes,
        u64::try_from(cleanup.code_offset)
            .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(cleanup.byte_count)
            .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
    );
    Ok(())
}

fn encode_internal_unit_call(
    bytes: &mut Vec<u8>,
    installed: &TerminalInstalledInternalUnitCall,
) -> Result<(), TerminalInstallationError> {
    let custody = &installed.custody;
    push_u64(bytes, installed.machine.get());
    push_u64(
        bytes,
        u64::try_from(installed.text_offset)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    push_u64(bytes, custody.psi_operation.get());
    push_u64(bytes, custody.target.get());
    push_u64(
        bytes,
        u64::try_from(custody.operation_ordinal)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(custody.code_offset)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(custody.byte_count)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    push_u32(
        bytes,
        u32::try_from(custody.arguments.len())
            .map_err(|_| TerminalInstallationError::TooManyInternalUnitCallArguments)?,
    );
    for argument in &custody.arguments {
        encode_structural_argument(
            bytes,
            &StructuralArgument {
                place: argument.place,
                path: argument.path.clone(),
            },
        )?;
        push_u64(bytes, argument.root_structural_type.get());
        push_u64(bytes, argument.structural_type.get());
        encode_shape(bytes, argument.shape)?;
        push_u32(bytes, argument.source_byte_offset);
        push_u32(bytes, argument.source_home_byte_offset);
        push_u32(bytes, argument.call_stack_bytes);
        match (argument.fixed_array_length, argument.element_stride) {
            (Some(length), Some(stride)) => {
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(bytes, length);
                push_u32(bytes, stride);
            }
            (None, None) => {
                bytes.push(0);
                bytes.extend_from_slice(&[0; 3]);
            }
            _ => {
                return Err(TerminalInstallationError::InvalidInternalUnitCall(
                    installed.machine,
                ));
            }
        }
        encode_direct_placement(bytes, &argument.source)?;
        encode_direct_placement(bytes, &argument.destination)?;
        push_u64(
            bytes,
            u64::try_from(argument.code_offset)
                .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(argument.byte_count)
                .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
        );
        push_u32(
            bytes,
            u32::try_from(argument.bytes.len())
                .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
        );
        bytes.extend_from_slice(&argument.bytes);
    }
    push_u32(
        bytes,
        u32::try_from(custody.claim_transfers.len())
            .map_err(|_| TerminalInstallationError::TooManyInternalUnitCallClaims)?,
    );
    for transfer in &custody.claim_transfers {
        push_u64(bytes, transfer.claim.get());
        push_u32(bytes, transfer.argument_index);
    }
    Ok(())
}

fn encode_direct_placement(
    bytes: &mut Vec<u8>,
    placement: &ValuePlacement,
) -> Result<(), TerminalInstallationError> {
    encode_shape(bytes, placement.shape)?;
    push_u32(
        bytes,
        u32::try_from(placement.locations.len())
            .map_err(|_| TerminalInstallationError::UnsupportedInternalUnitCallPlacement)?,
    );
    for location in &placement.locations {
        match location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                bytes.push(1);
                bytes.push(register_tag(*register)?);
                push_u16(bytes, *value_byte_offset);
                push_u16(bytes, *byte_size);
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(2);
                bytes.push(0);
                push_u16(bytes, *value_byte_offset);
                push_u16(bytes, *byte_size);
                push_u16(bytes, *alignment);
                push_u32(bytes, *stack_byte_offset);
            }
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(3);
                match pointer {
                    omega_calling_conventions::IndirectPointerLocation::Register(register) => {
                        bytes.push(1);
                        bytes.push(register_tag(*register)?);
                        bytes.push(0);
                    }
                    omega_calling_conventions::IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        alignment,
                    } => {
                        bytes.push(2);
                        bytes.push(0);
                        bytes.push(0);
                        push_u32(bytes, *stack_byte_offset);
                        push_u16(bytes, *alignment);
                    }
                }
                match copy_stack_byte_offset {
                    Some(offset) => {
                        bytes.push(1);
                        push_u32(bytes, *offset);
                    }
                    None => bytes.push(0),
                }
                push_u16(bytes, *byte_size);
                push_u16(bytes, *alignment);
            }
        }
    }
    Ok(())
}

fn decode_internal_unit_call(
    reader: &mut Reader<'_>,
) -> Result<TerminalInstalledInternalUnitCall, TerminalInstallationError> {
    let machine =
        MachineId::new(reader.u64()?).ok_or(TerminalInstallationError::ZeroFunctionIdentity)?;
    let text_offset = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let psi_operation = OperationId::new(reader.u64()?)
        .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?;
    let target = MachineId::new(reader.u64()?)
        .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?;
    let operation_ordinal = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let code_offset = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let byte_count = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let argument_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyInternalUnitCallArguments)?;
    if argument_count > reader.remaining() / 80 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut arguments = Vec::with_capacity(argument_count);
    for _ in 0..argument_count {
        let argument = decode_structural_argument(reader)?;
        let root_structural_type = StructuralTypeId::new(reader.u64()?)
            .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?;
        let structural_type = StructuralTypeId::new(reader.u64()?)
            .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?;
        let shape = decode_shape(reader)?;
        let source_byte_offset = reader.u32()?;
        let source_home_byte_offset = reader.u32()?;
        let call_stack_bytes = reader.u32()?;
        let has_array = decode_boolean(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        let (fixed_array_length, element_stride) = if has_array {
            (Some(reader.u64()?), Some(reader.u32()?))
        } else {
            (None, None)
        };
        let source = decode_direct_placement(reader)?;
        let destination = decode_direct_placement(reader)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let encoded_count = usize::try_from(reader.u32()?)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let bytes = reader.take(encoded_count)?.to_vec();
        arguments.push(
            omega_terminal_machine_code::TerminalInternalUnitCallArgumentRecord {
                place: argument.place,
                path: argument.path,
                root_structural_type,
                structural_type,
                shape,
                source_byte_offset,
                source_home_byte_offset,
                call_stack_bytes,
                fixed_array_length,
                element_stride,
                source,
                destination,
                code_offset,
                byte_count,
                bytes,
            },
        );
    }
    let claim_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyInternalUnitCallClaims)?;
    if claim_count > reader.remaining() / 12 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut claim_transfers = Vec::with_capacity(claim_count);
    for _ in 0..claim_count {
        claim_transfers.push(psi_terminal::ClaimTransfer {
            claim: ClaimId::new(reader.u64()?)
                .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?,
            argument_index: reader.u32()?,
        });
    }
    Ok(TerminalInstalledInternalUnitCall {
        machine,
        text_offset,
        custody: omega_terminal_machine_code::TerminalInternalUnitCallRecord {
            psi_operation,
            target,
            arguments,
            claim_transfers,
            operation_ordinal,
            code_offset,
            byte_count,
        },
    })
}

fn decode_direct_placement(
    reader: &mut Reader<'_>,
) -> Result<ValuePlacement, TerminalInstallationError> {
    let shape = decode_shape(reader)?;
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::UnsupportedInternalUnitCallPlacement)?;
    if count == 0 || count > reader.remaining() / 6 {
        return Err(TerminalInstallationError::UnsupportedInternalUnitCallPlacement);
    }
    let mut locations = Vec::with_capacity(count);
    for _ in 0..count {
        locations.push(match reader.u8()? {
            1 => ValueLocation::Register {
                register: decode_register(reader.u8()?)?,
                value_byte_offset: reader.u16()?,
                byte_size: reader.u16()?,
            },
            2 => {
                if reader.u8()? != 0 {
                    return Err(TerminalInstallationError::NonzeroReservedField);
                }
                ValueLocation::Stack {
                    value_byte_offset: reader.u16()?,
                    byte_size: reader.u16()?,
                    alignment: reader.u16()?,
                    stack_byte_offset: reader.u32()?,
                }
            }
            3 => {
                let pointer = match reader.u8()? {
                    1 => {
                        let register = decode_register(reader.u8()?)?;
                        if reader.u8()? != 0 {
                            return Err(TerminalInstallationError::NonzeroReservedField);
                        }
                        omega_calling_conventions::IndirectPointerLocation::Register(register)
                    }
                    2 => {
                        if reader.take(2)? != [0; 2] {
                            return Err(TerminalInstallationError::NonzeroReservedField);
                        }
                        omega_calling_conventions::IndirectPointerLocation::Stack {
                            stack_byte_offset: reader.u32()?,
                            alignment: reader.u16()?,
                        }
                    }
                    _ => {
                        return Err(
                            TerminalInstallationError::UnsupportedInternalUnitCallPlacement,
                        );
                    }
                };
                let copy_stack_byte_offset = match decode_boolean(reader.u8()?)? {
                    true => Some(reader.u32()?),
                    false => None,
                };
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size: reader.u16()?,
                    alignment: reader.u16()?,
                }
            }
            _ => return Err(TerminalInstallationError::UnsupportedInternalUnitCallPlacement),
        });
    }
    Ok(ValuePlacement { shape, locations })
}

fn encode_trivial_affine_local(
    bytes: &mut Vec<u8>,
    local: &StructuralPlaceDeclaration,
) -> Result<(), TerminalInstallationError> {
    let psi_core::StructuralPlaceKind::TrivialAffineLocal {
        declaration_ordinal,
        structural_type,
    } = local.kind
    else {
        return Err(TerminalInstallationError::InvalidStructuralReturnLocal);
    };
    push_u64(bytes, local.id.get());
    push_u32(bytes, declaration_ordinal);
    push_u32(bytes, 0);
    push_u64(bytes, structural_type.get());
    Ok(())
}

fn encode_trivial_affine_local_type(
    bytes: &mut Vec<u8>,
    declaration: &psi_terminal::StructuralTypeDeclaration,
) -> Result<(), TerminalInstallationError> {
    let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(TerminalInstallationError::InvalidStructuralReturnLocal);
    };
    if !fields.is_empty() {
        return Err(TerminalInstallationError::InvalidStructuralReturnLocal);
    }
    push_u64(bytes, declaration.id.get());
    push_u32(
        bytes,
        u32::try_from(declaration.identity.len())
            .map_err(|_| TerminalInstallationError::StructuralTypeIdentityTooLong)?,
    );
    bytes.extend_from_slice(declaration.identity.as_bytes());
    push_u32(bytes, 0);
    Ok(())
}

fn encode_structural_parameter(
    bytes: &mut Vec<u8>,
    parameter: &StructuralParameterDeclaration,
) -> Result<(), TerminalInstallationError> {
    push_u64(bytes, parameter.place.get());
    push_u32(bytes, parameter.position);
    bytes.push(u8::from(parameter.is_self));
    bytes.push(multiplicity_tag(parameter.multiplicity));
    bytes.extend_from_slice(&[0; 2]);
    push_u64(bytes, parameter.structural_type.get());
    encode_domains(bytes, &parameter.qualifications)
}

fn encode_structural_result(
    bytes: &mut Vec<u8>,
    result: &StructuralResultDeclaration,
) -> Result<(), TerminalInstallationError> {
    push_u64(bytes, result.place.get());
    push_u64(bytes, result.structural_type.get());
    bytes.push(multiplicity_tag(result.multiplicity));
    bytes.extend_from_slice(&[0; 3]);
    encode_domains(bytes, &result.qualifications)
}

fn encode_domains(
    bytes: &mut Vec<u8>,
    domains: &[StructuralDomainId],
) -> Result<(), TerminalInstallationError> {
    push_u32(
        bytes,
        u32::try_from(domains.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralQualifications)?,
    );
    for domain in domains {
        push_u64(bytes, domain.get());
    }
    Ok(())
}

fn encode_shape(bytes: &mut Vec<u8>, shape: ValueShape) -> Result<(), TerminalInstallationError> {
    if shape.class != ValueClass::Integer {
        return Err(TerminalInstallationError::UnsupportedStructuralReturnShape);
    }
    bytes.push(1);
    bytes.push(0);
    push_u16(bytes, shape.byte_size);
    push_u16(bytes, shape.alignment);
    push_u16(bytes, 0);
    Ok(())
}

fn encode_placement(
    bytes: &mut Vec<u8>,
    placement: &ValuePlacement,
) -> Result<(), TerminalInstallationError> {
    encode_shape(bytes, placement.shape)?;
    let [location] = placement.locations.as_slice() else {
        return Err(TerminalInstallationError::UnsupportedStructuralReturnPlacement);
    };
    match location {
        ValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } => {
            bytes.push(1);
            bytes.push(register_tag(*register)?);
            push_u16(bytes, *value_byte_offset);
            push_u16(bytes, *byte_size);
            push_u16(bytes, 0);
        }
        ValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset,
            byte_size,
            alignment,
        } => {
            bytes.push(2);
            bytes.push(0);
            push_u16(bytes, *value_byte_offset);
            push_u16(bytes, *byte_size);
            push_u16(bytes, *alignment);
            push_u32(bytes, *stack_byte_offset);
        }
        ValueLocation::Indirect { .. } => {
            return Err(TerminalInstallationError::UnsupportedStructuralReturnPlacement);
        }
    }
    Ok(())
}

fn decode_structural_return(
    reader: &mut Reader<'_>,
) -> Result<TerminalInstalledStructuralReturn, TerminalInstallationError> {
    let machine =
        MachineId::new(reader.u64()?).ok_or(TerminalInstallationError::ZeroFunctionIdentity)?;
    let psi_edge = EdgeId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("edge"),
    )?;
    let parameter_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?;
    let mut parameters = Vec::with_capacity(parameter_count);
    for _ in 0..parameter_count {
        parameters.push(decode_structural_parameter(reader)?);
    }
    let placement_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?;
    let mut parameter_placements = Vec::with_capacity(placement_count);
    for _ in 0..placement_count {
        parameter_placements.push(decode_placement(reader)?);
    }
    let source = decode_structural_parameter(reader)?;
    let result = decode_structural_result(reader)?;
    let shape = decode_shape(reader)?;
    let source_placement = decode_placement(reader)?;
    let result_placement = decode_placement(reader)?;
    let claim_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnClaims)?;
    let mut returned_claims = Vec::with_capacity(claim_count);
    for _ in 0..claim_count {
        returned_claims.push(ClaimId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("claim"),
        )?);
    }
    let local_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?;
    let mut trivial_affine_locals = Vec::with_capacity(local_count);
    for _ in 0..local_count {
        let operation = OperationId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("local operation"),
        )?;
        trivial_affine_locals.push((
            operation,
            decode_trivial_affine_local(reader)?,
            decode_trivial_affine_local_type(reader)?,
        ));
    }
    let cleanup_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?;
    let mut trivial_affine_discards = Vec::with_capacity(cleanup_count);
    for _ in 0..cleanup_count {
        trivial_affine_discards.push(PlaceId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("cleanup place"),
        )?);
    }
    Ok(TerminalInstalledStructuralReturn {
        machine,
        returned: TerminalStructuralReturnRecord {
            psi_edge,
            parameters,
            parameter_placements,
            source,
            result,
            shape,
            source_placement,
            result_placement,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
            code_offset: usize::try_from(reader.u64()?)
                .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
            byte_count: usize::try_from(reader.u64()?)
                .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
        },
    })
}

fn decode_unit_affine_cleanup(
    reader: &mut Reader<'_>,
) -> Result<omega_terminal_machine_code::TerminalUnitAffineCleanupRecord, TerminalInstallationError>
{
    let psi_edge = EdgeId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("Unit cleanup edge"),
    )?;
    let local_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?;
    let mut locals = Vec::with_capacity(local_count);
    for _ in 0..local_count {
        let operation = OperationId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("Unit local operation"),
        )?;
        locals.push((
            operation,
            decode_trivial_affine_local(reader)?,
            decode_trivial_affine_local_type(reader)?,
        ));
    }
    let discard_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?;
    let mut discards = Vec::with_capacity(discard_count);
    for _ in 0..discard_count {
        discards.push(PlaceId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("Unit cleanup place"),
        )?);
    }
    Ok(
        omega_terminal_machine_code::TerminalUnitAffineCleanupRecord {
            psi_edge,
            locals,
            discards,
            code_offset: usize::try_from(reader.u64()?)
                .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
            byte_count: usize::try_from(reader.u64()?)
                .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
        },
    )
}

fn decode_trivial_affine_local(
    reader: &mut Reader<'_>,
) -> Result<StructuralPlaceDeclaration, TerminalInstallationError> {
    let id = PlaceId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("local place"),
    )?;
    let declaration_ordinal = reader.u32()?;
    if reader.u32()? != 0 {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("local type"),
    )?;
    Ok(StructuralPlaceDeclaration {
        id,
        kind: psi_core::StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
        },
    })
}

fn decode_trivial_affine_local_type(
    reader: &mut Reader<'_>,
) -> Result<psi_terminal::StructuralTypeDeclaration, TerminalInstallationError> {
    let id = StructuralTypeId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("local type declaration"),
    )?;
    let identity_len = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::StructuralTypeIdentityTooLong)?;
    let identity = std::str::from_utf8(reader.take(identity_len)?)
        .map_err(|_| TerminalInstallationError::InvalidStructuralTypeIdentity)?
        .to_owned();
    if identity.is_empty() {
        return Err(TerminalInstallationError::InvalidStructuralTypeIdentity);
    }
    if reader.u32()? != 0 {
        return Err(TerminalInstallationError::InvalidStructuralReturnLocal);
    }
    Ok(psi_terminal::StructuralTypeDeclaration {
        id,
        identity,
        shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
    })
}

fn decode_structural_parameter(
    reader: &mut Reader<'_>,
) -> Result<StructuralParameterDeclaration, TerminalInstallationError> {
    let place = PlaceId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("source place"),
    )?;
    let position = reader.u32()?;
    let is_self = decode_boolean(reader.u8()?)?;
    let multiplicity = decode_multiplicity(reader.u8()?)?;
    if reader.u16()? != 0 {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("source type"),
    )?;
    Ok(StructuralParameterDeclaration {
        place,
        position,
        is_self,
        structural_type,
        multiplicity,
        qualifications: decode_domains(reader)?,
    })
}

fn decode_structural_result(
    reader: &mut Reader<'_>,
) -> Result<StructuralResultDeclaration, TerminalInstallationError> {
    let place = PlaceId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("result place"),
    )?;
    let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("result type"),
    )?;
    let multiplicity = decode_multiplicity(reader.u8()?)?;
    if reader.take(3)? != [0; 3] {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    Ok(StructuralResultDeclaration {
        place,
        structural_type,
        multiplicity,
        qualifications: decode_domains(reader)?,
    })
}

fn decode_domains(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralDomainId>, TerminalInstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralQualifications)?;
    let mut domains = Vec::with_capacity(count);
    for _ in 0..count {
        domains.push(StructuralDomainId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("domain"),
        )?);
    }
    Ok(domains)
}

fn decode_shape(reader: &mut Reader<'_>) -> Result<ValueShape, TerminalInstallationError> {
    if reader.u8()? != 1 || reader.u8()? != 0 {
        return Err(TerminalInstallationError::UnsupportedStructuralReturnShape);
    }
    let byte_size = reader.u16()?;
    let alignment = reader.u16()?;
    if reader.u16()? != 0 {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    Ok(ValueShape::integer(byte_size, alignment))
}

fn decode_placement(reader: &mut Reader<'_>) -> Result<ValuePlacement, TerminalInstallationError> {
    let shape = decode_shape(reader)?;
    let location_kind = reader.u8()?;
    let detail = reader.u8()?;
    let location = match location_kind {
        1 => {
            let value_byte_offset = reader.u16()?;
            let byte_size = reader.u16()?;
            if reader.u16()? != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            ValueLocation::Register {
                register: decode_register(detail)?,
                value_byte_offset,
                byte_size,
            }
        }
        2 => {
            if detail != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            ValueLocation::Stack {
                value_byte_offset: reader.u16()?,
                byte_size: reader.u16()?,
                alignment: reader.u16()?,
                stack_byte_offset: reader.u32()?,
            }
        }
        _ => return Err(TerminalInstallationError::UnsupportedStructuralReturnPlacement),
    };
    Ok(ValuePlacement {
        shape,
        locations: vec![location],
    })
}

fn multiplicity_tag(value: StructuralMultiplicity) -> u8 {
    match value {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    }
}

fn decode_multiplicity(value: u8) -> Result<StructuralMultiplicity, TerminalInstallationError> {
    match value {
        1 => Ok(StructuralMultiplicity::Unrestricted),
        2 => Ok(StructuralMultiplicity::Affine),
        3 => Ok(StructuralMultiplicity::Linear),
        _ => Err(TerminalInstallationError::InvalidStructuralMultiplicity(
            value,
        )),
    }
}

fn register_tag(register: MachineRegister) -> Result<u8, TerminalInstallationError> {
    match register {
        MachineRegister::X86Rax => Ok(1),
        MachineRegister::X86Rcx => Ok(2),
        MachineRegister::X86Rdi => Ok(3),
        MachineRegister::Aarch64X(0) => Ok(4),
        MachineRegister::X86Rsi => Ok(5),
        MachineRegister::X86Rdx => Ok(6),
        MachineRegister::Aarch64X(1) => Ok(7),
        MachineRegister::X86R8 => Ok(8),
        MachineRegister::X86R9 => Ok(9),
        MachineRegister::Aarch64X(2) => Ok(10),
        MachineRegister::Aarch64X(3) => Ok(11),
        MachineRegister::Aarch64X(4) => Ok(12),
        MachineRegister::Aarch64X(5) => Ok(13),
        MachineRegister::Aarch64X(6) => Ok(14),
        MachineRegister::Aarch64X(7) => Ok(15),
        _ => Err(TerminalInstallationError::UnsupportedStructuralReturnRegister(register)),
    }
}

fn decode_register(value: u8) -> Result<MachineRegister, TerminalInstallationError> {
    match value {
        1 => Ok(MachineRegister::X86Rax),
        2 => Ok(MachineRegister::X86Rcx),
        3 => Ok(MachineRegister::X86Rdi),
        4 => Ok(MachineRegister::Aarch64X(0)),
        5 => Ok(MachineRegister::X86Rsi),
        6 => Ok(MachineRegister::X86Rdx),
        7 => Ok(MachineRegister::Aarch64X(1)),
        8 => Ok(MachineRegister::X86R8),
        9 => Ok(MachineRegister::X86R9),
        10 => Ok(MachineRegister::Aarch64X(2)),
        11 => Ok(MachineRegister::Aarch64X(3)),
        12 => Ok(MachineRegister::Aarch64X(4)),
        13 => Ok(MachineRegister::Aarch64X(5)),
        14 => Ok(MachineRegister::Aarch64X(6)),
        15 => Ok(MachineRegister::Aarch64X(7)),
        _ => Err(TerminalInstallationError::InvalidStructuralReturnRegister(
            value,
        )),
    }
}

fn hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(
        u64::try_from(bytes.len())
            .expect("terminal artifact bytes fit the digest domain")
            .to_le_bytes(),
    );
    digest.update(bytes);
    digest.finalize().into()
}

fn architecture_tag(architecture: Architecture) -> u8 {
    match architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }
}

fn decode_architecture(tag: u8) -> Result<Architecture, TerminalInstallationError> {
    match tag {
        1 => Ok(Architecture::Aarch64),
        2 => Ok(Architecture::X86_64),
        _ => Err(TerminalInstallationError::InvalidArchitectureTag(tag)),
    }
}

fn object_format_tag(object_format: ObjectFormat) -> u8 {
    match object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }
}

fn decode_object_format(tag: u8) -> Result<ObjectFormat, TerminalInstallationError> {
    match tag {
        1 => Ok(ObjectFormat::Elf),
        2 => Ok(ObjectFormat::MachO),
        3 => Ok(ObjectFormat::Coff),
        _ => Err(TerminalInstallationError::InvalidObjectFormatTag(tag)),
    }
}

fn decode_boolean(value: u8) -> Result<bool, TerminalInstallationError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(TerminalInstallationError::InvalidBoolean(value)),
    }
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_hex(formatter: &mut std::fmt::Formatter<'_>, bytes: &[u8; 32]) -> std::fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Reader<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, len: usize) -> Result<&'bytes [u8], TerminalInstallationError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(TerminalInstallationError::UnexpectedEnd)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(TerminalInstallationError::UnexpectedEnd)?;
        self.offset = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], TerminalInstallationError> {
        self.take(N)?
            .try_into()
            .map_err(|_| TerminalInstallationError::UnexpectedEnd)
    }

    fn u8(&mut self) -> Result<u8, TerminalInstallationError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, TerminalInstallationError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, TerminalInstallationError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, TerminalInstallationError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInstallationError {
    InvalidMagic,
    UnsupportedFormatMarker(u16),
    UnsupportedVocabularyMarker(u16),
    InvalidArchitectureTag(u8),
    InvalidObjectFormatTag(u8),
    InvalidBoolean(u8),
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
    ZeroProviderPlan,
    DuplicateProviderExecution,
    NonCanonicalProviderPlanOrder,
    TooManyProviderPlans,
    TooManyInstalledFunctions,
    TooManyStructuralReturns,
    TooManyInternalUnitCalls,
    TooManyInternalUnitCallArguments,
    TooManyInternalUnitCallClaims,
    TooManyStructuralReturnParameters,
    TooManyStructuralReturnClaims,
    TooManyStructuralReturnCleanups,
    TooManyStructuralQualifications,
    TooManyFuelAttributions,
    TooManyPortEffects,
    TooManyBoundarySettlements,
    TooManySettlementArguments,
    TooManySettlementArgumentPathSegments,
    SettlementArgumentFieldTooLong,
    TooManyCompletionReceipts,
    SettlementOffsetNotRepresentable,
    FunctionOffsetNotRepresentable,
    StructuralReturnOffsetNotRepresentable,
    InternalUnitCallOffsetNotRepresentable,
    FuelAttributionOffsetNotRepresentable,
    PortEffectOffsetNotRepresentable,
    ZeroFunctionIdentity,
    ZeroStructuralReturnIdentity(&'static str),
    ZeroInternalUnitCallIdentity,
    InvalidStructuralMultiplicity(u8),
    UnsupportedStructuralReturnShape,
    UnsupportedStructuralReturnPlacement,
    UnsupportedInternalUnitCallPlacement,
    UnsupportedStructuralReturnRegister(MachineRegister),
    InvalidStructuralReturnRegister(u8),
    InvalidStructuralReturnLocal,
    StructuralTypeIdentityTooLong,
    InvalidStructuralTypeIdentity,
    ZeroFuelScheduleIdentity,
    ZeroFuelAttributionIdentity(&'static str),
    InvalidFuelSiteTag(u8),
    ZeroPortEffectIdentity(&'static str),
    ZeroSettlementIdentity(&'static str),
    InvalidSettlementArgumentPathTag(u8),
    InvalidSettlementArgumentField,
    ZeroProviderExecutionEvidence,
    NoInstalledFunctions,
    NonCanonicalInstalledFunctions,
    StructuralReturnMachineMissing(MachineId),
    InvalidStructuralReturn(MachineId),
    InvalidInternalUnitCall(MachineId),
    InvalidUnitAffineCleanup(MachineId),
    FuelAttributionMachineMissing(MachineId),
    NonCanonicalFuelAttributionOrder,
    DuplicateFuelAttributionSite {
        machine: MachineId,
        site: TerminalNativeFuelSite,
    },
    InvalidFuelAttribution {
        machine: MachineId,
        site: TerminalNativeFuelSite,
    },
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
    BoundaryRealizationMismatch {
        machine: MachineId,
        operation: OperationId,
    },
    CountNotRepresentable(&'static str),
    MissingCompilerTextValidation,
    ImageBindingMismatch,
}

impl std::fmt::Display for TerminalInstallationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalInstallationError {}
