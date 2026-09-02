use omega_register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};
use psi_core::IntegerValue;

use crate::aarch64_physical_register_model;

mod scalar_call;

pub use scalar_call::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64SelectedFormFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub writes_nzcv: bool,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAarch64SelectedFormEncoding {
    bytes: Vec<u8>,
    footprint: Aarch64SelectedFormFootprint,
}

impl ValidatedAarch64SelectedFormEncoding {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn footprint(&self) -> &Aarch64SelectedFormFootprint {
        &self.footprint
    }
}

/// Canonical 64-bit `MOVN` seed for a shortest complement-seeded immediate
/// materialization. `halfword` is the architectural `hw` field, in `0..=3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Aarch64MovnSeed {
    halfword: u8,
    immediate: u16,
}

impl Aarch64MovnSeed {
    pub const fn halfword(&self) -> u8 {
        self.halfword
    }

    pub const fn immediate(&self) -> u16 {
        self.immediate
    }
}

/// One canonical 64-bit `MOVK` patch following a complement-seeded `MOVN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Aarch64MovkPatch {
    halfword: u8,
    immediate: u16,
}

impl Aarch64MovkPatch {
    pub const fn halfword(&self) -> u8 {
        self.halfword
    }

    pub const fn immediate(&self) -> u16 {
        self.immediate
    }
}

/// The unique shortest `MOVN`-seeded recipe that is strictly smaller than the
/// baseline zero-seeded `MOVZ`/`MOVK` materialization.
///
/// Equal-length recipes choose the lowest possible seed halfword. Patches are
/// then ordered by strictly ascending halfword index.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Aarch64ShortestMovnMaterializationRecipe {
    seed: Aarch64MovnSeed,
    patches: Vec<Aarch64MovkPatch>,
    baseline_byte_count: usize,
}

impl Aarch64ShortestMovnMaterializationRecipe {
    pub const fn seed(&self) -> Aarch64MovnSeed {
        self.seed
    }

    pub fn patches(&self) -> &[Aarch64MovkPatch] {
        &self.patches
    }

    pub const fn baseline_byte_count(&self) -> usize {
        self.baseline_byte_count
    }

    pub fn encoded_byte_count(&self) -> usize {
        (1 + self.patches.len()) * 4
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64SelectedFormEncodingError {
    NonCanonicalPhysicalModel,
    LayoutDependentForm,
    AlternativeMismatch,
    OperandCountMismatch,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideI64Bits,
    ImmediateOutsideU12,
    BranchDisplacementMisaligned,
    BranchDisplacementOutsideImm19,
    MovnMaterializationDoesNotShrink,
    MalformedEncoding,
    EncodedFormMismatch,
}

impl std::fmt::Display for Aarch64SelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid AArch64 selected-form encoding: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64SelectedFormEncodingError {}

/// Encode the canonical layout-resolved realization of
/// `ConditionalBranchNonZero`. AArch64 conditional-branch displacement is
/// measured from the branch instruction address and scaled by four bytes.
pub fn encode_aarch64_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
    let word = 0x5400_0001 | (((word_displacement as u32) & 0x7ffff) << 5);
    let bytes = word.to_le_bytes();
    validate_aarch64_selected_nonzero_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction,
        &bytes,
    )
}

pub fn validate_aarch64_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    branch_word_displacement(byte_displacement_from_instruction)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .filter(|word| word & 0xff00_001f == 0x5400_0001)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    let encoded_imm19 = ((word >> 5) & 0x7ffff) as i32;
    let decoded_words = (encoded_imm19 << 13) >> 13;
    let decoded_bytes = i64::from(decoded_words) * 4;
    if decoded_bytes != byte_displacement_from_instruction {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(SelectedInstructionKind::ConditionalBranchNonZero, &[]),
    })
}

/// Encode the canonical AArch64 unsigned-lower conditional branch. `B.LO`
/// uses condition code `0b0011`, with its signed imm19 measured from the
/// branch instruction address and scaled by four bytes.
pub fn encode_aarch64_selected_u64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchU64LessThan,
    )?;
    let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
    let word = 0x5400_0003 | (((word_displacement as u32) & 0x7ffff) << 5);
    validate_aarch64_selected_u64_less_than_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction,
        &word.to_le_bytes(),
    )
}

/// Independently decode exactly one canonical AArch64 `B.LO imm19`.
pub fn validate_aarch64_selected_u64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchU64LessThan,
    )?;
    branch_word_displacement(byte_displacement_from_instruction)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .filter(|word| word & 0xff00_001f == 0x5400_0003)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    let encoded_imm19 = ((word >> 5) & 0x7ffff) as i32;
    let decoded_words = (encoded_imm19 << 13) >> 13;
    if i64::from(decoded_words) * 4 != byte_displacement_from_instruction {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(SelectedInstructionKind::ConditionalBranchU64LessThan, &[]),
    })
}

/// Encode the canonical AArch64 signed-less-than conditional branch. `B.LT`
/// uses condition code `0b1011`, with its signed imm19 measured from the
/// branch instruction address and scaled by four bytes.
pub fn encode_aarch64_selected_i64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchI64LessThan,
    )?;
    let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
    let word = 0x5400_000b | (((word_displacement as u32) & 0x7ffff) << 5);
    validate_aarch64_selected_i64_less_than_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction,
        &word.to_le_bytes(),
    )
}

/// Independently decode exactly one canonical AArch64 `B.LT imm19`.
pub fn validate_aarch64_selected_i64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchI64LessThan,
    )?;
    branch_word_displacement(byte_displacement_from_instruction)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .filter(|word| word & 0xff00_001f == 0x5400_000b)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    let encoded_imm19 = ((word >> 5) & 0x7ffff) as i32;
    let decoded_words = (encoded_imm19 << 13) >> 13;
    if i64::from(decoded_words) * 4 != byte_displacement_from_instruction {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(SelectedInstructionKind::ConditionalBranchI64LessThan, &[]),
    })
}

fn validate_branch_request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    family: MachineAlternativeFamily,
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    Ok(())
}

fn branch_word_displacement(
    byte_displacement: i64,
) -> Result<i32, Aarch64SelectedFormEncodingError> {
    if byte_displacement % 4 != 0 {
        return Err(Aarch64SelectedFormEncodingError::BranchDisplacementMisaligned);
    }
    let words = byte_displacement / 4;
    if !(-(1_i64 << 18)..(1_i64 << 18)).contains(&words) {
        return Err(Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm19);
    }
    Ok(words as i32)
}

/// Encode the layout-resolved AArch64 realization selected by
/// `Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1`. The displacement is
/// measured from the `CBNZ` instruction address and scaled by four bytes.
pub fn encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
    physical: &ValidatedPhysicalRegisterModel,
    source: RegisterViewId,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = validate_cbnz_request(physical, source)?;
    let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
    let word = 0xb500_0000 | (((word_displacement as u32) & 0x7ffff) << 5) | u32::from(register);
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
        physical,
        source,
        byte_displacement_from_instruction,
        &word.to_le_bytes(),
    )
}

/// Independently decode and validate the exact 64-bit `CBNZ` realization.
pub fn validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
    physical: &ValidatedPhysicalRegisterModel,
    source: RegisterViewId,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = validate_cbnz_request(physical, source)?;
    branch_word_displacement(byte_displacement_from_instruction)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .filter(|word| word & 0xff00_0000 == 0xb500_0000)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    if word & 0x1f != u32::from(register) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let encoded_imm19 = ((word >> 5) & 0x7ffff) as i32;
    let decoded_words = (encoded_imm19 << 13) >> 13;
    if i64::from(decoded_words) * 4 != byte_displacement_from_instruction {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: cbnz_footprint(source),
    })
}

fn validate_cbnz_request(
    physical: &ValidatedPhysicalRegisterModel,
    source: RegisterViewId,
) -> Result<u8, Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    let registers = resolve_registers(physical, &[source])?;
    registers
        .first()
        .copied()
        .ok_or(Aarch64SelectedFormEncodingError::OperandCountMismatch)
}

fn cbnz_footprint(source: RegisterViewId) -> Aarch64SelectedFormFootprint {
    let physical = aarch64_physical_register_model();
    let pc = physical.view_named("pc").unwrap().units.clone();
    Aarch64SelectedFormFootprint {
        register_reads: vec![source],
        register_writes: vec![],
        writes_nzcv: false,
        encoded: MachineEncodedEffects {
            // The selected branch has no operand zero. The optimizer artifact
            // separately qualifies this read by the compare instruction and
            // operand that own it.
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: pc.clone(),
            implicit_unit_defs: pc,
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
        },
    }
}

/// Derive the unique shortest 64-bit `MOVN` seed plus ascending `MOVK`
/// patches. The recipe is available only when it is strictly smaller than the
/// existing zero-seeded selected-form materialization.
pub fn aarch64_shortest_movn_materialization_recipe(
    value: IntegerValue,
) -> Result<Aarch64ShortestMovnMaterializationRecipe, Aarch64SelectedFormEncodingError> {
    shortest_movn_materialization_recipe(integer_bits(value)?)
}

/// Encode the ISA-owned shortest complement-seeded realization of one exact
/// selected `i64` bit pattern. This does not change the baseline selected-form
/// encoder, which remains zero-seeded.
pub fn encode_aarch64_shortest_movn_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
    value: IntegerValue,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = validate_materialization_destination(physical, destination)?;
    let recipe = aarch64_shortest_movn_materialization_recipe(value)?;
    let bytes = encode_movn_materialization_recipe(register, &recipe);
    validate_aarch64_shortest_movn_materialization(physical, destination, value, &bytes)
}

/// Independently decode and validate one exact complement-seeded
/// materialization. The decoder reconstructs both destination and full 64-bit
/// value, then requires the canonical lowest seed, ascending patches, and a
/// strict byte reduction against the unchanged baseline encoding.
pub fn validate_aarch64_shortest_movn_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
    value: IntegerValue,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = validate_materialization_destination(physical, destination)?;
    let value_bits = integer_bits(value)?;
    let expected = shortest_movn_materialization_recipe(value_bits)?;
    let decoded = decode_words(bytes)?;
    let (decoded_value, decoded_recipe) = decode_movn_materialization(&decoded, register)
        .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    if decoded_value != value_bits
        || decoded_recipe.seed != expected.seed
        || decoded_recipe.patches != expected.patches
        || decoded_recipe.baseline_byte_count != expected.baseline_byte_count
        || bytes.len() >= expected.baseline_byte_count
        || bytes != encode_movn_materialization_recipe(register, &expected)
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(
            SelectedInstructionKind::MaterializeI64 { value },
            &[destination],
        ),
    })
}

fn validate_materialization_destination(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
) -> Result<u8, Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    resolve_registers(physical, &[destination])?
        .first()
        .copied()
        .ok_or(Aarch64SelectedFormEncodingError::OperandCountMismatch)
}

pub fn encode_aarch64_selected_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_registers(physical, operands)?;
    validate_return_home(kind, &registers)?;
    let bytes = encode_unchecked(kind, &registers)?;
    validate_aarch64_selected_form_encoding(physical, kind, alternative, operands, &bytes)
}

pub fn validate_aarch64_selected_form_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_registers(physical, operands)?;
    validate_return_home(kind, &registers)?;
    let decoded = decode_words(bytes)?;
    validate_decoded(kind, &registers, &decoded)?;
    let canonical = encode_unchecked(kind, &registers)?;
    if bytes != canonical {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(kind, operands),
    })
}

fn validate_request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    let (family, count) = family_and_operand_count(kind)?;
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    if operands.len() != count {
        return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
    }
    Ok(())
}

fn family_and_operand_count(
    kind: SelectedInstructionKind,
) -> Result<(MachineAlternativeFamily, usize), Aarch64SelectedFormEncodingError> {
    Ok(match kind {
        SelectedInstructionKind::CompareI64Zero => (MachineAlternativeFamily::CompareI64Zero, 1),
        SelectedInstructionKind::CompareI64 => (MachineAlternativeFamily::CompareI64, 2),
        SelectedInstructionKind::MaterializeI64 { .. } => {
            (MachineAlternativeFamily::MaterializeI64, 1)
        }
        SelectedInstructionKind::CopyI64 => (MachineAlternativeFamily::CopyI64, 2),
        SelectedInstructionKind::ExactAddI64 { .. } => (MachineAlternativeFamily::ExactAddI64, 3),
        SelectedInstructionKind::ExactSubtractI64 { .. } => {
            (MachineAlternativeFamily::ExactSubtractI64, 3)
        }
        SelectedInstructionKind::ExactAddI64Immediate { .. } => {
            (MachineAlternativeFamily::ExactAddI64Immediate, 2)
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
            (MachineAlternativeFamily::ExactSubtractI64Immediate, 2)
        }
        SelectedInstructionKind::ReturnI64 => (MachineAlternativeFamily::ReturnI64, 1),
        SelectedInstructionKind::ReturnUnit => (MachineAlternativeFamily::ReturnUnit, 0),
        SelectedInstructionKind::ConditionalBranchNonZero => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::ConditionalBranchU64LessThan => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::ConditionalBranchI64LessThan => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::CallI64 { .. } => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
    })
}

fn validate_return_home(
    kind: SelectedInstructionKind,
    registers: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if matches!(kind, SelectedInstructionKind::ReturnI64) && registers != [0] {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

fn resolve_registers(
    physical: &ValidatedPhysicalRegisterModel,
    operands: &[RegisterViewId],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    operands
        .iter()
        .map(|id| {
            let view = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == *id)
                .ok_or(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id))?;
            let Some(index) = view.name.strip_prefix('x') else {
                return Err(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id));
            };
            let index = index
                .parse::<u8>()
                .ok()
                .filter(|index| *index <= 30)
                .ok_or(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id))?;
            if view.bits != 64 || !view.allocatable {
                return Err(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id));
            }
            Ok(index)
        })
        .collect()
}

fn integer_bits(value: IntegerValue) -> Result<u64, Aarch64SelectedFormEncodingError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .map(|value| value as u64)
            .map_err(|_| Aarch64SelectedFormEncodingError::IntegerOutsideI64Bits),
        IntegerValue::Unsigned(value) => u64::try_from(value)
            .map_err(|_| Aarch64SelectedFormEncodingError::IntegerOutsideI64Bits),
    }
}

fn u12(value: IntegerValue) -> Result<u16, Aarch64SelectedFormEncodingError> {
    match value {
        IntegerValue::Unsigned(value) if value <= 4095 => Ok(value as u16),
        _ => Err(Aarch64SelectedFormEncodingError::ImmediateOutsideU12),
    }
}

fn encode_unchecked(
    kind: SelectedInstructionKind,
    registers: &[u8],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    let mut words = Vec::new();
    match kind {
        SelectedInstructionKind::MaterializeI64 { value } => {
            append_canonical_materialization(&mut words, registers[0], integer_bits(value)?);
        }
        SelectedInstructionKind::CopyI64 => {
            words.push(0xaa00_03e0 | (u32::from(registers[0]) << 16) | u32::from(registers[1]));
        }
        SelectedInstructionKind::CompareI64Zero => {
            words.push(0xf100_001f | (u32::from(registers[0]) << 5));
        }
        SelectedInstructionKind::CompareI64 => {
            words.push(
                0xeb00_001f | (u32::from(registers[1]) << 16) | (u32::from(registers[0]) << 5),
            );
        }
        SelectedInstructionKind::ExactAddI64 { .. } => {
            words.push(
                0x8b00_0000
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[2]),
            );
        }
        SelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            words.push(
                0x9100_0000
                    | (u32::from(u12(immediate)?) << 10)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[1]),
            );
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => {
            words.push(
                0xcb00_0000
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[2]),
            );
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
            words.push(
                0xd100_0000
                    | (u32::from(u12(immediate)?) << 10)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[1]),
            );
        }
        SelectedInstructionKind::ReturnI64 | SelectedInstructionKind::ReturnUnit => {
            words.push(0xd65f_03c0)
        }
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::CallI64 { .. } => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
    }
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
}

fn append_canonical_materialization(words: &mut Vec<u32>, register: u8, value: u64) {
    let chunks = std::array::from_fn::<_, 4, _>(|index| ((value >> (index * 16)) & 0xffff) as u16);
    words.push(0xd280_0000 | (u32::from(chunks[0]) << 5) | u32::from(register));
    for (index, chunk) in chunks.into_iter().enumerate().skip(1) {
        if chunk != 0 {
            words.push(
                0xf280_0000
                    | (u32::try_from(index).expect("halfword index fits u32") << 21)
                    | (u32::from(chunk) << 5)
                    | u32::from(register),
            );
        }
    }
}

fn shortest_movn_materialization_recipe(
    value: u64,
) -> Result<Aarch64ShortestMovnMaterializationRecipe, Aarch64SelectedFormEncodingError> {
    let chunks = std::array::from_fn::<_, 4, _>(|index| ((value >> (index * 16)) & 0xffff) as u16);
    let seed_halfword = chunks
        .iter()
        .position(|chunk| *chunk != u16::MAX)
        .unwrap_or(0) as u8;
    let seed = Aarch64MovnSeed {
        halfword: seed_halfword,
        immediate: !chunks[usize::from(seed_halfword)],
    };
    let patches = chunks
        .into_iter()
        .enumerate()
        .filter_map(|(halfword, immediate)| {
            (halfword != usize::from(seed_halfword) && immediate != u16::MAX).then_some(
                Aarch64MovkPatch {
                    halfword: halfword as u8,
                    immediate,
                },
            )
        })
        .collect::<Vec<_>>();
    let mut baseline_words = Vec::new();
    append_canonical_materialization(&mut baseline_words, 0, value);
    let baseline_byte_count = baseline_words.len() * 4;
    let recipe = Aarch64ShortestMovnMaterializationRecipe {
        seed,
        patches,
        baseline_byte_count,
    };
    if recipe.encoded_byte_count() >= baseline_byte_count {
        return Err(Aarch64SelectedFormEncodingError::MovnMaterializationDoesNotShrink);
    }
    Ok(recipe)
}

fn encode_movn_materialization_recipe(
    register: u8,
    recipe: &Aarch64ShortestMovnMaterializationRecipe,
) -> Vec<u8> {
    let mut words = Vec::with_capacity(1 + recipe.patches.len());
    words.push(
        0x9280_0000
            | (u32::from(recipe.seed.halfword) << 21)
            | (u32::from(recipe.seed.immediate) << 5)
            | u32::from(register),
    );
    words.extend(recipe.patches.iter().map(|patch| {
        0xf280_0000
            | (u32::from(patch.halfword) << 21)
            | (u32::from(patch.immediate) << 5)
            | u32::from(register)
    }));
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecodedWord {
    MovN {
        register: u8,
        shift: u8,
        immediate: u16,
    },
    MovZ {
        register: u8,
        shift: u8,
        immediate: u16,
    },
    MovK {
        register: u8,
        shift: u8,
        immediate: u16,
    },
    Copy {
        source: u8,
        destination: u8,
    },
    CompareZero {
        source: u8,
    },
    Compare {
        left: u8,
        right: u8,
    },
    Add {
        left: u8,
        right: u8,
        destination: u8,
    },
    AddImmediate {
        source: u8,
        immediate: u16,
        destination: u8,
    },
    Subtract {
        left: u8,
        right: u8,
        destination: u8,
    },
    SubtractImmediate {
        source: u8,
        immediate: u16,
        destination: u8,
    },
    Return,
}

fn decode_words(bytes: &[u8]) -> Result<Vec<DecodedWord>, Aarch64SelectedFormEncodingError> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| decode_word(u32::from_le_bytes(*bytes)))
        .collect()
}

fn decode_word(word: u32) -> Result<DecodedWord, Aarch64SelectedFormEncodingError> {
    let register = (word & 0x1f) as u8;
    let shift = ((word >> 21) & 0x3) as u8;
    let immediate = ((word >> 5) & 0xffff) as u16;
    if word & 0xff80_0000 == 0x9280_0000 {
        return Ok(DecodedWord::MovN {
            register,
            shift,
            immediate,
        });
    }
    if word & 0xffe0_0000 == 0xd280_0000 {
        return Ok(DecodedWord::MovZ {
            register,
            shift,
            immediate,
        });
    }
    if word & 0xff80_0000 == 0xf280_0000 {
        return Ok(DecodedWord::MovK {
            register,
            shift,
            immediate,
        });
    }
    if word & 0xffe0_ffe0 == 0xaa00_03e0 {
        return Ok(DecodedWord::Copy {
            source: ((word >> 16) & 0x1f) as u8,
            destination: register,
        });
    }
    if word & 0xffff_fc1f == 0xf100_001f {
        return Ok(DecodedWord::CompareZero {
            source: ((word >> 5) & 0x1f) as u8,
        });
    }
    if word & 0xffe0_fc1f == 0xeb00_001f {
        return Ok(DecodedWord::Compare {
            left: ((word >> 5) & 0x1f) as u8,
            right: ((word >> 16) & 0x1f) as u8,
        });
    }
    if word & 0xffe0_fc00 == 0x8b00_0000 {
        return Ok(DecodedWord::Add {
            left: ((word >> 5) & 0x1f) as u8,
            right: ((word >> 16) & 0x1f) as u8,
            destination: register,
        });
    }
    if word & 0xffc0_0000 == 0x9100_0000 {
        return Ok(DecodedWord::AddImmediate {
            source: ((word >> 5) & 0x1f) as u8,
            immediate: ((word >> 10) & 0xfff) as u16,
            destination: register,
        });
    }
    if word & 0xffe0_fc00 == 0xcb00_0000 {
        return Ok(DecodedWord::Subtract {
            left: ((word >> 5) & 0x1f) as u8,
            right: ((word >> 16) & 0x1f) as u8,
            destination: register,
        });
    }
    if word & 0xffc0_0000 == 0xd100_0000 {
        return Ok(DecodedWord::SubtractImmediate {
            source: ((word >> 5) & 0x1f) as u8,
            immediate: ((word >> 10) & 0xfff) as u16,
            destination: register,
        });
    }
    if word == 0xd65f_03c0 {
        return Ok(DecodedWord::Return);
    }
    Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
}

fn validate_decoded(
    kind: SelectedInstructionKind,
    registers: &[u8],
    decoded: &[DecodedWord],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    let valid = match kind {
        SelectedInstructionKind::MaterializeI64 { value } => {
            decode_materialization(decoded, registers[0]) == integer_bits(value).ok()
        }
        SelectedInstructionKind::CopyI64 => {
            decoded
                == [DecodedWord::Copy {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::CompareI64Zero => {
            decoded
                == [DecodedWord::CompareZero {
                    source: registers[0],
                }]
        }
        SelectedInstructionKind::CompareI64 => {
            decoded
                == [DecodedWord::Compare {
                    left: registers[0],
                    right: registers[1],
                }]
        }
        SelectedInstructionKind::ExactAddI64 { .. } => {
            decoded
                == [DecodedWord::Add {
                    left: registers[0],
                    right: registers[1],
                    destination: registers[2],
                }]
        }
        SelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            decoded
                == [DecodedWord::AddImmediate {
                    source: registers[0],
                    immediate: u12(immediate)?,
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => {
            decoded
                == [DecodedWord::Subtract {
                    left: registers[0],
                    right: registers[1],
                    destination: registers[2],
                }]
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
            decoded
                == [DecodedWord::SubtractImmediate {
                    source: registers[0],
                    immediate: u12(immediate)?,
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::ReturnI64 | SelectedInstructionKind::ReturnUnit => {
            decoded == [DecodedWord::Return]
        }
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::CallI64 { .. } => false,
    };
    if valid {
        Ok(())
    } else {
        Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    }
}

fn decode_materialization(decoded: &[DecodedWord], register: u8) -> Option<u64> {
    let (mut value, start) = match decoded.first()? {
        DecodedWord::MovZ {
            register: actual,
            shift: 0,
            immediate,
        } if *actual == register => (u64::from(*immediate), 1),
        _ => return None,
    };
    let mut previous_shift = 0;
    for word in &decoded[start..] {
        let DecodedWord::MovK {
            register: actual,
            shift,
            immediate,
        } = word
        else {
            return None;
        };
        if *actual != register || *shift <= previous_shift || *shift > 3 || *immediate == 0 {
            return None;
        }
        previous_shift = *shift;
        let shift = u64::from(*shift) * 16;
        value = (value & !(0xffff_u64 << shift)) | (u64::from(*immediate) << shift);
    }
    Some(value)
}

fn decode_movn_materialization(
    decoded: &[DecodedWord],
    register: u8,
) -> Option<(u64, Aarch64ShortestMovnMaterializationRecipe)> {
    let DecodedWord::MovN {
        register: actual,
        shift,
        immediate,
    } = *decoded.first()?
    else {
        return None;
    };
    if actual != register || shift > 3 {
        return None;
    }
    let seed_shift = u64::from(shift) * 16;
    let mut value = u64::MAX;
    value = (value & !(0xffff_u64 << seed_shift)) | (u64::from(!immediate) << seed_shift);
    let mut patches = Vec::with_capacity(decoded.len().saturating_sub(1));
    let mut previous_halfword = None;
    for word in &decoded[1..] {
        let DecodedWord::MovK {
            register: actual,
            shift,
            immediate,
        } = *word
        else {
            return None;
        };
        if actual != register
            || shift > 3
            || previous_halfword.is_some_and(|previous| shift <= previous)
        {
            return None;
        }
        previous_halfword = Some(shift);
        let patch_shift = u64::from(shift) * 16;
        value = (value & !(0xffff_u64 << patch_shift)) | (u64::from(immediate) << patch_shift);
        patches.push(Aarch64MovkPatch {
            halfword: shift,
            immediate,
        });
    }
    Some((
        value,
        Aarch64ShortestMovnMaterializationRecipe {
            seed: Aarch64MovnSeed {
                halfword: shift,
                immediate,
            },
            patches,
            baseline_byte_count: {
                let mut baseline = Vec::new();
                append_canonical_materialization(&mut baseline, register, value);
                baseline.len() * 4
            },
        },
    ))
}

fn footprint(
    kind: SelectedInstructionKind,
    operands: &[RegisterViewId],
) -> Aarch64SelectedFormFootprint {
    let (reads, writes, writes_nzcv) = match kind {
        SelectedInstructionKind::MaterializeI64 { .. } => (vec![], vec![operands[0]], false),
        SelectedInstructionKind::CopyI64 => (vec![operands[0]], vec![operands[1]], false),
        SelectedInstructionKind::CompareI64Zero => (vec![operands[0]], vec![], true),
        SelectedInstructionKind::CompareI64 => (vec![operands[0], operands[1]], vec![], true),
        SelectedInstructionKind::ExactAddI64 { .. }
        | SelectedInstructionKind::ExactSubtractI64 { .. } => {
            (vec![operands[0], operands[1]], vec![operands[2]], false)
        }
        SelectedInstructionKind::ExactAddI64Immediate { .. }
        | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
            (vec![operands[0]], vec![operands[1]], false)
        }
        SelectedInstructionKind::ReturnI64 | SelectedInstructionKind::ReturnUnit => {
            (vec![], vec![], false)
        }
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::CallI64 { .. } => (vec![], vec![], false),
    };
    let physical = aarch64_physical_register_model();
    let units = |name: &str| physical.view_named(name).unwrap().units.clone();
    let encoded = if matches!(
        kind,
        SelectedInstructionKind::ReturnI64 | SelectedInstructionKind::ReturnUnit
    ) {
        MachineEncodedEffects {
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: units("x30"),
            implicit_unit_defs: units("pc"),
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                target: physical.view_named("x30").unwrap().id,
            },
        }
    } else if matches!(
        kind,
        SelectedInstructionKind::ConditionalBranchNonZero
            | SelectedInstructionKind::ConditionalBranchU64LessThan
            | SelectedInstructionKind::ConditionalBranchI64LessThan
    ) {
        let mut uses = units("nzcv");
        uses.extend(units("pc"));
        uses.sort_unstable();
        uses.dedup();
        MachineEncodedEffects {
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: uses,
            implicit_unit_defs: units("pc"),
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
        }
    } else {
        let mut effects = MachineEncodedEffects::fallthrough_v1(
            match kind {
                SelectedInstructionKind::MaterializeI64 { .. } => vec![],
                SelectedInstructionKind::CopyI64
                | SelectedInstructionKind::CompareI64Zero
                | SelectedInstructionKind::ExactAddI64Immediate { .. }
                | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => vec![0],
                SelectedInstructionKind::CompareI64 => vec![0, 1],
                SelectedInstructionKind::ExactAddI64 { .. }
                | SelectedInstructionKind::ExactSubtractI64 { .. } => vec![0, 1],
                _ => unreachable!("control forms handled separately"),
            },
            match kind {
                SelectedInstructionKind::MaterializeI64 { .. } => vec![0],
                SelectedInstructionKind::CopyI64
                | SelectedInstructionKind::ExactAddI64Immediate { .. }
                | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => vec![1],
                SelectedInstructionKind::ExactAddI64 { .. }
                | SelectedInstructionKind::ExactSubtractI64 { .. } => vec![2],
                SelectedInstructionKind::CompareI64Zero => vec![],
                SelectedInstructionKind::CompareI64 => vec![],
                _ => unreachable!("control forms handled separately"),
            },
        );
        if writes_nzcv {
            effects.implicit_unit_defs = units("nzcv");
        }
        effects
    };
    Aarch64SelectedFormFootprint {
        register_reads: reads,
        register_writes: writes,
        writes_nzcv,
        encoded,
    }
}

#[cfg(test)]
mod tests {
    use omega_register_model::validate_physical_register_model;
    use omega_selected_instructions::MachineAlternativeFamily;
    use psi_core::{IntegerValue, MachineId, ObligationId};

    use super::*;

    fn alternative(family: MachineAlternativeFamily) -> MachineAlternativeKey {
        MachineAlternativeKey { family, variant: 0 }
    }

    #[test]
    fn scalar_call_is_explicitly_refused_before_encoding() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        assert_eq!(
            encode_aarch64_selected_form(
                &physical,
                SelectedInstructionKind::CallI64 {
                    callee: MachineId::new(1).unwrap(),
                },
                alternative(MachineAlternativeFamily::ReturnUnit),
                &[],
            ),
            Err(Aarch64SelectedFormEncodingError::LayoutDependentForm),
        );
    }

    fn movn(register: u8, halfword: u8, immediate: u16) -> [u8; 4] {
        (0x9280_0000
            | (u32::from(halfword) << 21)
            | (u32::from(immediate) << 5)
            | u32::from(register))
        .to_le_bytes()
    }

    fn movk(register: u8, halfword: u8, immediate: u16) -> [u8; 4] {
        (0xf280_0000
            | (u32::from(halfword) << 21)
            | (u32::from(immediate) << 5)
            | u32::from(register))
        .to_le_bytes()
    }

    #[test]
    fn zero_seeded_materialization_is_canonical_and_decoded_independently() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let x9 = physical.model().view_named("x9").unwrap().id;
        for (value, byte_count) in [
            (IntegerValue::Unsigned(0), 4),
            (IntegerValue::Unsigned(u64::MAX as u128), 16),
            (IntegerValue::Unsigned(0x1234_0000_5678_0000), 12),
            (IntegerValue::Unsigned(0x1234_5678_9abc_def0), 16),
        ] {
            let kind = SelectedInstructionKind::MaterializeI64 { value };
            let encoded = encode_aarch64_selected_form(
                &physical,
                kind,
                alternative(MachineAlternativeFamily::MaterializeI64),
                &[x9],
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), byte_count);
            let mut corrupted = encoded.bytes().to_vec();
            corrupted[0] ^= 0x20;
            assert!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative(MachineAlternativeFamily::MaterializeI64),
                    &[x9],
                    &corrupted,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn shortest_movn_materialization_shrinks_all_ones_and_high_ones() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let x9 = physical.model().view_named("x9").unwrap().id;
        for (value, baseline_bytes, movn_bytes, seed, patches) in [
            (
                IntegerValue::Unsigned(u64::MAX as u128),
                16,
                4,
                Aarch64MovnSeed {
                    halfword: 0,
                    immediate: 0,
                },
                vec![],
            ),
            (
                IntegerValue::Unsigned(0xffff_ffff_0000_0000),
                12,
                8,
                Aarch64MovnSeed {
                    halfword: 0,
                    immediate: u16::MAX,
                },
                vec![Aarch64MovkPatch {
                    halfword: 1,
                    immediate: 0,
                }],
            ),
        ] {
            let recipe = aarch64_shortest_movn_materialization_recipe(value).unwrap();
            assert_eq!(recipe.seed(), seed);
            assert_eq!(recipe.patches(), patches);
            assert_eq!(recipe.baseline_byte_count(), baseline_bytes);
            assert_eq!(recipe.encoded_byte_count(), movn_bytes);

            let baseline = encode_aarch64_selected_form(
                &physical,
                SelectedInstructionKind::MaterializeI64 { value },
                alternative(MachineAlternativeFamily::MaterializeI64),
                &[x9],
            )
            .unwrap();
            let encoded =
                encode_aarch64_shortest_movn_materialization(&physical, x9, value).unwrap();
            assert_eq!(baseline.bytes().len(), baseline_bytes);
            assert_eq!(encoded.bytes().len(), movn_bytes);
            assert!(encoded.bytes().len() < baseline.bytes().len());
            assert_eq!(encoded.footprint().register_writes, [x9]);
            validate_aarch64_shortest_movn_materialization(&physical, x9, value, encoded.bytes())
                .unwrap();
        }
    }

    #[test]
    fn movn_recipe_rejects_zero_small_and_equal_length_baselines() {
        for value in [
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(0x5678_ffff_0000_1234),
        ] {
            assert_eq!(
                aarch64_shortest_movn_materialization_recipe(value),
                Err(Aarch64SelectedFormEncodingError::MovnMaterializationDoesNotShrink)
            );
        }

        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let x9 = physical.model().view_named("x9").unwrap().id;
        let zero_movn = [
            movn(9, 0, u16::MAX),
            movk(9, 1, 0),
            movk(9, 2, 0),
            movk(9, 3, 0),
        ]
        .concat();
        assert_eq!(
            validate_aarch64_shortest_movn_materialization(
                &physical,
                x9,
                IntegerValue::Unsigned(0),
                &zero_movn,
            ),
            Err(Aarch64SelectedFormEncodingError::MovnMaterializationDoesNotShrink)
        );
    }

    #[test]
    fn movn_recipe_chooses_the_lowest_seed_among_minimum_count_recipes() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let x9 = physical.model().view_named("x9").unwrap().id;
        let value = IntegerValue::Unsigned(0xffff_ffff_0000_0000);
        let recipe = aarch64_shortest_movn_materialization_recipe(value).unwrap();
        assert_eq!(recipe.seed().halfword(), 0);

        // Seeding halfword one has the same instruction count and reconstructs
        // the same bits, but it is not the canonical lowest seed.
        let noncanonical = [movn(9, 1, u16::MAX), movk(9, 0, 0)].concat();
        assert!(
            validate_aarch64_shortest_movn_materialization(&physical, x9, value, &noncanonical,)
                .is_err()
        );
    }

    #[test]
    fn movn_recipe_is_bit_pattern_canonical_across_signedness() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let x9 = physical.model().view_named("x9").unwrap().id;
        for (signed, unsigned) in [
            (
                IntegerValue::Signed(-1),
                IntegerValue::Unsigned(u64::MAX as u128),
            ),
            (
                IntegerValue::Signed(-4_294_967_296),
                IntegerValue::Unsigned(0xffff_ffff_0000_0000),
            ),
        ] {
            assert_eq!(
                aarch64_shortest_movn_materialization_recipe(signed).unwrap(),
                aarch64_shortest_movn_materialization_recipe(unsigned).unwrap()
            );
            assert_eq!(
                encode_aarch64_shortest_movn_materialization(&physical, x9, signed)
                    .unwrap()
                    .bytes(),
                encode_aarch64_shortest_movn_materialization(&physical, x9, unsigned)
                    .unwrap()
                    .bytes()
            );
        }
    }

    #[test]
    fn movn_validation_rejects_opcode_destination_value_recipe_and_order_corruption() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let x9 = physical.model().view_named("x9").unwrap().id;
        let x10 = physical.model().view_named("x10").unwrap().id;
        let value = IntegerValue::Unsigned(0xffff_0003_0002_0001);
        let encoded = encode_aarch64_shortest_movn_materialization(&physical, x9, value).unwrap();
        assert_eq!(encoded.bytes().len(), 12);

        let rejects = |bytes: &[u8]| {
            assert!(
                validate_aarch64_shortest_movn_materialization(&physical, x9, value, bytes,)
                    .is_err()
            );
        };

        let mut wrong_opcode = encoded.bytes().to_vec();
        wrong_opcode[..4].copy_from_slice(&0xd280_0009_u32.to_le_bytes());
        rejects(&wrong_opcode);

        let mut wrong_destination = encoded.bytes().to_vec();
        wrong_destination[0] = (wrong_destination[0] & !0x1f) | 10;
        rejects(&wrong_destination);
        assert!(
            validate_aarch64_shortest_movn_materialization(&physical, x10, value, encoded.bytes(),)
                .is_err()
        );

        let all_ones = IntegerValue::Unsigned(u64::MAX as u128);
        for corrupted in [movn(9, 1, 0).to_vec(), movn(9, 0, 1).to_vec()] {
            assert!(
                validate_aarch64_shortest_movn_materialization(
                    &physical,
                    x9,
                    all_ones,
                    &corrupted,
                )
                .is_err()
            );
        }

        let mut reversed_patches = encoded.bytes().to_vec();
        let first_patch = reversed_patches[4..8].to_vec();
        let second_patch = reversed_patches[8..12].to_vec();
        reversed_patches[4..8].copy_from_slice(&second_patch);
        reversed_patches[8..12].copy_from_slice(&first_patch);
        rejects(&reversed_patches);

        let mut wrong_immediate = encoded.bytes().to_vec();
        wrong_immediate[4] ^= 0x20;
        rejects(&wrong_immediate);

        let mut redundant_patch = encode_aarch64_shortest_movn_materialization(
            &physical,
            x9,
            IntegerValue::Unsigned(u64::MAX as u128),
        )
        .unwrap()
        .bytes()
        .to_vec();
        redundant_patch.extend_from_slice(&movk(9, 1, u16::MAX));
        assert!(
            validate_aarch64_shortest_movn_materialization(
                &physical,
                x9,
                IntegerValue::Unsigned(u64::MAX as u128),
                &redundant_patch,
            )
            .is_err()
        );

        assert!(
            validate_aarch64_shortest_movn_materialization(
                &physical,
                x9,
                IntegerValue::Unsigned(0xffff_0003_0002_0000),
                encoded.bytes(),
            )
            .is_err()
        );
    }

    #[test]
    fn scalar_forms_report_exact_decoded_footprints() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let views = ["x3", "x4", "x5"].map(|name| physical.model().view_named(name).unwrap().id);
        let fact = omega_optimization_core::AcceptedObligationFactIdentity::from_bytes([7; 32]);
        let cases = [
            (
                SelectedInstructionKind::CopyI64,
                MachineAlternativeFamily::CopyI64,
                2,
            ),
            (
                SelectedInstructionKind::CompareI64Zero,
                MachineAlternativeFamily::CompareI64Zero,
                1,
            ),
            (
                SelectedInstructionKind::CompareI64,
                MachineAlternativeFamily::CompareI64,
                2,
            ),
            (
                SelectedInstructionKind::ExactAddI64 {
                    obligation: ObligationId::new(1).unwrap(),
                    accepted_fact: fact,
                },
                MachineAlternativeFamily::ExactAddI64,
                3,
            ),
            (
                SelectedInstructionKind::ExactAddI64Immediate {
                    immediate: IntegerValue::Unsigned(4095),
                    obligation: ObligationId::new(2).unwrap(),
                    accepted_fact: fact,
                },
                MachineAlternativeFamily::ExactAddI64Immediate,
                2,
            ),
            (
                SelectedInstructionKind::ExactSubtractI64 {
                    obligation: ObligationId::new(3).unwrap(),
                    accepted_fact: fact,
                },
                MachineAlternativeFamily::ExactSubtractI64,
                3,
            ),
            (
                SelectedInstructionKind::ExactSubtractI64Immediate {
                    immediate: IntegerValue::Unsigned(5),
                    obligation: ObligationId::new(4).unwrap(),
                    accepted_fact: fact,
                },
                MachineAlternativeFamily::ExactSubtractI64Immediate,
                2,
            ),
        ];
        for (kind, family, count) in cases {
            let encoded =
                encode_aarch64_selected_form(&physical, kind, alternative(family), &views[..count])
                    .unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            if matches!(
                kind,
                SelectedInstructionKind::ExactSubtractI64Immediate { .. }
            ) {
                assert_eq!(encoded.bytes(), [0x64, 0x14, 0x00, 0xd1]);
                assert!(!encoded.footprint().writes_nzcv);
                assert!(encoded.footprint().encoded.implicit_unit_defs.is_empty());
            }
        }
        let compare = encode_aarch64_selected_form(
            &physical,
            SelectedInstructionKind::CompareI64,
            alternative(MachineAlternativeFamily::CompareI64),
            &views[..2],
        )
        .unwrap();
        assert_eq!(compare.bytes(), [0x7f, 0x00, 0x04, 0xeb]);
        assert_eq!(compare.footprint().register_reads, views[..2]);
        assert!(compare.footprint().register_writes.is_empty());
        assert!(compare.footprint().writes_nzcv);
        assert_eq!(compare.footprint().encoded.external_operand_reads, [0, 1]);
    }

    #[test]
    fn ret_x30_is_exact_and_separates_abi_result_custody_from_encoded_effects() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let x0 = physical.model().view_named("x0").unwrap().id;
        let x1 = physical.model().view_named("x1").unwrap().id;
        let x30 = physical.model().view_named("x30").unwrap();
        let pc = physical.model().view_named("pc").unwrap();
        let kind = SelectedInstructionKind::ReturnI64;
        let alternative = alternative(MachineAlternativeFamily::ReturnI64);
        let encoded = encode_aarch64_selected_form(&physical, kind, alternative, &[x0]).unwrap();

        assert_eq!(encoded.bytes(), [0xc0, 0x03, 0x5f, 0xd6]);
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(encoded.footprint().encoded.external_operand_reads, []);
        assert_eq!(encoded.footprint().encoded.external_operand_writes, []);
        assert_eq!(encoded.footprint().encoded.implicit_unit_uses, x30.units);
        assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
        assert_eq!(
            encoded.footprint().encoded.memory,
            MachineEncodedMemoryEffect::NoneV1
        );
        assert_eq!(
            encoded.footprint().encoded.stack,
            MachineEncodedStackEffect::UnchangedV1
        );
        assert_eq!(
            encoded.footprint().encoded.trap,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        );
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target: x30.id }
        );
        assert!(encode_aarch64_selected_form(&physical, kind, alternative, &[x1]).is_err());
        assert!(
            validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[x0],
                &0xd65f_03a0_u32.to_le_bytes()
            )
            .is_err()
        );
        assert!(
            validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[x0],
                &[0xc0, 0x03, 0x5f, 0xd6, 0xc0, 0x03, 0x5f, 0xd6]
            )
            .is_err()
        );
    }

    #[test]
    fn unit_return_is_a_distinct_zero_operand_ret_x30() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::ReturnUnit;
        let return_alternative = alternative(MachineAlternativeFamily::ReturnUnit);
        let encoded =
            encode_aarch64_selected_form(&physical, kind, return_alternative, &[]).unwrap();

        assert_eq!(encoded.bytes(), [0xc0, 0x03, 0x5f, 0xd6]);
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert!(matches!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. }
        ));
        assert!(
            encode_aarch64_selected_form(
                &physical,
                kind,
                alternative(MachineAlternativeFamily::ReturnI64),
                &[]
            )
            .is_err()
        );
    }

    #[test]
    fn nonzero_branch_has_exact_instruction_relative_imm19_and_effects() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero);
        for displacement in [-1_048_576, -4, 0, 4, 1_048_572] {
            let encoded =
                encode_aarch64_selected_nonzero_branch_form(&physical, alternative, displacement)
                    .unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            assert_eq!(encoded.bytes()[0] & 0x1f, 1);
            assert!(encoded.footprint().register_reads.is_empty());
            assert!(encoded.footprint().register_writes.is_empty());
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
        }
        for displacement in [-1_048_580, 1_048_576, 2] {
            assert!(
                encode_aarch64_selected_nonzero_branch_form(&physical, alternative, displacement)
                    .is_err()
            );
        }
        assert!(
            validate_aarch64_selected_nonzero_branch_form(
                &physical,
                alternative,
                0,
                &0x5400_0000_u32.to_le_bytes()
            )
            .is_err()
        );
        assert!(
            validate_aarch64_selected_nonzero_branch_form(
                &physical,
                alternative,
                0,
                &[1, 0, 0, 0, 0]
            )
            .is_err()
        );
    }

    #[test]
    fn u64_less_than_branch_is_exact_b_lo_imm19_with_flag_control_effects() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let nzcv = physical.model().view_named("nzcv").unwrap();
        let pc = physical.model().view_named("pc").unwrap();
        let alternative = alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan);
        for (displacement, expected) in [
            (-4, [0xe3, 0xff, 0xff, 0x54]),
            (0, [0x03, 0x00, 0x00, 0x54]),
            (4, [0x23, 0x00, 0x00, 0x54]),
        ] {
            let encoded = encode_aarch64_selected_u64_less_than_branch_form(
                &physical,
                alternative,
                displacement,
            )
            .unwrap();
            assert_eq!(encoded.bytes(), expected);
            assert!(encoded.footprint().register_reads.is_empty());
            assert!(encoded.footprint().register_writes.is_empty());
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
            assert!(nzcv.units.iter().all(|unit| {
                encoded
                    .footprint()
                    .encoded
                    .implicit_unit_uses
                    .contains(unit)
            }));
            assert!(pc.units.iter().all(|unit| {
                encoded
                    .footprint()
                    .encoded
                    .implicit_unit_uses
                    .contains(unit)
            }));
            assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
        }
        for displacement in [-1_048_576, 1_048_572] {
            assert!(
                encode_aarch64_selected_u64_less_than_branch_form(
                    &physical,
                    alternative,
                    displacement,
                )
                .is_ok()
            );
        }
        for displacement in [-1_048_580, 1_048_576, 2] {
            assert!(
                encode_aarch64_selected_u64_less_than_branch_form(
                    &physical,
                    alternative,
                    displacement,
                )
                .is_err()
            );
        }
        for bytes in [
            0x5400_0002_u32.to_le_bytes(),
            0x5400_0009_u32.to_le_bytes(),
            0x1400_0003_u32.to_le_bytes(),
        ] {
            assert_eq!(
                validate_aarch64_selected_u64_less_than_branch_form(
                    &physical,
                    alternative,
                    0,
                    &bytes,
                ),
                Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
            );
        }
        assert_eq!(
            validate_aarch64_selected_u64_less_than_branch_form(
                &physical,
                alternative,
                0,
                &0x5400_0023_u32.to_le_bytes(),
            ),
            Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
        );
    }

    #[test]
    fn i64_less_than_branch_is_exact_b_lt_imm19() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let alternative = alternative(MachineAlternativeFamily::ConditionalBranchI64LessThan);
        for (displacement, expected) in [
            (-4, [0xeb, 0xff, 0xff, 0x54]),
            (0, [0x0b, 0x00, 0x00, 0x54]),
            (4, [0x2b, 0x00, 0x00, 0x54]),
        ] {
            let encoded = encode_aarch64_selected_i64_less_than_branch_form(
                &physical,
                alternative,
                displacement,
            )
            .unwrap();
            assert_eq!(encoded.bytes(), expected);
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
        }
        assert_eq!(
            validate_aarch64_selected_i64_less_than_branch_form(
                &physical,
                alternative,
                0,
                &0x5400_0003_u32.to_le_bytes(),
            ),
            Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
        );
    }

    #[test]
    fn fused_cbnz_is_exact_rejects_nearby_opcodes_and_does_not_read_nzcv() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let x0 = physical.model().view_named("x0").unwrap();
        let x30 = physical.model().view_named("x30").unwrap();
        let pc = physical.model().view_named("pc").unwrap();
        let nzcv = physical.model().view_named("nzcv").unwrap();

        for (source, register) in [(x0.id, 0_u8), (x30.id, 30_u8)] {
            for displacement in [-1_048_576, -4, 0, 4, 1_048_572] {
                let encoded = encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                    &physical,
                    source,
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), 4);
                assert_eq!(encoded.bytes()[0] & 0x1f, register);
                assert_eq!(encoded.footprint().register_reads, [source]);
                assert!(encoded.footprint().register_writes.is_empty());
                assert!(!encoded.footprint().writes_nzcv);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, []);
                assert_eq!(encoded.footprint().encoded.implicit_unit_uses, pc.units);
                assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
                assert!(
                    encoded
                        .footprint()
                        .encoded
                        .implicit_unit_uses
                        .iter()
                        .all(|unit| !nzcv.units.contains(unit))
                );
            }
        }

        for displacement in [-1_048_580, 1_048_576, 2] {
            assert!(
                encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                    &physical,
                    x0.id,
                    displacement,
                )
                .is_err()
            );
        }
        for word in [
            0xb400_0000_u32, // 64-bit CBZ
            0x3500_0000_u32, // 32-bit CBNZ
            0xb500_001e_u32, // wrong source register
        ] {
            assert!(
                validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                    &physical,
                    x0.id,
                    0,
                    &word.to_le_bytes(),
                )
                .is_err()
            );
        }
        assert!(
            validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                &physical,
                x0.id,
                0,
                &[0, 0, 0, 0, 0],
            )
            .is_err()
        );
    }
}
