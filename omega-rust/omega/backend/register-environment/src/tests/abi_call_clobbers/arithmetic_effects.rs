use super::{
    Architecture, BTreeSet, EffectRejection, RegisterConstraintKey, assert_target_semantic_error,
    baseline_target_register_environment, convention_for, produced_effects, row_mut,
    scalar_abi_cases, target_constraint_catalog, target_physical_register_model, units_for_names,
    validate_effects, validate_physical_register_model, validate_target_register_environment,
    validated_effects,
};
use register_model::RegisterOperandAccess;
use selected_instructions::SaturatingCarrier;
use selected_instructions::{
    MachineAlternativeApplicability, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalog, MachineEffectCatalogValidationError, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior,
};

/// Every selected arithmetic rule key with the semantic families the effect
/// catalog must declare for it. The sweep is keyed off the validated
/// environment's selection, never off the effect producer's own roster;
/// shared keys carry one declaration per declared semantic — the Boolean
/// materializations, the copy/normalization forms, and the add/subtract
/// realization families.
fn selected_arithmetic_rules(
    environment: &crate::ValidatedTargetRegisterEnvironment,
) -> Vec<(RegisterConstraintKey, MachineSemanticKind)> {
    use MachineSemanticKind::*;
    let keys = environment.selected_keys();
    [
        (Some(keys.materialize_i64), &[MaterializeI64][..]),
        (
            Some(keys.materialize_boolean),
            &[
                MaterializeBooleanEqual,
                MaterializeBooleanU64LessThan,
                MaterializeBooleanI64LessThan,
                MaterializeBooleanU64LessOrEqual,
                MaterializeBooleanI64LessOrEqual,
            ][..],
        ),
        (
            Some(keys.copy_i64),
            &[
                CopyI64,
                ZeroExtendU8,
                ZeroExtendU16,
                SignExtendI8,
                SignExtendI16,
                SignExtendI32,
                ZeroExtendU32,
            ][..],
        ),
        (keys.float32_to_bits, &[Float32ToBits][..]),
        (keys.float64_to_bits, &[Float64ToBits][..]),
        (keys.bits_to_float32, &[BitsToFloat32][..]),
        (keys.bits_to_float64, &[BitsToFloat64][..]),
        (
            Some(keys.add_i64),
            &[ByteViewAddress, ExactAddI64, WrappingAddI64][..],
        ),
        (Some(keys.add_i64_immediate), &[ExactAddI64Immediate][..]),
        (
            Some(keys.subtract_i64),
            &[BitwiseAndI64, BitwiseXorI64, ExactSubtractI64][..],
        ),
        (Some(keys.multiply_i64), &[ExactMultiplyI64][..]),
        (
            Some(keys.subtract_i64_immediate),
            &[ExactSubtractI64Immediate][..],
        ),
        // Operand shape, not width, selects the saturating rows: unsigned
        // subtract and the u64 add saturate on the carry flag in three
        // operands, unsigned divide shares the exact unsigned divide row,
        // and every other carrier clamps through an early-clobber scratch.
        (
            Some(keys.saturating_subtract_unsigned),
            &[
                SaturatingSubtract(SaturatingCarrier::U8),
                SaturatingSubtract(SaturatingCarrier::U16),
                SaturatingSubtract(SaturatingCarrier::U32),
                SaturatingSubtract(SaturatingCarrier::U64),
            ][..],
        ),
        (
            Some(keys.saturating_add_u64),
            &[SaturatingAdd(SaturatingCarrier::U64)][..],
        ),
        (
            Some(keys.divide_u64),
            &[
                ExactDivideU64,
                SaturatingDivide(SaturatingCarrier::U8),
                SaturatingDivide(SaturatingCarrier::U16),
                SaturatingDivide(SaturatingCarrier::U32),
                SaturatingDivide(SaturatingCarrier::U64),
            ][..],
        ),
        // Saturating remainder shares the ordinary remainder rows: the
        // mathematical remainder already lies inside its carrier.
        (
            Some(keys.remainder_u64),
            &[
                SaturatingRemainder(SaturatingCarrier::U8),
                SaturatingRemainder(SaturatingCarrier::U16),
                SaturatingRemainder(SaturatingCarrier::U32),
                SaturatingRemainder(SaturatingCarrier::U64),
            ][..],
        ),
        (
            Some(keys.remainder_i64),
            &[
                WrappingRemainderI64,
                SaturatingRemainder(SaturatingCarrier::I8),
                SaturatingRemainder(SaturatingCarrier::I16),
                SaturatingRemainder(SaturatingCarrier::I32),
                SaturatingRemainder(SaturatingCarrier::I64),
            ][..],
        ),
        (
            Some(keys.saturating_add_clamped),
            &[
                SaturatingAdd(SaturatingCarrier::I8),
                SaturatingAdd(SaturatingCarrier::I16),
                SaturatingAdd(SaturatingCarrier::I32),
                SaturatingAdd(SaturatingCarrier::I64),
                SaturatingAdd(SaturatingCarrier::U8),
                SaturatingAdd(SaturatingCarrier::U16),
                SaturatingAdd(SaturatingCarrier::U32),
            ][..],
        ),
        (
            Some(keys.saturating_subtract_clamped),
            &[
                SaturatingSubtract(SaturatingCarrier::I8),
                SaturatingSubtract(SaturatingCarrier::I16),
                SaturatingSubtract(SaturatingCarrier::I32),
                SaturatingSubtract(SaturatingCarrier::I64),
            ][..],
        ),
        (
            Some(keys.saturating_divide_signed),
            &[
                SaturatingDivide(SaturatingCarrier::I8),
                SaturatingDivide(SaturatingCarrier::I16),
                SaturatingDivide(SaturatingCarrier::I32),
                SaturatingDivide(SaturatingCarrier::I64),
            ][..],
        ),
        (Some(keys.compare_i64_zero), &[CompareI64Zero][..]),
        (Some(keys.compare_i64), &[CompareI64][..]),
        (Some(keys.compare_i64_immediate), &[CompareI64Immediate][..]),
    ]
    .into_iter()
    .flat_map(|(key, semantics)| {
        key.into_iter()
            .flat_map(move |key| semantics.iter().map(move |semantic| (key, *semantic)))
    })
    .collect()
}

/// Where the architecture's condition-code view sits in one rule's custody:
/// the Boolean materializations read it, the compares and the AArch64
/// saturating forms define it, and the x86-64 flag-writing realizations
/// clobber it.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum FlagsCustody {
    #[default]
    None,
    Use,
    Def,
    Clobber,
}

/// The declared arithmetic contract for one selected semantic on one ISA:
/// the constraint row's operand access pattern, its float-class and
/// early-clobber positions, any ABI-pinned operand views, its condition-code
/// and extra clobber custody, the encoded trap surface, and each admitted
/// alternative's applicability, size knowledge, and external operand custody.
struct ArithmeticContract {
    accesses: &'static [RegisterOperandAccess],
    float_operands: &'static [u16],
    early_clobbers: &'static [u16],
    fixed_views: &'static [(u16, &'static str)],
    flags: FlagsCustody,
    extra_clobbers: &'static [&'static str],
    faulting: bool,
    alternatives: Vec<AlternativeContract>,
}

impl ArithmeticContract {
    fn plain(
        accesses: &'static [RegisterOperandAccess],
        alternatives: Vec<AlternativeContract>,
    ) -> Self {
        Self {
            accesses,
            float_operands: &[],
            early_clobbers: &[],
            fixed_views: &[],
            flags: FlagsCustody::None,
            extra_clobbers: &[],
            faulting: false,
            alternatives,
        }
    }
}

struct AlternativeContract {
    applicability: MachineAlternativeApplicability,
    size: MachineSizeKnowledge,
    reads: &'static [u16],
    writes: &'static [u16],
}

/// The declared size knowledge for one selected arithmetic semantic on each
/// ISA: the x86-64 encoder-resolved envelopes and multi-instruction forms,
/// and the fixed-width AArch64 encodings.
fn expected_size(
    semantic: MachineSemanticKind,
    architecture: Architecture,
) -> MachineSizeKnowledge {
    use MachineSemanticKind::*;
    let resolved = |minimum_bytes: u16, maximum_bytes: u16| MachineSizeKnowledge::EncoderResolved {
        minimum_bytes,
        maximum_bytes: Some(maximum_bytes),
    };
    match architecture {
        Architecture::X86_64 => match semantic {
            MaterializeI64 => MachineSizeKnowledge::ExactBytes(10),
            MaterializeBooleanEqual
            | MaterializeBooleanU64LessThan
            | MaterializeBooleanI64LessThan
            | MaterializeBooleanU64LessOrEqual
            | MaterializeBooleanI64LessOrEqual => MachineSizeKnowledge::ExactBytes(8),
            CopyI64 | ZeroExtendU32 | SignExtendI32 | CompareI64Zero | CompareI64
            | ExactDivideU64 => MachineSizeKnowledge::ExactBytes(3),
            ZeroExtendU8 | ZeroExtendU16 | SignExtendI8 | SignExtendI16 => {
                MachineSizeKnowledge::ExactBytes(4)
            }
            Float32ToBits | BitsToFloat32 => resolved(4, 5),
            Float64ToBits | BitsToFloat64 => MachineSizeKnowledge::ExactBytes(5),
            WrappingRemainderI64 => MachineSizeKnowledge::ExactBytes(17),
            // mov/add plus movabs/cmp/cmov per clamped bound (17 each); the
            // i64 forms derive the saturated value with mov/not/sar/btc (15)
            // before mov/add/cmovo (10); the i64 divide guards MIN / -1 with
            // cmp/sbb/or/neg/lea/cmovo (21) before cqo/idiv (5).
            SaturatingAdd(SaturatingCarrier::U64) => MachineSizeKnowledge::ExactBytes(19),
            SaturatingAdd(SaturatingCarrier::I64) | SaturatingSubtract(SaturatingCarrier::I64) => {
                MachineSizeKnowledge::ExactBytes(25)
            }
            SaturatingAdd(carrier) | SaturatingSubtract(carrier) if carrier.is_signed() => {
                MachineSizeKnowledge::ExactBytes(40)
            }
            SaturatingAdd(_) => MachineSizeKnowledge::ExactBytes(23),
            SaturatingSubtract(_) => MachineSizeKnowledge::ExactBytes(13),
            SaturatingDivide(SaturatingCarrier::I64) => MachineSizeKnowledge::ExactBytes(26),
            SaturatingDivide(carrier) if carrier.is_signed() => {
                MachineSizeKnowledge::ExactBytes(22)
            }
            SaturatingDivide(_) => MachineSizeKnowledge::ExactBytes(3),
            // Signed remainder is cqo + idiv + the mov out of RDX; unsigned
            // remainder is xor-edx + div + the same move.
            SaturatingRemainder(carrier) if carrier.is_signed() => {
                MachineSizeKnowledge::ExactBytes(17)
            }
            SaturatingRemainder(_) => MachineSizeKnowledge::ExactBytes(9),
            CompareI64Immediate => MachineSizeKnowledge::ExactBytes(7),
            BitwiseAndI64 | BitwiseXorI64 => resolved(3, 6),
            ByteViewAddress | WrappingAddI64 | ExactAddI64 => resolved(4, 5),
            ExactAddI64Immediate | ExactSubtractI64Immediate => resolved(4, 8),
            ExactSubtractI64 | ExactMultiplyI64 => {
                unreachable!("x86-64 subtract/multiply declare alias-dependent alternatives")
            }
            other => panic!("{other:?} is not a selected arithmetic rule"),
        },
        Architecture::Aarch64 => match semantic {
            MaterializeI64 => resolved(4, 16),
            WrappingRemainderI64 => MachineSizeKnowledge::ExactBytes(8),
            // One arithmetic word plus mov/cmp/csel per clamped bound; the
            // i64 add and subtract use asr/eor/adds/csel; the i64 divide uses
            // sdiv/mov/cmp/ccmn/mov/csel; the flag-select u64 add and every
            // unsigned subtract are two words; unsigned divide is one.
            SaturatingAdd(SaturatingCarrier::U64) => MachineSizeKnowledge::ExactBytes(8),
            SaturatingAdd(SaturatingCarrier::I64) | SaturatingSubtract(SaturatingCarrier::I64) => {
                MachineSizeKnowledge::ExactBytes(16)
            }
            SaturatingAdd(carrier) | SaturatingSubtract(carrier) if carrier.is_signed() => {
                MachineSizeKnowledge::ExactBytes(28)
            }
            SaturatingAdd(_) => MachineSizeKnowledge::ExactBytes(16),
            SaturatingSubtract(_) => MachineSizeKnowledge::ExactBytes(8),
            SaturatingDivide(SaturatingCarrier::I64) => MachineSizeKnowledge::ExactBytes(24),
            SaturatingDivide(carrier) if carrier.is_signed() => {
                MachineSizeKnowledge::ExactBytes(16)
            }
            SaturatingDivide(_) => MachineSizeKnowledge::ExactBytes(4),
            // Divide plus MSUB: two words like the ordinary remainder row.
            SaturatingRemainder(_) => MachineSizeKnowledge::ExactBytes(8),
            _ => MachineSizeKnowledge::ExactBytes(4),
        },
    }
}

/// The declared arithmetic contract one selected semantic must carry on each
/// ISA. The two ISAs share operand shapes but own different flag custody:
/// x86-64 realizes flag-writing forms as clobbers and pins the divide and
/// remainder rows to `rax`/`rdx`, while AArch64 names `nzcv` definitions on
/// the compares and saturating forms and leaves everything else allocatable.
fn arithmetic_contract(
    semantic: MachineSemanticKind,
    architecture: Architecture,
) -> ArithmeticContract {
    use MachineSemanticKind::*;
    use RegisterOperandAccess::{Def, Use};
    let x86 = architecture == Architecture::X86_64;
    let one = |reads: &'static [u16], writes: &'static [u16]| {
        vec![AlternativeContract {
            applicability: MachineAlternativeApplicability::Always,
            size: expected_size(semantic, architecture),
            reads,
            writes,
        }]
    };
    match semantic {
        MaterializeI64 => ArithmeticContract::plain(&[Def], one(&[], &[0])),
        MaterializeBooleanEqual
        | MaterializeBooleanU64LessThan
        | MaterializeBooleanI64LessThan
        | MaterializeBooleanU64LessOrEqual
        | MaterializeBooleanI64LessOrEqual => ArithmeticContract {
            flags: FlagsCustody::Use,
            ..ArithmeticContract::plain(&[Def], one(&[], &[0]))
        },
        CopyI64 | ZeroExtendU8 | ZeroExtendU16 | ZeroExtendU32 | SignExtendI8 | SignExtendI16
        | SignExtendI32 => ArithmeticContract::plain(&[Use, Def], one(&[0], &[1])),
        Float32ToBits | Float64ToBits => ArithmeticContract {
            float_operands: &[0],
            ..ArithmeticContract::plain(&[Use, Def], one(&[0], &[1]))
        },
        BitsToFloat32 | BitsToFloat64 => ArithmeticContract {
            float_operands: &[1],
            ..ArithmeticContract::plain(&[Use, Def], one(&[0], &[1]))
        },
        ByteViewAddress | WrappingAddI64 | ExactAddI64 => {
            ArithmeticContract::plain(&[Use, Use, Def], one(&[0, 1], &[2]))
        }
        ExactAddI64Immediate | ExactSubtractI64Immediate => {
            ArithmeticContract::plain(&[Use, Def], one(&[0], &[1]))
        }
        BitwiseAndI64 | BitwiseXorI64 => ArithmeticContract {
            flags: if x86 {
                FlagsCustody::Clobber
            } else {
                FlagsCustody::None
            },
            ..ArithmeticContract::plain(&[Use, Use, Def], one(&[0, 1], &[2]))
        },
        ExactSubtractI64 => ArithmeticContract {
            flags: if x86 {
                FlagsCustody::Clobber
            } else {
                FlagsCustody::None
            },
            alternatives: if x86 {
                vec![
                    // The alias-safe realization family: `xor` when both
                    // inputs share the result view, `sub` against the left or
                    // right input, and `mov; sub` when the result is distinct.
                    AlternativeContract {
                        applicability: MachineAlternativeApplicability::ResultAliasesOperands {
                            result: 2,
                            left: 0,
                            right: 1,
                        },
                        size: MachineSizeKnowledge::ExactBytes(3),
                        reads: &[],
                        writes: &[2],
                    },
                    AlternativeContract {
                        applicability:
                            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                                result: 2,
                                aliased_operand: 0,
                                distinct_operand: 1,
                            },
                        size: MachineSizeKnowledge::ExactBytes(3),
                        reads: &[0, 1],
                        writes: &[2],
                    },
                    AlternativeContract {
                        applicability:
                            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                                result: 2,
                                aliased_operand: 1,
                                distinct_operand: 0,
                            },
                        size: MachineSizeKnowledge::ExactBytes(6),
                        reads: &[0, 1],
                        writes: &[2],
                    },
                    AlternativeContract {
                        applicability:
                            MachineAlternativeApplicability::ResultDistinctFromOperands {
                                result: 2,
                                left: 0,
                                right: 1,
                            },
                        size: MachineSizeKnowledge::ExactBytes(6),
                        reads: &[0, 1],
                        writes: &[2],
                    },
                ]
            } else {
                one(&[0, 1], &[2])
            },
            ..ArithmeticContract::plain(&[Use, Use, Def], Vec::new())
        },
        // `imul` reads its in-place destination input even when every operand
        // shares one view, so the fully-aliased variant still reads both
        // logical operands — unlike the `xor`-realized subtract.
        ExactMultiplyI64 => ArithmeticContract {
            flags: if x86 {
                FlagsCustody::Clobber
            } else {
                FlagsCustody::None
            },
            alternatives: if x86 {
                vec![
                    AlternativeContract {
                        applicability: MachineAlternativeApplicability::ResultAliasesOperands {
                            result: 2,
                            left: 0,
                            right: 1,
                        },
                        size: MachineSizeKnowledge::ExactBytes(4),
                        reads: &[0, 1],
                        writes: &[2],
                    },
                    AlternativeContract {
                        applicability:
                            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                                result: 2,
                                aliased_operand: 0,
                                distinct_operand: 1,
                            },
                        size: MachineSizeKnowledge::ExactBytes(4),
                        reads: &[0, 1],
                        writes: &[2],
                    },
                    AlternativeContract {
                        applicability:
                            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                                result: 2,
                                aliased_operand: 1,
                                distinct_operand: 0,
                            },
                        size: MachineSizeKnowledge::ExactBytes(4),
                        reads: &[0, 1],
                        writes: &[2],
                    },
                    AlternativeContract {
                        applicability:
                            MachineAlternativeApplicability::ResultDistinctFromOperands {
                                result: 2,
                                left: 0,
                                right: 1,
                            },
                        size: MachineSizeKnowledge::ExactBytes(7),
                        reads: &[0, 1],
                        writes: &[2],
                    },
                ]
            } else {
                one(&[0, 1], &[2])
            },
            ..ArithmeticContract::plain(&[Use, Use, Def], Vec::new())
        },
        SaturatingAdd(SaturatingCarrier::U64)
        | SaturatingSubtract(
            SaturatingCarrier::U8
            | SaturatingCarrier::U16
            | SaturatingCarrier::U32
            | SaturatingCarrier::U64,
        ) => ArithmeticContract {
            early_clobbers: if x86 { &[2] } else { &[] },
            flags: if x86 {
                FlagsCustody::Clobber
            } else {
                FlagsCustody::Def
            },
            ..ArithmeticContract::plain(&[Use, Use, Def], one(&[0, 1], &[2]))
        },
        ExactDivideU64
        | SaturatingDivide(
            SaturatingCarrier::U8
            | SaturatingCarrier::U16
            | SaturatingCarrier::U32
            | SaturatingCarrier::U64,
        ) => ArithmeticContract {
            fixed_views: if x86 {
                &[(0, "rax"), (2, "rax"), (3, "rdx")]
            } else {
                &[]
            },
            flags: if x86 {
                FlagsCustody::Clobber
            } else {
                FlagsCustody::None
            },
            extra_clobbers: if x86 { &["rdx"] } else { &[] },
            faulting: x86,
            alternatives: if x86 {
                one(&[0, 1, 3], &[2])
            } else {
                one(&[0, 1], &[2])
            },
            ..ArithmeticContract::plain(
                if x86 {
                    &[Use, Use, Def, Use]
                } else {
                    &[Use, Use, Def]
                },
                Vec::new(),
            )
        },
        // x86-64 pins the divisor to RCX — a fixed view disjoint from RAX and
        // RDX — so the realized form may zero RDX before reading the divisor;
        // the RDX scratch is an ordinary late definition rather than a fixed
        // early clobber.
        WrappingRemainderI64 => ArithmeticContract {
            early_clobbers: if x86 { &[] } else { &[2] },
            fixed_views: if x86 {
                &[(0, "rax"), (1, "rcx"), (2, "rax"), (3, "rdx")]
            } else {
                &[]
            },
            flags: if x86 {
                FlagsCustody::Clobber
            } else {
                FlagsCustody::None
            },
            faulting: x86,
            alternatives: if x86 {
                one(&[0, 1], &[2, 3])
            } else {
                one(&[0, 1], &[2])
            },
            ..ArithmeticContract::plain(
                if x86 {
                    &[Use, Use, Def, Def]
                } else {
                    &[Use, Use, Def]
                },
                Vec::new(),
            )
        },
        // Saturating remainder rides the shared remainder rows: x86-64 pins
        // the divisor to RCX and defines RDX as the quotient-side scratch;
        // AArch64 divides then recovers the remainder through MSUB under an
        // early-clobber result.
        SaturatingRemainder(_) => ArithmeticContract {
            early_clobbers: if x86 { &[] } else { &[2] },
            fixed_views: if x86 {
                &[(0, "rax"), (1, "rcx"), (2, "rax"), (3, "rdx")]
            } else {
                &[]
            },
            flags: if x86 {
                FlagsCustody::Clobber
            } else {
                FlagsCustody::None
            },
            faulting: x86,
            alternatives: if x86 {
                one(&[0, 1], &[2, 3])
            } else {
                one(&[0, 1], &[2])
            },
            ..ArithmeticContract::plain(
                if x86 {
                    &[Use, Use, Def, Def]
                } else {
                    &[Use, Use, Def]
                },
                Vec::new(),
            )
        },
        // The clamped saturating forms (every add but u64, every signed
        // subtract) clamp through an early-clobber bound scratch; x86-64
        // additionally accumulates in an early-clobber result.
        SaturatingAdd(_) | SaturatingSubtract(_) => ArithmeticContract {
            early_clobbers: &[2, 3],
            flags: if x86 {
                FlagsCustody::Clobber
            } else {
                FlagsCustody::Def
            },
            ..ArithmeticContract::plain(&[Use, Use, Def, Def], one(&[0, 1], &[2, 3]))
        },
        // x86-64 keeps the unsigned-division operand shape (an explicit RDX
        // input that CQO discards and the clamp reuses); AArch64 clamps through
        // an early-clobber scratch beside an early-clobber result.
        // Signed division: x86-64 pins it to `rax`/`rdx` like unsigned
        // division; AArch64 clamps or guards through the bound scratch.
        SaturatingDivide(_) => ArithmeticContract {
            early_clobbers: if x86 { &[] } else { &[2, 3] },
            fixed_views: if x86 {
                &[(0, "rax"), (2, "rax"), (3, "rdx")]
            } else {
                &[]
            },
            flags: if x86 {
                FlagsCustody::Clobber
            } else {
                FlagsCustody::Def
            },
            extra_clobbers: if x86 { &["rdx"] } else { &[] },
            faulting: x86,
            alternatives: if x86 {
                one(&[0, 1, 3], &[2])
            } else {
                one(&[0, 1], &[2, 3])
            },
            ..ArithmeticContract::plain(
                if x86 {
                    &[Use, Use, Def, Use]
                } else {
                    &[Use, Use, Def, Def]
                },
                Vec::new(),
            )
        },
        CompareI64Zero | CompareI64Immediate => ArithmeticContract {
            flags: FlagsCustody::Def,
            ..ArithmeticContract::plain(&[Use], one(&[0], &[]))
        },
        CompareI64 => ArithmeticContract {
            flags: FlagsCustody::Def,
            ..ArithmeticContract::plain(&[Use, Use], one(&[0, 1], &[]))
        },
        other => panic!("{other:?} is not a selected arithmetic rule"),
    }
}

/// Several arithmetic semantics share one constraint key, so arithmetic
/// declarations are found by (key, semantic) rather than by key alone.
fn arithmetic_declaration_mut(
    catalog: &mut MachineEffectCatalog,
    key: RegisterConstraintKey,
    semantic: MachineSemanticKind,
) -> &mut MachineEffectDeclaration {
    catalog
        .declarations
        .iter_mut()
        .find(|declaration| declaration.constraint == key && declaration.semantic == semantic)
        .unwrap_or_else(|| {
            panic!("effect catalog missing arithmetic declaration {key:?}/{semantic:?}")
        })
}

/// Every selected arithmetic rule on every declared target/ABI pair is bound
/// to the operand custody, condition-code custody, and fall-through encoding
/// that pair's ABI declares.
#[test]
fn every_selected_arithmetic_rule_binds_the_declared_abi_arithmetic_contract() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let convention = convention_for(case, environment.physical());
        let catalog = validated_effects(case, environment.constraints());
        let keys = environment.selected_keys();
        let arithmetic_rules = selected_arithmetic_rules(&environment);
        assert_eq!(
            arithmetic_rules.len(),
            63,
            "{} selects an unexpected arithmetic roster",
            case.convention
        );

        // Arithmetic authority is declared for exactly the selected
        // arithmetic rules: every declaration carrying an arithmetic semantic
        // names its selected constraint key, and no selected rule may lack
        // its declared semantic set.
        let declared_arithmetic = catalog
            .catalog()
            .declarations
            .iter()
            .filter(|declaration| {
                arithmetic_rules
                    .iter()
                    .any(|(_, semantic)| *semantic == declaration.semantic)
            })
            .map(|declaration| (declaration.constraint, declaration.semantic))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            declared_arithmetic,
            arithmetic_rules.iter().copied().collect::<BTreeSet<_>>(),
            "{}",
            case.convention
        );

        // The catalog is bound to this exact validated constraint catalog,
        // target, and key selection — provenance, not a re-derived roster.
        assert_eq!(catalog.catalog().target, case.target);
        assert_eq!(
            catalog.catalog().register_constraints,
            environment.constraints().identity()
        );
        assert_eq!(catalog.catalog().selected_keys, keys);

        let flags = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rflags",
                Architecture::Aarch64 => "nzcv",
            })
            .expect("arithmetic matrix names a condition-codes view");
        let integer_class = model.view_named(case.arguments[0]).unwrap().class;
        let float_class = model.view_named(case.float_result).unwrap().class;
        // The ABI leaves the condition codes volatile on every pair: an
        // arithmetic rule may read, define, or clobber them but can never
        // touch preserved or fixed state.
        assert!(
            flags.units.iter().all(|unit| {
                convention.caller_saved.contains(unit)
                    && !convention.callee_saved.contains(unit)
                    && !convention.fixed.contains(unit)
            }),
            "{} must keep the condition codes volatile",
            case.convention
        );

        for (key, semantic) in &arithmetic_rules {
            let contract = arithmetic_contract(*semantic, case.target.architecture);
            let row = environment
                .constraint(*key)
                .unwrap_or_else(|| panic!("environment missing selected arithmetic row {key:?}"));
            let declaration = catalog
                .catalog()
                .declarations
                .iter()
                .find(|entry| entry.constraint == *key && entry.semantic == *semantic)
                .unwrap_or_else(|| panic!("effect catalog missing arithmetic declaration {key:?}"));
            assert_eq!(declaration.semantic, *semantic, "{key:?}");
            assert_eq!(declaration.barrier, MachineBarrier::None, "{key:?}");
            assert_eq!(declaration.call, MachineCallEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.cleanup, MachineCleanupEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.memory, MachineMemoryEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.trap, MachineTrapBehavior::NeverV1, "{key:?}");

            // ABI operand structure: dense operand numbers with the declared
            // access pattern, class membership, ABI pinning, and
            // early-clobber marking.
            assert_eq!(row.operands.len(), contract.accesses.len(), "{key:?}");
            for (position, operand) in row.operands.iter().enumerate() {
                assert_eq!(operand.operand as usize, position, "{key:?}");
                assert_eq!(operand.access, contract.accesses[position], "{key:?}");
                assert_eq!(
                    operand.class,
                    if contract.float_operands.contains(&(position as u16)) {
                        float_class
                    } else {
                        integer_class
                    },
                    "{key:?}"
                );
                assert_eq!(
                    operand.early_clobber,
                    contract.early_clobbers.contains(&(position as u16)),
                    "{key:?}"
                );
                assert!(operand.tied_to.is_none(), "{key:?}");
                let fixed = contract
                    .fixed_views
                    .iter()
                    .find(|(number, _)| *number as usize == position)
                    .map(|(_, name)| model.view_named(name).unwrap().id);
                assert_eq!(operand.fixed_view, fixed, "{key:?}");
                if let Some(view_id) = operand.fixed_view {
                    let view = model.views.iter().find(|view| view.id == view_id).unwrap();
                    assert!(
                        view.units
                            .iter()
                            .all(|unit| convention.caller_saved.contains(unit)),
                        "{key:?} operand {position} is pinned to non-volatile {}",
                        view.name
                    );
                }
            }

            // Implicit custody is exactly the declared condition-code state:
            // the flags view where the contract names it, plus any extra
            // named clobber the ISA's realization writes.
            assert_eq!(
                row.implicit_uses,
                if contract.flags == FlagsCustody::Use {
                    flags.units.clone()
                } else {
                    Vec::new()
                },
                "{key:?}"
            );
            assert_eq!(
                row.implicit_defs,
                if contract.flags == FlagsCustody::Def {
                    flags.units.clone()
                } else {
                    Vec::new()
                },
                "{key:?}"
            );
            let mut expected_clobbers = if contract.flags == FlagsCustody::Clobber {
                flags.units.clone()
            } else {
                Vec::new()
            };
            expected_clobbers.extend(units_for_names(model, contract.extra_clobbers));
            expected_clobbers.sort_unstable();
            expected_clobbers.dedup();
            assert_eq!(row.clobbers, expected_clobbers, "{key:?}");
            assert!(
                row.implicit_uses
                    .iter()
                    .chain(&row.implicit_defs)
                    .chain(&row.clobbers)
                    .all(|unit| {
                        convention.caller_saved.contains(unit)
                            && !convention.callee_saved.contains(unit)
                            && !convention.fixed.contains(unit)
                    }),
                "{key:?} implicit state must stay inside volatile units"
            );

            // Each canonical alternative replays the exact ABI arithmetic
            // mechanism for this target: its applicability window, size
            // knowledge, and external custody refine the shared row.
            assert_eq!(
                declaration.alternatives.len(),
                contract.alternatives.len(),
                "{key:?}"
            );
            for (index, (alternative, expected)) in declaration
                .alternatives
                .iter()
                .zip(&contract.alternatives)
                .enumerate()
            {
                assert_eq!(alternative.key.family, (*semantic).into(), "{key:?}");
                assert_eq!(alternative.key.variant, index as u32, "{key:?}");
                assert_eq!(alternative.applicability, expected.applicability, "{key:?}");
                assert_eq!(alternative.size, expected.size, "{key:?}");
                let encoded = &alternative.encoded;
                assert_eq!(encoded.external_operand_reads, expected.reads, "{key:?}");
                assert_eq!(encoded.external_operand_writes, expected.writes, "{key:?}");
                // Encoded implicit state refines exactly to the constraint
                // row's declared custody for every selected arithmetic rule.
                assert_eq!(encoded.implicit_unit_uses, row.implicit_uses, "{key:?}");
                assert_eq!(encoded.implicit_unit_defs, row.implicit_defs, "{key:?}");
                assert_eq!(encoded.implicit_unit_clobbers, row.clobbers, "{key:?}");
                assert_eq!(
                    encoded.memory,
                    MachineEncodedMemoryEffect::NoneV1,
                    "{key:?}"
                );
                assert_eq!(
                    encoded.stack,
                    MachineEncodedStackEffect::UnchangedV1,
                    "{key:?}"
                );
                assert_eq!(
                    encoded.trap,
                    if contract.faulting {
                        MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                    } else {
                        MachineEncodedTrapBehavior::NeverV1
                    },
                    "{key:?}"
                );
                assert_eq!(
                    encoded.control,
                    MachineEncodedControlEffect::FallThroughV1,
                    "{key:?}"
                );
            }
        }
    }
}

/// Every selected arithmetic rule replays its encoded fall-through form:
/// borrowing a foreign control shape, inventing a memory or stack footprint,
/// drifting the trap surface, or forging or losing custody must reject for
/// every alternative of every rule on every declared target/ABI pair.
#[test]
fn every_selected_arithmetic_rule_rejects_encoded_effect_forgery_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let catalog = produced_effects(case, environment.constraints());
        let stack_pointer = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rsp",
                Architecture::Aarch64 => "sp",
            })
            .unwrap();
        let flags = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rflags",
                Architecture::Aarch64 => "nzcv",
            })
            .unwrap();
        for (key, semantic) in selected_arithmetic_rules(&environment) {
            let contract = arithmetic_contract(semantic, case.target.architecture);
            let row = environment.constraint(key).unwrap();
            let alternatives = catalog
                .declarations
                .iter()
                .find(|declaration| {
                    declaration.constraint == key && declaration.semantic == semantic
                })
                .unwrap()
                .alternatives
                .len();
            for index in 0..alternatives {
                // A fall-through rule cannot borrow any control-transfer or
                // hosted shape: every foreign control encoding fails the
                // control/barrier join.
                for control in [
                    MachineEncodedControlEffect::ConditionalRelativeBranchV1,
                    MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
                    MachineEncodedControlEffect::DirectRelativeCallV1,
                    MachineEncodedControlEffect::ReturnFromActivationStackV1,
                    MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                        target: stack_pointer.id,
                    },
                    MachineEncodedControlEffect::HostedExitOrTrapV1,
                ] {
                    let mut corrupted = catalog.clone();
                    arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                        .encoded
                        .control = control;
                    assert_eq!(
                        validate_effects(case, environment.constraints(), corrupted),
                        Err(EffectRejection::Structural(
                            MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                        )),
                        "{key:?} foreign control shape {control:?} must reject"
                    );
                }

                // An arithmetic rule cannot grow a memory footprint or a
                // stack lifecycle.
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .encoded
                    .memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: 8,
                };
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} forged memory footprint must reject"
                );
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .encoded
                    .stack = MachineEncodedStackEffect::PopBytesV1 {
                    stack_pointer: stack_pointer.id,
                    byte_count: 8,
                };
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} forged stack lifecycle must reject"
                );

                // Trap drift stays inside the admitted fallthrough surface
                // for both admitted values, so only canonical ISA replay
                // rejects it; a hosted trap shape is outside that surface.
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .encoded
                    .trap = if contract.faulting {
                    MachineEncodedTrapBehavior::NeverV1
                } else {
                    MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                };
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::SemanticMismatch),
                    "{key:?} encoded trap drift must reject"
                );
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .encoded
                    .trap = MachineEncodedTrapBehavior::HostedWriteFailureV1;
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} hosted trap shape must reject"
                );

                // Encoded operand custody: losing a declared read or write
                // understates the contracted surface and fails structural
                // admission; canonical replay never sees the forgery.
                let expected = &contract.alternatives[index];
                if let Some(&dropped) = expected.reads.last() {
                    let mut corrupted = catalog.clone();
                    let reads = &mut arithmetic_declaration_mut(&mut corrupted, key, semantic)
                        .alternatives[index]
                        .encoded
                        .external_operand_reads;
                    reads.retain(|read| *read != dropped);
                    assert_eq!(
                        validate_effects(case, environment.constraints(), corrupted),
                        Err(EffectRejection::Structural(
                            MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                        )),
                        "{key:?} lost operand read must reject"
                    );
                }
                if let Some(&dropped) = expected.writes.last() {
                    let mut corrupted = catalog.clone();
                    let writes = &mut arithmetic_declaration_mut(&mut corrupted, key, semantic)
                        .alternatives[index]
                        .encoded
                        .external_operand_writes;
                    writes.retain(|write| *write != dropped);
                    assert_eq!(
                        validate_effects(case, environment.constraints(), corrupted),
                        Err(EffectRejection::Structural(
                            MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                        )),
                        "{key:?} lost operand write must reject"
                    );
                }
                // A read the row does offer but the form does not take — the
                // zeroing subtraction form — is just as contracted out: the
                // all-aliased surface owns its empty read set.
                let unread = row.operands.iter().find(|operand| {
                    matches!(
                        operand.access,
                        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                    ) && !expected.reads.contains(&operand.operand)
                });
                if let Some(unread) = unread {
                    let mut corrupted = catalog.clone();
                    let reads = &mut arithmetic_declaration_mut(&mut corrupted, key, semantic)
                        .alternatives[index]
                        .encoded
                        .external_operand_reads;
                    reads.push(unread.operand);
                    reads.sort_unstable();
                    assert_eq!(
                        validate_effects(case, environment.constraints(), corrupted),
                        Err(EffectRejection::Structural(
                            MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                        )),
                        "{key:?} invented operand read must reject"
                    );
                }
                let mut corrupted = catalog.clone();
                let reads = &mut arithmetic_declaration_mut(&mut corrupted, key, semantic)
                    .alternatives[index]
                    .encoded
                    .external_operand_reads;
                reads.push(row.operands.len() as u16);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} out-of-row operand read must reject"
                );
                if let Some(write_operand) = row
                    .operands
                    .iter()
                    .find(|operand| {
                        matches!(
                            operand.access,
                            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                        )
                    })
                    .map(|operand| operand.operand)
                {
                    let mut corrupted = catalog.clone();
                    let reads = &mut arithmetic_declaration_mut(&mut corrupted, key, semantic)
                        .alternatives[index]
                        .encoded
                        .external_operand_reads;
                    reads.push(write_operand);
                    reads.sort_unstable();
                    assert_eq!(
                        validate_effects(case, environment.constraints(), corrupted),
                        Err(EffectRejection::Structural(
                            MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                        )),
                        "{key:?} reading a defined operand must reject"
                    );
                }
                let mut corrupted = catalog.clone();
                let forged = row
                    .operands
                    .iter()
                    .find(|operand| operand.access == RegisterOperandAccess::Use)
                    .map(|operand| operand.operand)
                    .unwrap_or_else(|| {
                        // A read-free row's only operand is already written;
                        // writing it twice breaks canonical ordering instead.
                        expected.writes.first().copied().unwrap_or(0)
                    });
                let writes = &mut arithmetic_declaration_mut(&mut corrupted, key, semantic)
                    .alternatives[index]
                    .encoded
                    .external_operand_writes;
                writes.push(forged);
                writes.sort_unstable();
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} forged operand write must reject"
                );

                // Encoded implicit custody: a unit the row does not declare
                // fails structural admission, and so does losing a declared
                // one — the encoded lists restate the row exactly.
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .encoded
                    .implicit_unit_uses
                    .push(stack_pointer.units[0]);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} forged implicit use must reject"
                );
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .encoded
                    .implicit_unit_defs
                    .push(stack_pointer.units[0]);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} forged implicit definition must reject"
                );
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .encoded
                    .implicit_unit_clobbers
                    .push(stack_pointer.units[0]);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} forged implicit clobber must reject"
                );
                if contract.flags == FlagsCustody::Use {
                    let mut corrupted = catalog.clone();
                    arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                        .encoded
                        .implicit_unit_uses
                        .retain(|unit| !flags.units.contains(unit));
                    assert_eq!(
                        validate_effects(case, environment.constraints(), corrupted),
                        Err(EffectRejection::Structural(
                            MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                        )),
                        "{key:?} lost condition-code use must reject"
                    );
                }
                if contract.flags == FlagsCustody::Def {
                    let mut corrupted = catalog.clone();
                    arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                        .encoded
                        .implicit_unit_defs
                        .retain(|unit| !flags.units.contains(unit));
                    assert_eq!(
                        validate_effects(case, environment.constraints(), corrupted),
                        Err(EffectRejection::Structural(
                            MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                        )),
                        "{key:?} lost condition-code definition must reject"
                    );
                }
                if contract.flags == FlagsCustody::Clobber || !contract.extra_clobbers.is_empty() {
                    let mut corrupted = catalog.clone();
                    arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                        .encoded
                        .implicit_unit_clobbers
                        .remove(0);
                    assert_eq!(
                        validate_effects(case, environment.constraints(), corrupted),
                        Err(EffectRejection::Structural(
                            MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                        )),
                        "{key:?} lost encoded clobber must reject"
                    );
                }

                // The alternative stays bound to its own family and admitted
                // window: a borrowed family fails admission, a self-aliasing
                // window is invalid for every operand shape, and a window the
                // row does admit but the form does not declare needs
                // canonical replay.
                let mut corrupted = catalog.clone();
                let declaration = arithmetic_declaration_mut(&mut corrupted, key, semantic);
                declaration.alternatives[index].key.family =
                    if semantic == MachineSemanticKind::CopyI64 {
                        MachineSemanticKind::ExactAddI64.into()
                    } else {
                        MachineSemanticKind::CopyI64.into()
                    };
                // A borrowed family that leaves the roster ordered reaches
                // the family check; one that reorders it — borrowing a
                // lower family onto a later alternative — fails canonical
                // ordering first.
                let expected_rejection = if declaration
                    .alternatives
                    .windows(2)
                    .all(|pair| pair[0].key < pair[1].key)
                {
                    MachineEffectCatalogValidationError::AlternativeFamilyMismatch(semantic)
                } else {
                    MachineEffectCatalogValidationError::NonCanonicalAlternatives(semantic)
                };
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(expected_rejection)),
                    "{key:?} borrowed alternative family must reject"
                );
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .applicability = MachineAlternativeApplicability::ResultAliasesOperand {
                    result: 0,
                    operand: 0,
                };
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidAlternativeApplicability(
                            semantic
                        )
                    )),
                    "{key:?} self-aliasing applicability must reject"
                );
                let admitted = row.operands.iter().find_map(|result| {
                    if !matches!(
                        result.access,
                        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                    ) {
                        return None;
                    }
                    row.operands
                        .iter()
                        .find(|input| {
                            input.operand != result.operand
                                && matches!(
                                    input.access,
                                    RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                                )
                                && input.class == result.class
                        })
                        .map(|input| (result.operand, input.operand))
                });
                if let Some((result, operand)) = admitted {
                    let mut corrupted = catalog.clone();
                    arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                        .applicability =
                        MachineAlternativeApplicability::ResultAliasesOperand { result, operand };
                    // The drifted window contracts every input back into the
                    // read surface: a form whose honest reads are already the
                    // full contract still needs canonical replay, while the
                    // all-aliased subtract's empty read set now understates
                    // its own applicability and fails structural admission.
                    let contracted_reads = row
                        .operands
                        .iter()
                        .filter(|operand| {
                            matches!(
                                operand.access,
                                RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                            )
                        })
                        .map(|operand| operand.operand)
                        .collect::<Vec<u16>>();
                    let expected_rejection = if expected.reads == contracted_reads.as_slice() {
                        EffectRejection::SemanticMismatch
                    } else {
                        EffectRejection::Structural(
                            MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic),
                        )
                    };
                    assert_eq!(
                        validate_effects(case, environment.constraints(), corrupted),
                        Err(expected_rejection),
                        "{key:?} admitted alias-window drift must reject"
                    );
                }

                // Size knowledge: a degenerate bound fails admission, while a
                // plausible drift needs canonical replay.
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .size = MachineSizeKnowledge::ExactBytes(0);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidSizeKnowledge(semantic)
                    )),
                    "{key:?} empty size bound must reject"
                );
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .size = MachineSizeKnowledge::ExactBytes(997);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::SemanticMismatch),
                    "{key:?} size drift must reject"
                );

                // A single-alternative declaration keeps a canonical roster
                // under a variant drift; a multi-alternative roster breaks
                // ordering first.
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .key
                    .variant = (index + 1) as u32;
                let expected_rejection = if index + 1 < alternatives {
                    // Bumping a variant into its successor duplicates the key
                    // and breaks canonical ordering before replay.
                    EffectRejection::Structural(
                        MachineEffectCatalogValidationError::NonCanonicalAlternatives(semantic),
                    )
                } else {
                    // The last variant stays ordered; only canonical replay
                    // sees the drift.
                    EffectRejection::SemanticMismatch
                };
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(expected_rejection),
                    "{key:?} variant drift must reject"
                );
            }

            // The alternative roster itself: an empty roster fails admission,
            // and a duplicated alternative breaks canonical ordering.
            let mut corrupted = catalog.clone();
            arithmetic_declaration_mut(&mut corrupted, key, semantic)
                .alternatives
                .clear();
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::EmptyAlternatives(semantic)
                )),
                "{key:?} empty alternatives must reject"
            );
            let mut corrupted = catalog.clone();
            let declaration = arithmetic_declaration_mut(&mut corrupted, key, semantic);
            let first = declaration.alternatives[0].clone();
            declaration.alternatives.push(first);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::NonCanonicalAlternatives(semantic)
                )),
                "{key:?} duplicated alternative must reject"
            );
        }
    }
}

/// The remaining arithmetic-contract corruptions are checked once per
/// selected arithmetic declaration and once per selected arithmetic key on
/// every declared target/ABI pair: each declaration proves the effect join
/// sees its barrier, call, memory, trap, and key binding, each constraint row
/// proves the environment join sees its exact ABI custody, and a borrowed
/// sibling row still fails canonical re-derivation.
#[test]
fn every_arithmetic_family_rejects_arithmetic_contract_corruption_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let convention = convention_for(case, environment.physical());
        let keys = environment.selected_keys();
        let catalog = produced_effects(case, environment.constraints());
        let raw = target_physical_register_model(case.target);
        let physical = validate_physical_register_model(raw.clone()).unwrap();
        let constraints = target_constraint_catalog(case.target, &physical);
        let integer_class = model.view_named(case.arguments[0]).unwrap().class;
        let float_class = model.view_named(case.float_result).unwrap().class;
        let abi_views = [
            model.view_named(case.arguments[0]).unwrap().id,
            model.view_named(case.arguments[1]).unwrap().id,
        ];
        let float_view = model.view_named(case.float_result).unwrap().id;
        let stack_pointer_units = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rsp",
                Architecture::Aarch64 => "sp",
            })
            .unwrap()
            .units
            .clone();
        let caller_unit = convention.caller_saved[0];

        let arithmetic_rules = selected_arithmetic_rules(&environment);
        for (key, semantic) in &arithmetic_rules {
            // An arithmetic declaration cannot borrow a foreign barrier
            // class, whichever sibling class it takes.
            for barrier in [
                MachineBarrier::ControlFlow,
                MachineBarrier::Call,
                MachineBarrier::ExternalEffect,
            ] {
                let mut corrupted = catalog.clone();
                arithmetic_declaration_mut(&mut corrupted, *key, *semantic).barrier = barrier;
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::BarrierMismatch(*semantic)
                    )),
                    "{key:?} borrowed {barrier:?} barrier must reject"
                );
            }

            // An arithmetic rule cannot claim a call effect.
            let mut corrupted = catalog.clone();
            arithmetic_declaration_mut(&mut corrupted, *key, *semantic).call =
                MachineCallEffect::DirectInternalNormalReturnV1 {
                    pre_call_stack_alignment: convention.stack_alignment,
                };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(*semantic)
                )),
                "{key:?} forged call effect must reject"
            );

            // Declaration trap drift: a hosted trap result names its owning
            // hosted operation and cannot be borrowed, and claiming the
            // bare architectural fault overstates the declared surface a
            // never-faulting rule owns.
            let mut corrupted = catalog.clone();
            arithmetic_declaration_mut(&mut corrupted, *key, *semantic).trap =
                MachineTrapBehavior::HostedReadFailureV1;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(*semantic)
                )),
                "{key:?} borrowed hosted trap must reject"
            );
            let mut corrupted = catalog.clone();
            arithmetic_declaration_mut(&mut corrupted, *key, *semantic).trap =
                MachineTrapBehavior::MayArchitecturalFaultV1;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(*semantic)
                )),
                "{key:?} claimed architectural fault must reject"
            );

            // Declaration memory drift: the declared NeverV1 trap cannot
            // coexist with a memory footprint.
            let mut corrupted = catalog.clone();
            arithmetic_declaration_mut(&mut corrupted, *key, *semantic).memory =
                MachineMemoryEffect::ReadPointerV1;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(*semantic)
                )),
                "{key:?} forged memory footprint must reject"
            );

            // The declaration stays bound to its selected key: pointing it at
            // a sibling arithmetic key breaks the canonical roster, and so
            // does dropping the declaration outright.
            let mut corrupted = catalog.clone();
            arithmetic_declaration_mut(&mut corrupted, *key, *semantic).constraint =
                if *key == keys.copy_i64 {
                    keys.add_i64
                } else {
                    keys.copy_i64
                };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::DeclarationRosterMismatch
                )),
                "{key:?} borrowed sibling key must reject"
            );
            let mut corrupted = catalog.clone();
            corrupted.declarations.retain(|declaration| {
                !(declaration.constraint == *key && declaration.semantic == *semantic)
            });
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::DeclarationRosterMismatch
                )),
                "{key:?} dropped declaration must reject"
            );
        }

        // The constraint rows are exercised once per selected arithmetic key;
        // the shared-key declarations ride on one row each.
        let mut seen = BTreeSet::new();
        for (key, _) in &arithmetic_rules {
            if !seen.insert(*key) {
                continue;
            }
            let key = *key;
            let row = environment.constraint(key).unwrap().clone();

            // Flipping the leading operand's access direction is invisible to
            // structure but not to the owning ISA's canonical row.
            let mut corrupted = constraints.clone();
            let operand = &mut row_mut(&mut corrupted, key).operands[0];
            operand.access = match operand.access {
                RegisterOperandAccess::Use => RegisterOperandAccess::Def,
                _ => RegisterOperandAccess::Use,
            };
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("a flipped arithmetic operand access must reject");
            assert_target_semantic_error(case.target, key, error);

            // Every allocatable operand pinned to a same-class ABI view still
            // fails canonical re-derivation.
            let mut corrupted = constraints.clone();
            for (position, operand) in row.operands.iter().enumerate() {
                let candidate = if operand.class == float_class {
                    float_view
                } else {
                    *abi_views
                        .iter()
                        .find(|view| Some(**view) != operand.fixed_view)
                        .unwrap_or(&abi_views[0])
                };
                row_mut(&mut corrupted, key).operands[position].fixed_view = Some(candidate);
            }
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("an ABI-pinned arithmetic operand must reject");
            assert_target_semantic_error(case.target, key, error);

            // An operand moved to the other operand class is still a known,
            // allocatable class — the canonical row rejects it. A row whose
            // operands are all fixed instead takes a foreign-class fixed view:
            // the substitution stays structural and still rejects.
            let position = row
                .operands
                .iter()
                .position(|operand| operand.fixed_view.is_none());
            let mut corrupted = constraints.clone();
            match position {
                Some(position) => {
                    let operand = &mut row_mut(&mut corrupted, key).operands[position];
                    operand.class = if operand.class == integer_class {
                        float_class
                    } else {
                        integer_class
                    };
                }
                None => {
                    let operand = &mut row_mut(&mut corrupted, key).operands[0];
                    operand.class = float_class;
                    operand.fixed_view = Some(float_view);
                }
            }
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("a foreign-class arithmetic operand must reject");
            assert_target_semantic_error(case.target, key, error);

            // A canonical one-way tie the row does not declare: a defining
            // operand tied to an earlier same-class input.
            let tie = row.operands.iter().enumerate().find_map(|(index, result)| {
                if !matches!(
                    result.access,
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                ) || index == 0
                {
                    return None;
                }
                row.operands[..index]
                    .iter()
                    .find(|input| {
                        input.tied_to.is_none()
                            && matches!(
                                input.access,
                                RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                            )
                            && input.class == result.class
                            && (input.fixed_view.is_none()
                                || result.fixed_view.is_none()
                                || input.fixed_view == result.fixed_view)
                    })
                    .map(|input| (index, input.operand))
            });
            if let Some((index, tied_to)) = tie {
                let mut corrupted = constraints.clone();
                row_mut(&mut corrupted, key).operands[index].tied_to = Some(tied_to);
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("an invented arithmetic operand tie must reject");
                assert_target_semantic_error(case.target, key, error);
            }

            // Early-clobber scratch custody is part of the ABI row: losing a
            // declared early clobber, or inventing one on a defining operand,
            // must reject.
            if let Some(position) = row
                .operands
                .iter()
                .position(|operand| operand.early_clobber)
            {
                let mut corrupted = constraints.clone();
                row_mut(&mut corrupted, key).operands[position].early_clobber = false;
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("an arithmetic row must retain early-clobber scratch");
                assert_target_semantic_error(case.target, key, error);
            } else if let Some(position) = row.operands.iter().position(|operand| {
                matches!(
                    operand.access,
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                )
            }) {
                let mut corrupted = constraints.clone();
                row_mut(&mut corrupted, key).operands[position].early_clobber = true;
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("an invented arithmetic early clobber must reject");
                assert_target_semantic_error(case.target, key, error);
            }

            // Implicit custody: declared condition-code uses and definitions
            // cannot be dropped, and stack-pointer custody cannot be invented
            // on a row that declares none.
            if row.implicit_uses.is_empty() {
                let mut corrupted = constraints.clone();
                let uses = &mut row_mut(&mut corrupted, key).implicit_uses;
                uses.extend(stack_pointer_units.iter().copied());
                uses.sort_unstable();
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("a plain arithmetic row cannot gain stack-pointer custody");
                assert_target_semantic_error(case.target, key, error);
            } else {
                let mut corrupted = constraints.clone();
                row_mut(&mut corrupted, key).implicit_uses.remove(0);
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("a Boolean row must retain its condition-code use");
                assert_target_semantic_error(case.target, key, error);
            }
            if row.implicit_defs.is_empty() {
                let mut corrupted = constraints.clone();
                let defs = &mut row_mut(&mut corrupted, key).implicit_defs;
                defs.extend(stack_pointer_units.iter().copied());
                defs.sort_unstable();
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("a plain arithmetic row cannot gain an implicit definition");
                assert_target_semantic_error(case.target, key, error);
            } else {
                let mut corrupted = constraints.clone();
                row_mut(&mut corrupted, key).implicit_defs.remove(0);
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("a flag-defining arithmetic row must retain its definition");
                assert_target_semantic_error(case.target, key, error);
            }

            // A pure row cannot silently claim a caller-saved clobber, and a
            // flag-writing row cannot drop one.
            if row.clobbers.is_empty() {
                let mut corrupted = constraints.clone();
                let clobbers = &mut row_mut(&mut corrupted, key).clobbers;
                clobbers.push(caller_unit);
                clobbers.sort_unstable();
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("an invented arithmetic-row clobber must reject");
                assert_target_semantic_error(case.target, key, error);
            } else {
                let mut corrupted = constraints.clone();
                row_mut(&mut corrupted, key).clobbers.remove(0);
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("an arithmetic row must retain its declared clobbers");
                assert_target_semantic_error(case.target, key, error);
            }
        }

        // The produced catalog also proves it is bound to this environment's
        // key selection: duplicating a selected key or reordering the
        // declarations fails admission before any row is replayed.
        let mut corrupted = catalog.clone();
        corrupted.selected_keys.materialize_i64 = corrupted.selected_keys.copy_i64;
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::DuplicateSelectedConstraintKey
            )),
            "duplicated selected key must reject"
        );
        let mut corrupted = catalog.clone();
        corrupted.declarations.swap(0, 1);
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::NonCanonicalDeclarations
            )),
            "reordered declarations must reject"
        );

        // Sibling-row borrowing: copying a sibling arithmetic row under this
        // key keeps a structurally valid catalog that only canonical
        // re-derivation can reject. Each ISA names pairs whose custody
        // actually differs.
        let sibling_pairs: &[(RegisterConstraintKey, RegisterConstraintKey)] =
            match case.target.architecture {
                Architecture::X86_64 => &[
                    (keys.subtract_i64, keys.add_i64),
                    (keys.materialize_boolean, keys.materialize_i64),
                    (keys.divide_u64, keys.remainder_i64),
                    (keys.compare_i64, keys.compare_i64_zero),
                    (keys.bits_to_float32.unwrap(), keys.float32_to_bits.unwrap()),
                ],
                Architecture::Aarch64 => &[
                    (keys.compare_i64, keys.add_i64),
                    (keys.saturating_subtract_unsigned, keys.subtract_i64),
                    (keys.materialize_boolean, keys.materialize_i64),
                    (keys.remainder_i64, keys.divide_u64),
                    (keys.bits_to_float32.unwrap(), keys.float32_to_bits.unwrap()),
                ],
            };
        for (donor_key, borrowed_key) in sibling_pairs {
            let donor = environment.constraint(*donor_key).unwrap().clone();
            let mut corrupted = constraints.clone();
            let row = row_mut(&mut corrupted, *borrowed_key);
            row.operands = donor.operands;
            row.implicit_uses = donor.implicit_uses;
            row.implicit_defs = donor.implicit_defs;
            row.clobbers = donor.clobbers;
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("borrowing a sibling arithmetic row must reject");
            assert_target_semantic_error(case.target, *borrowed_key, error);
        }
    }
}
