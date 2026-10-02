//! Target-constrained proposals for compiler-owned native builtins.
//!
//! Provider selection and checked intrinsic review establish the exact
//! declaration/target identity. This module performs the later executable
//! projection: only compiler-intrinsic requirements actually called by the
//! canonical Terminal artifact receive a structural proposal. The consuming
//! lowerer independently accepts it through its local target catalog.

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

#[derive(Debug)]
pub struct CompilerIntrinsicSettlementProposal {
    pub requirement_identity: String,
    pub plan_index: usize,
    pub execution: target_operations::CompilerBuiltinExecution,
}

pub fn demanded_boundary_identities(
    module: &terminal_psi::TerminalModule,
) -> Result<BTreeSet<String>, Vec<Diagnostic>> {
    let declarations = module
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary.identity.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut demanded = BTreeSet::new();
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
            continue;
        };
        let Some(identity) = declarations.get(boundary).copied() else {
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
pub fn boundary_float_formats(
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
            Some((boundary.identity.clone(), format))
        })
        .collect()
}

struct SelectedIntrinsicRow<'a> {
    requirement_identity: &'a str,
    plan_index: usize,
    plan_name: &'a str,
    execution: Option<CompilerIntrinsicExecutionIdentity>,
}

/// Project called intrinsic rows into native catalog proposals. Provenance
/// stays borrowed and source-free; every plan is checked even without demand.
/// The receiving target catalog independently admits these proposals later.
pub fn derive_selected_intrinsic_settlement_proposals<'a>(
    plans: &[ProviderPlan],
    provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'a>>,
    demanded_boundaries: &BTreeSet<String>,
    float_formats: &BTreeMap<String, semantic_vocabulary::IeeeFloatFormat>,
) -> Result<Vec<CompilerIntrinsicSettlementProposal>, Vec<Diagnostic>> {
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
                    requirement_identity: &row.requirement_identity,
                    plan_index,
                    plan_name: &plan.name,
                    execution: *execution,
                });
            }
        }
    }
    // Keep the row/execution association from its producing plan. Sorting only
    // this borrowed selection leaves plan identity and row ownership unchanged.
    selected_rows
        .sort_unstable_by(|left, right| left.requirement_identity.cmp(right.requirement_identity));

    // Both sides are lexical. Each selected row is consumed once, including
    // complete duplicate groups, without allocating a match list per demand.
    let mut remaining_rows = selected_rows.as_slice();
    let mut evidence = Vec::new();
    let mut diagnostics = Vec::new();
    for requirement in demanded_boundaries {
        let preceding_count = remaining_rows
            .iter()
            .take_while(|row| row.requirement_identity < requirement.as_str())
            .count();
        remaining_rows = &remaining_rows[preceding_count..];
        let matching_count = remaining_rows
            .iter()
            .take_while(|row| row.requirement_identity == requirement)
            .count();
        let (matches, later_rows) = remaining_rows.split_at(matching_count);
        remaining_rows = later_rows;
        let [selected] = matches else {
            if !matches.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` resolves to {} selected compiler-intrinsic rows",
                    matches.len(),
                )));
            }
            continue;
        };
        let Some(execution) = selected.execution else {
            diagnostics.push(Diagnostic::error(format!(
                "selected compiler intrinsic `{}` for Terminal boundary `{requirement}` has no closed native catalog identity",
                selected.plan_name,
            )));
            continue;
        };
        let Some(execution) =
            compiler_builtin_execution(execution, float_formats.get(requirement).copied())
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected compiler intrinsic `{}` for Terminal boundary `{requirement}` has no native boundary realization",
                selected.plan_name,
            )));
            continue;
        };
        evidence.push(CompilerIntrinsicSettlementProposal {
            requirement_identity: requirement.clone(),
            plan_index: selected.plan_index,
            execution,
        });
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
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
        CompilerIntrinsicExecutionIdentity::HostedReadByte => {
            return Some(CompilerBuiltinExecution::HostedReadByte);
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            // Only arithmetic is call-demanded. Selected comparisons settle
            // their emitted Terminal IEEE comparisons, never a boundary call.
            use effects::CompilerPrimitiveFloatBinaryOperation as Primitive;
            let operation = match operation {
                Primitive::Add => Arithmetic::Add,
                Primitive::Subtract => Arithmetic::Subtract,
                Primitive::Multiply => Arithmetic::Multiply,
                Primitive::Divide => Arithmetic::Divide,
                Primitive::Equal
                | Primitive::NotEqual
                | Primitive::Less
                | Primitive::LessOrEqual
                | Primitive::Greater
                | Primitive::GreaterOrEqual => return None,
            };
            (operation, ieee_format(format))
        }
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            // Format-bearing identities must not borrow an unrelated boundary's
            // result format. Complete signature admission below remains required.
            match function {
                symbols::BuiltinFunction::FloatAddTowardZeroF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Add,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardZero,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatAddTowardZeroF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Add,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardZero,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatAddTowardPositiveF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Add,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatAddTowardPositiveF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Add,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatAddTowardNegativeF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Add,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatAddTowardNegativeF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Add,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatSubtractTowardZeroF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Subtract,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardZero,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatSubtractTowardZeroF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Subtract,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardZero,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatSubtractTowardPositiveF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Subtract,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatSubtractTowardPositiveF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Subtract,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatSubtractTowardNegativeF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Subtract,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatSubtractTowardNegativeF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Subtract,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatMultiplyTowardZeroF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Multiply,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardZero,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatMultiplyTowardZeroF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Multiply,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardZero,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatMultiplyTowardPositiveF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Multiply,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatMultiplyTowardPositiveF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Multiply,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatMultiplyTowardNegativeF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Multiply,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatMultiplyTowardNegativeF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Multiply,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatDivideTowardZeroF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Divide,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardZero,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatDivideTowardZeroF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Divide,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardZero,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatDivideTowardPositiveF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Divide,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatDivideTowardPositiveF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Divide,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatDivideTowardNegativeF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Divide,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatDivideTowardNegativeF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::DirectedBinary {
                            operation: target_operations::IeeeFloatBinaryOperation::Divide,
                            rounding: target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        },
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatFusedMultiplyAddF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::FusedMultiplyAdd,
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatFusedMultiplyAddF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::FusedMultiplyAdd,
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                symbols::BuiltinFunction::FloatMultiplyThenAddF32 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::MultiplyThenAdd,
                        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    });
                }
                symbols::BuiltinFunction::FloatMultiplyThenAddF64 => {
                    return Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: Arithmetic::MultiplyThenAdd,
                        format: semantic_vocabulary::IeeeFloatFormat::Binary64,
                    });
                }
                _ => {}
            }
            let operation = match function {
                symbols::BuiltinFunction::Min => Arithmetic::Minimum,
                symbols::BuiltinFunction::Max => Arithmetic::Maximum,
                symbols::BuiltinFunction::Sqrt => Arithmetic::SquareRoot,
                symbols::BuiltinFunction::FloatIsNan => Arithmetic::IsNan,
                symbols::BuiltinFunction::FloatIsInfinite => Arithmetic::IsInfinite,
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
mod tests;
