use super::{
    CompilerIntrinsicExecutionIdentity, CompilerIntrinsicPlanProvenance, ProviderPlan,
    derive_selected_intrinsic_settlement_proposals,
};
use effects::provider_plan::{ProviderBinding, ProviderPlanRow};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn classification_requires_the_called_boundary_format() {
    use semantic_vocabulary::IeeeFloatFormat::{Binary32, Binary64};
    use target_operations::{CompilerBuiltinExecution, IeeeFloatArithmeticOperation};
    for (function, operation) in [
        (
            symbols::BuiltinFunction::FloatIsNan,
            IeeeFloatArithmeticOperation::IsNan,
        ),
        (
            symbols::BuiltinFunction::FloatIsInfinite,
            IeeeFloatArithmeticOperation::IsInfinite,
        ),
    ] {
        let identity = CompilerIntrinsicExecutionIdentity::BuiltinFunction(function);
        assert_eq!(super::compiler_builtin_execution(identity, None), None);
        for format in [Binary32, Binary64] {
            assert_eq!(
                super::compiler_builtin_execution(identity, Some(format)),
                Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format })
            );
        }
    }
}

#[test]
fn multiply_then_add_keeps_selected_format_and_distinct_fused_identity() {
    use semantic_vocabulary::IeeeFloatFormat::{Binary32, Binary64};
    use symbols::BuiltinFunction;
    use target_operations::{CompilerBuiltinExecution, IeeeFloatArithmeticOperation};

    for (function, format) in [
        (BuiltinFunction::FloatMultiplyThenAddF32, Binary32),
        (BuiltinFunction::FloatMultiplyThenAddF64, Binary64),
    ] {
        // The selected builtin carries its format; a proposed signature
        // cannot retarget it. Signature admission separately rejects that
        // mismatch, rather than authenticating a same-shaped declaration.
        for proposed in [None, Some(Binary32), Some(Binary64)] {
            assert_eq!(
                super::compiler_builtin_execution(
                    CompilerIntrinsicExecutionIdentity::BuiltinFunction(function),
                    proposed,
                ),
                Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                    operation: IeeeFloatArithmeticOperation::MultiplyThenAdd,
                    format,
                }),
            );
        }
    }
    for function in [
        BuiltinFunction::FloatFusedMultiplyAddF32,
        BuiltinFunction::FloatFusedMultiplyAddF64,
    ] {
        assert!(
            !matches!(
                super::compiler_builtin_execution(
                    CompilerIntrinsicExecutionIdentity::BuiltinFunction(function),
                    Some(Binary32),
                ),
                Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                    operation: IeeeFloatArithmeticOperation::MultiplyThenAdd,
                    ..
                })
            ),
            "a fused requirement cannot select two roundings"
        );
    }
}

#[derive(Clone)]
struct RetainedPlan {
    plan: ProviderPlan,
    row_compiler_intrinsic_executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>,
}

#[test]
fn fused_identity_keeps_its_own_format_and_one_rounding() {
    use semantic_vocabulary::IeeeFloatFormat::{Binary32, Binary64};
    use symbols::BuiltinFunction;
    use target_operations::{CompilerBuiltinExecution, IeeeFloatArithmeticOperation};
    for (function, format) in [
        (BuiltinFunction::FloatFusedMultiplyAddF32, Binary32),
        (BuiltinFunction::FloatFusedMultiplyAddF64, Binary64),
    ] {
        for proposed in [None, Some(Binary32), Some(Binary64)] {
            let execution = CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                operation: IeeeFloatArithmeticOperation::FusedMultiplyAdd,
                format,
            };
            let identity = CompilerIntrinsicExecutionIdentity::BuiltinFunction(function);
            assert_eq!(
                super::compiler_builtin_execution(identity, proposed),
                Some(execution)
            );
            assert_eq!(
                super::super::compiler_intrinsic_execution_identity(execution),
                identity
            );
        }
    }
}

fn provenance(
    plans: &[RetainedPlan],
) -> impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'_>> {
    plans
        .iter()
        .map(|retained| CompilerIntrinsicPlanProvenance {
            plan: &retained.plan,
            row_executions: &retained.row_compiler_intrinsic_executions,
        })
}

fn selected_plan(
    name: &str,
    rows: &[(&str, Option<CompilerIntrinsicExecutionIdentity>)],
) -> (ProviderPlan, RetainedPlan) {
    let plan = ProviderPlan {
        name: name.to_owned(),
        rows: rows
            .iter()
            .map(|(requirement, _)| ProviderPlanRow {
                method: "operation".to_owned(),
                requirement_identity: (*requirement).to_owned(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::CompilerIntrinsic {
                    machine: "realization".to_owned(),
                },
            })
            .collect(),
        ..ProviderPlan::default()
    };
    let retained = RetainedPlan {
        plan: plan.clone(),
        row_compiler_intrinsic_executions: rows.iter().map(|(_, execution)| *execution).collect(),
    };
    (plan, retained)
}

fn demands(identities: &[&str]) -> BTreeSet<String> {
    identities
        .iter()
        .map(|identity| (*identity).to_owned())
        .collect()
}

#[test]
fn called_intrinsics_keep_original_plan_indices_and_execution_identity() {
    use CompilerIntrinsicExecutionIdentity::{
        HostedExitProcessI32, HostedReadByte, HostedWriteByteI32,
    };
    let (first, first_retained) = selected_plan(
        "first",
        &[
            ("zeta", Some(HostedExitProcessI32)),
            ("beta", None),
            ("unused", None),
        ],
    );
    let (mut second, mut second_retained) = selected_plan(
        "second",
        &[
            ("alpha", Some(HostedReadByte)),
            ("middle", Some(HostedWriteByteI32)),
            ("zeta", None),
        ],
    );
    // A nonintrinsic row with the same identity must not become a duplicate
    // intrinsic candidate or donate its absent execution identity.
    second.rows[2].binding = ProviderBinding::Syscall { number: 0 };
    second_retained.plan = second.clone();
    let plans = [first, second];
    let retained = [first_retained, second_retained];
    let proposals = derive_selected_intrinsic_settlement_proposals(
        &plans,
        provenance(&retained),
        &demands(&["zeta", "middle", "alpha", "missing"]),
        &BTreeMap::new(),
    )
    .expect("called catalog rows project in lexical demand order");
    assert_eq!(
        proposals
            .iter()
            .map(|proposal| (
                proposal.requirement_identity.as_str(),
                proposal.plan_index,
                proposal.execution
            ))
            .collect::<Vec<_>>(),
        [
            (
                "alpha",
                1,
                target_operations::CompilerBuiltinExecution::HostedReadByte
            ),
            (
                "middle",
                1,
                target_operations::CompilerBuiltinExecution::HostedWriteByteI32
            ),
            (
                "zeta",
                0,
                target_operations::CompilerBuiltinExecution::HostedExitProcessI32
            ),
        ]
    );
    assert!(
        derive_selected_intrinsic_settlement_proposals(
            &plans,
            provenance(&retained),
            &BTreeSet::new(),
            &BTreeMap::new(),
        )
        .expect("undemanded rows need no executable identity")
        .is_empty()
    );
}

#[test]
fn duplicate_missing_and_unsupported_executions_keep_lexical_diagnostics() {
    use CompilerIntrinsicExecutionIdentity::{BuiltinFunction, HostedReadByte};
    // A directed FMA still has no native realization; neither the nearest
    // FMA nor a float-to-integer conversion can substitute for it.
    let unsupported = BuiltinFunction(symbols::BuiltinFunction::FloatFusedMultiplyAddTowardZeroF64);
    let (first, first_retained) = selected_plan(
        "first",
        &[
            ("zeta", Some(unsupported)),
            ("beta", None),
            ("alpha", Some(HostedReadByte)),
            ("alpha", None),
        ],
    );
    let (second, second_retained) = selected_plan("second", &[("alpha", Some(HostedReadByte))]);
    let diagnostics = derive_selected_intrinsic_settlement_proposals(
        &[first, second],
        provenance(&[first_retained, second_retained]),
        &demands(&["zeta", "missing", "alpha", "beta"]),
        &BTreeMap::new(),
    )
    .expect_err("duplicates must not choose a row or hide later diagnostics");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            "Terminal boundary `alpha` resolves to 3 selected compiler-intrinsic rows",
            "selected compiler intrinsic `first` for Terminal boundary `beta` has no closed native catalog identity",
            "selected compiler intrinsic `first` for Terminal boundary `zeta` has no native boundary realization",
        ]
    );
}

#[test]
fn undemanded_plans_still_require_complete_aligned_provenance() {
    let (plan, retained) = selected_plan("unused", &[("unused", None)]);
    for demanded in [BTreeSet::new(), demands(&["unmatched"])] {
        let diagnostics = derive_selected_intrinsic_settlement_proposals(
            std::slice::from_ref(&plan),
            provenance(&[]),
            &demanded,
            &BTreeMap::new(),
        )
        .expect_err("missing retained plan rejects even without demand");
        assert!(diagnostics[0].message.contains("plans are not aligned"));
        for corrupt_plan in [false, true] {
            let mut malformed = retained.clone();
            if corrupt_plan {
                malformed.plan.rows[0].requirement_identity = "substituted".to_owned();
            } else {
                malformed.row_compiler_intrinsic_executions.clear();
            }
            let diagnostics = derive_selected_intrinsic_settlement_proposals(
                std::slice::from_ref(&plan),
                provenance(&[malformed]),
                &demanded,
                &BTreeMap::new(),
            )
            .expect_err("all retained plan and execution alignments are checked");
            assert_eq!(
                diagnostics[0].message,
                "selected compiler-intrinsic plan `unused` has misaligned retained settlement provenance"
            );
        }
    }
}

#[test]
fn float_arithmetic_rows_project_exact_executions_and_comparisons_do_not() {
    use effects::CompilerPrimitiveFloatBinaryOperation as Primitive;
    use numerics::literals::FloatFormat;
    use semantic_vocabulary::IeeeFloatFormat;
    use target_operations::{CompilerBuiltinExecution, IeeeFloatArithmeticOperation as Arithmetic};
    let binary = |operation, format| CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
        operation,
        format,
    };
    let (plan, retained) = selected_plan(
        "floats",
        &[
            ("a_add32", Some(binary(Primitive::Add, FloatFormat::F32))),
            (
                "b_subtract64",
                Some(binary(Primitive::Subtract, FloatFormat::F64)),
            ),
            (
                "c_multiply32",
                Some(binary(Primitive::Multiply, FloatFormat::F32)),
            ),
            (
                "d_divide64",
                Some(binary(Primitive::Divide, FloatFormat::F64)),
            ),
        ],
    );
    let proposals = derive_selected_intrinsic_settlement_proposals(
        std::slice::from_ref(&plan),
        provenance(std::slice::from_ref(&retained)),
        &demands(&["a_add32", "b_subtract64", "c_multiply32", "d_divide64"]),
        &BTreeMap::new(),
    )
    .expect("every arithmetic operation and format has a native realization");
    let expected = [
        (Arithmetic::Add, IeeeFloatFormat::Binary32),
        (Arithmetic::Subtract, IeeeFloatFormat::Binary64),
        (Arithmetic::Multiply, IeeeFloatFormat::Binary32),
        (Arithmetic::Divide, IeeeFloatFormat::Binary64),
    ];
    assert_eq!(
        proposals
            .iter()
            .map(|proposal| proposal.execution)
            .collect::<Vec<_>>(),
        expected.map(
            |(operation, format)| CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                operation,
                format
            }
        )
    );

    // A comparison selected for a called boundary has no call realization:
    // IEEE comparisons settle their emitted Terminal operations instead.
    // A duplicated arithmetic row cannot choose one of its copies.
    let (plan, retained) = selected_plan(
        "floats",
        &[
            ("equal", Some(binary(Primitive::Equal, FloatFormat::F64))),
            ("twice", Some(binary(Primitive::Add, FloatFormat::F64))),
            ("twice", Some(binary(Primitive::Add, FloatFormat::F64))),
        ],
    );
    let diagnostics = derive_selected_intrinsic_settlement_proposals(
        std::slice::from_ref(&plan),
        provenance(std::slice::from_ref(&retained)),
        &demands(&["equal", "twice"]),
        &BTreeMap::new(),
    )
    .expect_err("comparison and duplicate rows reject");
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        [
            "selected compiler intrinsic `floats` for Terminal boundary `equal` has no native boundary realization",
            "Terminal boundary `twice` resolves to 2 selected compiler-intrinsic rows",
        ]
    );
}

#[test]
fn named_float_rows_project_exact_executions_and_unrealized_rows_do_not() {
    use effects::CompilerNumericType as Numeric;
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::literals::FloatFormat;
    use semantic_vocabulary::IeeeFloatFormat::{Binary32, Binary64};
    use target_operations::{
        CompilerBuiltinExecution, IeeeFloatArithmeticOperation as Arithmetic,
        IeeeFloatIntegerCarrier as Integer, IeeeFloatToIntegerDomain,
    };
    let conversion =
        |source, target, domain| CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
            source,
            target,
            domain,
        };
    let exact = ArithmeticDomain::Exact;
    let builtin = CompilerIntrinsicExecutionIdentity::BuiltinFunction;
    let (plan, retained) = selected_plan(
        "named",
        &[
            ("a_min32", Some(builtin(symbols::BuiltinFunction::Min))),
            ("b_max64", Some(builtin(symbols::BuiltinFunction::Max))),
            ("c_sqrt64", Some(builtin(symbols::BuiltinFunction::Sqrt))),
            (
                "d_negate32",
                Some(CompilerIntrinsicExecutionIdentity::NamedFloatNegation(
                    FloatFormat::F32,
                )),
            ),
            (
                "e_widen",
                Some(conversion(Numeric::F32, Numeric::F64, exact)),
            ),
            (
                "f_narrow",
                Some(conversion(Numeric::F64, Numeric::F32, exact)),
            ),
            ("g_i32", Some(conversion(Numeric::I32, Numeric::F64, exact))),
            ("h_i64", Some(conversion(Numeric::I64, Numeric::F32, exact))),
            ("i_u32", Some(conversion(Numeric::U32, Numeric::F64, exact))),
            ("i_u8", Some(conversion(Numeric::U8, Numeric::F32, exact))),
            (
                "j_to_i32",
                Some(conversion(Numeric::F64, Numeric::I32, exact)),
            ),
            (
                "k_to_i64",
                Some(conversion(
                    Numeric::F32,
                    Numeric::I64,
                    ArithmeticDomain::Saturating,
                )),
            ),
            (
                "l_to_u32",
                Some(conversion(
                    Numeric::F64,
                    Numeric::U32,
                    ArithmeticDomain::Saturating,
                )),
            ),
            (
                "m_from_u64",
                Some(conversion(Numeric::U64, Numeric::F64, exact)),
            ),
            (
                "n_to_u64",
                Some(conversion(Numeric::F32, Numeric::U64, exact)),
            ),
            (
                "o_to_u64_saturating",
                Some(conversion(
                    Numeric::F64,
                    Numeric::U64,
                    ArithmeticDomain::Saturating,
                )),
            ),
            (
                "p_to_i32_trapping",
                Some(conversion(
                    Numeric::F64,
                    Numeric::I32,
                    ArithmeticDomain::Trapping,
                )),
            ),
        ],
    );
    let float_results = [
        ("a_min32", Binary32),
        ("b_max64", Binary64),
        ("c_sqrt64", Binary64),
    ]
    .map(|(identity, format)| (identity.to_owned(), format))
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let proposals = derive_selected_intrinsic_settlement_proposals(
        std::slice::from_ref(&plan),
        provenance(std::slice::from_ref(&retained)),
        &demands(&[
            "a_min32",
            "b_max64",
            "c_sqrt64",
            "d_negate32",
            "e_widen",
            "f_narrow",
            "g_i32",
            "h_i64",
            "i_u32",
            "i_u8",
            "j_to_i32",
            "k_to_i64",
            "l_to_u32",
            "m_from_u64",
            "n_to_u64",
            "o_to_u64_saturating",
            "p_to_i32_trapping",
        ]),
        &float_results,
    )
    .expect("every named operation and exact conversion has a native realization");
    let expected = [
        (Arithmetic::Minimum, Binary32),
        (Arithmetic::Maximum, Binary64),
        (Arithmetic::SquareRoot, Binary64),
        (Arithmetic::Negate, Binary32),
        (Arithmetic::ConvertFormat, Binary64),
        (Arithmetic::ConvertFormat, Binary32),
        (Arithmetic::FromInteger(Integer::I32), Binary64),
        (Arithmetic::FromInteger(Integer::I64), Binary32),
        (Arithmetic::FromInteger(Integer::U32), Binary64),
        (Arithmetic::FromInteger(Integer::U8), Binary32),
        (
            Arithmetic::TruncateToInteger {
                target: Integer::I32,
                domain: IeeeFloatToIntegerDomain::Exact,
            },
            Binary64,
        ),
        (
            Arithmetic::TruncateToInteger {
                target: Integer::I64,
                domain: IeeeFloatToIntegerDomain::Saturating,
            },
            Binary32,
        ),
        (
            Arithmetic::TruncateToInteger {
                target: Integer::U32,
                domain: IeeeFloatToIntegerDomain::Saturating,
            },
            Binary64,
        ),
        (Arithmetic::FromInteger(Integer::U64), Binary64),
        (
            Arithmetic::TruncateToInteger {
                target: Integer::U64,
                domain: IeeeFloatToIntegerDomain::Exact,
            },
            Binary32,
        ),
        (
            Arithmetic::TruncateToInteger {
                target: Integer::U64,
                domain: IeeeFloatToIntegerDomain::Saturating,
            },
            Binary64,
        ),
        (
            Arithmetic::TruncateToInteger {
                target: Integer::I32,
                domain: IeeeFloatToIntegerDomain::Trapping,
            },
            Binary64,
        ),
    ];
    assert_eq!(
        proposals
            .iter()
            .map(|proposal| proposal.execution)
            .collect::<Vec<_>>(),
        expected.map(
            |(operation, format)| CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                operation,
                format
            }
        )
    );

    // Wrapping is not a float-to-integer contract. Same-format conversion and
    // a format-free builtin without an IEEE boundary format stay unrealized.
    let (plan, retained) = selected_plan(
        "named",
        &[
            (
                "wrapping",
                Some(conversion(
                    Numeric::F64,
                    Numeric::I32,
                    ArithmeticDomain::Wrapping,
                )),
            ),
            ("same", Some(conversion(Numeric::F64, Numeric::F64, exact))),
            ("unformatted", Some(builtin(symbols::BuiltinFunction::Min))),
        ],
    );
    let diagnostics = derive_selected_intrinsic_settlement_proposals(
        std::slice::from_ref(&plan),
        provenance(std::slice::from_ref(&retained)),
        &demands(&["wrapping", "same", "unformatted"]),
        &BTreeMap::new(),
    )
    .expect_err("unrealized conversions reject");
    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic
            .message
            .ends_with("has no native boundary realization")
    }));
}

#[test]
fn directed_binary_identity_cannot_borrow_a_boundary_mode_or_format() {
    use semantic_vocabulary::IeeeFloatFormat as Format;
    use symbols::BuiltinFunction;
    use target_operations::{
        CompilerBuiltinExecution, IeeeFloatArithmeticOperation as Arithmetic,
        IeeeFloatBinaryOperation as Binary, IeeeFloatRoundingDirection as Direction,
    };
    for (function, operation, rounding, format) in [
        (
            BuiltinFunction::FloatAddTowardZeroF32,
            Binary::Add,
            Direction::TowardZero,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatAddTowardZeroF64,
            Binary::Add,
            Direction::TowardZero,
            Format::Binary64,
        ),
        (
            BuiltinFunction::FloatAddTowardPositiveF32,
            Binary::Add,
            Direction::TowardPositive,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatAddTowardPositiveF64,
            Binary::Add,
            Direction::TowardPositive,
            Format::Binary64,
        ),
        (
            BuiltinFunction::FloatAddTowardNegativeF32,
            Binary::Add,
            Direction::TowardNegative,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatAddTowardNegativeF64,
            Binary::Add,
            Direction::TowardNegative,
            Format::Binary64,
        ),
        (
            BuiltinFunction::FloatSubtractTowardZeroF32,
            Binary::Subtract,
            Direction::TowardZero,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatSubtractTowardZeroF64,
            Binary::Subtract,
            Direction::TowardZero,
            Format::Binary64,
        ),
        (
            BuiltinFunction::FloatSubtractTowardPositiveF32,
            Binary::Subtract,
            Direction::TowardPositive,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatSubtractTowardPositiveF64,
            Binary::Subtract,
            Direction::TowardPositive,
            Format::Binary64,
        ),
        (
            BuiltinFunction::FloatSubtractTowardNegativeF32,
            Binary::Subtract,
            Direction::TowardNegative,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatSubtractTowardNegativeF64,
            Binary::Subtract,
            Direction::TowardNegative,
            Format::Binary64,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardZeroF32,
            Binary::Multiply,
            Direction::TowardZero,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardZeroF64,
            Binary::Multiply,
            Direction::TowardZero,
            Format::Binary64,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardPositiveF32,
            Binary::Multiply,
            Direction::TowardPositive,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardPositiveF64,
            Binary::Multiply,
            Direction::TowardPositive,
            Format::Binary64,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardNegativeF32,
            Binary::Multiply,
            Direction::TowardNegative,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardNegativeF64,
            Binary::Multiply,
            Direction::TowardNegative,
            Format::Binary64,
        ),
        (
            BuiltinFunction::FloatDivideTowardZeroF32,
            Binary::Divide,
            Direction::TowardZero,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatDivideTowardZeroF64,
            Binary::Divide,
            Direction::TowardZero,
            Format::Binary64,
        ),
        (
            BuiltinFunction::FloatDivideTowardPositiveF32,
            Binary::Divide,
            Direction::TowardPositive,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatDivideTowardPositiveF64,
            Binary::Divide,
            Direction::TowardPositive,
            Format::Binary64,
        ),
        (
            BuiltinFunction::FloatDivideTowardNegativeF32,
            Binary::Divide,
            Direction::TowardNegative,
            Format::Binary32,
        ),
        (
            BuiltinFunction::FloatDivideTowardNegativeF64,
            Binary::Divide,
            Direction::TowardNegative,
            Format::Binary64,
        ),
    ] {
        let identity = CompilerIntrinsicExecutionIdentity::BuiltinFunction(function);
        let execution = CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
            operation: Arithmetic::DirectedBinary {
                operation,
                rounding,
            },
            format,
        };
        for proposed in [None, Some(Format::Binary32), Some(Format::Binary64)] {
            assert_eq!(
                super::compiler_builtin_execution(identity, proposed),
                Some(execution)
            );
        }
        assert_eq!(
            super::super::compiler_intrinsic_execution_identity(execution),
            identity
        );
        let other = match format {
            Format::Binary32 => Format::Binary64,
            Format::Binary64 => Format::Binary32,
        };
        assert!(
            !(Arithmetic::DirectedBinary {
                operation,
                rounding
            })
            .signature_matches(
                format,
                &[semantic_vocabulary::ScalarType::IeeeFloat(other); 2],
                semantic_vocabulary::ScalarType::IeeeFloat(other)
            )
        );
    }
}
