//! Canonical format-33 installation header codec.
//!
//! The parent retains validation and all count conversions so extraction does
//! not change encode error precedence. This module owns the fixed header bytes.

use omega_image::CompilerTextValidationEvidence;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::ProfileDecisionId;
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::{
    MAGIC, Reader, TERMINAL_INSTALLATION_FORMAT_MARKER, TerminalImageFingerprint,
    TerminalInstallationError, TerminalInstallationRecord, decode_boolean, push_u16, push_u64,
};

pub(super) struct DecodedInstallationHeader {
    pub(super) terminal_psi: TerminalPsiIdentity,
    pub(super) target: NativeTarget,
    pub(super) subsystem: Option<u16>,
    pub(super) profile_decision: ProfileDecisionId,
    pub(super) image: TerminalImageFingerprint,
    pub(super) compiler_text_validation: CompilerTextValidationEvidence,
}

pub(super) fn encode_installation_header(
    bytes: &mut Vec<u8>,
    record: &TerminalInstallationRecord,
    text_relocation_count: u64,
    checked_instruction_validation_count: u64,
) -> Result<(), TerminalInstallationError> {
    bytes.extend_from_slice(MAGIC);
    push_u16(bytes, TERMINAL_INSTALLATION_FORMAT_MARKER);
    push_u16(bytes, record.terminal_psi.vocabulary_marker.get());
    bytes.extend_from_slice(record.terminal_psi.program_fingerprint.as_bytes());
    bytes.push(architecture_tag(record.target.architecture));
    bytes.push(object_format_tag(record.target.object_format));
    bytes.push(u8::from(record.subsystem.is_some()));
    bytes.push(0);
    push_u64(
        bytes,
        u64::try_from(record.target.pointer_size)
            .map_err(|_| TerminalInstallationError::TargetPointerFactNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(record.target.pointer_alignment)
            .map_err(|_| TerminalInstallationError::TargetPointerFactNotRepresentable)?,
    );
    push_u16(bytes, record.subsystem.unwrap_or(0));
    push_u16(bytes, 0);
    push_u64(bytes, record.profile_decision.get());
    bytes.extend_from_slice(record.image.as_bytes());
    push_u64(
        bytes,
        record.compiler_text_validation.encoded_text_fingerprint,
    );
    push_u64(
        bytes,
        record
            .compiler_text_validation
            .final_compiler_text_fingerprint,
    );
    push_u64(
        bytes,
        record
            .compiler_text_validation
            .relocation_envelope_fingerprint,
    );
    push_u64(
        bytes,
        record
            .compiler_text_validation
            .checked_instruction_validation_fingerprint,
    );
    push_u64(
        bytes,
        record
            .compiler_text_validation
            .checked_instruction_footprint_fingerprint,
    );
    push_u64(
        bytes,
        record.compiler_text_validation.derivation_fingerprint,
    );
    push_u64(bytes, text_relocation_count);
    push_u64(bytes, checked_instruction_validation_count);
    Ok(())
}

pub(super) fn decode_installation_header(
    reader: &mut Reader<'_>,
) -> Result<DecodedInstallationHeader, TerminalInstallationError> {
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
    Ok(DecodedInstallationHeader {
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
    })
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
