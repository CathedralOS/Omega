//! Target-constrained proposals for compiler-owned native builtins.
//!
//! Provider selection and checked intrinsic review establish the exact
//! declaration/target identity. This module performs the later executable
//! projection: only compiler-intrinsic requirements actually called by the
//! canonical Terminal artifact receive a structural proposal, bound here to
//! its selected plan and Terminal boundary. The consuming lowerer admits it
//! through its local target catalog.

use super::external_satisfaction::{reconcile_numeric_meaning, selected_execution};
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
    external_providers: &[terminal_psi::ExternalProviderConformance],
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
        let identity = match selected_execution(
            external_providers,
            identities,
            &boundary.owner,
            &plans[selected_row.plan_index],
            selected_row.row,
            selected_row.execution,
        ) {
            Ok(identity) => identity,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` {reason}"
                )));
                continue;
            }
        };
        if let Err(reason) = reconcile_numeric_meaning(boundary.numeric_intrinsic, identity) {
            diagnostics.push(Diagnostic::error(format!(
                "Terminal boundary `{requirement}` {reason}"
            )));
            continue;
        }
        let format = boundary
            .numeric_intrinsic
            .and_then(|meaning| meaning.builtin_format());
        let settlement = match identity {
            None => super::TerminalCompilerBuiltinSettlement::NoClosedCatalogIdentity,
            Some(identity) => match compiler_builtin_execution(identity, format) {
                Some(execution) => super::TerminalCompilerBuiltinSettlement::Execution {
                    identity,
                    execution,
                },
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

/// Format-bearing builtin symbols name an operation, a directed mode, and
/// one IEEE format. `nearest_arithmetic`, `nearest_directed_binary`, and
/// `nearest_directed_square_root` already build that execution row.
fn format_bearing_builtin(
    function: symbols::BuiltinFunction,
) -> Option<target_operations::CompilerBuiltinExecution> {
    use semantic_vocabulary::IeeeFloatFormat::{Binary32, Binary64};
    use symbols::BuiltinFunction as Function;
    use target_operations::IeeeFloatArithmeticOperation as Arithmetic;
    use target_operations::IeeeFloatBinaryOperation as Binary;
    use target_operations::IeeeFloatRoundingDirection as Rounding;

    let classify = Arithmetic::Classify;
    let fused = Arithmetic::FusedMultiplyAdd;
    let multiply_add = Arithmetic::MultiplyThenAdd;
    let directed =
        |operation, rounding, format| nearest_directed_binary(operation, rounding, format);
    let square_root = |rounding, format| nearest_directed_square_root(rounding, format);
    let arithmetic = |operation, format| nearest_arithmetic(operation, format);
    Some(match function {
        Function::FloatClassifyF32 => arithmetic(classify, Binary32),
        Function::FloatClassifyF64 => arithmetic(classify, Binary64),
        Function::FloatAddTowardZeroF32 => directed(Binary::Add, Rounding::TowardZero, Binary32),
        Function::FloatAddTowardZeroF64 => directed(Binary::Add, Rounding::TowardZero, Binary64),
        Function::FloatAddTowardPositiveF32 => {
            directed(Binary::Add, Rounding::TowardPositive, Binary32)
        }
        Function::FloatAddTowardPositiveF64 => {
            directed(Binary::Add, Rounding::TowardPositive, Binary64)
        }
        Function::FloatAddTowardNegativeF32 => {
            directed(Binary::Add, Rounding::TowardNegative, Binary32)
        }
        Function::FloatAddTowardNegativeF64 => {
            directed(Binary::Add, Rounding::TowardNegative, Binary64)
        }
        Function::FloatSubtractTowardZeroF32 => {
            directed(Binary::Subtract, Rounding::TowardZero, Binary32)
        }
        Function::FloatSubtractTowardZeroF64 => {
            directed(Binary::Subtract, Rounding::TowardZero, Binary64)
        }
        Function::FloatSubtractTowardPositiveF32 => {
            directed(Binary::Subtract, Rounding::TowardPositive, Binary32)
        }
        Function::FloatSubtractTowardPositiveF64 => {
            directed(Binary::Subtract, Rounding::TowardPositive, Binary64)
        }
        Function::FloatSubtractTowardNegativeF32 => {
            directed(Binary::Subtract, Rounding::TowardNegative, Binary32)
        }
        Function::FloatSubtractTowardNegativeF64 => {
            directed(Binary::Subtract, Rounding::TowardNegative, Binary64)
        }
        Function::FloatMultiplyTowardZeroF32 => {
            directed(Binary::Multiply, Rounding::TowardZero, Binary32)
        }
        Function::FloatMultiplyTowardZeroF64 => {
            directed(Binary::Multiply, Rounding::TowardZero, Binary64)
        }
        Function::FloatMultiplyTowardPositiveF32 => {
            directed(Binary::Multiply, Rounding::TowardPositive, Binary32)
        }
        Function::FloatMultiplyTowardPositiveF64 => {
            directed(Binary::Multiply, Rounding::TowardPositive, Binary64)
        }
        Function::FloatMultiplyTowardNegativeF32 => {
            directed(Binary::Multiply, Rounding::TowardNegative, Binary32)
        }
        Function::FloatMultiplyTowardNegativeF64 => {
            directed(Binary::Multiply, Rounding::TowardNegative, Binary64)
        }
        Function::FloatDivideTowardZeroF32 => {
            directed(Binary::Divide, Rounding::TowardZero, Binary32)
        }
        Function::FloatDivideTowardZeroF64 => {
            directed(Binary::Divide, Rounding::TowardZero, Binary64)
        }
        Function::FloatDivideTowardPositiveF32 => {
            directed(Binary::Divide, Rounding::TowardPositive, Binary32)
        }
        Function::FloatDivideTowardPositiveF64 => {
            directed(Binary::Divide, Rounding::TowardPositive, Binary64)
        }
        Function::FloatDivideTowardNegativeF32 => {
            directed(Binary::Divide, Rounding::TowardNegative, Binary32)
        }
        Function::FloatDivideTowardNegativeF64 => {
            directed(Binary::Divide, Rounding::TowardNegative, Binary64)
        }
        Function::FloatSqrtTowardZeroF32 => square_root(Rounding::TowardZero, Binary32),
        Function::FloatSqrtTowardZeroF64 => square_root(Rounding::TowardZero, Binary64),
        Function::FloatSqrtTowardPositiveF32 => square_root(Rounding::TowardPositive, Binary32),
        Function::FloatSqrtTowardPositiveF64 => square_root(Rounding::TowardPositive, Binary64),
        Function::FloatSqrtTowardNegativeF32 => square_root(Rounding::TowardNegative, Binary32),
        Function::FloatSqrtTowardNegativeF64 => square_root(Rounding::TowardNegative, Binary64),
        Function::FloatFusedMultiplyAddF32 => arithmetic(fused, Binary32),
        Function::FloatFusedMultiplyAddF64 => arithmetic(fused, Binary64),
        Function::FloatMultiplyThenAddF32 => arithmetic(multiply_add, Binary32),
        Function::FloatMultiplyThenAddF64 => arithmetic(multiply_add, Binary64),
        _ => return None,
    })
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
            if let Some(execution) = format_bearing_builtin(function) {
                return Some(execution);
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

#[cfg(test)]
mod tests {
    use super::*;
    use language_semantics::numeric_intrinsics::NumericIntrinsic;

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
