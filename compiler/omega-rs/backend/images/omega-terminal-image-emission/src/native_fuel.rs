//! Independent object-boundary replay of native logical-fuel instrumentation.
//!
//! Source semantics are admitted through the ordinary object constructor. This
//! owner then reconstructs every inserted hot charge and appended cold dispatch
//! from the immutable source plan and rejects any producer-owned offset or byte
//! claim that differs. The result deliberately is not yet an executable object:
//! relocation and symbol translation remain a later, separately validated step.

use omega_object_file::{ObjectPlan, RelocationPlan, SectionKind};
use omega_target::Architecture;
use omega_terminal_installation_evidence::{
    NativeFuelTargetPlanProjection, TerminalFuelAttributionSite,
};
use omega_terminal_machine_code::{
    TerminalMachineCodeFunction, TerminalNativeFuelAttribution, TerminalNativeFuelChargeRecord,
    TerminalNativeFuelInstrumentedPlan, TerminalNativeFuelSite,
};
use psi_core::MachineId;

use super::{TerminalObjectArtifact, TerminalObjectError, build_terminal_object_artifact};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalNativeFuelArtifact {
    semantic_artifact: TerminalObjectArtifact,
    target_policy: NativeFuelTargetPlanProjection,
    object: ObjectPlan,
    relocations: RelocationPlan,
    text_bytes: Vec<u8>,
    functions: Vec<ValidatedTerminalNativeFuelFunction>,
}

impl ValidatedTerminalNativeFuelArtifact {
    pub const fn semantic_artifact(&self) -> &TerminalObjectArtifact {
        &self.semantic_artifact
    }

    pub const fn target_policy(&self) -> NativeFuelTargetPlanProjection {
        self.target_policy
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

    pub fn functions(&self) -> &[ValidatedTerminalNativeFuelFunction] {
        &self.functions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalNativeFuelFunction {
    pub machine: MachineId,
    pub text_offset: usize,
    pub byte_count: usize,
    pub semantic_end_offset: usize,
    pub charges: Vec<TerminalNativeFuelChargeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalNativeFuelValidationError {
    SemanticObject(TerminalObjectError),
    TargetMismatch,
    FunctionCountMismatch,
    FunctionMismatch(MachineId),
    NonCanonicalAttribution(MachineId),
    RecordMismatch(MachineId),
    ByteMismatch(MachineId),
    SizeOverflow,
    Encoding(String),
    ObjectTranslation,
}

impl std::fmt::Display for TerminalNativeFuelValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalNativeFuelValidationError {}

/// Validate source semantics first, then reconstruct the complete metered text
/// without trusting producer-supplied charge offsets, sizes, or bytes.
pub fn validate_terminal_native_fuel_plan(
    plan: &TerminalNativeFuelInstrumentedPlan,
) -> Result<ValidatedTerminalNativeFuelArtifact, TerminalNativeFuelValidationError> {
    if plan.source.target != plan.target_policy.target
        || plan.target_policy.profile.native_target() != plan.source.target
    {
        return Err(TerminalNativeFuelValidationError::TargetMismatch);
    }
    let semantic_artifact = build_terminal_object_artifact(&plan.source)
        .map_err(TerminalNativeFuelValidationError::SemanticObject)?;
    if plan.source.functions.len() != plan.functions.len() {
        return Err(TerminalNativeFuelValidationError::FunctionCountMismatch);
    }

    let architecture = plan.source.target.architecture;
    let mut text_bytes = Vec::new();
    let mut functions = Vec::with_capacity(plan.functions.len());
    for (source, supplied) in plan.source.functions.iter().zip(&plan.functions) {
        if source.machine != supplied.machine {
            return Err(TerminalNativeFuelValidationError::FunctionMismatch(
                source.machine,
            ));
        }
        let text_offset = text_bytes.len();
        let (expected_bytes, semantic_end_offset, charges) =
            replay_function(architecture, &plan.target_policy, source, text_offset)?;
        if supplied.semantic_end_offset != semantic_end_offset || supplied.charges != charges {
            return Err(TerminalNativeFuelValidationError::RecordMismatch(
                source.machine,
            ));
        }
        if supplied.bytes != expected_bytes {
            return Err(TerminalNativeFuelValidationError::ByteMismatch(
                source.machine,
            ));
        }
        let byte_count = expected_bytes.len();
        text_bytes.extend_from_slice(&expected_bytes);
        functions.push(ValidatedTerminalNativeFuelFunction {
            machine: source.machine,
            text_offset,
            byte_count,
            semantic_end_offset,
            charges,
        });
    }

    let (object, relocations) = translate_object(
        &semantic_artifact,
        &plan.source.functions,
        &functions,
        text_bytes.len(),
        architecture,
    )?;
    Ok(ValidatedTerminalNativeFuelArtifact {
        semantic_artifact,
        target_policy: plan.target_policy,
        object,
        relocations,
        text_bytes,
        functions,
    })
}

fn translate_object(
    semantic: &TerminalObjectArtifact,
    source_functions: &[TerminalMachineCodeFunction],
    metered_functions: &[ValidatedTerminalNativeFuelFunction],
    text_size: usize,
    architecture: Architecture,
) -> Result<(ObjectPlan, RelocationPlan), TerminalNativeFuelValidationError> {
    let mut object = semantic.object.clone();
    let text_section = object
        .layout
        .sections
        .iter()
        .find_map(|(handle, section)| (section.kind == SectionKind::Text).then_some(handle))
        .ok_or(TerminalNativeFuelValidationError::ObjectTranslation)?;
    object.layout.sections.get_mut(text_section).size = text_size;

    for (semantic_function, metered_function) in semantic.functions.iter().zip(metered_functions) {
        if semantic_function.machine != metered_function.machine {
            return Err(TerminalNativeFuelValidationError::ObjectTranslation);
        }
        let symbol = object.layout.symbols.get_mut(semantic_function.symbol);
        if symbol.section != omega_object_file::SymbolSection::Section(SectionKind::Text)
            || symbol.offset != semantic_function.text_offset
            || symbol.size != semantic_function.byte_count
        {
            return Err(TerminalNativeFuelValidationError::ObjectTranslation);
        }
        symbol.offset = metered_function.text_offset;
        symbol.size = metered_function.byte_count;
    }

    let mut relocations = semantic.relocations.clone();
    let handles = relocations
        .records()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in handles {
        let record = relocations.record_set.records.get_mut(handle);
        if record.section != SectionKind::Text {
            return Err(TerminalNativeFuelValidationError::ObjectTranslation);
        }
        let owner_symbol = record.origin.symbol_handle();
        let semantic_function = semantic
            .functions
            .iter()
            .find(|function| function.symbol == owner_symbol)
            .ok_or(TerminalNativeFuelValidationError::ObjectTranslation)?;
        let source_function = source_functions
            .iter()
            .find(|function| function.machine == semantic_function.machine)
            .ok_or(TerminalNativeFuelValidationError::ObjectTranslation)?;
        let metered_function = metered_functions
            .iter()
            .find(|function| function.machine == semantic_function.machine)
            .ok_or(TerminalNativeFuelValidationError::ObjectTranslation)?;
        let local_offset = record
            .offset
            .checked_sub(semantic_function.text_offset)
            .filter(|offset| *offset <= source_function.bytes.len())
            .ok_or(TerminalNativeFuelValidationError::ObjectTranslation)?;
        let metered_local_offset = translate_semantic_offset(
            local_offset,
            &source_function.fuel_attribution,
            architecture,
        )?;
        record.offset = metered_function
            .text_offset
            .checked_add(metered_local_offset)
            .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?;
    }
    Ok((object, relocations))
}

fn translate_semantic_offset(
    source_offset: usize,
    attributions: &[TerminalNativeFuelAttribution],
    architecture: Architecture,
) -> Result<usize, TerminalNativeFuelValidationError> {
    let preceding_charges = attributions.partition_point(|row| row.code_offset <= source_offset);
    source_offset
        .checked_add(
            hot_charge_byte_count(architecture)
                .checked_mul(preceding_charges)
                .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?,
        )
        .ok_or(TerminalNativeFuelValidationError::SizeOverflow)
}

fn replay_function(
    architecture: Architecture,
    policy: &NativeFuelTargetPlanProjection,
    source: &TerminalMachineCodeFunction,
    function_text_offset: usize,
) -> Result<(Vec<u8>, usize, Vec<TerminalNativeFuelChargeRecord>), TerminalNativeFuelValidationError>
{
    if source
        .fuel_attribution
        .windows(2)
        .any(|pair| pair[0].code_offset > pair[1].code_offset)
    {
        return Err(TerminalNativeFuelValidationError::NonCanonicalAttribution(
            source.machine,
        ));
    }
    let hot_size = hot_charge_byte_count(architecture);
    let cold_size = cold_dispatch_byte_count(architecture);
    let hot_bytes = hot_size
        .checked_mul(source.fuel_attribution.len())
        .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?;
    let semantic_end_offset = source
        .bytes
        .len()
        .checked_add(hot_bytes)
        .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?;
    let final_size = semantic_end_offset
        .checked_add(
            cold_size
                .checked_mul(source.fuel_attribution.len())
                .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?,
        )
        .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?;
    let mut bytes = Vec::with_capacity(final_size);
    let mut charges = Vec::with_capacity(source.fuel_attribution.len());
    let mut source_cursor = 0usize;

    for (ordinal, attribution) in source.fuel_attribution.iter().copied().enumerate() {
        let charge_code_offset = attribution
            .code_offset
            .checked_add(
                hot_size
                    .checked_mul(ordinal)
                    .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?,
            )
            .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?;
        bytes.extend_from_slice(
            source
                .bytes
                .get(source_cursor..attribution.code_offset)
                .ok_or(TerminalNativeFuelValidationError::NonCanonicalAttribution(
                    source.machine,
                ))?,
        );
        if bytes.len() != charge_code_offset {
            return Err(TerminalNativeFuelValidationError::RecordMismatch(
                source.machine,
            ));
        }
        let cold_dispatch_code_offset = semantic_end_offset
            .checked_add(
                cold_size
                    .checked_mul(ordinal)
                    .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?,
            )
            .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?;
        let branch_origin = charge_code_offset
            .checked_add(failure_branch_origin(architecture))
            .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?;
        let branch_distance = signed_distance(cold_dispatch_code_offset, branch_origin)?;
        bytes.extend_from_slice(&encode_hot_charge(
            architecture,
            policy,
            attribution.units,
            branch_distance,
        )?);
        let semantic_code_offset = bytes.len();
        charges.push(TerminalNativeFuelChargeRecord {
            attribution,
            charge_code_offset,
            charge_byte_count: hot_size,
            semantic_code_offset,
            cold_dispatch_code_offset,
            cold_dispatch_byte_count: cold_size,
        });
        source_cursor = attribution.code_offset;
    }
    bytes.extend_from_slice(&source.bytes[source_cursor..]);
    if bytes.len() != semantic_end_offset {
        return Err(TerminalNativeFuelValidationError::RecordMismatch(
            source.machine,
        ));
    }
    for charge in &charges {
        let retry_text_offset = function_text_offset
            .checked_add(charge.charge_code_offset)
            .ok_or(TerminalNativeFuelValidationError::SizeOverflow)?;
        bytes.extend_from_slice(&encode_cold_dispatch(
            architecture,
            policy,
            charge.attribution,
            u64::try_from(retry_text_offset)
                .map_err(|_| TerminalNativeFuelValidationError::SizeOverflow)?,
        )?);
    }
    if bytes.len() != final_size {
        return Err(TerminalNativeFuelValidationError::SizeOverflow);
    }
    Ok((bytes, semantic_end_offset, charges))
}

fn encode_hot_charge(
    architecture: Architecture,
    policy: &NativeFuelTargetPlanProjection,
    units: u64,
    distance: isize,
) -> Result<Vec<u8>, TerminalNativeFuelValidationError> {
    match architecture {
        Architecture::X86_64 => {
            omega_isa_x86_64::encode_native_fuel_charge(policy, units, distance)
        }
        Architecture::Aarch64 => {
            omega_isa_aarch64::encode_native_fuel_charge(policy, units, distance)
        }
    }
    .map_err(|diagnostic| TerminalNativeFuelValidationError::Encoding(diagnostic.to_string()))
}

fn encode_cold_dispatch(
    architecture: Architecture,
    policy: &NativeFuelTargetPlanProjection,
    attribution: TerminalNativeFuelAttribution,
    retry_text_offset: u64,
) -> Result<Vec<u8>, TerminalNativeFuelValidationError> {
    let site = match attribution.site {
        TerminalNativeFuelSite::Operation(operation) => {
            TerminalFuelAttributionSite::Operation(operation)
        }
        TerminalNativeFuelSite::Edge(edge) => TerminalFuelAttributionSite::Edge(edge),
    };
    match architecture {
        Architecture::X86_64 => omega_isa_x86_64::encode_native_fuel_cold_dispatch(
            policy,
            site,
            attribution.units,
            retry_text_offset,
        ),
        Architecture::Aarch64 => omega_isa_aarch64::encode_native_fuel_cold_dispatch(
            policy,
            site,
            attribution.units,
            retry_text_offset,
        ),
    }
    .map_err(|diagnostic| TerminalNativeFuelValidationError::Encoding(diagnostic.to_string()))
}

const fn hot_charge_byte_count(architecture: Architecture) -> usize {
    match architecture {
        Architecture::X86_64 => omega_isa_x86_64::X86_NATIVE_FUEL_CHARGE_BYTE_COUNT,
        Architecture::Aarch64 => omega_isa_aarch64::AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT,
    }
}

const fn cold_dispatch_byte_count(architecture: Architecture) -> usize {
    match architecture {
        Architecture::X86_64 => omega_isa_x86_64::X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT,
        Architecture::Aarch64 => omega_isa_aarch64::AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT,
    }
}

const fn failure_branch_origin(architecture: Architecture) -> usize {
    match architecture {
        Architecture::X86_64 => omega_isa_x86_64::X86_NATIVE_FUEL_FAILURE_BRANCH_END_OFFSET,
        Architecture::Aarch64 => omega_isa_aarch64::AARCH64_NATIVE_FUEL_FAILURE_BRANCH_OFFSET,
    }
}

fn signed_distance(
    target: usize,
    origin: usize,
) -> Result<isize, TerminalNativeFuelValidationError> {
    isize::try_from(target)
        .map_err(|_| TerminalNativeFuelValidationError::SizeOverflow)?
        .checked_sub(
            isize::try_from(origin).map_err(|_| TerminalNativeFuelValidationError::SizeOverflow)?,
        )
        .ok_or(TerminalNativeFuelValidationError::SizeOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;
    use omega_target::TargetProfile;
    use omega_terminal_installation_evidence::{NativeFuelContextLayout, SponsorContextTransport};
    use omega_terminal_machine_code::{
        TerminalMachineCodePlan, TerminalNativeFuelAttribution, TerminalNativeFuelSite,
    };
    use omega_terminal_target_operations::TerminalPsiProvenance;
    use psi_core::OperationId;
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
    use psi_terminal_fuel::TerminalFuelSchedule;

    fn policy(profile: TargetProfile) -> NativeFuelTargetPlanProjection {
        let register = match profile.native_target().architecture {
            Architecture::X86_64 => MachineRegister::X86Rbx,
            Architecture::Aarch64 => MachineRegister::Aarch64X(28),
        };
        NativeFuelTargetPlanProjection {
            profile,
            target: profile.native_target(),
            transport: SponsorContextTransport::ReservedNonvolatileRegister { register },
            context: NativeFuelContextLayout {
                byte_size: 256,
                alignment: 16,
                remaining_units_offset: 24,
                unpaid_site_kind_offset: 32,
                unpaid_site_identity_offset: 40,
                required_units_offset: 48,
                transfer_entry_offset: 56,
                retry_code_offset_offset: 64,
                sponsor_stack_top_offset: 72,
                activation_state_offset: 80,
                activation_state_byte_count: 176,
            },
            transfer_plan_identity: 9,
        }
    }

    fn source(profile: TargetProfile) -> TerminalMachineCodePlan {
        let machine = MachineId::new(1).unwrap();
        let operation = OperationId::new(2).unwrap();
        let bytes = match profile.native_target().architecture {
            Architecture::X86_64 => vec![0xc3],
            Architecture::Aarch64 => 0xd65f_03c0_u32.to_le_bytes().to_vec(),
        };
        TerminalMachineCodePlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([5; 32]),
            },
            target: profile.native_target(),
            entry: machine,
            functions: vec![omega_terminal_machine_code::TerminalMachineCodeFunction {
                machine,
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation],
                    edges: Vec::new(),
                },
                bytes: bytes.clone(),
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
                fuel_attribution: vec![TerminalNativeFuelAttribution {
                    schedule: TerminalFuelSchedule::CURRENT.identity(),
                    site: TerminalNativeFuelSite::Operation(operation),
                    units: 3,
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: bytes.len(),
                }],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                structural_return: None,
            }],
        }
    }

    fn instrumented(profile: TargetProfile) -> TerminalNativeFuelInstrumentedPlan {
        omega_terminal_machine_emission::instrument_native_fuel(source(profile), policy(profile))
            .expect("producer instrumentation")
    }

    #[test]
    fn independently_replays_x86_and_aarch64_metered_text() {
        for profile in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let plan = instrumented(profile);
            let validated = validate_terminal_native_fuel_plan(&plan).expect("independent replay");
            assert_eq!(
                validated.semantic_artifact().text_bytes(),
                plan.source.functions[0].bytes
            );
            assert_eq!(validated.text_bytes(), plan.functions[0].bytes);
            assert_eq!(validated.functions()[0].text_offset, 0);
            assert_eq!(validated.functions()[0].charges, plan.functions[0].charges);
            let text_section = validated
                .object()
                .layout
                .sections
                .iter()
                .find(|(_, section)| section.kind == SectionKind::Text)
                .unwrap()
                .1;
            assert_eq!(text_section.size, validated.text_bytes().len());
            let symbol = validated.semantic_artifact().functions()[0].symbol;
            assert_eq!(validated.object().layout.symbols.get(symbol).offset, 0);
            assert_eq!(
                validated.object().layout.symbols.get(symbol).size,
                validated.text_bytes().len()
            );
            assert_eq!(validated.relocations().record_count(), 0);
            let image = crate::emit_terminal_native_fuel_executable_image(&validated, 3)
                .expect("metered direct image");
            assert_eq!(image.output().final_text_bytes, validated.text_bytes());
            let projected =
                omega_terminal_installation_evidence::TerminalNativeFuelImageEvidence::charges(
                    &image,
                );
            assert_eq!(projected.len(), 1);
            assert_eq!(projected[0].attribution.text_offset, 0);
            assert_eq!(
                projected[0].charge_text_offset,
                validated.functions()[0].charges[0].charge_code_offset
            );
            assert_eq!(
                projected[0].cold_dispatch_text_offset,
                validated.functions()[0].charges[0].cold_dispatch_code_offset
            );
            assert_eq!(
                image
                    .output()
                    .compiler_text_validation
                    .expect("exact final text")
                    .text_relocation_count,
                0
            );
        }
    }

    #[test]
    fn producer_byte_and_record_mutations_reject() {
        let mut bytes_changed = instrumented(TargetProfile::LinuxX64);
        bytes_changed.functions[0].bytes[0] ^= 1;
        assert_eq!(
            validate_terminal_native_fuel_plan(&bytes_changed),
            Err(TerminalNativeFuelValidationError::ByteMismatch(
                MachineId::new(1).unwrap()
            ))
        );

        let mut record_changed = instrumented(TargetProfile::LinuxX64);
        record_changed.functions[0].charges[0].semantic_code_offset += 1;
        assert_eq!(
            validate_terminal_native_fuel_plan(&record_changed),
            Err(TerminalNativeFuelValidationError::RecordMismatch(
                MachineId::new(1).unwrap()
            ))
        );
    }
}
