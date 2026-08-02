use std::num::NonZeroU64;

use omega_image::CompilerTextValidationEvidence;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::ProfileDecisionId;
use psi_terminal::{SemanticFingerprint, SemanticVersion, TerminalPsiIdentity};
use sha2::{Digest, Sha256};

use crate::{TerminalExecutableImage, can_emit_terminal_executable_image};

pub const TERMINAL_INSTALLATION_FORMAT_VERSION: u16 = 1;
const MAGIC: &[u8; 8] = b"PSIINST\0";
const IMAGE_DOMAIN: &[u8] = b"omega-terminal-installed-image-v1\0";
const RECORD_DOMAIN: &[u8] = b"omega-terminal-installation-record-v1\0";

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

    pub const fn image(&self) -> TerminalImageFingerprint {
        self.image
    }

    pub const fn compiler_text_validation(&self) -> CompilerTextValidationEvidence {
        self.compiler_text_validation
    }
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
    push_u16(&mut bytes, TERMINAL_INSTALLATION_FORMAT_VERSION);
    push_u16(&mut bytes, record.terminal_psi.semantic_version.get());
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
        record.compiler_text_validation.derivation_fingerprint,
    );
    push_u64(&mut bytes, text_relocation_count);
    push_u64(&mut bytes, checked_instruction_validation_count);
    push_u32(&mut bytes, provider_count);
    for provider in &record.selected_provider_plans {
        push_u64(&mut bytes, provider.get());
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
    let format_version = reader.u16()?;
    if format_version != TERMINAL_INSTALLATION_FORMAT_VERSION {
        return Err(TerminalInstallationError::UnsupportedFormatVersion(
            format_version,
        ));
    }
    let semantic_version_raw = reader.u16()?;
    let semantic_version = SemanticVersion::new(semantic_version_raw)
        .ok_or(TerminalInstallationError::ZeroSemanticVersion)?;
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
    if reader.remaining() != 0 {
        return Err(TerminalInstallationError::TrailingBytes(reader.remaining()));
    }

    let record = TerminalInstallationRecord {
        terminal_psi: TerminalPsiIdentity {
            semantic_version,
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
        image,
        compiler_text_validation: CompilerTextValidationEvidence {
            encoded_text_fingerprint,
            final_compiler_text_fingerprint,
            relocation_envelope_fingerprint,
            checked_instruction_validation_fingerprint,
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
    UnsupportedFormatVersion(u16),
    ZeroSemanticVersion,
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
