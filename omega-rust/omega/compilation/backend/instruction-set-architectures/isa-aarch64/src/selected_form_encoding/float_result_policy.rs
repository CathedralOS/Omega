//! Adapt completed IEEE payloads without evaluating floating-point arithmetic.
//!
//! All inputs are saved into v16..v17 before the result register is used as
//! scratch. Thus the output may alias any input, while no temporary general
//! register becomes an undeclared clobber. Integer field extraction detects
//! nonfinite exponents, infinity and either signed zero; NaN payloads never
//! pass through a floating-point operation. FPCR is neither read nor changed.
//! NZCV and the scratch vectors are explicit clobbers.
//!
//! Saturation keeps the raw result unless input eligibility is true and the
//! result is infinity. The scalar selector already checked input carriers and zero divisors. Subtracting one
//! from either infinity's raw encoding produces its signed maximum finite
//! neighbor. Trapping rejects every all-ones exponent at this operation's
//! own BRK, before publishing the result.
//!
//! Word fields follow Arm's A64 UBFM and B.cond encodings:
//! https://documentation-service.arm.com/static/67e40f3398aa3c3b6eea6a85
//! https://documentation-service.arm.com/static/685eb35c3cb5b729562ba389
use crate::machine_effects::form_alternative;

use super::{Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint};
use register_model::{RegisterConstraintKey, RegisterUnitId, RegisterViewId};
use selected_instructions::{
    FloatResultPolicyForm, FloatResultPolicyKind, IeeeFloatFormat, MachineAlternative,
    MachineAlternativeApplicability, MachineAlternativeFamily, MachineAlternativeKey,
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration,
    MachineEncodedEffects, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior,
};

fn clobbers(form: FloatResultPolicyForm) -> Vec<RegisterUnitId> {
    let physical = crate::aarch64_physical_register_model_ref();
    let names: &[&str] = if form.can_trap() {
        &["q16", "nzcv"]
    } else {
        &["q16", "q17", "nzcv"]
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
    MachineEncodedEffects::float_result_policy_v1(
        form.operand_count() as u16,
        clobbers(form),
        form.can_trap(),
    )
}

pub(crate) fn declaration(
    form: FloatResultPolicyForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    let trapping = form.policy == FloatResultPolicyKind::Trapping;
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::FloatResultPolicy(form),
        constraint,
        memory: MachineMemoryEffect::NoneV1,
        trap: if trapping {
            MachineTrapBehavior::TrappingIntegerV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        barrier: if trapping {
            MachineBarrier::ExternalEffect
        } else {
            MachineBarrier::None
        },
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: form_alternative!(
            MachineAlternativeFamily::FloatResultPolicy(form),
            MachineSizeKnowledge::ExactBytes((program(form, &[0; 4]).len() * 4) as u16),
            effects(form)
        ),
    }
}

pub(crate) fn footprint(
    form: FloatResultPolicyForm,
    operands: &[RegisterViewId],
) -> Aarch64SelectedFormFootprint {
    let inputs = form.operand_count();
    Aarch64SelectedFormFootprint {
        register_reads: operands[..inputs].to_vec(),
        register_writes: operands[inputs..inputs + 1].to_vec(),
        writes_nzcv: true,
        encoded: effects(form),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Word {
    Save {
        wide: bool,
        general: u8,
        vector: u8,
    },
    Restore {
        wide: bool,
        vector: u8,
        general: u8,
    },
    Extract {
        wide: bool,
        source: u8,
        result: u8,
        first: u8,
        count: u8,
    },
    Compare {
        wide: bool,
        source: u8,
        immediate: u16,
    },
    Conditional {
        equal: bool,
        words: i32,
    },
    Jump {
        words: i32,
    },
    SubtractOne {
        wide: bool,
        general: u8,
    },
    Crash,
}

fn program(form: FloatResultPolicyForm, registers: &[u8]) -> Vec<Word> {
    let wide = form.format == IeeeFloatFormat::Binary64;
    let (fraction, exponent) = if wide { (52, 11) } else { (23, 8) };
    let exponent_mask = (1_u16 << exponent) - 1;
    let inputs = form.operand_count();
    let result = registers[inputs];
    let mut words = Vec::new();
    for (position, &general) in registers[..inputs].iter().enumerate() {
        words.push(Word::Save {
            wide,
            general,
            vector: 16 + position as u8,
        });
    }
    let restore = |vector| Word::Restore {
        wide,
        vector,
        general: result,
    };
    let extract = |first, count| Word::Extract {
        wide,
        source: result,
        result,
        first,
        count,
    };
    let compare = |immediate| Word::Compare {
        wide,
        source: result,
        immediate,
    };
    if form.policy == FloatResultPolicyKind::Trapping {
        words.extend([
            restore(16),
            extract(fraction, exponent),
            compare(exponent_mask),
            Word::Conditional {
                equal: false,
                words: 2,
            },
            Word::Crash,
            restore(16),
        ]);
        return words;
    }
    let mut retain_raw_branches = Vec::new();
    words.extend([restore(16), extract(0, 8), compare(0)]);
    retain_raw_branches.push(words.len());
    words.push(Word::Conditional {
        equal: true,
        words: 0,
    });
    // Infinity has an all-ones exponent and an empty fraction. An unordered
    // result is preserved, not converted to a finite value.
    for (first, count, immediate) in [(fraction, exponent, exponent_mask), (0, fraction, 0)] {
        words.extend([restore(17), extract(first, count), compare(immediate)]);
        retain_raw_branches.push(words.len());
        words.push(Word::Conditional {
            equal: false,
            words: 0,
        });
    }
    words.extend([
        restore(17),
        Word::SubtractOne {
            wide,
            general: result,
        },
        Word::Jump { words: 2 },
    ]);
    let retain_raw = words.len();
    words.push(restore(17));
    for position in retain_raw_branches {
        if let Word::Conditional { words: offset, .. } = &mut words[position] {
            *offset = (retain_raw - position) as i32;
        }
    }
    words
}

fn encode_word(word: Word) -> u32 {
    match word {
        Word::Save {
            wide,
            general,
            vector,
        } => {
            (if wide { 0x9e67_0000 } else { 0x1e27_0000 })
                | u32::from(general) << 5
                | u32::from(vector)
        }
        Word::Restore {
            wide,
            vector,
            general,
        } => {
            (if wide { 0x9e66_0000 } else { 0x1e26_0000 })
                | u32::from(vector) << 5
                | u32::from(general)
        }
        Word::Extract {
            wide,
            source,
            result,
            first,
            count,
        } => {
            (if wide { 0xd340_0000 } else { 0x5300_0000 })
                | u32::from(first) << 16
                | u32::from(first + count - 1) << 10
                | u32::from(source) << 5
                | u32::from(result)
        }
        Word::Compare {
            wide,
            source,
            immediate,
        } => {
            (if wide { 0xf100_001f } else { 0x7100_001f })
                | u32::from(immediate) << 10
                | u32::from(source) << 5
        }
        Word::Conditional { equal, words } => {
            0x5400_0000 | ((words as u32 & 0x7ffff) << 5) | u32::from(!equal)
        }
        Word::Jump { words } => 0x1400_0000 | (words as u32 & 0x3ff_ffff),
        Word::SubtractOne { wide, general } => {
            (if wide { 0xd100_0400 } else { 0x5100_0400 })
                | u32::from(general) << 5
                | u32::from(general)
        }
        Word::Crash => 0xd420_0000,
    }
}

/// Decode opcode fields without consulting the producer or requested form.
fn decode_word(word: u32) -> Option<Word> {
    let register = |shift: u32| ((word >> shift) & 31_u32) as u8;
    let wide = word >> 31 != 0;
    match word & 0xffff_fc00 {
        0x9e67_0000 | 0x1e27_0000 => {
            return Some(Word::Save {
                wide,
                general: register(5),
                vector: register(0),
            });
        }
        0x9e66_0000 | 0x1e26_0000 => {
            return Some(Word::Restore {
                wide,
                vector: register(5),
                general: register(0),
            });
        }
        0xd100_0400 | 0x5100_0400 if register(5) == register(0) => {
            return Some(Word::SubtractOne {
                wide,
                general: register(0),
            });
        }
        _ => {}
    }
    if matches!(word & 0xffc0_0000, 0xd340_0000 | 0x5300_0000) {
        let first = ((word >> 16) & 63) as u8;
        let last = ((word >> 10) & 63) as u8;
        if first > last || last >= if wide { 64 } else { 32 } {
            return None;
        }
        return Some(Word::Extract {
            wide,
            source: register(5),
            result: register(0),
            first,
            count: last - first + 1,
        });
    }
    if matches!(word & 0xffc0_001f, 0xf100_001f | 0x7100_001f) {
        return Some(Word::Compare {
            wide,
            source: register(5),
            immediate: ((word >> 10) & 4095) as u16,
        });
    }
    if word & 0xff00_001e == 0x5400_0000 {
        return Some(Word::Conditional {
            equal: word & 1 == 0,
            words: ((word as i32) << 8) >> 13,
        });
    }
    if word & 0xfc00_0000 == 0x1400_0000 {
        return Some(Word::Jump {
            words: ((word as i32) << 6) >> 6,
        });
    }
    (word == 0xd420_0000).then_some(Word::Crash)
}

pub(crate) fn encode(form: FloatResultPolicyForm, registers: &[u8]) -> Result<Vec<u8>, Error> {
    if registers.len() != form.operand_count() + 1
        || registers.iter().any(|register| *register > 30)
    {
        return Err(Error::OperandCountMismatch);
    }
    Ok(program(form, registers)
        .into_iter()
        .flat_map(|word| encode_word(word).to_le_bytes())
        .collect())
}

pub(crate) fn validate(
    form: FloatResultPolicyForm,
    registers: &[u8],
    bytes: &[u8],
) -> Result<(), Error> {
    if registers.len() != form.operand_count() + 1
        || registers.iter().any(|register| *register > 30)
    {
        return Err(Error::OperandCountMismatch);
    }
    let (chunks, remainder) = bytes.as_chunks::<4>();
    if !remainder.is_empty() {
        return Err(Error::MalformedEncoding);
    }
    let decoded = chunks
        .iter()
        .map(|word| decode_word(u32::from_le_bytes(*word)))
        .collect::<Option<Vec<_>>>()
        .ok_or(Error::MalformedEncoding)?;
    if decoded != program(form, registers) {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
