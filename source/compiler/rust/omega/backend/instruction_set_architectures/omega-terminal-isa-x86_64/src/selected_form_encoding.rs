use omega_register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternativeFamily, TerminalMachineAlternativeKey,
    TerminalSelectedInstructionKind,
};
use psi_core::IntegerValue;

use crate::x86_64_physical_register_model;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64SelectedFormFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub writes_rflags: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64SelectedFormEncoding {
    bytes: Vec<u8>,
    footprint: X86_64SelectedFormFootprint,
}

impl ValidatedX86_64SelectedFormEncoding {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn footprint(&self) -> &X86_64SelectedFormFootprint {
        &self.footprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64SelectedFormEncodingError {
    NonCanonicalPhysicalModel,
    LayoutDependentOrControlForm,
    AlternativeMismatch,
    OperandCountMismatch,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideI64Bits,
    ImmediateOutsideU12,
    MalformedEncoding,
    EncodedFormMismatch,
}

impl std::fmt::Display for X86_64SelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid x86-64 selected-form encoding: {self:?}")
    }
}

impl std::error::Error for X86_64SelectedFormEncodingError {}

pub fn encode_x86_64_terminal_selected_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: TerminalSelectedInstructionKind,
    alternative: TerminalMachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_registers(physical, operands)?;
    validate_alias_partition(kind, alternative, &registers)?;
    let bytes = encode_unchecked(kind, alternative, &registers)?;
    validate_x86_64_terminal_selected_form_encoding(physical, kind, alternative, operands, &bytes)
}

pub fn validate_x86_64_terminal_selected_form_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    kind: TerminalSelectedInstructionKind,
    alternative: TerminalMachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_registers(physical, operands)?;
    validate_alias_partition(kind, alternative, &registers)?;
    let decoded = decode_all(bytes)?;
    validate_decoded(kind, alternative, &registers, &decoded)?;
    let canonical = encode_unchecked(kind, alternative, &registers)?;
    if bytes != canonical {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(kind, operands),
    })
}

fn validate_request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: TerminalSelectedInstructionKind,
    alternative: TerminalMachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    let (family, count, variants) = family_and_operand_count(kind)?;
    if alternative.family != family || !variants.contains(&alternative.variant) {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    if operands.len() != count {
        return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
    }
    Ok(())
}

fn family_and_operand_count(
    kind: TerminalSelectedInstructionKind,
) -> Result<
    (
        TerminalMachineAlternativeFamily,
        usize,
        std::ops::RangeInclusive<u32>,
    ),
    X86_64SelectedFormEncodingError,
> {
    Ok(match kind {
        TerminalSelectedInstructionKind::CompareI64Zero => {
            (TerminalMachineAlternativeFamily::CompareI64Zero, 1, 0..=0)
        }
        TerminalSelectedInstructionKind::MaterializeI64 { .. } => {
            (TerminalMachineAlternativeFamily::MaterializeI64, 1, 0..=0)
        }
        TerminalSelectedInstructionKind::CopyI64 => {
            (TerminalMachineAlternativeFamily::CopyI64, 2, 0..=0)
        }
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => {
            (TerminalMachineAlternativeFamily::ExactAddI64, 3, 0..=0)
        }
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => {
            (TerminalMachineAlternativeFamily::ExactSubtractI64, 3, 0..=3)
        }
        TerminalSelectedInstructionKind::ExactAddI64Immediate { .. } => (
            TerminalMachineAlternativeFamily::ExactAddI64Immediate,
            2,
            0..=0,
        ),
        TerminalSelectedInstructionKind::ConditionalBranchNonZero
        | TerminalSelectedInstructionKind::ReturnI64 => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentOrControlForm);
        }
    })
}

fn resolve_registers(
    physical: &ValidatedPhysicalRegisterModel,
    operands: &[RegisterViewId],
) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
    const NAMES: [&str; 16] = [
        "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12",
        "r13", "r14", "r15",
    ];
    operands
        .iter()
        .map(|id| {
            let (code, view) = NAMES
                .iter()
                .enumerate()
                .find_map(|(code, name)| {
                    physical
                        .model()
                        .view_named(name)
                        .filter(|view| view.id == *id)
                        .map(|view| (code as u8, view))
                })
                .ok_or(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(*id))?;
            if code == 4 || view.bits != 64 || !view.allocatable {
                return Err(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(*id));
            }
            Ok(code)
        })
        .collect()
}

fn validate_alias_partition(
    kind: TerminalSelectedInstructionKind,
    alternative: TerminalMachineAlternativeKey,
    registers: &[u8],
) -> Result<(), X86_64SelectedFormEncodingError> {
    if !matches!(
        kind,
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. }
    ) {
        return Ok(());
    }
    let [left, right, result] = registers else {
        return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
    };
    let expected = match (result == left, result == right) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    };
    if alternative.variant != expected {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    Ok(())
}

fn integer_bits(value: IntegerValue) -> Result<u64, X86_64SelectedFormEncodingError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .map(|value| value as u64)
            .map_err(|_| X86_64SelectedFormEncodingError::IntegerOutsideI64Bits),
        IntegerValue::Unsigned(value) => {
            u64::try_from(value).map_err(|_| X86_64SelectedFormEncodingError::IntegerOutsideI64Bits)
        }
    }
}

fn u12(value: IntegerValue) -> Result<u32, X86_64SelectedFormEncodingError> {
    match value {
        IntegerValue::Unsigned(value) if value <= 4095 => Ok(value as u32),
        _ => Err(X86_64SelectedFormEncodingError::ImmediateOutsideU12),
    }
}

fn rex(register: u8, index: u8, base: u8) -> u8 {
    0x48 | ((register >> 3) << 2) | ((index >> 3) << 1) | (base >> 3)
}

fn modrm(mode: u8, register: u8, rm: u8) -> u8 {
    (mode << 6) | ((register & 7) << 3) | (rm & 7)
}

fn append_register_binary(bytes: &mut Vec<u8>, opcode: u8, source: u8, destination: u8) {
    bytes.extend([
        rex(source, 0, destination),
        opcode,
        modrm(3, source, destination),
    ]);
}

fn append_lea_register(bytes: &mut Vec<u8>, left: u8, right: u8, destination: u8) {
    let (base, index) = if left & 7 == 5 && right & 7 != 5 {
        (right, left)
    } else {
        (left, right)
    };
    let needs_zero_displacement = base & 7 == 5;
    bytes.extend([
        rex(destination, index, base),
        0x8d,
        modrm(u8::from(needs_zero_displacement), destination, 4),
        ((index & 7) << 3) | (base & 7),
    ]);
    if needs_zero_displacement {
        bytes.push(0);
    }
}

fn append_lea_immediate(bytes: &mut Vec<u8>, base: u8, destination: u8, displacement: u32) {
    let use_disp8 = displacement <= 127;
    let uses_sib = base & 7 == 4;
    bytes.extend([
        rex(destination, 0, base),
        0x8d,
        modrm(
            if use_disp8 { 1 } else { 2 },
            destination,
            if uses_sib { 4 } else { base },
        ),
    ]);
    if uses_sib {
        bytes.push(0x20 | (base & 7));
    }
    if use_disp8 {
        bytes.push(displacement as u8);
    } else {
        bytes.extend(displacement.to_le_bytes());
    }
}

fn encode_unchecked(
    kind: TerminalSelectedInstructionKind,
    alternative: TerminalMachineAlternativeKey,
    registers: &[u8],
) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
    let mut bytes = Vec::new();
    match kind {
        TerminalSelectedInstructionKind::MaterializeI64 { value } => {
            bytes.extend([0x48 | (registers[0] >> 3), 0xb8 | (registers[0] & 7)]);
            bytes.extend(integer_bits(value)?.to_le_bytes());
        }
        TerminalSelectedInstructionKind::CopyI64 => {
            append_register_binary(&mut bytes, 0x89, registers[0], registers[1]);
        }
        TerminalSelectedInstructionKind::CompareI64Zero => {
            append_register_binary(&mut bytes, 0x85, registers[0], registers[0]);
        }
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => {
            append_lea_register(&mut bytes, registers[0], registers[1], registers[2]);
        }
        TerminalSelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            append_lea_immediate(&mut bytes, registers[0], registers[1], u12(immediate)?);
        }
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => match alternative.variant {
            0 => append_register_binary(&mut bytes, 0x31, registers[2], registers[2]),
            1 => append_register_binary(&mut bytes, 0x29, registers[1], registers[2]),
            2 => {
                bytes.extend([rex(0, 0, registers[2]), 0xf7, modrm(3, 3, registers[2])]);
                append_register_binary(&mut bytes, 0x01, registers[0], registers[2]);
            }
            3 => {
                append_register_binary(&mut bytes, 0x89, registers[0], registers[2]);
                append_register_binary(&mut bytes, 0x29, registers[1], registers[2]);
            }
            _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
        },
        TerminalSelectedInstructionKind::ConditionalBranchNonZero
        | TerminalSelectedInstructionKind::ReturnI64 => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentOrControlForm);
        }
    }
    Ok(bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecodedInstruction {
    Materialize {
        destination: u8,
        value: u64,
    },
    Move {
        source: u8,
        destination: u8,
    },
    Test {
        register: u8,
    },
    Lea {
        destination: u8,
        base: u8,
        index: Option<u8>,
        displacement: i32,
    },
    Xor {
        source: u8,
        destination: u8,
    },
    Subtract {
        source: u8,
        destination: u8,
    },
    Negate {
        destination: u8,
    },
    Add {
        source: u8,
        destination: u8,
    },
}

fn decode_all(bytes: &[u8]) -> Result<Vec<DecodedInstruction>, X86_64SelectedFormEncodingError> {
    if bytes.is_empty() {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    let mut decoded = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let (instruction, length) = decode_one(&bytes[offset..])?;
        decoded.push(instruction);
        offset = offset
            .checked_add(length)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    }
    Ok(decoded)
}

fn decode_one(
    bytes: &[u8],
) -> Result<(DecodedInstruction, usize), X86_64SelectedFormEncodingError> {
    let (&rex, rest) = bytes
        .split_first()
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    if !(0x48..=0x4f).contains(&rex) {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    let (&opcode, _) = rest
        .split_first()
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    let rex_r = (rex >> 2) & 1;
    let rex_x = (rex >> 1) & 1;
    let rex_b = rex & 1;
    if (0xb8..=0xbf).contains(&opcode) {
        let immediate = bytes
            .get(2..10)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        return Ok((
            DecodedInstruction::Materialize {
                destination: (opcode & 7) | (rex_b << 3),
                value: immediate,
            },
            10,
        ));
    }
    let modrm = *bytes
        .get(2)
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    let mode = modrm >> 6;
    let reg = ((modrm >> 3) & 7) | (rex_r << 3);
    let rm_low = modrm & 7;
    let rm = rm_low | (rex_b << 3);
    if matches!(opcode, 0x89 | 0x85 | 0x31 | 0x29 | 0x01 | 0xf7) {
        if mode != 3 || bytes.len() < 3 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let decoded = match opcode {
            0x89 => DecodedInstruction::Move {
                source: reg,
                destination: rm,
            },
            0x85 if reg == rm => DecodedInstruction::Test { register: reg },
            0x31 => DecodedInstruction::Xor {
                source: reg,
                destination: rm,
            },
            0x29 => DecodedInstruction::Subtract {
                source: reg,
                destination: rm,
            },
            0x01 => DecodedInstruction::Add {
                source: reg,
                destination: rm,
            },
            0xf7 if (modrm >> 3) & 7 == 3 => DecodedInstruction::Negate { destination: rm },
            _ => return Err(X86_64SelectedFormEncodingError::MalformedEncoding),
        };
        return Ok((decoded, 3));
    }
    if opcode != 0x8d || mode == 3 {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    let mut length = 3;
    let (base, index) = if rm_low == 4 {
        let sib = *bytes
            .get(length)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        length += 1;
        if sib >> 6 != 0 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let index_low = (sib >> 3) & 7;
        let base_low = sib & 7;
        if mode == 0 && base_low == 5 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        (
            base_low | (rex_b << 3),
            if index_low == 4 && rex_x == 0 {
                None
            } else {
                Some(index_low | (rex_x << 3))
            },
        )
    } else {
        if mode == 0 && rm_low == 5 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        (rm, None)
    };
    let displacement = match mode {
        0 => 0,
        1 => {
            let value = *bytes
                .get(length)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            length += 1;
            i32::from(value as i8)
        }
        2 => {
            let value = bytes
                .get(length..length + 4)
                .and_then(|bytes| bytes.try_into().ok())
                .map(i32::from_le_bytes)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            length += 4;
            value
        }
        _ => return Err(X86_64SelectedFormEncodingError::MalformedEncoding),
    };
    Ok((
        DecodedInstruction::Lea {
            destination: reg,
            base,
            index,
            displacement,
        },
        length,
    ))
}

fn validate_decoded(
    kind: TerminalSelectedInstructionKind,
    alternative: TerminalMachineAlternativeKey,
    registers: &[u8],
    decoded: &[DecodedInstruction],
) -> Result<(), X86_64SelectedFormEncodingError> {
    let valid = match kind {
        TerminalSelectedInstructionKind::MaterializeI64 { value } => {
            decoded
                == [DecodedInstruction::Materialize {
                    destination: registers[0],
                    value: integer_bits(value)?,
                }]
        }
        TerminalSelectedInstructionKind::CopyI64 => {
            decoded
                == [DecodedInstruction::Move {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        TerminalSelectedInstructionKind::CompareI64Zero => {
            decoded
                == [DecodedInstruction::Test {
                    register: registers[0],
                }]
        }
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => {
            matches!(decoded, [DecodedInstruction::Lea { destination, base, index: Some(index), displacement: 0 }]
                if *destination == registers[2]
                    && ((*base == registers[0] && *index == registers[1])
                        || (*base == registers[1] && *index == registers[0])))
        }
        TerminalSelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            decoded
                == [DecodedInstruction::Lea {
                    destination: registers[1],
                    base: registers[0],
                    index: None,
                    displacement: i32::try_from(u12(immediate)?).expect("u12 fits i32"),
                }]
        }
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => match alternative.variant {
            0 => {
                decoded
                    == [DecodedInstruction::Xor {
                        source: registers[2],
                        destination: registers[2],
                    }]
            }
            1 => {
                decoded
                    == [DecodedInstruction::Subtract {
                        source: registers[1],
                        destination: registers[2],
                    }]
            }
            2 => {
                decoded
                    == [
                        DecodedInstruction::Negate {
                            destination: registers[2],
                        },
                        DecodedInstruction::Add {
                            source: registers[0],
                            destination: registers[2],
                        },
                    ]
            }
            3 => {
                decoded
                    == [
                        DecodedInstruction::Move {
                            source: registers[0],
                            destination: registers[2],
                        },
                        DecodedInstruction::Subtract {
                            source: registers[1],
                            destination: registers[2],
                        },
                    ]
            }
            _ => false,
        },
        TerminalSelectedInstructionKind::ConditionalBranchNonZero
        | TerminalSelectedInstructionKind::ReturnI64 => false,
    };
    if valid {
        Ok(())
    } else {
        Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
    }
}

fn footprint(
    kind: TerminalSelectedInstructionKind,
    operands: &[RegisterViewId],
) -> X86_64SelectedFormFootprint {
    let (reads, writes, writes_rflags) = match kind {
        TerminalSelectedInstructionKind::MaterializeI64 { .. } => {
            (vec![], vec![operands[0]], false)
        }
        TerminalSelectedInstructionKind::CopyI64 => (vec![operands[0]], vec![operands[1]], false),
        TerminalSelectedInstructionKind::CompareI64Zero => (vec![operands[0]], vec![], true),
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => {
            (vec![operands[0], operands[1]], vec![operands[2]], false)
        }
        TerminalSelectedInstructionKind::ExactAddI64Immediate { .. } => {
            (vec![operands[0]], vec![operands[1]], false)
        }
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => {
            (vec![operands[0], operands[1]], vec![operands[2]], true)
        }
        TerminalSelectedInstructionKind::ConditionalBranchNonZero
        | TerminalSelectedInstructionKind::ReturnI64 => {
            unreachable!("control forms reject before footprint")
        }
    };
    X86_64SelectedFormFootprint {
        register_reads: reads,
        register_writes: writes,
        writes_rflags,
    }
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::AcceptedObligationFactIdentity;
    use omega_register_model::validate_physical_register_model;
    use psi_core::ObligationId;

    use super::*;

    fn alternative(
        family: TerminalMachineAlternativeFamily,
        variant: u32,
    ) -> TerminalMachineAlternativeKey {
        TerminalMachineAlternativeKey { family, variant }
    }

    fn exact_add() -> TerminalSelectedInstructionKind {
        TerminalSelectedInstructionKind::ExactAddI64 {
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
        }
    }

    #[test]
    fn r12_is_a_valid_rex_extended_sib_index() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let r12 = physical.model().view_named("r12").unwrap().id;
        let rax = physical.model().view_named("rax").unwrap().id;
        let encoded = encode_x86_64_terminal_selected_form(
            &physical,
            exact_add(),
            alternative(TerminalMachineAlternativeFamily::ExactAddI64, 0),
            &[r12, r12, rax],
        )
        .unwrap();
        assert_eq!(encoded.bytes(), [0x4b, 0x8d, 0x04, 0x24]);
    }

    #[test]
    fn scalar_sizes_and_subtraction_alias_partitions_are_exact() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let views = ["rax", "rbx", "rcx"].map(|name| physical.model().view_named(name).unwrap().id);
        let materialize = encode_x86_64_terminal_selected_form(
            &physical,
            TerminalSelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Signed(-1),
            },
            alternative(TerminalMachineAlternativeFamily::MaterializeI64, 0),
            &views[..1],
        )
        .unwrap();
        assert_eq!(materialize.bytes().len(), 10);
        for (homes, variant, size) in [
            ([views[0], views[0], views[0]], 0, 3),
            ([views[0], views[1], views[0]], 1, 3),
            ([views[0], views[1], views[1]], 2, 6),
            ([views[0], views[1], views[2]], 3, 6),
        ] {
            let kind = TerminalSelectedInstructionKind::ExactSubtractI64 {
                obligation: ObligationId::new(2).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([4; 32]),
            };
            let encoded = encode_x86_64_terminal_selected_form(
                &physical,
                kind,
                alternative(TerminalMachineAlternativeFamily::ExactSubtractI64, variant),
                &homes,
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), size);
            let mut corrupted = encoded.bytes().to_vec();
            corrupted[1] ^= 1;
            assert!(
                validate_x86_64_terminal_selected_form_encoding(
                    &physical,
                    kind,
                    alternative(TerminalMachineAlternativeFamily::ExactSubtractI64, variant),
                    &homes,
                    &corrupted,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn immediate_lea_uses_declared_size_extremes() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rax = physical.model().view_named("rax").unwrap().id;
        let r12 = physical.model().view_named("r12").unwrap().id;
        let fact = AcceptedObligationFactIdentity::from_bytes([5; 32]);
        for (base, immediate, size) in [(rax, 0, 4), (rax, 4095, 7), (r12, 0, 5), (r12, 4095, 8)] {
            let kind = TerminalSelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(immediate),
                obligation: ObligationId::new(3).unwrap(),
                accepted_fact: fact,
            };
            let encoded = encode_x86_64_terminal_selected_form(
                &physical,
                kind,
                alternative(TerminalMachineAlternativeFamily::ExactAddI64Immediate, 0),
                &[base, rax],
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), size);
        }
    }
}
