//! Raw IEEE result adaptation with integer predicates, independent of MXCSR.
//!
//! MOVD/Q saves all inputs into volatile xmm3..5 before the output becomes
//! scratch, so the output can alias any input. Integer shifts expose exponent,
//! fraction and signed-zero predicates without quieting NaNs or repeating the
//! provider's arithmetic. A finite overflow's infinity encoding minus one is
//! its signed maximum finite neighbor. Trapping uses an operation-owned UD2.
//! RFLAGS and the vector scratch are explicit clobbers; no memory is touched.
//!
//! Opcode fields follow Intel's instruction reference, MOVD/MOVQ, SAL/SHR,
//! CMP, SUB, Jcc, JMP and UD2:
//! https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html

use super::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError as Error,
    X86_64SelectedFormFootprint,
};
use register_model::{
    RegisterConstraintKey, RegisterUnitId, RegisterViewId, ValidatedPhysicalRegisterModel,
};
use selected_instructions::{
    FloatResultPolicyForm, FloatResultPolicyKind, FloatSaturationRule, MachineAlternative,
    MachineAlternativeApplicability, MachineAlternativeFamily, MachineAlternativeKey,
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior, SelectedInstructionKind,
};

pub(crate) fn clobbers(form: FloatResultPolicyForm) -> Vec<RegisterUnitId> {
    let physical = crate::x86_64_physical_register_model_ref();
    let names: &[&str] = if form.can_trap() {
        &["xmm3", "rflags"]
    } else {
        &["xmm3", "xmm4", "xmm5", "rflags"]
    };
    let mut units = names
        .iter()
        .copied()
        .flat_map(|name| physical.views.iter().filter(move |view| view.name == name))
        .flat_map(|view| view.units.iter().copied())
        .collect::<Vec<_>>();
    units.sort_unstable();
    units.dedup();
    units
}

pub(crate) fn effects(form: FloatResultPolicyForm) -> MachineEncodedEffects {
    let inputs = form.operand_count() as u16;
    let mut effects = MachineEncodedEffects {
        implicit_unit_clobbers: clobbers(form),
        ..MachineEncodedEffects::fallthrough_v1((0..inputs).collect(), vec![inputs])
    };
    if form.can_trap() {
        effects.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
        effects.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
    }
    effects
}

pub(crate) fn footprint(
    form: FloatResultPolicyForm,
    operands: &[RegisterViewId],
) -> X86_64SelectedFormFootprint {
    let inputs = form.operand_count();
    X86_64SelectedFormFootprint {
        register_reads: operands[..inputs].to_vec(),
        register_writes: operands[inputs..inputs + 1].to_vec(),
        writes_rflags: true,
        encoded: effects(form),
    }
}

pub(crate) fn size(form: FloatResultPolicyForm) -> MachineSizeKnowledge {
    // Always encode REX, including binary32 low registers, so branch geometry
    // and exact form size do not depend on register assignment.
    MachineSizeKnowledge::ExactBytes(
        program(form, &[0; 4])
            .iter()
            .map(|instruction| instruction.size())
            .sum::<usize>() as u16,
    )
}

pub(crate) fn declaration(
    form: FloatResultPolicyForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::FloatResultPolicy(form),
        constraint,
        memory: MachineMemoryEffect::NoneV1,
        trap: if form.can_trap() {
            MachineTrapBehavior::TrappingIntegerV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        barrier: if form.can_trap() {
            MachineBarrier::ExternalEffect
        } else {
            MachineBarrier::None
        },
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::FloatResultPolicy(form),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: size(form),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(form),
        }],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Instruction {
    Transfer {
        save: bool,
        wide: bool,
        vector: u8,
        general: u8,
    },
    Shift {
        left: bool,
        wide: bool,
        general: u8,
        count: u8,
    },
    Compare {
        wide: bool,
        general: u8,
        immediate: u32,
    },
    Conditional {
        equal: bool,
        displacement: i32,
    },
    Jump {
        displacement: i32,
    },
    SubtractOne {
        wide: bool,
        general: u8,
    },
    Crash,
}

impl Instruction {
    fn size(self) -> usize {
        match self {
            Self::Transfer { .. } | Self::Jump { .. } => 5,
            Self::Shift { .. } | Self::SubtractOne { .. } => 4,
            Self::Compare { .. } => 7,
            Self::Conditional { .. } => 6,
            Self::Crash => 2,
        }
    }
}

fn program(form: FloatResultPolicyForm, registers: &[u8]) -> Vec<Instruction> {
    let wide = !form.is_binary32();
    let fraction = if wide { 52 } else { 23 };
    let exponent_mask = if wide { 2047 } else { 255 };
    let inputs = form.operand_count();
    let general = registers[inputs];
    let restore = |vector| Instruction::Transfer {
        save: false,
        wide,
        vector,
        general,
    };
    let shift = |left, count| Instruction::Shift {
        left,
        wide,
        general,
        count,
    };
    let compare = |immediate| Instruction::Compare {
        wide,
        general,
        immediate,
    };
    let mut instructions = registers[..inputs]
        .iter()
        .enumerate()
        .map(|(position, &source)| Instruction::Transfer {
            save: true,
            wide,
            vector: 3 + position as u8,
            general: source,
        })
        .collect::<Vec<_>>();
    if form.can_trap() {
        instructions.extend([
            restore(3),
            shift(true, 1),
            shift(false, fraction + 1),
            compare(exponent_mask),
            Instruction::Conditional {
                equal: false,
                displacement: 2,
            },
            Instruction::Crash,
            restore(3),
        ]);
        return instructions;
    }
    let mut retain_raw_branches = Vec::new();
    for vector in [3, 4] {
        instructions.extend([
            restore(vector),
            shift(true, 1),
            shift(false, fraction + 1),
            compare(exponent_mask),
        ]);
        retain_raw_branches.push(instructions.len());
        instructions.push(Instruction::Conditional {
            equal: true,
            displacement: 0,
        });
    }
    if form.policy == FloatResultPolicyKind::Saturating(FloatSaturationRule::FiniteDivisionOverflow)
    {
        instructions.extend([restore(4), shift(true, 1), compare(0)]);
        retain_raw_branches.push(instructions.len());
        instructions.push(Instruction::Conditional {
            equal: true,
            displacement: 0,
        });
    }
    instructions.extend([
        restore(5),
        shift(true, 1),
        shift(false, fraction + 1),
        compare(exponent_mask),
    ]);
    retain_raw_branches.push(instructions.len());
    instructions.push(Instruction::Conditional {
        equal: false,
        displacement: 0,
    });
    instructions.extend([
        restore(5),
        shift(true, if wide { 12 } else { 9 }),
        compare(0),
    ]);
    retain_raw_branches.push(instructions.len());
    instructions.push(Instruction::Conditional {
        equal: false,
        displacement: 0,
    });
    instructions.extend([
        restore(5),
        Instruction::SubtractOne { wide, general },
        Instruction::Jump { displacement: 5 },
    ]);
    let retain_raw_offset = instructions
        .iter()
        .map(|instruction| instruction.size())
        .sum::<usize>();
    instructions.push(restore(5));
    for position in retain_raw_branches {
        let next_offset = instructions[..=position]
            .iter()
            .map(|instruction| instruction.size())
            .sum::<usize>();
        if let Instruction::Conditional { displacement, .. } = &mut instructions[position] {
            *displacement = (retain_raw_offset - next_offset) as i32;
        }
    }
    instructions
}

fn encode_instruction(instruction: Instruction, bytes: &mut Vec<u8>) {
    let rex = |wide, general: u8| 0x40 | if wide { 8 } else { 0 } | (general >> 3);
    match instruction {
        Instruction::Transfer {
            save,
            wide,
            vector,
            general,
        } => bytes.extend([
            0x66,
            rex(wide, general),
            0x0f,
            if save { 0x6e } else { 0x7e },
            0xc0 | vector << 3 | general & 7,
        ]),
        Instruction::Shift {
            left,
            wide,
            general,
            count,
        } => bytes.extend([
            rex(wide, general),
            0xc1,
            0xc0 | (if left { 4 } else { 5 }) << 3 | general & 7,
            count,
        ]),
        Instruction::Compare {
            wide,
            general,
            immediate,
        } => {
            bytes.extend([rex(wide, general), 0x81, 0xf8 | general & 7]);
            bytes.extend(immediate.to_le_bytes());
        }
        Instruction::Conditional {
            equal,
            displacement,
        } => {
            bytes.extend([0x0f, if equal { 0x84 } else { 0x85 }]);
            bytes.extend(displacement.to_le_bytes());
        }
        Instruction::Jump { displacement } => {
            bytes.push(0xe9);
            bytes.extend(displacement.to_le_bytes());
        }
        Instruction::SubtractOne { wide, general } => {
            bytes.extend([rex(wide, general), 0x83, 0xe8 | general & 7, 1])
        }
        Instruction::Crash => bytes.extend([0x0f, 0x0b]),
    }
}

/// Decode only the canonical instruction subset from opcode fields. Neither
/// requested policy nor the producer's instruction plan is consulted.
fn decode_instruction(bytes: &[u8]) -> Option<(Instruction, &[u8])> {
    let immediate = |offset| {
        Some(u32::from_le_bytes(
            bytes.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    match bytes {
        [0x0f, 0x0b, tail @ ..] => Some((Instruction::Crash, tail)),
        [0x0f, condition @ (0x84 | 0x85), ..] => Some((
            Instruction::Conditional {
                equal: *condition == 0x84,
                displacement: immediate(2)? as i32,
            },
            bytes.get(6..)?,
        )),
        [0xe9, ..] => Some((
            Instruction::Jump {
                displacement: immediate(1)? as i32,
            },
            bytes.get(5..)?,
        )),
        [0x66, rex, 0x0f, opcode @ (0x6e | 0x7e), operands, tail @ ..]
            if rex & 0xf6 == 0x40 && operands & 0xc0 == 0xc0 =>
        {
            Some((
                Instruction::Transfer {
                    save: *opcode == 0x6e,
                    wide: rex & 8 != 0,
                    vector: (operands >> 3) & 7,
                    general: (operands & 7) | (rex & 1) << 3,
                },
                tail,
            ))
        }
        [rex, opcode, operands, ..] if rex & 0xf6 == 0x40 && operands & 0xc0 == 0xc0 => {
            let wide = rex & 8 != 0;
            let general = (operands & 7) | (rex & 1) << 3;
            let extension = (operands >> 3) & 7;
            match (*opcode, extension) {
                (0xc1, 4 | 5) => {
                    let count = *bytes.get(3)?;
                    if count == 0 || count >= if wide { 64 } else { 32 } {
                        return None;
                    }
                    Some((
                        Instruction::Shift {
                            left: extension == 4,
                            wide,
                            general,
                            count,
                        },
                        bytes.get(4..)?,
                    ))
                }
                (0x81, 7) => Some((
                    Instruction::Compare {
                        wide,
                        general,
                        immediate: immediate(3)?,
                    },
                    bytes.get(7..)?,
                )),
                (0x83, 5) if bytes.get(3) == Some(&1) => {
                    Some((Instruction::SubtractOne { wide, general }, bytes.get(4..)?))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// A checker cursor validates decoded instructions and actual byte targets;
/// it does not reconstruct or invoke the producer's program.
struct Replay<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Replay<'_> {
    fn next(&mut self) -> Result<Instruction, Error> {
        let bytes = self
            .bytes
            .get(self.position..)
            .ok_or(Error::MalformedEncoding)?;
        let (instruction, tail) = decode_instruction(bytes).ok_or(Error::MalformedEncoding)?;
        self.position += bytes.len() - tail.len();
        Ok(instruction)
    }
    fn require(&mut self, expected: Instruction) -> Result<(), Error> {
        if self.next()? == expected {
            Ok(())
        } else {
            Err(Error::EncodedFormMismatch)
        }
    }
    fn branch(&mut self, equal: bool, destination: usize) -> Result<(), Error> {
        let Instruction::Conditional {
            equal: actual_equal,
            displacement,
        } = self.next()?
        else {
            return Err(Error::EncodedFormMismatch);
        };
        if actual_equal == equal
            && self.position as i64 + i64::from(displacement) == destination as i64
        {
            Ok(())
        } else {
            Err(Error::EncodedFormMismatch)
        }
    }
    fn exponent(&mut self, wide: bool, vector: u8, general: u8) -> Result<(), Error> {
        self.require(Instruction::Transfer {
            save: false,
            wide,
            vector,
            general,
        })?;
        self.require(Instruction::Shift {
            left: true,
            wide,
            general,
            count: 1,
        })?;
        self.require(Instruction::Shift {
            left: false,
            wide,
            general,
            count: if wide { 53 } else { 24 },
        })?;
        self.require(Instruction::Compare {
            wide,
            general,
            immediate: if wide { 2047 } else { 255 },
        })
    }
    fn shifted_zero(
        &mut self,
        wide: bool,
        vector: u8,
        general: u8,
        count: u8,
    ) -> Result<(), Error> {
        self.require(Instruction::Transfer {
            save: false,
            wide,
            vector,
            general,
        })?;
        self.require(Instruction::Shift {
            left: true,
            wide,
            general,
            count,
        })?;
        self.require(Instruction::Compare {
            wide,
            general,
            immediate: 0,
        })
    }
}

fn validate_program(
    form: FloatResultPolicyForm,
    registers: &[u8],
    bytes: &[u8],
) -> Result<(), Error> {
    let wide = !form.is_binary32();
    let inputs = form.operand_count();
    let general = registers[inputs];
    let mut replay = Replay { bytes, position: 0 };
    for (position, &source) in registers[..inputs].iter().enumerate() {
        replay.require(Instruction::Transfer {
            save: true,
            wide,
            vector: 3 + position as u8,
            general: source,
        })?;
    }
    if form.can_trap() {
        replay.exponent(wide, 3, general)?;
        replay.branch(false, replay.position + 8)?;
        replay.require(Instruction::Crash)?;
        replay.require(Instruction::Transfer {
            save: false,
            wide,
            vector: 3,
            general,
        })?;
    } else {
        let raw_destination = bytes.len().checked_sub(5).ok_or(Error::MalformedEncoding)?;
        for vector in [3, 4] {
            replay.exponent(wide, vector, general)?;
            replay.branch(true, raw_destination)?;
        }
        if form.policy
            == FloatResultPolicyKind::Saturating(FloatSaturationRule::FiniteDivisionOverflow)
        {
            replay.shifted_zero(wide, 4, general, 1)?;
            replay.branch(true, raw_destination)?;
        }
        replay.exponent(wide, 5, general)?;
        replay.branch(false, raw_destination)?;
        replay.shifted_zero(wide, 5, general, if wide { 12 } else { 9 })?;
        replay.branch(false, raw_destination)?;
        replay.require(Instruction::Transfer {
            save: false,
            wide,
            vector: 5,
            general,
        })?;
        replay.require(Instruction::SubtractOne { wide, general })?;
        let Instruction::Jump { displacement } = replay.next()? else {
            return Err(Error::EncodedFormMismatch);
        };
        if replay.position != raw_destination
            || replay.position as i64 + i64::from(displacement) != bytes.len() as i64
        {
            return Err(Error::EncodedFormMismatch);
        }
        replay.require(Instruction::Transfer {
            save: false,
            wide,
            vector: 5,
            general,
        })?;
    }
    if replay.position != bytes.len() {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(())
}

fn registers(
    physical: &ValidatedPhysicalRegisterModel,
    form: FloatResultPolicyForm,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<Vec<u8>, Error> {
    super::request_validation::validate_request(
        physical,
        SelectedInstructionKind::FloatResultPolicy { form },
        alternative,
        operands,
    )?;
    super::request_validation::resolve_registers(physical, operands)
}

pub(crate) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    form: FloatResultPolicyForm,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let registers = registers(physical, form, alternative, operands)?;
    let mut bytes = Vec::new();
    for instruction in program(form, &registers) {
        encode_instruction(instruction, &mut bytes);
    }
    validate(physical, form, alternative, operands, &bytes)
}

pub(crate) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    form: FloatResultPolicyForm,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let registers = registers(physical, form, alternative, operands)?;
    validate_program(form, &registers, bytes)?;
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(form, operands),
    })
}

#[cfg(test)]
mod tests;
