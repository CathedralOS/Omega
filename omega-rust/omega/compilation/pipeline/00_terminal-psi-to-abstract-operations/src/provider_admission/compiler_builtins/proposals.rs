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
use effects::provider_plan::ProviderPlan;
use std::collections::{BTreeMap, BTreeSet};

/// The source-free part of one retained provider review. The complete plan is
/// compared before its row-aligned execution identities can supply a proposal.
pub struct CompilerIntrinsicPlanProvenance<'a> {
    pub plan: &'a ProviderPlan,
    pub row_executions: &'a [Option<CompilerIntrinsicExecutionIdentity>],
}

pub(crate) fn demanded_boundary_identities(
    module: &terminal_psi::TerminalModule,
) -> Result<BTreeSet<String>, Vec<Diagnostic>> {
    let declarations = module
        .boundary_machines
        .iter()
        .map(|boundary| {
            (
                boundary.id,
                module
                    .identities
                    .text(boundary.identity)
                    .unwrap_or_default(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut demanded = BTreeSet::new();
    for boundary in module.blocks().flat_map(|block| {
        block
            .operations
            .iter()
            .filter_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::BoundaryCall { boundary, .. } => Some(*boundary),
                _ => None,
            })
            .chain(match &block.terminator {
                terminal_psi::Terminator::ExternalCompletion { boundary, .. } => Some(*boundary),
                _ => None,
            })
    }) {
        let Some(identity) = declarations.get(&boundary).copied() else {
            return Err(vec![Diagnostic::error(format!(
                "Terminal intrinsic settlement demand cites absent boundary {:?}",
                boundary,
            ))]);
        };
        demanded.insert(identity.to_owned());
    }
    Ok(demanded)
}

/// The boundary's IEEE format: prefer its result, otherwise its first IEEE
/// operand. Classification returns Bool, not the classified carrier, so
/// format-free builtin identities cannot infer their format from results alone.
/// This is only a proposal; admission independently checks the complete
/// signature, rejecting mixed carriers or a substituted result.
fn boundary_float_formats(
    identities: &terminal_psi::TerminalIdentityTable,
    boundaries: &[terminal_psi::BoundaryMachineDeclaration],
) -> BTreeMap<String, semantic_vocabulary::IeeeFloatFormat> {
    boundaries
        .iter()
        .filter_map(|boundary| {
            let format = match boundary.result {
                terminal_psi::BoundaryMachineResult::Scalar(
                    semantic_vocabulary::ScalarType::IeeeFloat(format),
                ) => Some(format),
                _ => boundary
                    .scalar_parameters
                    .iter()
                    .find_map(|parameter| match parameter {
                        semantic_vocabulary::ScalarType::IeeeFloat(format) => Some(*format),
                        _ => None,
                    }),
            }?;
            Some((
                identities
                    .text(boundary.identity)
                    .unwrap_or_default()
                    .to_string(),
                format,
            ))
        })
        .collect()
}

struct SelectedIntrinsicRow<'a> {
    requirement_identity: &'a str,
    plan_index: usize,
    execution: Option<CompilerIntrinsicExecutionIdentity>,
}

/// Project called intrinsic rows into native catalog proposals, each bound
/// once to its selected plan and the one Terminal boundary it settles.
/// Provenance stays borrowed and source-free; every plan is checked even
/// without demand. The receiving target catalog admits these proposals later.
pub(crate) fn derive_selected_intrinsic_settlement_proposals<'a>(
    selected: &effects::SelectedProviderPlanFacts,
    provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'a>>,
    demanded_boundaries: &BTreeSet<String>,
    boundaries: &[terminal_psi::BoundaryMachineDeclaration],
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<Vec<super::TerminalCompilerBuiltinProposal>, Vec<Diagnostic>> {
    let plans = selected.plans();
    let float_formats = boundary_float_formats(identities, boundaries);
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
        let settlement = match selected_row.execution {
            None => super::TerminalCompilerBuiltinSettlement::NoClosedCatalogIdentity,
            Some(execution) => {
                match compiler_builtin_execution(execution, float_formats.get(requirement).copied())
                {
                    Some(execution) => {
                        super::TerminalCompilerBuiltinSettlement::Execution(execution)
                    }
                    None => super::TerminalCompilerBuiltinSettlement::NoNativeBoundaryRealization,
                }
            }
        };
        evidence.push(super::TerminalCompilerBuiltinProposal::new(
            selected_row.requirement_identity.to_owned(),
            selected_row.plan_index,
            boundary.id,
            settlement,
        ));
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
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
/// catalog has one. `boundary_format` is the called boundary's IEEE
/// format, which the format-free builtin identities need; admission rejoins
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
