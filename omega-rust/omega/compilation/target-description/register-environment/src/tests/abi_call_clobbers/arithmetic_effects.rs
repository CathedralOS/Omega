/*
We verify that the machine-effect catalog declares the exact operand access,
condition-code custody, clobber sets, trap behavior, and applicability windows
for every selected arithmetic, bitwise, comparison, and boolean-materialization
rule across all supported targets, and rejects per-fact corruptions.
*/

use super::{
    Architecture, BTreeSet, EffectRejection, RegisterConstraintKey, abi_case_context,
    assert_storage_free, baseline_target_register_environment, binds_catalog_provenance,
    declaration_mut, declared_pairs, encoded_mut, flags_view, produced_effects,
    row_and_declaration, scalar_abi_cases, stack_pointer_view, units_for_names, validate_effects,
};
use register_model::RegisterOperandAccess;
use selected_instructions::SaturatingCarrier;
use selected_instructions::{
    MachineAlternativeApplicability, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalogValidationError, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};

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
        (
            Some(keys.saturating_multiply_clamped),
            &[
                SaturatingMultiply(SaturatingCarrier::I8),
                SaturatingMultiply(SaturatingCarrier::I16),
                SaturatingMultiply(SaturatingCarrier::I32),
                SaturatingMultiply(SaturatingCarrier::I64),
                SaturatingMultiply(SaturatingCarrier::U8),
                SaturatingMultiply(SaturatingCarrier::U16),
                SaturatingMultiply(SaturatingCarrier::U32),
            ][..],
        ),
        (
            Some(keys.saturating_multiply_u64),
            &[SaturatingMultiply(SaturatingCarrier::U64)][..],
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

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum FlagsCustody {
    #[default]
    None,
    Use,
    Def,
    Clobber,
}

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
    applicability: ExpectedApplicability,
    size: MachineSizeKnowledge,
    reads: &'static [u16],
    writes: &'static [u16],
}

enum ExpectedApplicability {
    Window(MachineAlternativeApplicability),
    OptimizationClobberingFlags(&'static str),
}

const RESULT_ALIASES_BOTH: MachineAlternativeApplicability =
    MachineAlternativeApplicability::ResultAliasesOperands {
        result: 2,
        left: 0,
        right: 1,
    };
const RESULT_ALIASES_LEFT: MachineAlternativeApplicability =
    MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
        result: 2,
        aliased_operand: 0,
        distinct_operand: 1,
    };
const RESULT_ALIASES_RIGHT: MachineAlternativeApplicability =
    MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
        result: 2,
        aliased_operand: 1,
        distinct_operand: 0,
    };
const RESULT_ALIASES_NEITHER: MachineAlternativeApplicability =
    MachineAlternativeApplicability::ResultDistinctFromOperands {
        result: 2,
        left: 0,
        right: 1,
    };

fn result_alternative(
    applicability: MachineAlternativeApplicability,
    size: u16,
    reads: &'static [u16],
) -> AlternativeContract {
    AlternativeContract {
        applicability: ExpectedApplicability::Window(applicability),
        size: MachineSizeKnowledge::ExactBytes(size),
        reads,
        writes: &[2],
    }
}

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
            SaturatingRemainder(carrier) if carrier.is_signed() => {
                MachineSizeKnowledge::ExactBytes(17)
            }
            SaturatingRemainder(_) => MachineSizeKnowledge::ExactBytes(9),
            SaturatingMultiply(SaturatingCarrier::U64) => MachineSizeKnowledge::ExactBytes(9),
            SaturatingMultiply(SaturatingCarrier::I64) => MachineSizeKnowledge::ExactBytes(29),
            SaturatingMultiply(carrier) if carrier.is_signed() => {
                MachineSizeKnowledge::ExactBytes(41)
            }
            SaturatingMultiply(_) => MachineSizeKnowledge::ExactBytes(24),
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
            SaturatingRemainder(_) => MachineSizeKnowledge::ExactBytes(8),
            SaturatingMultiply(SaturatingCarrier::U64) => MachineSizeKnowledge::ExactBytes(16),
            SaturatingMultiply(SaturatingCarrier::I64) => MachineSizeKnowledge::ExactBytes(24),
            SaturatingMultiply(carrier) if carrier.is_signed() => {
                MachineSizeKnowledge::ExactBytes(28)
            }
            SaturatingMultiply(_) => MachineSizeKnowledge::ExactBytes(16),
            _ => MachineSizeKnowledge::ExactBytes(4),
        },
    }
}

const fn flag_custody(x86: bool, aarch64: FlagsCustody) -> FlagsCustody {
    if x86 { FlagsCustody::Clobber } else { aarch64 }
}

fn either<T>(x86: bool, x86_value: T, aarch64_value: T) -> T {
    if x86 { x86_value } else { aarch64_value }
}

fn arithmetic_contract(
    semantic: MachineSemanticKind,
    architecture: Architecture,
) -> ArithmeticContract {
    use MachineSemanticKind::*;
    use RegisterOperandAccess::{Def, Use};
    let x86 = architecture == Architecture::X86_64;
    let one = |reads: &'static [u16], writes: &'static [u16]| {
        vec![AlternativeContract {
            applicability: ExpectedApplicability::Window(MachineAlternativeApplicability::Always),
            size: expected_size(semantic, architecture),
            reads,
            writes,
        }]
    };
    match semantic {
        MaterializeI64 => ArithmeticContract::plain(&[Def], {
            let mut alternatives = one(&[], &[0]);
            if x86 {
                alternatives.push(AlternativeContract {
                    applicability: ExpectedApplicability::OptimizationClobberingFlags(
                        "X86SelectXorZeroI64MaterializationV1",
                    ),
                    size: MachineSizeKnowledge::ExactBytes(3),
                    reads: &[],
                    writes: &[0],
                });
            }
            alternatives
        }),
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
            flags: flag_custody(x86, FlagsCustody::None),
            ..ArithmeticContract::plain(&[Use, Use, Def], one(&[0, 1], &[2]))
        },
        ExactSubtractI64 => ArithmeticContract {
            flags: flag_custody(x86, FlagsCustody::None),
            alternatives: if x86 {
                vec![
                    result_alternative(RESULT_ALIASES_BOTH, 3, &[]),
                    result_alternative(RESULT_ALIASES_LEFT, 3, &[0, 1]),
                    result_alternative(RESULT_ALIASES_RIGHT, 6, &[0, 1]),
                    result_alternative(RESULT_ALIASES_NEITHER, 6, &[0, 1]),
                ]
            } else {
                one(&[0, 1], &[2])
            },
            ..ArithmeticContract::plain(&[Use, Use, Def], Vec::new())
        },
        ExactMultiplyI64 => ArithmeticContract {
            flags: flag_custody(x86, FlagsCustody::None),
            alternatives: if x86 {
                vec![
                    result_alternative(RESULT_ALIASES_BOTH, 4, &[0, 1]),
                    result_alternative(RESULT_ALIASES_LEFT, 4, &[0, 1]),
                    result_alternative(RESULT_ALIASES_RIGHT, 4, &[0, 1]),
                    result_alternative(RESULT_ALIASES_NEITHER, 7, &[0, 1]),
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
            flags: flag_custody(x86, FlagsCustody::Def),
            ..ArithmeticContract::plain(&[Use, Use, Def], one(&[0, 1], &[2]))
        },
        ExactDivideU64
        | SaturatingDivide(
            SaturatingCarrier::U8
            | SaturatingCarrier::U16
            | SaturatingCarrier::U32
            | SaturatingCarrier::U64,
        ) => ArithmeticContract {
            fixed_views: either(x86, &[(0, "rax"), (2, "rax"), (3, "rdx")], &[]),
            flags: flag_custody(x86, FlagsCustody::None),
            extra_clobbers: if x86 { &["rdx"] } else { &[] },
            faulting: x86,
            alternatives: either(x86, one(&[0, 1, 3], &[2]), one(&[0, 1], &[2])),
            ..ArithmeticContract::plain(
                either(x86, &[Use, Use, Def, Use], &[Use, Use, Def]),
                Vec::new(),
            )
        },
        WrappingRemainderI64 => ArithmeticContract {
            early_clobbers: if x86 { &[] } else { &[2] },
            fixed_views: either(x86, &[(0, "rax"), (1, "rcx"), (2, "rax"), (3, "rdx")], &[]),
            flags: flag_custody(x86, FlagsCustody::None),
            faulting: x86,
            alternatives: either(x86, one(&[0, 1], &[2, 3]), one(&[0, 1], &[2])),
            ..ArithmeticContract::plain(
                either(x86, &[Use, Use, Def, Def], &[Use, Use, Def]),
                Vec::new(),
            )
        },
        SaturatingRemainder(_) => ArithmeticContract {
            early_clobbers: if x86 { &[] } else { &[2] },
            fixed_views: either(x86, &[(0, "rax"), (1, "rcx"), (2, "rax"), (3, "rdx")], &[]),
            flags: flag_custody(x86, FlagsCustody::None),
            faulting: x86,
            alternatives: either(x86, one(&[0, 1], &[2, 3]), one(&[0, 1], &[2])),
            ..ArithmeticContract::plain(
                either(x86, &[Use, Use, Def, Def], &[Use, Use, Def]),
                Vec::new(),
            )
        },
        SaturatingAdd(_) | SaturatingSubtract(_) => ArithmeticContract {
            early_clobbers: &[2, 3],
            flags: flag_custody(x86, FlagsCustody::Def),
            ..ArithmeticContract::plain(&[Use, Use, Def, Def], one(&[0, 1], &[2, 3]))
        },
        SaturatingDivide(_) => ArithmeticContract {
            early_clobbers: if x86 { &[] } else { &[2, 3] },
            fixed_views: either(x86, &[(0, "rax"), (2, "rax"), (3, "rdx")], &[]),
            flags: flag_custody(x86, FlagsCustody::Def),
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
        SaturatingMultiply(SaturatingCarrier::U64) if x86 => ArithmeticContract {
            fixed_views: &[(0, "rax"), (1, "rcx"), (2, "rax"), (3, "rdx")],
            flags: FlagsCustody::Clobber,
            ..ArithmeticContract::plain(&[Use, Use, Def, Def], one(&[0, 1], &[2, 3]))
        },
        SaturatingMultiply(_) => ArithmeticContract {
            early_clobbers: &[2, 3],
            flags: flag_custody(x86, FlagsCustody::Def),
            ..ArithmeticContract::plain(&[Use, Use, Def, Def], one(&[0, 1], &[2, 3]))
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

#[test]
fn every_selected_arithmetic_rule_binds_the_declared_abi_arithmetic_contract() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let (model, convention, catalog, keys) = abi_case_context(&environment, case);
        let arithmetic_rules = selected_arithmetic_rules(&environment);
        assert_eq!(
            arithmetic_rules.len(),
            71,
            "{} selects an unexpected arithmetic roster",
            case.convention
        );

        let declared = declared_pairs(&catalog, |declaration| {
            arithmetic_rules
                .iter()
                .any(|(_, semantic)| *semantic == declaration.semantic)
        });
        assert_eq!(
            declared,
            arithmetic_rules.iter().copied().collect::<BTreeSet<_>>(),
            "{}",
            case.convention
        );

        binds_catalog_provenance(&catalog, case, &environment, keys);

        let flags = flags_view(model, case);
        let integer_class = model.view_named(case.arguments[0]).unwrap().class;
        let float_class = model.view_named(case.float_result).unwrap().class;
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
            let (row, declaration) = row_and_declaration(&environment, &catalog, *key, *semantic);
            assert_eq!(declaration.semantic, *semantic, "{key:?}");
            assert_eq!(declaration.barrier, MachineBarrier::None, "{key:?}");
            assert_eq!(declaration.call, MachineCallEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.cleanup, MachineCleanupEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.memory, MachineMemoryEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.trap, MachineTrapBehavior::NeverV1, "{key:?}");

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
                    let view = model.view(view_id).unwrap();
                    assert!(
                        view.units
                            .iter()
                            .all(|unit| convention.caller_saved.contains(unit)),
                        "{key:?} operand {position} is pinned to non-volatile {}",
                        view.name
                    );
                }
            }

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
                let mut expected_clobbers = row.clobbers.clone();
                match expected.applicability {
                    ExpectedApplicability::Window(window) => {
                        assert_eq!(alternative.applicability, window, "{key:?}");
                    }
                    ExpectedApplicability::OptimizationClobberingFlags(name) => {
                        let MachineAlternativeApplicability::OptimizationSelected { optimization } =
                            alternative.applicability
                        else {
                            panic!("{key:?} alternative {index} must be optimization-selected");
                        };
                        assert_eq!(optimization.build_case_name(), name, "{key:?}");
                        expected_clobbers.extend(&flags.units);
                        expected_clobbers.sort_unstable();
                        expected_clobbers.dedup();
                    }
                }
                assert_eq!(alternative.size, expected.size, "{key:?}");
                let encoded = &alternative.encoded;
                assert_eq!(encoded.external_operand_reads, expected.reads, "{key:?}");
                assert_eq!(encoded.external_operand_writes, expected.writes, "{key:?}");
                assert_eq!(encoded.implicit_unit_uses, row.implicit_uses, "{key:?}");
                assert_eq!(encoded.implicit_unit_defs, row.implicit_defs, "{key:?}");
                assert_eq!(encoded.implicit_unit_clobbers, expected_clobbers, "{key:?}");
                assert_storage_free(encoded, *key);
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

#[test]
fn every_selected_arithmetic_rule_rejects_encoded_effect_forgery_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let catalog = produced_effects(case, environment.constraints());
        let stack_pointer = stack_pointer_view(model, case);
        let flags = flags_view(model, case);
        for (key, semantic) in selected_arithmetic_rules(&environment) {
            let contract = arithmetic_contract(semantic, case.target.architecture);
            macro_rules! reject_encoded {
                ($corrupted:expr, $message:literal $(, $arg:expr)*) => {
                    assert_eq!(
                        validate_effects(case, environment.constraints(), $corrupted),
                        Err(EffectRejection::Structural(
                            MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                        )),
                        $message $(, $arg)*
                    );
                };
            }

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
                    encoded_mut(&mut corrupted, key, semantic, index).control = control;
                    reject_encoded!(
                        corrupted,
                        "{key:?} foreign control shape {control:?} must reject"
                    );
                }

                let mut corrupted = catalog.clone();
                encoded_mut(&mut corrupted, key, semantic, index).memory =
                    MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: 8,
                    };
                reject_encoded!(corrupted, "{key:?} forged memory footprint must reject");
                let mut corrupted = catalog.clone();
                encoded_mut(&mut corrupted, key, semantic, index).stack =
                    MachineEncodedStackEffect::PopBytesV1 {
                        stack_pointer: stack_pointer.id,
                        byte_count: 8,
                    };
                reject_encoded!(corrupted, "{key:?} forged stack lifecycle must reject");

                let mut corrupted = catalog.clone();
                encoded_mut(&mut corrupted, key, semantic, index).trap = if contract.faulting {
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
                encoded_mut(&mut corrupted, key, semantic, index).trap =
                    MachineEncodedTrapBehavior::HostedWriteFailureV1;
                reject_encoded!(corrupted, "{key:?} hosted trap shape must reject");

                let expected = &contract.alternatives[index];
                if let Some(&dropped) = expected.reads.last() {
                    let mut corrupted = catalog.clone();
                    let reads = &mut declaration_mut(&mut corrupted, key, semantic).alternatives
                        [index]
                        .encoded
                        .external_operand_reads;
                    reads.retain(|read| *read != dropped);
                    reject_encoded!(corrupted, "{key:?} lost operand read must reject");
                }
                if let Some(&dropped) = expected.writes.last() {
                    let mut corrupted = catalog.clone();
                    let writes = &mut declaration_mut(&mut corrupted, key, semantic).alternatives
                        [index]
                        .encoded
                        .external_operand_writes;
                    writes.retain(|write| *write != dropped);
                    reject_encoded!(corrupted, "{key:?} lost operand write must reject");
                }
                let unread = row.operands.iter().find(|operand| {
                    matches!(
                        operand.access,
                        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                    ) && !expected.reads.contains(&operand.operand)
                });
                if let Some(unread) = unread {
                    let mut corrupted = catalog.clone();
                    let reads = &mut declaration_mut(&mut corrupted, key, semantic).alternatives
                        [index]
                        .encoded
                        .external_operand_reads;
                    reads.push(unread.operand);
                    reads.sort_unstable();
                    reject_encoded!(corrupted, "{key:?} invented operand read must reject");
                }
                let mut corrupted = catalog.clone();
                let reads =
                    &mut encoded_mut(&mut corrupted, key, semantic, index).external_operand_reads;
                reads.push(row.operands.len() as u16);
                reject_encoded!(corrupted, "{key:?} out-of-row operand read must reject");
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
                    let reads = &mut declaration_mut(&mut corrupted, key, semantic).alternatives
                        [index]
                        .encoded
                        .external_operand_reads;
                    reads.push(write_operand);
                    reads.sort_unstable();
                    reject_encoded!(corrupted, "{key:?} reading a defined operand must reject");
                }
                let mut corrupted = catalog.clone();
                let forged = row
                    .operands
                    .iter()
                    .find(|operand| operand.access == RegisterOperandAccess::Use)
                    .map(|operand| operand.operand)
                    .unwrap_or_else(|| expected.writes.first().copied().unwrap_or(0));
                let writes = &mut declaration_mut(&mut corrupted, key, semantic).alternatives
                    [index]
                    .encoded
                    .external_operand_writes;
                writes.push(forged);
                writes.sort_unstable();
                reject_encoded!(corrupted, "{key:?} forged operand write must reject");

                let mut corrupted = catalog.clone();
                encoded_mut(&mut corrupted, key, semantic, index)
                    .implicit_unit_uses
                    .push(stack_pointer.units[0]);
                reject_encoded!(corrupted, "{key:?} forged implicit use must reject");
                let mut corrupted = catalog.clone();
                encoded_mut(&mut corrupted, key, semantic, index)
                    .implicit_unit_defs
                    .push(stack_pointer.units[0]);
                reject_encoded!(corrupted, "{key:?} forged implicit definition must reject");
                let mut corrupted = catalog.clone();
                encoded_mut(&mut corrupted, key, semantic, index)
                    .implicit_unit_clobbers
                    .push(stack_pointer.units[0]);
                reject_encoded!(corrupted, "{key:?} forged implicit clobber must reject");
                if contract.flags == FlagsCustody::Use {
                    let mut corrupted = catalog.clone();
                    encoded_mut(&mut corrupted, key, semantic, index)
                        .implicit_unit_uses
                        .retain(|unit| !flags.units.contains(unit));
                    reject_encoded!(corrupted, "{key:?} lost condition-code use must reject");
                }
                if contract.flags == FlagsCustody::Def {
                    let mut corrupted = catalog.clone();
                    encoded_mut(&mut corrupted, key, semantic, index)
                        .implicit_unit_defs
                        .retain(|unit| !flags.units.contains(unit));
                    reject_encoded!(
                        corrupted,
                        "{key:?} lost condition-code definition must reject"
                    );
                }
                if contract.flags == FlagsCustody::Clobber || !contract.extra_clobbers.is_empty() {
                    let mut corrupted = catalog.clone();
                    encoded_mut(&mut corrupted, key, semantic, index)
                        .implicit_unit_clobbers
                        .remove(0);
                    reject_encoded!(corrupted, "{key:?} lost encoded clobber must reject");
                }

                let mut corrupted = catalog.clone();
                let declaration = declaration_mut(&mut corrupted, key, semantic);
                declaration.alternatives[index].key.family =
                    if semantic == MachineSemanticKind::CopyI64 {
                        MachineSemanticKind::ExactAddI64.into()
                    } else {
                        MachineSemanticKind::CopyI64.into()
                    };
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
                declaration_mut(&mut corrupted, key, semantic).alternatives[index].applicability =
                    MachineAlternativeApplicability::ResultAliasesOperand {
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
                    declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                        .applicability =
                        MachineAlternativeApplicability::ResultAliasesOperand { result, operand };
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

                let mut corrupted = catalog.clone();
                declaration_mut(&mut corrupted, key, semantic).alternatives[index].size =
                    MachineSizeKnowledge::ExactBytes(0);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidSizeKnowledge(semantic)
                    )),
                    "{key:?} empty size bound must reject"
                );
                let mut corrupted = catalog.clone();
                declaration_mut(&mut corrupted, key, semantic).alternatives[index].size =
                    MachineSizeKnowledge::ExactBytes(997);
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::SemanticMismatch),
                    "{key:?} size drift must reject"
                );

                let mut corrupted = catalog.clone();
                declaration_mut(&mut corrupted, key, semantic).alternatives[index]
                    .key
                    .variant = (index + 1) as u32;
                let expected_rejection = if index + 1 < alternatives {
                    EffectRejection::Structural(
                        MachineEffectCatalogValidationError::NonCanonicalAlternatives(semantic),
                    )
                } else {
                    EffectRejection::SemanticMismatch
                };
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(expected_rejection),
                    "{key:?} variant drift must reject"
                );
            }

            let mut corrupted = catalog.clone();
            declaration_mut(&mut corrupted, key, semantic)
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
            let declaration = declaration_mut(&mut corrupted, key, semantic);
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
