//! Object-boundary binding for compiler-owned native-fuel transfer entries.
//!
//! This owner re-encodes the structural runtime plan, appends the two exact
//! entry bodies, and introduces only the sponsor-call and `.text`-base
//! relocations named by the target encoder. The result remains pre-install
//! artifact evidence and grants no executable custody.

use omega_installation_evidence::{
    NativeFuelRuntimeEntryIdentity, NativeFuelRuntimeTextSpan, NativeFuelSavedValue,
    NativeFuelTransferRuntimePlanProjection,
};
use omega_object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
    RelocationRecord, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
};
use omega_target::Architecture;
use psi_diagnostics::Diagnostic;

use super::ValidatedNativeFuelArtifact;

fn ranked_plan_preserves_rank_carrier(
    artifact: &ValidatedNativeFuelArtifact,
    plan: &NativeFuelTransferRuntimePlanProjection,
) -> bool {
    if !artifact
        .semantic_artifact()
        .functions()
        .iter()
        .any(|function| function.ranked_u32_countdown.is_some())
    {
        return true;
    }
    let required = match artifact.semantic_artifact().target().architecture {
        Architecture::X86_64 => omega_calling_conventions::MachineRegister::X86Rdi,
        Architecture::Aarch64 => omega_calling_conventions::MachineRegister::Aarch64X(0),
    };
    plan.activation_state_slots()
        .iter()
        .any(|slot| slot.value == NativeFuelSavedValue::Register(required))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NativeFuelTransferRuntimeEncoding {
    X86_64(omega_isa_x86_64::X86NativeFuelTransferRuntimeEncoding),
    Aarch64(omega_isa_aarch64::Aarch64NativeFuelTransferRuntimeEncoding),
}

impl NativeFuelTransferRuntimeEncoding {
    pub(super) fn transfer_bytes(&self) -> &[u8] {
        match self {
            Self::X86_64(encoding) => encoding.transfer_bytes(),
            Self::Aarch64(encoding) => encoding.transfer_bytes(),
        }
    }

    pub(super) fn resume_bytes(&self) -> &[u8] {
        match self {
            Self::X86_64(encoding) => encoding.resume_bytes(),
            Self::Aarch64(encoding) => encoding.resume_bytes(),
        }
    }

    pub(super) fn physical_state_footprint(
        &self,
    ) -> &omega_calling_conventions::StateFootprintEvidence {
        match self {
            Self::X86_64(encoding) => encoding.physical_state_footprint(),
            Self::Aarch64(encoding) => encoding.physical_state_footprint(),
        }
    }

    pub(super) fn realized_sponsor_stack_peak_bytes(&self) -> u64 {
        match self {
            Self::X86_64(encoding) => encoding.realized_sponsor_stack_peak_bytes(),
            Self::Aarch64(encoding) => encoding.realized_sponsor_stack_peak_bytes(),
        }
    }

    fn relocation_records(
        &self,
        transfer_offset: usize,
        resume_offset: usize,
        transfer_symbol: ObjectSymbolHandle,
        resume_symbol: ObjectSymbolHandle,
        sponsor_symbol: ObjectSymbolHandle,
    ) -> Result<Vec<RelocationRecord>, NativeFuelTransferRuntimeError> {
        let text_base_addend = i64::try_from(transfer_offset)
            .ok()
            .and_then(i64::checked_neg)
            .ok_or(NativeFuelTransferRuntimeError::SizeOverflow)?;
        let materialization = |symbol| RelocationOrigin::Materialization {
            object_symbol_handle: symbol,
        };
        let record = |origin, offset, symbol_handle, addend, kind| RelocationRecord {
            origin,
            section: SectionKind::Text,
            offset,
            byte_width: 4,
            symbol_handle,
            addend,
            kind,
        };
        match self {
            Self::X86_64(encoding) => Ok(vec![
                record(
                    materialization(transfer_symbol),
                    transfer_offset
                        .checked_add(encoding.sponsor_call_rel32_field_offset())
                        .ok_or(NativeFuelTransferRuntimeError::SizeOverflow)?,
                    sponsor_symbol,
                    0,
                    RelocationKind::X86_64Relative32,
                ),
                record(
                    materialization(resume_symbol),
                    resume_offset
                        .checked_add(encoding.retry_text_base_rel32_field_offset())
                        .ok_or(NativeFuelTransferRuntimeError::SizeOverflow)?,
                    transfer_symbol,
                    text_base_addend,
                    RelocationKind::X86_64Relative32,
                ),
            ]),
            Self::Aarch64(encoding) => Ok(vec![
                record(
                    materialization(transfer_symbol),
                    transfer_offset
                        .checked_add(encoding.sponsor_call_branch26_offset())
                        .ok_or(NativeFuelTransferRuntimeError::SizeOverflow)?,
                    sponsor_symbol,
                    0,
                    RelocationKind::Aarch64Branch26,
                ),
                record(
                    materialization(resume_symbol),
                    resume_offset
                        .checked_add(encoding.retry_text_page21_offset())
                        .ok_or(NativeFuelTransferRuntimeError::SizeOverflow)?,
                    transfer_symbol,
                    text_base_addend,
                    RelocationKind::Aarch64Page21,
                ),
                record(
                    materialization(resume_symbol),
                    resume_offset
                        .checked_add(encoding.retry_text_page_offset12_offset())
                        .ok_or(NativeFuelTransferRuntimeError::SizeOverflow)?,
                    transfer_symbol,
                    text_base_addend,
                    RelocationKind::Aarch64PageOffset12,
                ),
            ]),
        }
    }
}

fn encode_transfer_runtime(
    plan: &NativeFuelTransferRuntimePlanProjection,
) -> Result<NativeFuelTransferRuntimeEncoding, Diagnostic> {
    match plan.target().architecture {
        Architecture::X86_64 => omega_isa_x86_64::encode_native_fuel_transfer_runtime(plan)
            .map(NativeFuelTransferRuntimeEncoding::X86_64),
        Architecture::Aarch64 => omega_isa_aarch64::encode_native_fuel_transfer_runtime(plan)
            .map(NativeFuelTransferRuntimeEncoding::Aarch64),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFuelRuntimeEntryBinding {
    identity: NativeFuelRuntimeEntryIdentity,
    symbol: ObjectSymbolHandle,
    span: NativeFuelRuntimeTextSpan,
}

impl NativeFuelRuntimeEntryBinding {
    pub const fn identity(self) -> NativeFuelRuntimeEntryIdentity {
        self.identity
    }

    pub const fn symbol(self) -> ObjectSymbolHandle {
        self.symbol
    }

    pub const fn span(self) -> NativeFuelRuntimeTextSpan {
        self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNativeFuelTransferRuntimeArtifact {
    metered_artifact: ValidatedNativeFuelArtifact,
    plan: NativeFuelTransferRuntimePlanProjection,
    object: ObjectPlan,
    relocations: RelocationPlan,
    text_bytes: Vec<u8>,
    sponsor_symbol: ObjectSymbolHandle,
    transfer: NativeFuelRuntimeEntryBinding,
    resume: NativeFuelRuntimeEntryBinding,
}

impl ValidatedNativeFuelTransferRuntimeArtifact {
    pub const fn metered_artifact(&self) -> &ValidatedNativeFuelArtifact {
        &self.metered_artifact
    }

    pub const fn plan(&self) -> &NativeFuelTransferRuntimePlanProjection {
        &self.plan
    }

    pub const fn object(&self) -> &ObjectPlan {
        &self.object
    }

    pub const fn relocations(&self) -> &RelocationPlan {
        &self.relocations
    }

    pub fn text_bytes(&self) -> &[u8] {
        &self.text_bytes
    }

    pub const fn sponsor_symbol(&self) -> ObjectSymbolHandle {
        self.sponsor_symbol
    }

    pub const fn transfer(&self) -> NativeFuelRuntimeEntryBinding {
        self.transfer
    }

    pub const fn resume(&self) -> NativeFuelRuntimeEntryBinding {
        self.resume
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeFuelTransferRuntimeError {
    TargetPlanMismatch,
    UnsupportedTarget(String),
    InvalidSponsorSymbol,
    SymbolCollision,
    SizeOverflow,
}

impl std::fmt::Display for NativeFuelTransferRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for NativeFuelTransferRuntimeError {}

pub fn bind_native_fuel_transfer_runtime(
    metered_artifact: &ValidatedNativeFuelArtifact,
    plan: NativeFuelTransferRuntimePlanProjection,
    sponsor_symbol: ObjectSymbolHandle,
) -> Result<ValidatedNativeFuelTransferRuntimeArtifact, NativeFuelTransferRuntimeError> {
    if plan.target() != metered_artifact.semantic_artifact().target()
        || plan
            .validate_target_policy(metered_artifact.target_policy())
            .is_err()
        || !ranked_plan_preserves_rank_carrier(metered_artifact, &plan)
    {
        return Err(NativeFuelTransferRuntimeError::TargetPlanMismatch);
    }
    let encoding = encode_transfer_runtime(&plan).map_err(|diagnostic| {
        NativeFuelTransferRuntimeError::UnsupportedTarget(diagnostic.to_string())
    })?;

    let mut object = metered_artifact.object().clone();
    if !object.layout.symbols.is_valid(sponsor_symbol) {
        return Err(NativeFuelTransferRuntimeError::InvalidSponsorSymbol);
    }
    if metered_artifact
        .semantic_artifact()
        .functions()
        .iter()
        .any(|function| function.ranked_u32_countdown.is_some())
        && sponsor_symbol == metered_artifact.semantic_artifact().entry_function().symbol
    {
        return Err(NativeFuelTransferRuntimeError::InvalidSponsorSymbol);
    }
    let sponsor = object.layout.symbols.get(sponsor_symbol);
    if sponsor.kind != SymbolKind::Function
        || sponsor.section != SymbolSection::Section(SectionKind::Text)
        || sponsor.size == 0
    {
        return Err(NativeFuelTransferRuntimeError::InvalidSponsorSymbol);
    }

    let transfer_name = runtime_symbol_name("transfer", plan.transfer_entry());
    let resume_name = runtime_symbol_name("resume", plan.resume_entry());
    if object
        .layout
        .symbols
        .iter()
        .any(|(_, symbol)| symbol.name == transfer_name || symbol.name == resume_name)
    {
        return Err(NativeFuelTransferRuntimeError::SymbolCollision);
    }

    let mut text_bytes = metered_artifact.text_bytes().to_vec();
    let transfer_offset = text_bytes.len();
    text_bytes.extend_from_slice(encoding.transfer_bytes());
    let resume_offset = text_bytes.len();
    text_bytes.extend_from_slice(encoding.resume_bytes());

    let transfer_span = NativeFuelRuntimeTextSpan {
        text_offset: transfer_offset,
        byte_count: encoding.transfer_bytes().len(),
    };
    let resume_span = NativeFuelRuntimeTextSpan {
        text_offset: resume_offset,
        byte_count: encoding.resume_bytes().len(),
    };
    let transfer_identity = plan.transfer_entry();
    let resume_identity = plan.resume_entry();
    let transfer_symbol = object.layout.symbols.insert(SymbolPlan {
        name: transfer_name,
        section: SymbolSection::Section(SectionKind::Text),
        offset: transfer_offset,
        size: transfer_span.byte_count,
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    let resume_symbol = object.layout.symbols.insert(SymbolPlan {
        name: resume_name,
        section: SymbolSection::Section(SectionKind::Text),
        offset: resume_offset,
        size: resume_span.byte_count,
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    let text_section = object
        .layout
        .sections
        .iter()
        .find_map(|(handle, section)| (section.kind == SectionKind::Text).then_some(handle))
        .ok_or(NativeFuelTransferRuntimeError::SizeOverflow)?;
    object.layout.sections.get_mut(text_section).size = text_bytes.len();

    let mut relocations = metered_artifact.relocations().clone();
    for record in encoding.relocation_records(
        transfer_offset,
        resume_offset,
        transfer_symbol,
        resume_symbol,
        sponsor_symbol,
    )? {
        relocations.push_record(record);
    }

    Ok(ValidatedNativeFuelTransferRuntimeArtifact {
        metered_artifact: metered_artifact.clone(),
        plan,
        object,
        relocations,
        text_bytes,
        sponsor_symbol,
        transfer: NativeFuelRuntimeEntryBinding {
            identity: transfer_identity,
            symbol: transfer_symbol,
            span: transfer_span,
        },
        resume: NativeFuelRuntimeEntryBinding {
            identity: resume_identity,
            symbol: resume_symbol,
            span: resume_span,
        },
    })
}

fn runtime_symbol_name(kind: &str, identity: NativeFuelRuntimeEntryIdentity) -> String {
    format!(
        "omega_native_fuel_{kind}_{:016x}_{:016x}",
        identity.section_identity, identity.symbol_identity
    )
}

pub(super) fn replay_terminal_native_fuel_transfer_runtime_artifact(
    artifact: &ValidatedNativeFuelTransferRuntimeArtifact,
) -> Result<NativeFuelTransferRuntimeEncoding, Diagnostic> {
    let base = artifact.metered_artifact();
    artifact
        .plan()
        .validate_target_policy(base.target_policy())
        .map_err(|error| {
            Diagnostic::error(format!("native fuel runtime target-plan drift: {error}"))
        })?;
    if !ranked_plan_preserves_rank_carrier(base, artifact.plan()) {
        return Err(Diagnostic::error(
            "ranked native fuel transfer plan does not preserve its ABI rank carrier",
        ));
    }
    let encoding = encode_transfer_runtime(artifact.plan())?;
    if artifact.object.target != base.object().target
        || artifact.relocations.target != base.relocations().target
        || artifact.text_bytes().len()
            != base
                .text_bytes()
                .len()
                .checked_add(encoding.transfer_bytes().len())
                .and_then(|size| size.checked_add(encoding.resume_bytes().len()))
                .ok_or_else(|| Diagnostic::error("native fuel runtime text size overflows"))?
        || !artifact.text_bytes().starts_with(base.text_bytes())
    {
        return Err(Diagnostic::error(
            "native fuel runtime artifact does not extend the exact metered object",
        ));
    }

    for (handle, symbol) in base.object().layout.symbols.iter() {
        if !artifact.object.layout.symbols.is_valid(handle)
            || artifact.object.layout.symbols.get(handle) != symbol
        {
            return Err(Diagnostic::error(
                "native fuel runtime artifact changed a metered object symbol",
            ));
        }
    }
    let expected_symbol_count = base
        .object()
        .layout
        .symbols
        .len()
        .checked_add(2)
        .ok_or_else(|| Diagnostic::error("native fuel runtime symbol count overflows"))?;
    if artifact.object.layout.symbols.len() != expected_symbol_count {
        return Err(Diagnostic::error(
            "native fuel runtime artifact must append exactly two object symbols",
        ));
    }
    if artifact.object.layout.entry_symbol != base.object().layout.entry_symbol
        || artifact.object.layout.function_symbols != base.object().layout.function_symbols
    {
        return Err(Diagnostic::error(
            "native fuel runtime artifact changed metered object roots",
        ));
    }
    let mut saw_text = false;
    for (handle, section) in base.object().layout.sections.iter() {
        let retained = artifact.object.layout.sections.get(handle);
        if section.kind == SectionKind::Text {
            saw_text = true;
            if retained.kind != SectionKind::Text || retained.size != artifact.text_bytes().len() {
                return Err(Diagnostic::error(
                    "native fuel runtime artifact has an invalid extended text section",
                ));
            }
        } else if retained != section {
            return Err(Diagnostic::error(
                "native fuel runtime artifact changed a non-text section",
            ));
        }
    }
    if !saw_text || artifact.object.layout.sections.len() != base.object().layout.sections.len() {
        return Err(Diagnostic::error(
            "native fuel runtime artifact changed the object section inventory",
        ));
    }

    validate_entry_binding(
        artifact,
        artifact.transfer(),
        artifact.plan().transfer_entry(),
        base.text_bytes().len(),
        encoding.transfer_bytes(),
        "transfer",
    )?;
    validate_entry_binding(
        artifact,
        artifact.resume(),
        artifact.plan().resume_entry(),
        base.text_bytes()
            .len()
            .checked_add(encoding.transfer_bytes().len())
            .ok_or_else(|| Diagnostic::error("native fuel runtime resume offset overflows"))?,
        encoding.resume_bytes(),
        "resume",
    )?;

    if !base
        .object()
        .layout
        .symbols
        .is_valid(artifact.sponsor_symbol())
        || !artifact
            .object
            .layout
            .symbols
            .is_valid(artifact.sponsor_symbol())
    {
        return Err(Diagnostic::error(
            "native fuel runtime artifact lost its sponsor symbol",
        ));
    }
    let sponsor = artifact
        .object
        .layout
        .symbols
        .get(artifact.sponsor_symbol());
    if sponsor.kind != SymbolKind::Function
        || sponsor.section != SymbolSection::Section(SectionKind::Text)
        || sponsor.size == 0
    {
        return Err(Diagnostic::error(
            "native fuel runtime sponsor is not an exact text function",
        ));
    }

    for (handle, record) in base.relocations().records() {
        if !artifact.relocations.record_set.records.is_valid(handle)
            || artifact.relocations.record_set.records.get(handle) != record
        {
            return Err(Diagnostic::error(
                "native fuel runtime artifact changed a metered relocation",
            ));
        }
    }
    let expected_relocations = encoding
        .relocation_records(
            artifact.transfer.span.text_offset,
            artifact.resume.span.text_offset,
            artifact.transfer.symbol,
            artifact.resume.symbol,
            artifact.sponsor_symbol,
        )
        .map_err(|error| Diagnostic::error(error.to_string()))?;
    let expected_relocation_count = base
        .relocations()
        .record_count()
        .checked_add(expected_relocations.len())
        .ok_or_else(|| Diagnostic::error("native fuel runtime relocation count overflows"))?;
    if artifact.relocations.record_count() != expected_relocation_count {
        return Err(Diagnostic::error(format!(
            "native fuel runtime artifact must append exactly {} target relocations",
            expected_relocations.len()
        )));
    }
    for expected in expected_relocations {
        if artifact
            .relocations
            .records()
            .filter(|(_, record)| **record == expected)
            .count()
            != 1
        {
            return Err(Diagnostic::error(
                "native fuel runtime artifact has a missing or drifted typed relocation",
            ));
        }
    }
    Ok(encoding)
}

fn validate_entry_binding(
    artifact: &ValidatedNativeFuelTransferRuntimeArtifact,
    binding: NativeFuelRuntimeEntryBinding,
    identity: NativeFuelRuntimeEntryIdentity,
    text_offset: usize,
    expected_bytes: &[u8],
    kind: &str,
) -> Result<(), Diagnostic> {
    let expected_span = NativeFuelRuntimeTextSpan {
        text_offset,
        byte_count: expected_bytes.len(),
    };
    let end = text_offset
        .checked_add(expected_bytes.len())
        .ok_or_else(|| Diagnostic::error("native fuel runtime entry span overflows"))?;
    if binding.identity != identity
        || binding.span != expected_span
        || artifact.text_bytes().get(text_offset..end) != Some(expected_bytes)
        || !artifact.object.layout.symbols.is_valid(binding.symbol)
    {
        return Err(Diagnostic::error(format!(
            "native fuel runtime {kind} entry binding drifted"
        )));
    }
    let symbol = artifact.object.layout.symbols.get(binding.symbol);
    if symbol.name != runtime_symbol_name(kind, identity)
        || symbol.section != SymbolSection::Section(SectionKind::Text)
        || symbol.offset != text_offset
        || symbol.size != expected_bytes.len()
        || symbol.kind != SymbolKind::Function
        || !symbol.import_library.is_empty()
    {
        return Err(Diagnostic::error(format!(
            "native fuel runtime {kind} object symbol drifted"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet};
    use omega_installation_evidence::{
        NativeFuelActivationStateSlot, NativeFuelContextLayout, NativeFuelRuntimeEntryIdentity,
        NativeFuelSavedValue, NativeFuelSponsorStackPlan, NativeFuelTargetPlanProjection,
        SponsorContextTransport,
    };
    use omega_machine_code::{
        MachineCodeFunction, MachineCodePlan, NativeFuelAttribution, NativeFuelSite,
    };
    use omega_target::TargetProfile;
    use omega_target_operations::TerminalPsiProvenance;
    use psi_core::{MachineId, OperationId, ProfileDecisionId};
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
    use psi_terminal_fuel::TerminalFuelSchedule;

    fn transfer_plan() -> NativeFuelTransferRuntimePlanProjection {
        let state = MachineStateSet::new([
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::GeneralRegisters,
            MachineState::Flags,
        ]);
        NativeFuelTransferRuntimePlanProjection::new(
            TargetProfile::LinuxX64,
            TargetProfile::LinuxX64.native_target(),
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx,
            },
            NativeFuelContextLayout {
                byte_size: 96,
                alignment: 16,
                remaining_units_offset: 0,
                unpaid_site_kind_offset: 8,
                unpaid_site_identity_offset: 16,
                required_units_offset: 24,
                transfer_entry_offset: 32,
                retry_code_offset_offset: 40,
                sponsor_stack_top_offset: 48,
                activation_state_offset: 64,
                activation_state_byte_count: 24,
            },
            vec![
                NativeFuelActivationStateSlot {
                    value: NativeFuelSavedValue::Register(MachineRegister::X86Rax),
                    context_offset: 64,
                    byte_count: 8,
                },
                NativeFuelActivationStateSlot {
                    value: NativeFuelSavedValue::Flags,
                    context_offset: 72,
                    byte_count: 8,
                },
                NativeFuelActivationStateSlot {
                    value: NativeFuelSavedValue::StackPointer,
                    context_offset: 80,
                    byte_count: 8,
                },
            ],
            NativeFuelSponsorStackPlan {
                alignment: 16,
                byte_ceiling: 256,
            },
            state,
            state,
            state,
            NativeFuelRuntimeEntryIdentity {
                section_identity: 11,
                symbol_identity: 12,
            },
            NativeFuelRuntimeEntryIdentity {
                section_identity: 11,
                symbol_identity: 13,
            },
        )
        .expect("structural runtime plan")
    }

    fn aarch64_transfer_plan() -> NativeFuelTransferRuntimePlanProjection {
        let state = MachineStateSet::new([
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::GeneralRegisters,
            MachineState::VectorRegisters,
            MachineState::Flags,
        ]);
        NativeFuelTransferRuntimePlanProjection::new(
            TargetProfile::LinuxArm64,
            TargetProfile::LinuxArm64.native_target(),
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::Aarch64X(28),
            },
            NativeFuelContextLayout {
                byte_size: 112,
                alignment: 16,
                remaining_units_offset: 0,
                unpaid_site_kind_offset: 8,
                unpaid_site_identity_offset: 16,
                required_units_offset: 24,
                transfer_entry_offset: 32,
                retry_code_offset_offset: 40,
                sponsor_stack_top_offset: 48,
                activation_state_offset: 64,
                activation_state_byte_count: 40,
            },
            vec![
                NativeFuelActivationStateSlot {
                    value: NativeFuelSavedValue::Register(MachineRegister::Aarch64X(0)),
                    context_offset: 64,
                    byte_count: 8,
                },
                NativeFuelActivationStateSlot {
                    value: NativeFuelSavedValue::Flags,
                    context_offset: 72,
                    byte_count: 8,
                },
                NativeFuelActivationStateSlot {
                    value: NativeFuelSavedValue::Register(MachineRegister::Aarch64V(0)),
                    context_offset: 80,
                    byte_count: 16,
                },
                NativeFuelActivationStateSlot {
                    value: NativeFuelSavedValue::StackPointer,
                    context_offset: 96,
                    byte_count: 8,
                },
            ],
            NativeFuelSponsorStackPlan {
                alignment: 16,
                byte_ceiling: 256,
            },
            state,
            state,
            state,
            NativeFuelRuntimeEntryIdentity {
                section_identity: 21,
                symbol_identity: 22,
            },
            NativeFuelRuntimeEntryIdentity {
                section_identity: 21,
                symbol_identity: 23,
            },
        )
        .expect("structural AArch64 runtime plan")
    }

    fn policy(plan: &NativeFuelTransferRuntimePlanProjection) -> NativeFuelTargetPlanProjection {
        NativeFuelTargetPlanProjection {
            profile: plan.profile(),
            target: plan.target(),
            transport: plan.transport(),
            context: plan.context(),
            transfer_plan_identity: plan.normalized_identity(),
        }
    }

    fn metered_artifact(
        plan: &NativeFuelTransferRuntimePlanProjection,
    ) -> ValidatedNativeFuelArtifact {
        let machine = MachineId::new(1).unwrap();
        let operation = OperationId::new(2).unwrap();
        let source = MachineCodePlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
            },
            target: plan.target(),
            entry: machine,
            functions: vec![MachineCodeFunction {
                machine,
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation],
                    edges: Vec::new(),
                },
                bytes: match plan.target().architecture {
                    Architecture::X86_64 => vec![0xc3],
                    Architecture::Aarch64 => 0xd65f_03c0_u32.to_le_bytes().to_vec(),
                },
                unit_stack: None,
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
                unit_affine_cleanup: None,
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                ranked_u32_countdown: None,
                fuel_attribution: vec![NativeFuelAttribution {
                    schedule: TerminalFuelSchedule::CURRENT.identity(),
                    site: NativeFuelSite::Operation(operation),
                    units: 3,
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 1,
                }],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                structural_return: None,
            }],
        };
        let instrumented = omega_machine_emission::instrument_native_fuel(source, policy(plan))
            .expect("native fuel instrumentation");
        crate::validate_native_fuel_plan(&instrumented).expect("metered object replay")
    }

    fn bound_artifact() -> ValidatedNativeFuelTransferRuntimeArtifact {
        let plan = transfer_plan();
        let metered = metered_artifact(&plan);
        let sponsor = metered.semantic_artifact().entry_function().symbol;
        bind_native_fuel_transfer_runtime(&metered, plan, sponsor).expect("runtime binding")
    }

    fn bound_aarch64_artifact() -> ValidatedNativeFuelTransferRuntimeArtifact {
        let plan = aarch64_transfer_plan();
        let metered = metered_artifact(&plan);
        let sponsor = metered.semantic_artifact().entry_function().symbol;
        bind_native_fuel_transfer_runtime(&metered, plan, sponsor).expect("AArch64 runtime binding")
    }

    #[test]
    fn binds_relocates_and_emits_exact_runtime_evidence() {
        let artifact = bound_artifact();
        let base_len = artifact.metered_artifact().text_bytes().len();
        assert_eq!(artifact.transfer().span().text_offset, base_len);
        assert_eq!(artifact.relocations().record_count(), 2);

        let image = crate::emit_native_fuel_transfer_runtime_executable_image(&artifact, 3)
            .expect("runtime image");
        let evidence = image.transfer_runtime_evidence();
        assert_eq!(evidence.plan(), artifact.plan());
        assert_eq!(evidence.transfer_text().span(), artifact.transfer().span());
        assert_eq!(evidence.resume_text().span(), artifact.resume().span());
        assert_eq!(evidence.sponsor_stack_peak_bytes(), 24);
        assert_ne!(evidence.fingerprint(), 0);
        assert_eq!(image.output().final_image_relocations, 2);
        assert_eq!(
            image
                .output()
                .compiler_text_validation
                .expect("relocation envelope")
                .text_relocation_count,
            2
        );
        assert_ne!(
            evidence.transfer_text().unrelocated_bytes(),
            evidence.transfer_text().final_bytes()
        );
        assert_ne!(
            evidence.resume_text().unrelocated_bytes(),
            evidence.resume_text().final_bytes()
        );
    }

    #[test]
    fn replay_rejects_runtime_byte_symbol_and_relocation_drift() {
        let artifact = bound_artifact();

        let mut byte_drift = artifact.clone();
        byte_drift.text_bytes[byte_drift.transfer.span.text_offset] ^= 1;
        assert!(replay_terminal_native_fuel_transfer_runtime_artifact(&byte_drift).is_err());

        let mut symbol_drift = artifact.clone();
        symbol_drift
            .object
            .layout
            .symbols
            .get_mut(symbol_drift.transfer.symbol)
            .size += 1;
        assert!(replay_terminal_native_fuel_transfer_runtime_artifact(&symbol_drift).is_err());

        let mut relocation_drift = artifact.clone();
        let handle = relocation_drift
            .relocations
            .records()
            .last()
            .expect("runtime relocation")
            .0;
        relocation_drift
            .relocations
            .record_set
            .records
            .get_mut(handle)
            .addend += 1;
        assert!(replay_terminal_native_fuel_transfer_runtime_artifact(&relocation_drift).is_err());
    }

    #[test]
    fn aarch64_binding_appends_and_replays_exact_typed_relocations() {
        let artifact = bound_aarch64_artifact();
        let base_relocations = artifact.metered_artifact().relocations().record_count();
        let appended = artifact
            .relocations()
            .records()
            .skip(base_relocations)
            .map(|(_, relocation)| relocation.clone())
            .collect::<Vec<_>>();
        assert_eq!(appended.len(), 3);
        assert_eq!(appended[0].kind, RelocationKind::Aarch64Branch26);
        assert_eq!(appended[0].symbol_handle, artifact.sponsor_symbol());
        assert_eq!(
            appended[0].offset,
            artifact.transfer().span().text_offset + 40
        );
        assert_eq!(appended[1].kind, RelocationKind::Aarch64Page21);
        assert_eq!(appended[1].symbol_handle, artifact.transfer().symbol());
        assert_eq!(appended[1].offset, artifact.resume().span().text_offset);
        assert_eq!(appended[2].kind, RelocationKind::Aarch64PageOffset12);
        assert_eq!(appended[2].symbol_handle, artifact.transfer().symbol());
        assert_eq!(appended[2].offset, artifact.resume().span().text_offset + 4);
        assert_eq!(appended[1].addend, appended[2].addend);
        assert_eq!(
            appended[1].addend,
            -i64::try_from(artifact.transfer().span().text_offset).unwrap()
        );

        let replayed = replay_terminal_native_fuel_transfer_runtime_artifact(&artifact)
            .expect("AArch64 object replay");
        assert!(matches!(
            replayed,
            NativeFuelTransferRuntimeEncoding::Aarch64(_)
        ));

        let mut kind_drift = artifact.clone();
        let handle = kind_drift
            .relocations
            .records()
            .last()
            .expect("PageOffset12 relocation")
            .0;
        kind_drift
            .relocations
            .record_set
            .records
            .get_mut(handle)
            .kind = RelocationKind::Aarch64Page21;
        assert!(replay_terminal_native_fuel_transfer_runtime_artifact(&kind_drift).is_err());
    }

    #[test]
    fn aarch64_elf_image_replays_targets_and_physical_evidence() {
        let artifact = bound_aarch64_artifact();
        let image = crate::emit_native_fuel_transfer_runtime_executable_image(&artifact, 0)
            .expect("AArch64 transfer runtime image");
        let evidence = image.transfer_runtime_evidence();

        assert_eq!(image.output().format, "elf64-aarch64-executable");
        assert_eq!(image.output().final_image_relocations, 3);
        assert_eq!(
            image
                .output()
                .compiler_text_validation
                .as_ref()
                .expect("relocation envelope")
                .text_relocation_count,
            3
        );
        assert_eq!(evidence.plan(), artifact.plan());
        assert_eq!(evidence.sponsor_stack_peak_bytes(), 16);
        assert_eq!(
            evidence.physical_state_footprint().machine_state(),
            MachineStateSet::new([
                MachineState::GeneralRegisters,
                MachineState::VectorRegisters,
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
            ])
        );
        assert_ne!(
            evidence.transfer_text().unrelocated_bytes(),
            evidence.transfer_text().final_bytes(),
            "the Branch26 sponsor call must be materialized"
        );
        assert_ne!(evidence.fingerprint(), 0);
    }

    #[test]
    fn transfer_runtime_installations_round_trip_both_target_plans() {
        for artifact in [bound_artifact(), bound_aarch64_artifact()] {
            let image = crate::emit_native_fuel_transfer_runtime_executable_image(&artifact, 3)
                .expect("transfer runtime image");
            let record = crate::build_native_fuel_transfer_runtime_installation_record(
                &image,
                ProfileDecisionId::new(1).unwrap(),
            )
            .expect("transfer runtime installation");
            let installed = record
                .native_fuel_transfer_runtime()
                .expect("transfer section");
            assert_eq!(installed.evidence(), image.transfer_runtime_evidence());
            assert_eq!(
                installed.unrelocated_text_byte_count(),
                artifact.text_bytes().len()
            );
            assert_eq!(
                installed.final_text_byte_count(),
                image.output().final_text_bytes.len()
            );

            let encoded =
                crate::encode_installation_record(&record).expect("canonical installation bytes");
            let decoded =
                crate::decode_installation_record(&encoded).expect("canonical installation decode");
            assert_eq!(decoded, record);
            crate::validate_native_fuel_transfer_runtime_installation_record(&decoded, &image)
                .expect("decoded transfer evidence rejoins image");

            let mut fingerprint_drift = encoded;
            *fingerprint_drift
                .last_mut()
                .expect("encoded evidence fingerprint") ^= 1;
            assert!(crate::decode_installation_record(&fingerprint_drift).is_err());
        }
    }
}
