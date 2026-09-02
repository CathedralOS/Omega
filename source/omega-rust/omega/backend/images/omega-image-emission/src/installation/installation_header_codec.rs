//! Canonical installation format-55 header codec.
//!
//! The parent retains validation and all count conversions so extraction does
//! not change encode error precedence. This module owns the fixed header bytes.

use omega_image::{
    CompilerTextDerivationDigest, CompilerTextRelocationEnvelopeDigest,
    CompilerTextValidationEvidence, EncodedCompilerTextDigest, FinalCompilerTextDigest,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::ProfileDecisionId;
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::{
    INSTALLATION_FORMAT_MARKER, ImageFingerprint, InstallationError, InstallationRecord, MAGIC,
    Reader, decode_boolean, push_u16, push_u64,
};

pub(super) struct DecodedInstallationHeader {
    pub(super) psi: TerminalPsiIdentity,
    pub(super) target: NativeTarget,
    pub(super) subsystem: Option<u16>,
    pub(super) profile_decision: ProfileDecisionId,
    pub(super) component_progress: Option<super::InstalledComponentProgress>,
    pub(super) image: ImageFingerprint,
    pub(super) image_sections: super::InstalledImageSections,
    pub(super) compiler_text_validation: CompilerTextValidationEvidence,
}

pub(super) fn encode_installation_header(
    bytes: &mut Vec<u8>,
    record: &InstallationRecord,
    text_relocation_count: u64,
    checked_instruction_validation_count: u64,
) -> Result<(), InstallationError> {
    bytes.extend_from_slice(MAGIC);
    push_u16(bytes, INSTALLATION_FORMAT_MARKER);
    push_u16(bytes, record.psi.vocabulary_marker.get());
    bytes.extend_from_slice(record.psi.program_fingerprint.as_bytes());
    bytes.push(architecture_tag(record.target.architecture));
    bytes.push(object_format_tag(record.target.object_format));
    bytes.push(u8::from(record.subsystem.is_some()));
    bytes.push(u8::from(record.component_progress.is_some()));
    push_u64(
        bytes,
        u64::try_from(record.target.pointer_size)
            .map_err(|_| InstallationError::TargetPointerFactNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(record.target.pointer_alignment)
            .map_err(|_| InstallationError::TargetPointerFactNotRepresentable)?,
    );
    push_u16(bytes, record.subsystem.unwrap_or(0));
    push_u16(bytes, 0);
    push_u64(bytes, record.profile_decision.get());
    if let Some(progress) = record.component_progress {
        push_u64(bytes, progress.manifest_identity());
        push_u64(bytes, progress.acceptance_identity());
    }
    bytes.extend_from_slice(record.image.as_bytes());
    push_u64(bytes, record.image_sections.layout.text_address);
    push_u64(bytes, record.image_sections.layout.data_address);
    push_u64(bytes, record.image_sections.layout.bss_address);
    push_u64(
        bytes,
        u64::try_from(record.image_sections.text_byte_count)
            .map_err(|_| InstallationError::InvalidImageSectionLayout)?,
    );
    push_u64(
        bytes,
        u64::try_from(record.image_sections.data_byte_count)
            .map_err(|_| InstallationError::InvalidImageSectionLayout)?,
    );
    bytes.extend_from_slice(record.image_sections.final_data_fingerprint.as_bytes());
    bytes.extend_from_slice(
        record
            .compiler_text_validation
            .encoded_text_digest
            .as_bytes(),
    );
    bytes.extend_from_slice(
        record
            .compiler_text_validation
            .final_compiler_text_digest
            .as_bytes(),
    );
    bytes.extend_from_slice(
        record
            .compiler_text_validation
            .relocation_envelope_digest
            .as_bytes(),
    );
    bytes.extend_from_slice(record.compiler_text_validation.derivation_digest.as_bytes());
    push_u64(
        bytes,
        record
            .compiler_text_validation
            .encoded_text_report_fingerprint,
    );
    push_u64(
        bytes,
        record
            .compiler_text_validation
            .final_compiler_text_report_fingerprint,
    );
    push_u64(
        bytes,
        record
            .compiler_text_validation
            .relocation_envelope_report_fingerprint,
    );
    push_u64(
        bytes,
        record
            .compiler_text_validation
            .checked_instruction_validation_report_fingerprint,
    );
    push_u64(
        bytes,
        record
            .compiler_text_validation
            .checked_instruction_footprint_report_fingerprint,
    );
    push_u64(
        bytes,
        record
            .compiler_text_validation
            .derivation_report_fingerprint,
    );
    push_u64(bytes, text_relocation_count);
    push_u64(bytes, checked_instruction_validation_count);
    Ok(())
}

pub(super) fn decode_installation_header(
    reader: &mut Reader<'_>,
) -> Result<DecodedInstallationHeader, InstallationError> {
    if reader.array::<8>()? != *MAGIC {
        return Err(InstallationError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != INSTALLATION_FORMAT_MARKER {
        return Err(InstallationError::UnsupportedFormatMarker(format_marker));
    }
    let vocabulary_marker_raw = reader.u16()?;
    let vocabulary_marker = VocabularyMarker::new(vocabulary_marker_raw).ok_or(
        InstallationError::UnsupportedVocabularyMarker(vocabulary_marker_raw),
    )?;
    let program_fingerprint = SemanticFingerprint::from_bytes(reader.array()?);
    let architecture = decode_architecture(reader.u8()?)?;
    let object_format = decode_object_format(reader.u8()?)?;
    let subsystem_present = decode_boolean(reader.u8()?)?;
    let component_progress_present = decode_boolean(reader.u8()?)?;
    let pointer_size = usize::try_from(reader.u64()?)
        .map_err(|_| InstallationError::TargetPointerFactNotRepresentable)?;
    let pointer_alignment = usize::try_from(reader.u64()?)
        .map_err(|_| InstallationError::TargetPointerFactNotRepresentable)?;
    let subsystem_raw = reader.u16()?;
    if reader.u16()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let subsystem = if subsystem_present {
        Some(subsystem_raw)
    } else {
        if subsystem_raw != 0 {
            return Err(InstallationError::NonCanonicalSubsystem);
        }
        None
    };
    let profile_decision =
        ProfileDecisionId::new(reader.u64()?).ok_or(InstallationError::ZeroProfileDecision)?;
    let component_progress = component_progress_present
        .then(|| {
            let manifest = std::num::NonZeroU64::new(reader.u64()?)
                .ok_or(InstallationError::ZeroComponentProgressManifestIdentity)?;
            let acceptance = std::num::NonZeroU64::new(reader.u64()?)
                .ok_or(InstallationError::ZeroComponentProgressAcceptanceIdentity)?;
            Ok(super::InstalledComponentProgress {
                manifest,
                acceptance,
            })
        })
        .transpose()?;
    let image = ImageFingerprint(reader.array()?);
    let image_sections = super::InstalledImageSections {
        layout: omega_image::FinalImageLayout {
            text_address: reader.u64()?,
            data_address: reader.u64()?,
            bss_address: reader.u64()?,
        },
        text_byte_count: usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidImageSectionLayout)?,
        data_byte_count: usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidImageSectionLayout)?,
        final_data_fingerprint: super::InitializedDataFingerprint(reader.array()?),
    };
    let encoded_text_digest = EncodedCompilerTextDigest::from_digest(reader.array()?);
    let final_compiler_text_digest = FinalCompilerTextDigest::from_digest(reader.array()?);
    let relocation_envelope_digest =
        CompilerTextRelocationEnvelopeDigest::from_digest(reader.array()?);
    let derivation_digest = CompilerTextDerivationDigest::from_digest(reader.array()?);
    let encoded_text_report_fingerprint = reader.u64()?;
    let final_compiler_text_report_fingerprint = reader.u64()?;
    let relocation_envelope_report_fingerprint = reader.u64()?;
    let checked_instruction_validation_report_fingerprint = reader.u64()?;
    let checked_instruction_footprint_report_fingerprint = reader.u64()?;
    let derivation_report_fingerprint = reader.u64()?;
    let text_relocation_count = usize::try_from(reader.u64()?)
        .map_err(|_| InstallationError::CountNotRepresentable("text relocations"))?;
    let checked_instruction_validation_count = usize::try_from(reader.u64()?)
        .map_err(|_| InstallationError::CountNotRepresentable("checked instructions"))?;
    Ok(DecodedInstallationHeader {
        psi: TerminalPsiIdentity {
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
        component_progress,
        image,
        image_sections,
        compiler_text_validation: CompilerTextValidationEvidence {
            encoded_text_digest,
            final_compiler_text_digest,
            relocation_envelope_digest,
            derivation_digest,
            encoded_text_report_fingerprint,
            final_compiler_text_report_fingerprint,
            relocation_envelope_report_fingerprint,
            checked_instruction_validation_report_fingerprint,
            checked_instruction_footprint_report_fingerprint,
            derivation_report_fingerprint,
            text_relocation_count,
            checked_instruction_validation_count,
        },
    })
}

fn architecture_tag(architecture: Architecture) -> u8 {
    match architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }
}

fn decode_architecture(tag: u8) -> Result<Architecture, InstallationError> {
    match tag {
        1 => Ok(Architecture::Aarch64),
        2 => Ok(Architecture::X86_64),
        _ => Err(InstallationError::InvalidArchitectureTag(tag)),
    }
}

fn object_format_tag(object_format: ObjectFormat) -> u8 {
    match object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }
}

fn decode_object_format(tag: u8) -> Result<ObjectFormat, InstallationError> {
    match tag {
        1 => Ok(ObjectFormat::Elf),
        2 => Ok(ObjectFormat::MachO),
        3 => Ok(ObjectFormat::Coff),
        _ => Err(InstallationError::InvalidObjectFormatTag(tag)),
    }
}
