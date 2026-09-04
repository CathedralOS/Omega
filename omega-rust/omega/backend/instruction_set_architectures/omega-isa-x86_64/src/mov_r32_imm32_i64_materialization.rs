use omega_register_model::{
    RegisterUnitId, RegisterViewId, RegisterWriteSemantics, ValidatedPhysicalRegisterModel,
};
use omega_selected_instructions::MachineEncodedEffects;
use psi_core::IntegerValue;

use crate::x86_64_physical_register_model;

/// Exact machine-state footprint of a 32-bit GPR write used to realize a
/// zero-extended 64-bit value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64MovR32Imm32I64MaterializationFootprint {
    pub register_reads: Vec<RegisterViewId>,
    /// Semantic selected-instruction destination: the complete 64-bit view.
    pub register_writes: Vec<RegisterViewId>,
    /// The narrower view named by the actual `MOV r32, imm32` encoding.
    pub encoded_write_view: RegisterViewId,
    /// Storage occupied by the encoded 32-bit view before architectural
    /// zero-extension is accounted for.
    pub encoded_write_view_units: Vec<RegisterUnitId>,
    /// Complete storage modified by the architectural 32-bit write.
    pub encoded_write_units: Vec<RegisterUnitId>,
    pub encoded_write_semantics: RegisterWriteSemantics,
    pub writes_rflags: bool,
    pub encoded: MachineEncodedEffects,
}

/// Meaning reconstructed directly from canonical x86-64 instruction bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64DecodedMovR32Imm32I64Materialization {
    destination: RegisterViewId,
    encoded_write_view: RegisterViewId,
    value_bits: u64,
    footprint: X86_64MovR32Imm32I64MaterializationFootprint,
}

impl X86_64DecodedMovR32Imm32I64Materialization {
    pub const fn destination(&self) -> RegisterViewId {
        self.destination
    }

    pub const fn encoded_write_view(&self) -> RegisterViewId {
        self.encoded_write_view
    }

    pub const fn value_bits(&self) -> u64 {
        self.value_bits
    }

    pub const fn footprint(&self) -> &X86_64MovR32Imm32I64MaterializationFootprint {
        &self.footprint
    }
}

/// Canonical bytes together with independently decoded destination, value,
/// view, storage, and effect evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64MovR32Imm32I64Materialization {
    bytes: Vec<u8>,
    decoded: X86_64DecodedMovR32Imm32I64Materialization,
}

impl ValidatedX86_64MovR32Imm32I64Materialization {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn decoded(&self) -> &X86_64DecodedMovR32Imm32I64Materialization {
        &self.decoded
    }

    pub const fn destination(&self) -> RegisterViewId {
        self.decoded.destination()
    }

    pub const fn encoded_write_view(&self) -> RegisterViewId {
        self.decoded.encoded_write_view()
    }

    pub const fn value_bits(&self) -> u64 {
        self.decoded.value_bits()
    }

    pub const fn footprint(&self) -> &X86_64MovR32Imm32I64MaterializationFootprint {
        self.decoded.footprint()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64MovR32Imm32I64MaterializationError {
    NonCanonicalPhysicalModel,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideZeroExtendedU32Bits,
    MalformedEncoding,
    EncodedFormMismatch,
}

impl std::fmt::Display for X86_64MovR32Imm32I64MaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 MOV-r32-imm32 i64 materialization: {self:?}"
        )
    }
}

impl std::error::Error for X86_64MovR32Imm32I64MaterializationError {}

/// Produce the canonical `MOV r32, imm32` form for one exact i64 bit pattern
/// in `0..=u32::MAX`. Low GPRs use five bytes; r8d-r15d use canonical REX.B
/// and six bytes.
pub fn encode_x86_64_mov_r32_imm32_i64_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
    value: IntegerValue,
) -> Result<ValidatedX86_64MovR32Imm32I64Materialization, X86_64MovR32Imm32I64MaterializationError>
{
    require_canonical_model(physical)?;
    let register = GPR_NAMES
        .iter()
        .enumerate()
        .find_map(|(register, (gpr64, _))| {
            physical
                .model()
                .view_named(gpr64)
                .filter(|view| view.id == destination && view.bits == 64 && view.allocatable)
                .map(|_| register as u8)
        })
        .filter(|register| *register != 4)
        .ok_or(X86_64MovR32Imm32I64MaterializationError::UnknownOrNonGpr64View(destination))?;
    let immediate = zero_extended_u32(value)?;
    let mut bytes = Vec::with_capacity(if register < 8 { 5 } else { 6 });
    if register >= 8 {
        bytes.push(0x41);
    }
    bytes.push(0xb8 | (register & 7));
    bytes.extend_from_slice(&immediate.to_le_bytes());
    validate_x86_64_mov_r32_imm32_i64_materialization(physical, destination, value, &bytes)
}

/// Independently decode exactly one canonical `MOV r32, imm32`. No producer
/// encoding routine or producer-derived expected byte sequence participates
/// in this reconstruction.
pub fn decode_x86_64_mov_r32_imm32_i64_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    bytes: &[u8],
) -> Result<X86_64DecodedMovR32Imm32I64Materialization, X86_64MovR32Imm32I64MaterializationError> {
    require_canonical_model(physical)?;

    let (register, immediate_bytes) = match bytes {
        [opcode @ 0xb8..=0xbf, immediate @ ..] if immediate.len() == 4 => {
            (*opcode - 0xb8, immediate)
        }
        [0x41, opcode @ 0xb8..=0xbf, immediate @ ..] if immediate.len() == 4 => {
            (8 + (*opcode - 0xb8), immediate)
        }
        _ => return Err(X86_64MovR32Imm32I64MaterializationError::MalformedEncoding),
    };
    if register == 4 {
        return Err(X86_64MovR32Imm32I64MaterializationError::EncodedFormMismatch);
    }

    let (gpr64_name, gpr32_name) = GPR_NAMES
        .get(usize::from(register))
        .ok_or(X86_64MovR32Imm32I64MaterializationError::MalformedEncoding)?;
    let destination = physical
        .model()
        .view_named(gpr64_name)
        .ok_or(X86_64MovR32Imm32I64MaterializationError::MalformedEncoding)?;
    let write_view = physical
        .model()
        .view_named(gpr32_name)
        .ok_or(X86_64MovR32Imm32I64MaterializationError::MalformedEncoding)?;
    if destination.bits != 64
        || !destination.allocatable
        || destination.write_semantics != RegisterWriteSemantics::ExactView
        || write_view.bits != 32
        || !write_view.allocatable
        || write_view.write_semantics != RegisterWriteSemantics::ZeroExtendsParent
        || write_view.units.len() != 3
        || destination.units.len() != 4
        || write_view.units != destination.units[..3]
        || write_view.write_units != destination.units
    {
        return Err(X86_64MovR32Imm32I64MaterializationError::EncodedFormMismatch);
    }

    let immediate = u32::from_le_bytes(
        immediate_bytes
            .try_into()
            .map_err(|_| X86_64MovR32Imm32I64MaterializationError::MalformedEncoding)?,
    );
    let destination = destination.id;
    let write_view_id = write_view.id;
    let footprint = X86_64MovR32Imm32I64MaterializationFootprint {
        register_reads: vec![],
        register_writes: vec![destination],
        encoded_write_view: write_view_id,
        encoded_write_view_units: write_view.units.clone(),
        encoded_write_units: write_view.write_units.clone(),
        encoded_write_semantics: write_view.write_semantics,
        writes_rflags: false,
        encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![0]),
    };
    Ok(X86_64DecodedMovR32Imm32I64Materialization {
        destination,
        encoded_write_view: write_view_id,
        value_bits: u64::from(immediate),
        footprint,
    })
}

/// Check byte-reconstructed semantics against the requested 64-bit selected
/// destination and exact value. The decoder, rather than the request, owns the
/// recovered 32-bit write view and architectural zero-extension evidence.
pub fn validate_x86_64_mov_r32_imm32_i64_materialization(
    physical: &ValidatedPhysicalRegisterModel,
    destination: RegisterViewId,
    value: IntegerValue,
    bytes: &[u8],
) -> Result<ValidatedX86_64MovR32Imm32I64Materialization, X86_64MovR32Imm32I64MaterializationError>
{
    let expected_value = u64::from(zero_extended_u32(value)?);
    let decoded = decode_x86_64_mov_r32_imm32_i64_materialization(physical, bytes)?;
    if decoded.destination != destination || decoded.value_bits != expected_value {
        return Err(X86_64MovR32Imm32I64MaterializationError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64MovR32Imm32I64Materialization {
        bytes: bytes.to_vec(),
        decoded,
    })
}

fn require_canonical_model(
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), X86_64MovR32Imm32I64MaterializationError> {
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64MovR32Imm32I64MaterializationError::NonCanonicalPhysicalModel);
    }
    Ok(())
}

fn zero_extended_u32(value: IntegerValue) -> Result<u32, X86_64MovR32Imm32I64MaterializationError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .ok()
            .and_then(|value| u32::try_from(value).ok()),
        IntegerValue::Unsigned(value) => u32::try_from(value).ok(),
    }
    .ok_or(X86_64MovR32Imm32I64MaterializationError::IntegerOutsideZeroExtendedU32Bits)
}

// Architectural opcode numbering, not register-model declaration order.
const GPR_NAMES: [(&str, &str); 16] = [
    ("rax", "eax"),
    ("rcx", "ecx"),
    ("rdx", "edx"),
    ("rbx", "ebx"),
    ("rsp", "esp"),
    ("rbp", "ebp"),
    ("rsi", "esi"),
    ("rdi", "edi"),
    ("r8", "r8d"),
    ("r9", "r9d"),
    ("r10", "r10d"),
    ("r11", "r11d"),
    ("r12", "r12d"),
    ("r13", "r13d"),
    ("r14", "r14d"),
    ("r15", "r15d"),
];

#[cfg(test)]
mod tests {
    use omega_register_model::{
        RegisterViewId, RegisterWriteSemantics, validate_physical_register_model,
    };
    use omega_selected_instructions::{
        MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior,
    };

    use super::*;

    #[test]
    fn every_allocatable_gpr_and_boundary_value_round_trips_canonically() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rax = physical.model().view_named("rax").unwrap().id;
        let r8 = physical.model().view_named("r8").unwrap().id;
        assert_eq!(
            encode_x86_64_mov_r32_imm32_i64_materialization(
                &physical,
                rax,
                IntegerValue::Unsigned(0x1234_5678),
            )
            .unwrap()
            .bytes(),
            [0xb8, 0x78, 0x56, 0x34, 0x12]
        );
        assert_eq!(
            encode_x86_64_mov_r32_imm32_i64_materialization(
                &physical,
                r8,
                IntegerValue::Unsigned(0x1234_5678),
            )
            .unwrap()
            .bytes(),
            [0x41, 0xb8, 0x78, 0x56, 0x34, 0x12]
        );
        let values = [
            IntegerValue::Unsigned(0),
            IntegerValue::Signed(1),
            IntegerValue::Unsigned(u32::MAX as u128),
        ];

        for (register, (gpr64_name, gpr32_name)) in GPR_NAMES.iter().enumerate() {
            if register == 4 {
                continue;
            }
            let destination = physical.model().view_named(gpr64_name).unwrap().id;
            let write_view = physical.model().view_named(gpr32_name).unwrap();
            for value in values {
                let encoded =
                    encode_x86_64_mov_r32_imm32_i64_materialization(&physical, destination, value)
                        .unwrap();
                assert_eq!(encoded.bytes().len(), if register < 8 { 5 } else { 6 });
                if register < 8 {
                    assert_eq!(encoded.bytes()[0], 0xb8 | register as u8);
                } else {
                    assert_eq!(encoded.bytes()[..2], [0x41, 0xb8 | (register as u8 & 7)]);
                }
                assert_eq!(encoded.destination(), destination);
                assert_eq!(encoded.encoded_write_view(), write_view.id);
                assert_eq!(
                    encoded.value_bits(),
                    match value {
                        IntegerValue::Signed(value) => value as u64,
                        IntegerValue::Unsigned(value) => value as u64,
                    }
                );
                assert_eq!(
                    decode_x86_64_mov_r32_imm32_i64_materialization(&physical, encoded.bytes())
                        .unwrap(),
                    *encoded.decoded()
                );

                let footprint = encoded.footprint();
                assert!(footprint.register_reads.is_empty());
                assert_eq!(footprint.register_writes, [destination]);
                assert_eq!(footprint.encoded_write_view, write_view.id);
                assert_eq!(footprint.encoded_write_view_units, write_view.units);
                assert_eq!(footprint.encoded_write_units, write_view.write_units);
                assert_eq!(
                    footprint.encoded_write_semantics,
                    RegisterWriteSemantics::ZeroExtendsParent
                );
                assert!(!footprint.writes_rflags);
                assert!(footprint.encoded.external_operand_reads.is_empty());
                assert_eq!(footprint.encoded.external_operand_writes, [0]);
                assert!(footprint.encoded.implicit_unit_uses.is_empty());
                assert!(footprint.encoded.implicit_unit_defs.is_empty());
                assert!(footprint.encoded.implicit_unit_clobbers.is_empty());
                assert_eq!(footprint.encoded.memory, MachineEncodedMemoryEffect::NoneV1);
                assert_eq!(
                    footprint.encoded.stack,
                    MachineEncodedStackEffect::UnchangedV1
                );
                assert_eq!(footprint.encoded.trap, MachineEncodedTrapBehavior::NeverV1);
                assert_eq!(
                    footprint.encoded.control,
                    MachineEncodedControlEffect::FallThroughV1
                );
            }
        }
    }

    #[test]
    fn encoder_rejects_non_u32_values_and_non_semantic_destinations() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rax = physical.model().view_named("rax").unwrap().id;
        for value in [
            IntegerValue::Signed(-1),
            IntegerValue::Unsigned(u32::MAX as u128 + 1),
            IntegerValue::Signed(i128::from(u32::MAX) + 1),
        ] {
            assert_eq!(
                encode_x86_64_mov_r32_imm32_i64_materialization(&physical, rax, value),
                Err(X86_64MovR32Imm32I64MaterializationError::IntegerOutsideZeroExtendedU32Bits)
            );
        }
        for name in ["rsp", "eax", "rflags"] {
            let view = physical.model().view_named(name).unwrap().id;
            assert_eq!(
                encode_x86_64_mov_r32_imm32_i64_materialization(
                    &physical,
                    view,
                    IntegerValue::Unsigned(7),
                ),
                Err(X86_64MovR32Imm32I64MaterializationError::UnknownOrNonGpr64View(view))
            );
        }
        let unknown = RegisterViewId(u16::MAX);
        assert_eq!(
            encode_x86_64_mov_r32_imm32_i64_materialization(
                &physical,
                unknown,
                IntegerValue::Unsigned(7),
            ),
            Err(X86_64MovR32Imm32I64MaterializationError::UnknownOrNonGpr64View(unknown))
        );
    }

    #[test]
    fn decoder_rejects_framing_prefix_opcode_and_register_corruption() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        for bytes in [
            &[][..],
            &[0xb8][..],
            &[0xb8, 1, 0, 0][..],
            &[0xb8, 1, 0, 0, 0, 0][..],
            &[0x90, 1, 0, 0, 0][..],
            &[0x40, 0xb8, 1, 0, 0, 0][..],
            &[0x48, 0xb8, 1, 0, 0, 0][..],
            &[0x49, 0xb8, 1, 0, 0, 0][..],
            &[0x42, 0xb8, 1, 0, 0, 0][..],
            &[0x41, 0x90, 1, 0, 0, 0][..],
            &[0xbc, 1, 0, 0, 0][..],
        ] {
            assert!(
                decode_x86_64_mov_r32_imm32_i64_materialization(&physical, bytes).is_err(),
                "noncanonical bytes were accepted: {bytes:02x?}"
            );
        }
    }

    #[test]
    fn validation_rejects_register_value_view_and_write_semantics_corruption() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rax = physical.model().view_named("rax").unwrap().id;
        let rcx = physical.model().view_named("rcx").unwrap().id;
        let eax = physical.model().view_named("eax").unwrap().id;
        let bytes = [0xb8, 0x78, 0x56, 0x34, 0x12];

        assert_eq!(
            validate_x86_64_mov_r32_imm32_i64_materialization(
                &physical,
                rcx,
                IntegerValue::Unsigned(0x1234_5678),
                &bytes,
            ),
            Err(X86_64MovR32Imm32I64MaterializationError::EncodedFormMismatch)
        );
        assert_eq!(
            validate_x86_64_mov_r32_imm32_i64_materialization(
                &physical,
                rax,
                IntegerValue::Unsigned(0x1234_5679),
                &bytes,
            ),
            Err(X86_64MovR32Imm32I64MaterializationError::EncodedFormMismatch)
        );
        assert_eq!(
            validate_x86_64_mov_r32_imm32_i64_materialization(
                &physical,
                eax,
                IntegerValue::Unsigned(0x1234_5678),
                &bytes,
            ),
            Err(X86_64MovR32Imm32I64MaterializationError::EncodedFormMismatch)
        );

        let mut forged_model = x86_64_physical_register_model();
        let eax_index = usize::from(forged_model.view_named("eax").unwrap().id.0);
        forged_model.views[eax_index].write_semantics = RegisterWriteSemantics::PreservesUnwritten;
        let forged = validate_physical_register_model(forged_model).unwrap();
        assert_eq!(
            decode_x86_64_mov_r32_imm32_i64_materialization(&forged, &bytes),
            Err(X86_64MovR32Imm32I64MaterializationError::NonCanonicalPhysicalModel)
        );
        assert_eq!(
            validate_x86_64_mov_r32_imm32_i64_materialization(
                &forged,
                rax,
                IntegerValue::Unsigned(0x1234_5678),
                &bytes,
            ),
            Err(X86_64MovR32Imm32I64MaterializationError::NonCanonicalPhysicalModel)
        );
    }
}
