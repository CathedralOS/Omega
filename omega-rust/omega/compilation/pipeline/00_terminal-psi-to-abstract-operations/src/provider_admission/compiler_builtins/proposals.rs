//! Target-constrained proposals for compiler-owned native builtins.
//!
//! Provider selection and checked intrinsic review establish the exact
//! declaration/target identity. This module performs the later executable
//! projection: only compiler-intrinsic requirements actually called by the
//! canonical Terminal artifact receive a structural proposal, bound here to
//! its selected plan and Terminal boundary. The consuming lowerer admits it
//! through its local target catalog.

use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::{ProviderPlan, ServiceSchema};
use std::collections::BTreeSet;

/// The source-free part of one retained provider review. The complete plan is
/// compared before its row-aligned execution identities can supply a proposal.
pub struct CompilerIntrinsicPlanProvenance<'a> {
    pub plan: &'a ProviderPlan,
    /// The contract classification authenticated before projecting a direct
    /// callable's selection coordinate. Ordinary plans use their own schema.
    pub semantic_schema: &'a ServiceSchema,
    pub row_executions: &'a [Option<CompilerIntrinsicExecutionIdentity>],
}

struct SelectedIntrinsicRow<'a> {
    requirement_identity: &'a str,
    plan_index: usize,
    row: &'a effects::provider_plan::ProviderPlanRow,
    semantic_schema: &'a ServiceSchema,
    execution: Option<CompilerIntrinsicExecutionIdentity>,
}

/// Project called intrinsic rows into native catalog proposals, each bound
/// once to its selected plan and the one Terminal boundary it settles.
/// Provenance stays borrowed and source-free; every plan is checked even
/// without demand. The receiving target catalog admits these proposals later.
pub(crate) fn derive_selected_intrinsic_settlement_proposals<'a>(
    selected: &'a effects::SelectedProviderPlanFacts,
    provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'a>>,
    demanded_boundaries: &BTreeSet<String>,
    boundaries: &[terminal_psi::BoundaryMachineDeclaration],
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<Vec<super::TerminalCompilerBuiltinProposal>, Vec<Diagnostic>> {
    let plans = selected.plans();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected provider plans are not aligned with compiler-intrinsic settlement provenance",
        )]);
    }
    let mut selected_rows = Vec::new();
    for (plan_index, (plan, retained)) in plans.iter().zip(provenance).enumerate() {
        if retained.plan != plan || retained.row_executions.len() != plan.rows.len() {
            return Err(vec![Diagnostic::error(format!(
                "selected compiler-intrinsic plan `{}` has misaligned retained settlement provenance",
                plan.name,
            ))]);
        }
        if demanded_boundaries.is_empty() {
            continue;
        }
        for (row, execution) in plan.rows.iter().zip(retained.row_executions) {
            if matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
            ) {
                selected_rows.push(SelectedIntrinsicRow {
                    requirement_identity: plan.identities.text(row.requirement_identity),
                    plan_index,
                    row,
                    semantic_schema: retained.semantic_schema,
                    execution: *execution,
                });
            }
        }
    }
    // Keep the row/execution association from its producing plan. Sorting only
    // this borrowed selection leaves plan identity and row ownership unchanged.
    selected_rows
        .sort_unstable_by(|left, right| left.requirement_identity.cmp(right.requirement_identity));

    // Each demanded callable retains its own occurrence. A selected row may
    // realize both a portable family head and a direct requirement call.
    let mut evidence = Vec::new();
    let mut diagnostics = Vec::new();
    for requirement in demanded_boundaries {
        let actual_requirement = selected.selected_requirement_identity(requirement);
        let first =
            selected_rows.partition_point(|row| row.requirement_identity < actual_requirement);
        let matches = selected_rows[first..]
            .iter()
            .take_while(|row| row.requirement_identity == actual_requirement)
            .filter(|row| {
                selected.selected_row_matches_requirement(
                    requirement,
                    row.requirement_identity,
                    plans[row.plan_index].report_fingerprint(),
                )
            })
            .collect::<Vec<_>>();
        let [selected_row] = matches.as_slice() else {
            if !matches.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` resolves to {} selected compiler-intrinsic rows", matches.len(),
                )));
            }
            continue;
        };
        let mut matching_boundaries = boundaries
            .iter()
            .filter(|boundary| identities.text(boundary.identity) == Some(requirement.as_str()));
        let (Some(boundary), None) = (matching_boundaries.next(), matching_boundaries.next())
        else {
            diagnostics.push(Diagnostic::error(format!(
                "compiler-builtin proposal for `{requirement}` does not rejoin one Terminal boundary"
            )));
            continue;
        };
        // A row this compile could not close is retained with its reason, not
        // refused here: whether the boundary is executed is each entry's
        // execution-scope question, answered at admission.
        let execution = match selected_execution(
            boundary.numeric_intrinsic,
            &boundary.external_providers,
            identities,
            &plans[selected_row.plan_index],
            selected_row.row,
            selected_row.execution,
        ) {
            Ok(execution) => execution,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` {reason}"
                )));
                continue;
            }
        };
        let format = boundary
            .numeric_intrinsic
            .and_then(|meaning| meaning.builtin_format());
        let settlement = match execution {
            None => super::TerminalCompilerBuiltinSettlement::NoClosedCatalogIdentity,
            Some(execution) => match compiler_builtin_execution(execution, format) {
                Some(execution) => super::TerminalCompilerBuiltinSettlement::Execution(execution),
                None => super::TerminalCompilerBuiltinSettlement::NoNativeBoundaryRealization,
            },
        };
        evidence.push(super::TerminalCompilerBuiltinProposal::new(
            selected_row.requirement_identity.to_owned(),
            selected_row.plan_index,
            boundary.id,
            settlement,
            selected_row.semantic_schema,
            &plans[selected_row.plan_index].schema,
        ));
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
}

/// One directed square root on one IEEE format. The mode is the operation,
/// same as directed binary arithmetic, so nearest square root cannot satisfy it.
fn nearest_directed_square_root(
    rounding: target_operations::IeeeFloatRoundingDirection,
    format: semantic_vocabulary::IeeeFloatFormat,
) -> target_operations::CompilerBuiltinExecution {
    nearest_arithmetic(
        target_operations::IeeeFloatArithmeticOperation::DirectedSquareRoot { rounding },
        format,
    )
}

/// One directed binary rounding mode on one IEEE format realized as the
/// nearest-neighbor arithmetic builtin.
fn nearest_directed_binary(
    operation: target_operations::IeeeFloatBinaryOperation,
    rounding: target_operations::IeeeFloatRoundingDirection,
    format: semantic_vocabulary::IeeeFloatFormat,
) -> target_operations::CompilerBuiltinExecution {
    nearest_arithmetic(
        target_operations::IeeeFloatArithmeticOperation::DirectedBinary {
            operation,
            rounding,
        },
        format,
    )
}

/// One non-decomposed float arithmetic builtin on one IEEE format.
fn nearest_arithmetic(
    operation: target_operations::IeeeFloatArithmeticOperation,
    format: semantic_vocabulary::IeeeFloatFormat,
) -> target_operations::CompilerBuiltinExecution {
    target_operations::CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format }
}

/// The native realization of one selected intrinsic identity, if the local
/// catalog has one. `boundary_format` is the Psi-published IEEE
/// format, which the format-free custody identities need; admission rejoins
/// the complete boundary signature independently.
fn compiler_builtin_execution(
    identity: CompilerIntrinsicExecutionIdentity,
    boundary_format: Option<semantic_vocabulary::IeeeFloatFormat>,
) -> Option<target_operations::CompilerBuiltinExecution> {
    use target_operations::CompilerBuiltinExecution;
    use target_operations::IeeeFloatArithmeticOperation as Arithmetic;
    let ieee_format = |format| match format {
        numerics::literals::FloatFormat::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
        numerics::literals::FloatFormat::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
    };
    let (operation, format) = match identity {
        // A private compiler operation is not a selectable implementation of
        // an authored provider requirement. It is bound from its own retained
        // declaration and lifecycle contract, never from a Build intrinsic row.
        CompilerIntrinsicExecutionIdentity::PlacementLifecycle(_) => return None,
        CompilerIntrinsicExecutionIdentity::ProgramImageRetainedSuffixStart => {
            return Some(CompilerBuiltinExecution::ProgramImageRetainedSuffixStart);
        }
        CompilerIntrinsicExecutionIdentity::ProgramStorageEntry => {
            return Some(CompilerBuiltinExecution::ProgramStorageEntry);
        }
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32 => {
            return Some(CompilerBuiltinExecution::HostedExitProcessI32);
        }
        CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => {
            return Some(CompilerBuiltinExecution::HostedWriteByteI32);
        }
        CompilerIntrinsicExecutionIdentity::PlacedScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        } => {
            return Some(CompilerBuiltinExecution::PlacedScalarAccess {
                contract,
                placement_commitment,
                access_slot,
                operation: match operation {
                    effects::CompilerPlacedScalarOperation::Read => {
                        terminal_psi::TerminalPlacedAccessorKind::Read
                    }
                    effects::CompilerPlacedScalarOperation::Take => {
                        terminal_psi::TerminalPlacedAccessorKind::Take
                    }
                    effects::CompilerPlacedScalarOperation::Write => {
                        terminal_psi::TerminalPlacedAccessorKind::Write
                    }
                },
            });
        }
        CompilerIntrinsicExecutionIdentity::HostedReadByte => {
            return Some(CompilerBuiltinExecution::HostedReadByte);
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            // Comparisons demanded as boundary calls realize as the same
            // IEEE relation the emitted Terminal comparison selects; a
            // comparison selected on a use site still settles its emitted
            // `IeeeFloatCompare`, never this row.
            use effects::CompilerPrimitiveFloatBinaryOperation as Primitive;
            use semantic_vocabulary::IeeeFloatComparisonOperation as Relation;
            let operation = match operation {
                Primitive::Add => Arithmetic::Add,
                Primitive::Subtract => Arithmetic::Subtract,
                Primitive::Multiply => Arithmetic::Multiply,
                Primitive::Divide => Arithmetic::Divide,
                Primitive::Equal => Arithmetic::Compare(Relation::Equal),
                Primitive::NotEqual => Arithmetic::Compare(Relation::NotEqual),
                Primitive::Less => Arithmetic::Compare(Relation::Less),
                Primitive::LessOrEqual => Arithmetic::Compare(Relation::LessOrEqual),
                Primitive::Greater => Arithmetic::Compare(Relation::Greater),
                Primitive::GreaterOrEqual => Arithmetic::Compare(Relation::GreaterOrEqual),
            };
            (operation, ieee_format(format))
        }
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            // Format-bearing identities must not borrow an unrelated boundary's
            // result format. Complete signature admission below remains required.
            match function {
                symbols::BuiltinFunction::FloatClassifyF32 => {
                    return Some(nearest_arithmetic(
                        Arithmetic::Classify,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatClassifyF64 => {
                    return Some(nearest_arithmetic(
                        Arithmetic::Classify,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }

                symbols::BuiltinFunction::FloatAddTowardZeroF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatAddTowardZeroF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatAddTowardPositiveF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatAddTowardPositiveF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatAddTowardNegativeF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatAddTowardNegativeF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatSubtractTowardZeroF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatSubtractTowardZeroF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatSubtractTowardPositiveF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatSubtractTowardPositiveF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatSubtractTowardNegativeF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatSubtractTowardNegativeF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatMultiplyTowardZeroF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatMultiplyTowardZeroF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatMultiplyTowardPositiveF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatMultiplyTowardPositiveF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatMultiplyTowardNegativeF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatMultiplyTowardNegativeF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatDivideTowardZeroF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatDivideTowardZeroF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatDivideTowardPositiveF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatDivideTowardPositiveF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatDivideTowardNegativeF32 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatDivideTowardNegativeF64 => {
                    return Some(nearest_directed_binary(
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatSqrtTowardZeroF32 => {
                    return Some(nearest_directed_square_root(
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatSqrtTowardZeroF64 => {
                    return Some(nearest_directed_square_root(
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatSqrtTowardPositiveF32 => {
                    return Some(nearest_directed_square_root(
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatSqrtTowardPositiveF64 => {
                    return Some(nearest_directed_square_root(
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatSqrtTowardNegativeF32 => {
                    return Some(nearest_directed_square_root(
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatSqrtTowardNegativeF64 => {
                    return Some(nearest_directed_square_root(
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatFusedMultiplyAddF32 => {
                    return Some(nearest_arithmetic(
                        Arithmetic::FusedMultiplyAdd,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatFusedMultiplyAddF64 => {
                    return Some(nearest_arithmetic(
                        Arithmetic::FusedMultiplyAdd,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                symbols::BuiltinFunction::FloatMultiplyThenAddF32 => {
                    return Some(nearest_arithmetic(
                        Arithmetic::MultiplyThenAdd,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ));
                }
                symbols::BuiltinFunction::FloatMultiplyThenAddF64 => {
                    return Some(nearest_arithmetic(
                        Arithmetic::MultiplyThenAdd,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
                }
                _ => {}
            }
            let operation = match function {
                symbols::BuiltinFunction::FloatMinimum => Arithmetic::Minimum,
                symbols::BuiltinFunction::FloatMaximum => Arithmetic::Maximum,
                symbols::BuiltinFunction::FloatSquareRoot => Arithmetic::SquareRoot,
                symbols::BuiltinFunction::FloatIsNan => Arithmetic::IsNan,
                symbols::BuiltinFunction::FloatIsInfinite => Arithmetic::IsInfinite,
                symbols::BuiltinFunction::FloatIsFinite => Arithmetic::IsFinite,
                symbols::BuiltinFunction::FloatIsNormal => Arithmetic::IsNormal,
                symbols::BuiltinFunction::FloatIsSubnormal => Arithmetic::IsSubnormal,
                _ => return None,
            };
            (operation, boundary_format?)
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format) => {
            (Arithmetic::Negate, ieee_format(format))
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
            source,
            target,
            domain,
        } => {
            use effects::CompilerNumericType as Numeric;
            use numerics::arithmetic::ArithmeticDomain;
            use target_operations::{IeeeFloatIntegerCarrier as Carrier, IeeeFloatToIntegerDomain};
            let float = |numeric| match numeric {
                Numeric::F32 => Some(semantic_vocabulary::IeeeFloatFormat::Binary32),
                Numeric::F64 => Some(semantic_vocabulary::IeeeFloatFormat::Binary64),
                _ => None,
            };
            let carrier = |numeric| match numeric {
                Numeric::I8 => Some(Carrier::I8),
                Numeric::I16 => Some(Carrier::I16),
                Numeric::I32 => Some(Carrier::I32),
                Numeric::I64 => Some(Carrier::I64),
                Numeric::U8 => Some(Carrier::U8),
                Numeric::U16 => Some(Carrier::U16),
                Numeric::U32 => Some(Carrier::U32),
                Numeric::U64 => Some(Carrier::U64),
                _ => None,
            };
            // Conversions into a float format are total nearest-even Exact
            // requirements. Conversions to an integer truncate: Exact and
            // Saturating share clamped truncation on the Exact proof's valid
            // inputs. Trapping retains its invalid-input crash mechanism.
            match (float(source), float(target), domain) {
                (Some(from), Some(format), ArithmeticDomain::Exact) if from != format => {
                    (Arithmetic::ConvertFormat, format)
                }
                (None, Some(format), ArithmeticDomain::Exact) => {
                    (Arithmetic::FromInteger(carrier(source)?), format)
                }
                (
                    Some(format),
                    None,
                    ArithmeticDomain::Exact
                    | ArithmeticDomain::Saturating
                    | ArithmeticDomain::Trapping,
                ) => (
                    Arithmetic::TruncateToInteger {
                        target: carrier(target)?,
                        domain: match domain {
                            ArithmeticDomain::Exact => IeeeFloatToIntegerDomain::Exact,
                            ArithmeticDomain::Saturating => IeeeFloatToIntegerDomain::Saturating,
                            ArithmeticDomain::Trapping => IeeeFloatToIntegerDomain::Trapping,
                            _ => return None,
                        },
                    },
                    format,
                ),
                _ => return None,
            }
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison { .. } => return None,
    };
    Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format })
}

/// Join the selected external leaf to Psi's exact satisfaction edge. Numeric
/// meaning alone cannot authorize execution; a same-signature provider or a
/// declaration from another package cannot replace the selected realization.
fn selected_execution(
    meaning: Option<language_semantics::numeric_intrinsics::NumericIntrinsic>,
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    plan: &ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, &'static str> {
    let Some(meaning) = meaning else {
        reconcile_numeric_meaning(None, retained)?;
        return Ok(retained);
    };
    let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
    else {
        return Err("numeric provider does not select an external compiler intrinsic");
    };
    let mut candidates = external_providers.iter().filter(|candidate| {
        candidate.binding == language_semantics::ExternalBindingIdentity::CompilerIntrinsic
            && identities.text(candidate.realization.callable) == Some(machine.as_str())
            && candidate.realization_package == plan.origin_package_identity
            && match &candidate.provider {
                Some(provider) => {
                    identities.text(provider.identity) == Some(plan.provider_type.as_str())
                        && provider.package == plan.provider_type_package_identity
                }
                None => {
                    plan.provider_type.is_empty() && plan.provider_type_package_identity.is_none()
                }
            }
    });
    let (Some(candidate), None) = (candidates.next(), candidates.next()) else {
        return Err(
            "selected numeric provider does not rejoin one canonical external satisfaction",
        );
    };
    candidate.validate(identities)?;
    let execution = CompilerIntrinsicExecutionIdentity::from(meaning);
    // A source-review consumer can still supply its independently retained row.
    // It cannot replace the portable meaning. Native persistence omits it.
    if retained.is_some() {
        reconcile_numeric_meaning(Some(meaning), retained)?;
    }
    Ok(Some(execution))
}

/// Portable meaning never replaces selected-provider custody. In particular,
/// a forged numeric boundary cannot turn an unsupported selected row into an
/// executable provider, and a numeric provider cannot omit its portable meaning.
fn reconcile_numeric_meaning(
    meaning: Option<language_semantics::numeric_intrinsics::NumericIntrinsic>,
    selected: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<(), &'static str> {
    use CompilerIntrinsicExecutionIdentity as Execution;
    let selected_numeric = selected.is_some_and(Execution::is_numeric);
    match (meaning, selected, selected_numeric) {
        (Some(meaning), Some(execution), _)
            if meaning.signature().is_some() && Execution::from(meaning) == execution =>
        {
            Ok(())
        }
        (None, _, false) => Ok(()),
        _ => Err("numeric meaning disagrees with selected provider custody"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use language_semantics::numeric_intrinsics::NumericIntrinsic;

    #[test]
    fn numeric_provider_joins_portable_satisfaction_without_a_source_execution_row() {
        use terminal_psi::{
            CheckedMachineDeclaration, ExternalProviderConformance, ExternalProviderOwner,
            NominalDeclarationIdentity, TerminalIdentityTableBuilder,
        };
        let mut identities = TerminalIdentityTableBuilder::default();
        let callable = identities.intern("Realizer::negate(f32)->f32");
        let provider = identities.intern("Realizer");
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([31; 32]);
        let candidate = ExternalProviderConformance {
            realization: CheckedMachineDeclaration {
                machine: NominalDeclarationIdentity::new(callable),
                state: NominalDeclarationIdentity::new(callable),
                semantic_declaration: callable,
                parameter_types: vec![identities.intern("f32")],
                callable,
                result_type: identities.intern("f32"),
                contract: [22; 32],
                specialization: None,
            },
            realization_package: package,
            provider: Some(ExternalProviderOwner {
                declaration: NominalDeclarationIdentity::new(provider),
                identity: provider,
                package,
            }),
            binding: language_semantics::ExternalBindingIdentity::CompilerIntrinsic,
        };
        let identities = identities.build();
        let plan = ProviderPlan {
            provider_type: "Realizer".into(),
            provider_type_package_identity: package,
            origin_package_identity: package,
            ..ProviderPlan::default()
        };
        let row = effects::provider_plan::ProviderPlanRow {
            method: "negate".into(),
            requirement_identity: Default::default(),
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding: effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                machine: "Realizer::negate(f32)->f32".into(),
            },
        };
        let meaning = NumericIntrinsic::from_canonical_encoding([3, 0, 0, 0]).unwrap();
        assert_eq!(
            selected_execution(
                Some(meaning),
                std::slice::from_ref(&candidate),
                &identities,
                &plan,
                &row,
                None
            )
            .unwrap(),
            Some(meaning.into())
        );
        assert!(selected_execution(Some(meaning), &[], &identities, &plan, &row, None).is_err());
        for mutation in 0..6 {
            let mut altered = candidate.clone();
            match mutation {
                0 => altered.realization.callable = provider,
                1 => altered.realization_package = None,
                2 => altered.provider.as_mut().unwrap().identity = callable,
                3 => altered.provider.as_mut().unwrap().package = None,
                4 => {
                    altered.binding =
                        language_semantics::ExternalBindingIdentity::Syscall { number: 1 }
                }
                5 => altered.provider = None,
                _ => unreachable!(),
            }
            assert!(
                selected_execution(Some(meaning), &[altered], &identities, &plan, &row, None)
                    .is_err(),
                "mutation {mutation}"
            );
        }
        assert!(
            selected_execution(
                Some(meaning),
                &[candidate.clone(), candidate.clone()],
                &identities,
                &plan,
                &row,
                None
            )
            .is_err()
        );
        assert!(
            selected_execution(
                Some(meaning),
                &[candidate],
                &identities,
                &plan,
                &row,
                Some(CompilerIntrinsicExecutionIdentity::HostedWriteByteI32)
            )
            .is_err()
        );
    }

    #[test]
    fn portable_numeric_meaning_cannot_authorize_or_replace_provider_custody() {
        let meaning = NumericIntrinsic::from_canonical_encoding([0, 5, 1, 0]).unwrap();
        let selected = CompilerIntrinsicExecutionIdentity::from(meaning);
        assert!(reconcile_numeric_meaning(Some(meaning), Some(selected)).is_ok());
        assert!(reconcile_numeric_meaning(Some(meaning), None).is_err());
        assert!(reconcile_numeric_meaning(None, Some(selected)).is_err());
        let other = NumericIntrinsic::from_canonical_encoding([0, 6, 1, 0]).unwrap();
        assert!(reconcile_numeric_meaning(Some(other), Some(selected)).is_err());
        assert!(
            reconcile_numeric_meaning(
                Some(meaning),
                Some(CompilerIntrinsicExecutionIdentity::HostedExitProcessI32)
            )
            .is_err()
        );
        assert!(
            reconcile_numeric_meaning(
                None,
                Some(CompilerIntrinsicExecutionIdentity::HostedExitProcessI32)
            )
            .is_ok()
        );
        assert!(reconcile_numeric_meaning(None, None).is_ok());
        let format = meaning.builtin_format().unwrap();
        assert_eq!(format, semantic_vocabulary::IeeeFloatFormat::Binary64);
        assert!(matches!(
            compiler_builtin_execution(selected, Some(format)),
            Some(
                target_operations::CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                    operation: target_operations::IeeeFloatArithmeticOperation::IsNan,
                    format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                }
            )
        ));
    }
}
