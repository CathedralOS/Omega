use std::num::NonZeroU64;

use omega_image::CompilerTextValidationEvidence;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_machine_code::{
    TerminalBoundarySettlementRecord, TerminalPortEffectRecord, TerminalProviderExecutionRecord,
};
use omega_terminal_target_operations::TerminalMetadataOnlyPortRealization;
use psi_core::{
    BoundaryMachineId, ClaimId, MachineId, OperationId, PlaceId, ProfileDecisionId, ServiceId,
};
use psi_terminal::{ClaimSettlement, SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
use sha2::{Digest, Sha256};

use crate::{
    TerminalExecutableImage, TerminalObjectBoundarySettlement, TerminalObjectPortEffect,
    can_emit_terminal_executable_image,
};

pub const TERMINAL_INSTALLATION_FORMAT_MARKER: u16 = 2;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalInstalledFunction {
    pub machine: MachineId,
    pub text_offset: usize,
    pub byte_count: usize,
}

/// Build the canonical installation record for an emitted image.
///
/// Provider-plan order is not semantic, so construction sorts it. Duplicate
/// identities reject rather than silently changing a malformed provider
/// closure into a set.
pub fn build_terminal_installation_record(
    image: &TerminalExecutableImage,
    profile_decision: ProfileDecisionId,
    selected_provider_plans: impl IntoIterator<Item = SelectedProviderPlanIdentity>,
) -> Result<TerminalInstallationRecord, TerminalInstallationError> {
    let compiler_text_validation = image
        .output()
        .compiler_text_validation
        .ok_or(TerminalInstallationError::MissingCompilerTextValidation)?;
    let mut selected_provider_plans = selected_provider_plans.into_iter().collect::<Vec<_>>();
    selected_provider_plans.sort_unstable();
    if let Some(duplicate) = selected_provider_plans
        .windows(2)
        .find(|pair| pair[0] == pair[1])
        .map(|pair| pair[0])
    {
        return Err(TerminalInstallationError::DuplicateProviderPlan(duplicate));
    }
    let record = TerminalInstallationRecord {
        terminal_psi: image.terminal_psi(),
        target: image.target(),
        subsystem: image.subsystem(),
        profile_decision,
        selected_provider_plans,
        functions: image
            .functions()
            .iter()
            .map(|function| TerminalInstalledFunction {
                machine: function.machine,
                text_offset: function.text_offset,
                byte_count: function.byte_count,
            })
            .collect(),
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
            u32::try_from(settlement.argument_places.len())
                .map_err(|_| TerminalInstallationError::TooManySettlementArguments)?,
        );
        for place in &settlement.argument_places {
            push_u64(&mut bytes, place.get());
        }
        push_u32(
            &mut bytes,
            u32::try_from(settlement.claim_settlements.len())
                .map_err(|_| TerminalInstallationError::TooManyClaimSettlements)?,
        );
        for claim in &settlement.claim_settlements {
            push_u64(&mut bytes, claim.claim.get());
            push_u32(&mut bytes, claim.argument_index);
        }
    }
    Ok(bytes)
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
        functions.push(TerminalInstalledFunction {
            machine: MachineId::new(reader.u64()?)
                .ok_or(TerminalInstallationError::ZeroFunctionIdentity)?,
            text_offset: usize::try_from(reader.u64()?)
                .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?,
            byte_count: usize::try_from(reader.u64()?)
                .map_err(|_| TerminalInstallationError::FunctionOffsetNotRepresentable)?,
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
        if argument_count > reader.remaining() / 8 {
            return Err(TerminalInstallationError::UnexpectedEnd);
        }
        let mut argument_places = Vec::with_capacity(argument_count);
        for _ in 0..argument_count {
            argument_places.push(
                PlaceId::new(reader.u64()?)
                    .ok_or(TerminalInstallationError::ZeroSettlementIdentity("PlaceId"))?,
            );
        }
        let claim_count = usize::try_from(reader.u32()?)
            .map_err(|_| TerminalInstallationError::TooManyClaimSettlements)?;
        if claim_count > reader.remaining() / 12 {
            return Err(TerminalInstallationError::UnexpectedEnd);
        }
        let mut claim_settlements = Vec::with_capacity(claim_count);
        for _ in 0..claim_count {
            claim_settlements.push(ClaimSettlement {
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
                argument_places,
                claim_settlements,
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
        || record.port_effects != image.port_effects()
        || record.boundary_settlements != image.boundary_settlements()
        || record.functions.len() != image.functions().len()
        || record
            .functions
            .iter()
            .zip(image.functions())
            .any(|(installed, emitted)| {
                installed.machine != emitted.machine
                    || installed.text_offset != emitted.text_offset
                    || installed.byte_count != emitted.byte_count
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
    if !required.is_subset(&selected) {
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
    DuplicateProviderPlan(SelectedProviderPlanIdentity),
    NonCanonicalProviderPlanOrder,
    TooManyProviderPlans,
    TooManyInstalledFunctions,
    TooManyPortEffects,
    TooManyBoundarySettlements,
    TooManySettlementArguments,
    TooManyClaimSettlements,
    SettlementOffsetNotRepresentable,
    FunctionOffsetNotRepresentable,
    PortEffectOffsetNotRepresentable,
    ZeroFunctionIdentity,
    ZeroPortEffectIdentity(&'static str),
    ZeroSettlementIdentity(&'static str),
    ZeroProviderExecutionEvidence,
    NoInstalledFunctions,
    NonCanonicalInstalledFunctions,
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
    NonCanonicalBoundarySettlementOrder,
    DuplicateBoundarySettlementOperation {
        machine: MachineId,
        operation: OperationId,
    },
    InvalidBoundarySettlementOffset {
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
