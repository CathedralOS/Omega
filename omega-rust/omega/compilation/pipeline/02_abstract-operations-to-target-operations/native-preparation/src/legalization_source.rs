/*
We project validated target functions and their instruction/terminator graphs
into canonical `LegalizedScalarFunction` rosters (`derive_source_function_rosters`)
for consumption by instruction selection.
*/

use crate::LegalizationError;
use abstract_operations::AbstractOperationPlan;
use optimization_unit::PsiOptimizationUnit;
use target_operations::TargetOperationPlan;
pub(crate) mod custody {
    use crate::{LegalizationError, LegalizationError as Error};
    use target_operations::TargetOperationPlan;

    pub(crate) fn validate_source_custody(
        target: &TargetOperationPlan,
        abstract_plan: &crate::native_geometry::NativePlan<'_>,
        unit: &crate::native_geometry::NativeUnit<'_>,
    ) -> Result<(), LegalizationError> {
        if target.psi != abstract_plan.psi
            || target.psi != unit.psi
            || target.entry != abstract_plan.entry
            || target.entry != unit.entry
            || target.functions.len() != abstract_plan.functions.len()
            || target.functions.len() != unit.functions.len()
        {
            return Err(Error::custody());
        }
        Ok(())
    }
}
pub(crate) mod audit_roster {
    use super::TargetOperationPlan;
    use crate::LegalizationError as Error;
    use crate::LegalizationError;
    use crate::legalization::source::SourceFunctionRosters;

    pub(crate) fn derive_remaining(
        rosters: &mut SourceFunctionRosters,
        target: &TargetOperationPlan,
        abstract_plan: &crate::native_geometry::NativePlan<'_>,
        unit: &crate::native_geometry::NativeUnit<'_>,
    ) -> Result<(), LegalizationError> {
        if target.functions.len() != unit.functions.len() {
            return Err(Error::custody());
        }
        // Both stages retain the admitted function order. Join once instead of
        // scanning two whole-program body rosters for every function.
        for (target_function, optimized) in target.functions.iter().zip(&unit.functions) {
            if target_function.machine != optimized.machine {
                return Err(Error::custody());
            }
            let mut current_operation = None;
            let function = super::scalar_graph::derive(
                target_function,
                optimized,
                target,
                abstract_plan,
                unit,
                &mut current_operation,
            )
            .map_err(|error| error.attributed(target_function.machine, current_operation))?;
            rosters.scalar_functions.push(function);
        }
        Ok(())
    }
}
pub(crate) mod scalar_graph {
    use crate::legalization::scalar_graph_input;
    use crate::{LegalizationError, LegalizationError as Error};
    use legalized_operations::{
        LegalizedScalarBlock, LegalizedScalarFunction, LegalizedScalarParameter,
    };
    use target_operations::TargetOperationPlan;
    pub(crate) mod instruction {
        use super::{Error, TargetOperationPlan};
        use crate::legalization::runtime_indices::projection;
        use crate::structural_inputs::structural_reference_input::reference_source_geometry;
        use abstract_operations::AbstractOperation;
        use legalized_operations::{
            LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedValueDefinition,
        };
        use scalar_graph_input::aggregate_results::home_layout;
        pub(crate) mod call_instructions {
            use super::super::{Error, TargetOperationPlan};
            use crate::LegalizationError;
            use crate::legalization::scalar_graph_input;
            use abstract_operations::AbstractOperation;
            use legalized_operations::{
                LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarInstructionKind,
            };
            use scalar_graph_input::normalized_foreign::native_callbacks_at;
            use semantic_vocabulary::OperationId;

            pub(crate) fn project_call(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::Call {
                    psi_operation,
                    origin,
                    target,
                    call_plan,
                    result,
                    scalar_arguments,
                    arguments,
                    access_bindings,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                let mut lowered = scalar_arguments
                    .iter()
                    .map(|argument| LegalizedScalarArgument::Scalar {
                        source: argument.source.source_value(),
                        placement: argument.placement.clone(),
                    })
                    .collect::<Vec<_>>();
                for argument in arguments {
                    lowered.push(LegalizedScalarArgument::Structural {
                        indices: crate::legalization::runtime_indices::retained_operands(
                            facts,
                            machine,
                            *psi_operation,
                            &argument.indices,
                        )?,
                        semantic: terminal_psi::StructuralArgument {
                            place: argument.place,
                            path: argument.path.clone(),
                            access: argument.access,
                        },
                        target: argument.clone(),
                    });
                }
                let structural_result = match result {
                    target_operations::TargetCallResult::Structural { result, .. } => {
                        Some(result.clone())
                    }
                    _ => None,
                };
                let mut descriptor_origins = match result {
                    target_operations::TargetCallResult::Structural {
                        descriptor_origins, ..
                    } => descriptor_origins.clone(),
                    _ => Vec::new(),
                };
                if let target_operations::TargetCallResult::Structural {
                    reference_results, ..
                } = result
                {
                    descriptor_origins.extend(reference_results.iter().flat_map(|reference| {
                        reference.origins.iter().map(|origin| {
                            let Some((root, path, exact, indirect)) = origin.storage_envelope()
                            else {
                                return legalized_operations::LegalizedDescriptorOrigin::Opaque;
                            };
                            legalized_operations::LegalizedDescriptorOrigin::Argument {
                                source: terminal_psi::StructuralArgument {
                                    place: root,
                                    path,
                                    access: terminal_psi::StructuralAccess::SharedBorrow,
                                },
                                exact,
                                indirect,
                            }
                        })
                    }));
                }
                Ok(LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
                    descriptor_origins,
                    access_bindings: access_bindings.clone(),
                    target: *target,
                    arguments: lowered,
                    structural_result,
                    result_placement: call_plan.result.clone(),
                    call_plan: call_plan.clone(),
                    source: origin.clone(),
                    claim_transfers: claim_transfers.clone(),
                    requirement_obligations: requirement_obligations.clone(),
                    crash_continuations: crash_continuations.clone(),
                }))
            }

            pub(crate) fn project_boundary_call(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                native: &TargetOperationPlan,
                operation: OperationId,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::BoundaryCall {
                    boundary,
                    arguments,
                    ..
                } = &node.operation
                else {
                    unreachable!("dispatched project_boundary_call")
                };
                let kind = {
                    let [source] = arguments.as_slice() else {
                        return Err(Error::custody());
                    };
                    match scalar_graph_input::hosted_realization(
                        native,
                        optimized.machine,
                        operation,
                    )? {
                        (
                            execution,
                            target_operations::BoundaryRealization::HostedWriteByteI32(_),
                        ) => LegalizedScalarInstructionKind::HostedWriteByteI32 {
                            boundary: *boundary,
                            execution,
                            source: *source,
                        },
                        _ => return Err(Error::custody()),
                    }
                };
                Ok(kind)
            }

            pub(crate) fn project_normalized_foreign_call(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                native: &TargetOperationPlan,
                operation: OperationId,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::NormalizedForeignCall {
                    psi_operation,
                    boundary,
                    provider_execution,
                    binding,
                    scalar_arguments,
                    structural_arguments,
                    result,
                    descriptor_input,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                if *psi_operation != operation {
                    return Err(Error::custody());
                }
                let callbacks =
                    native_callbacks_at(native, operation, &binding.boundary_entry_plan)?
                        .into_iter()
                        .cloned()
                        .collect();
                Ok(LegalizedScalarInstructionKind::NormalizedForeignCall(
                    legalized_operations::LegalizedNormalizedForeignCall {
                        boundary: *boundary,
                        provider_execution: *provider_execution,
                        binding: binding.clone(),
                        scalar_arguments: scalar_arguments.clone(),
                        structural_indices: structural_arguments
                            .iter()
                            .map(|argument| {
                                crate::legalization::runtime_indices::retained_operands(
                                    facts,
                                    machine,
                                    operation,
                                    &argument.indices,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                        structural_arguments: structural_arguments.clone(),
                        result: result.clone(),
                        callbacks,
                        descriptor_input: *descriptor_input,
                    },
                ))
            }
        }
        pub(crate) mod scalar_instructions {
            use super::super::Error;
            use crate::LegalizationError;
            use crate::legalization::accepted_fact;
            use crate::legalization::facts;
            use crate::legalization::references_obligation;
            use crate::legalization::scalar_graph_input;
            use abstract_operations::{
                AbstractOperation, IntegerArithmetic, IntegerArithmeticOperator,
                IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy,
            };
            use legalized_operations::{
                LegalizedExactIntegerOperator, LegalizedScalarComparison,
                LegalizedScalarInstructionKind,
            };
            use semantic_vocabulary::ScalarType;

            pub(crate) fn project_integer_exact_cast(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::IntegerExactCast {
                    psi_operation,
                    operand,
                    source_type,
                    obligation,
                    ..
                } = &node.operation
                else {
                    unreachable!("dispatched project_integer_exact_cast")
                };
                let kind = {
                    let accepted_fact =
                        accepted_fact(unit, optimized, *psi_operation, *obligation)?;
                    if !references_obligation(optimized, *psi_operation, *obligation) {
                        return Err(Error::custody());
                    }
                    LegalizedScalarInstructionKind::IntegerExactCast {
                        operand: *operand,
                        source_type: unit.closed_integer(*source_type).ok_or(Error::custody())?,
                        obligation: *obligation,
                        accepted_fact,
                    }
                };
                Ok(kind)
            }

            #[derive(Clone, Copy)]
            pub(crate) struct IntegerArithmeticNode {
                pub(crate) psi_operation: semantic_vocabulary::OperationId,
                pub(crate) scalar_type: semantic_vocabulary::IntegerType,
                pub(crate) arithmetic: IntegerArithmetic,
                pub(crate) left: semantic_vocabulary::ValueId,
                pub(crate) right: semantic_vocabulary::ValueId,
            }

            pub(crate) fn project_integer_arithmetic(
                node: IntegerArithmeticNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                use IntegerArithmeticOperator as Operator;
                use IntegerOverflowPolicy as Policy;
                let IntegerArithmeticNode { left, right, .. } = node;
                match (node.arithmetic.policy(), node.arithmetic.operator()) {
                    (Policy::Wrapping, Operator::Add) => {
                        Ok(LegalizedScalarInstructionKind::WrappingAdd { left, right })
                    }
                    (Policy::Wrapping, Operator::Subtract) => {
                        Ok(LegalizedScalarInstructionKind::WrappingSubtract { left, right })
                    }
                    (Policy::Wrapping, Operator::Multiply) => {
                        Ok(LegalizedScalarInstructionKind::WrappingMultiply { left, right })
                    }
                    (Policy::Wrapping, Operator::Divide | Operator::Remainder) => {
                        project_wrapping_integer_division(node, optimized, unit)
                    }
                    (
                        Policy::Saturating,
                        Operator::Add | Operator::Subtract | Operator::Multiply,
                    ) => project_saturating_integer_arithmetic(node, optimized),
                    (Policy::Saturating, Operator::Divide | Operator::Remainder) => {
                        project_saturating_integer_division(node, optimized, unit)
                    }
                    (Policy::Exact, _) => project_exact_integer_arithmetic(node, optimized, unit),
                }
            }

            fn project_saturating_integer_arithmetic(
                node: IntegerArithmeticNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let IntegerArithmeticNode {
                    scalar_type,
                    arithmetic,
                    left,
                    right,
                    ..
                } = node;
                let carrier =
                    scalar_graph_input::saturating_carrier(scalar_type).ok_or(Error::custody())?;
                if [left, right].iter().any(|value| {
                    scalar_graph_input::value_type(optimized, *value)
                        != Some(ScalarType::Integer(scalar_type))
                }) {
                    return Err(Error::custody());
                }
                Ok(match arithmetic.operator() {
                    IntegerArithmeticOperator::Add => {
                        LegalizedScalarInstructionKind::SaturatingAdd {
                            carrier,
                            left,
                            right,
                        }
                    }
                    IntegerArithmeticOperator::Subtract => {
                        LegalizedScalarInstructionKind::SaturatingSubtract {
                            carrier,
                            left,
                            right,
                        }
                    }
                    _ => LegalizedScalarInstructionKind::SaturatingMultiply {
                        carrier,
                        left,
                        right,
                    },
                })
            }

            pub(crate) fn project_trapping_integer(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::TrappingInteger {
                    scalar_type,
                    operand_type,
                    operation,
                    ..
                } = &node.operation
                else {
                    unreachable!("dispatched project_trapping_integer");
                };
                let form = scalar_graph_input::trapping_form(
                    *scalar_type,
                    *operand_type,
                    operation.primitive(),
                )
                .ok_or(Error::custody())?;
                if !scalar_graph_input::trapping_operands_rejoin(
                    optimized,
                    *scalar_type,
                    *operand_type,
                    *operation,
                ) {
                    return Err(Error::custody());
                }
                Ok(match *operation {
                    terminal_psi::TrappingIntegerOperation::Convert { operand } => {
                        LegalizedScalarInstructionKind::TrappingConvert {
                            form,
                            source: scalar_graph_input::saturating_carrier(*operand_type)
                                .ok_or(Error::custody())?,
                            operand,
                        }
                    }
                    binary => {
                        let [left, right] = binary.operands()[..] else {
                            return Err(Error::custody());
                        };
                        LegalizedScalarInstructionKind::TrappingBinary { form, left, right }
                    }
                })
            }

            fn project_saturating_integer_division(
                node: IntegerArithmeticNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let IntegerArithmeticNode {
                    psi_operation,
                    scalar_type,
                    arithmetic,
                    left,
                    right,
                } = node;
                let obligation = arithmetic.obligation().ok_or(Error::custody())?;
                let carrier =
                    scalar_graph_input::saturating_carrier(scalar_type).ok_or(Error::custody())?;
                if [left, right].iter().any(|value| {
                    scalar_graph_input::value_type(optimized, *value)
                        != Some(ScalarType::Integer(scalar_type))
                }) {
                    return Err(Error::custody());
                }
                let accepted_fact =
                    accepted_nonzero_divisor_fact(optimized, unit, psi_operation, obligation)?;
                Ok(
                    if arithmetic.operator() == IntegerArithmeticOperator::Divide {
                        LegalizedScalarInstructionKind::SaturatingDivide {
                            carrier,
                            left,
                            right,
                            obligation,
                            accepted_fact,
                        }
                    } else {
                        LegalizedScalarInstructionKind::SaturatingRemainder {
                            carrier,
                            left,
                            right,
                            obligation,
                            accepted_fact,
                        }
                    },
                )
            }

            fn accepted_nonzero_divisor_fact(
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
                psi_operation: semantic_vocabulary::OperationId,
                obligation: semantic_vocabulary::ObligationId,
            ) -> Result<optimization_core::AcceptedObligationFactIdentity, LegalizationError>
            {
                let mut facts = facts(unit, optimized, psi_operation, obligation);
                let fact = facts.next().ok_or_else(Error::custody)?;
                if facts.next().is_some()
                    || !references_obligation(optimized, psi_operation, obligation)
                {
                    return Err(Error::custody());
                }
                Ok(fact.identity)
            }

            fn project_wrapping_integer_division(
                node: IntegerArithmeticNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let IntegerArithmeticNode {
                    psi_operation,
                    scalar_type,
                    arithmetic,
                    left,
                    right,
                } = node;
                let obligation = arithmetic.obligation().ok_or(Error::custody())?;
                wrapping_division_operands(optimized, scalar_type, left, right)?;
                let accepted_fact =
                    accepted_nonzero_divisor_fact(optimized, unit, psi_operation, obligation)?;
                Ok(
                    if arithmetic.operator() == IntegerArithmeticOperator::Divide {
                        LegalizedScalarInstructionKind::WrappingDivide {
                            left,
                            right,
                            obligation,
                            accepted_fact,
                        }
                    } else {
                        LegalizedScalarInstructionKind::WrappingRemainder {
                            left,
                            right,
                            obligation,
                            accepted_fact,
                        }
                    },
                )
            }

            fn wrapping_division_operands(
                optimized: &optimization_unit::PsiOptimizationFunction,
                scalar_type: semantic_vocabulary::IntegerType,
                left: semantic_vocabulary::ValueId,
                right: semantic_vocabulary::ValueId,
            ) -> Result<(), LegalizationError> {
                if !scalar_graph_input::supports_wrapping_division(scalar_type)
                    || [left, right].iter().any(|value| {
                        scalar_graph_input::value_type(optimized, *value)
                            != Some(ScalarType::Integer(scalar_type))
                    })
                {
                    return Err(Error::custody());
                }
                Ok(())
            }

            pub(crate) fn project_shift(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::IntegerShift {
                    psi_operation,
                    value_type,
                    count_type,
                    policy,
                    direction,
                    value,
                    count,
                    ..
                } = node.operation
                else {
                    unreachable!("dispatched project_shift")
                };
                if scalar_graph_input::scalar_shape(ScalarType::Integer(value_type)).is_none()
                    || scalar_graph_input::scalar_shape(ScalarType::Integer(count_type)).is_none()
                    || scalar_graph_input::value_type(optimized, value)
                        != Some(ScalarType::Integer(value_type))
                    || scalar_graph_input::value_type(optimized, count)
                        != Some(ScalarType::Integer(count_type))
                {
                    return Err(Error::custody());
                }
                let IntegerShiftPolicy::Exact { obligation } = policy else {
                    return Ok(match direction {
                        IntegerShiftDirection::Left => {
                            LegalizedScalarInstructionKind::WrappingShiftLeft { value, count }
                        }
                        IntegerShiftDirection::Right => {
                            LegalizedScalarInstructionKind::WrappingShiftRight { value, count }
                        }
                    });
                };
                let accepted_fact = {
                    let accepted_fact = accepted_fact(unit, optimized, psi_operation, obligation)?;
                    if !references_obligation(optimized, psi_operation, obligation) {
                        return Err(Error::custody());
                    }
                    accepted_fact
                };
                Ok(match direction {
                    IntegerShiftDirection::Left => LegalizedScalarInstructionKind::ExactShiftLeft {
                        value,
                        count,
                        obligation,
                        accepted_fact,
                    },
                    IntegerShiftDirection::Right => {
                        LegalizedScalarInstructionKind::ExactShiftRight {
                            value,
                            count,
                            obligation,
                            accepted_fact,
                        }
                    }
                })
            }

            fn project_exact_integer_arithmetic(
                node: IntegerArithmeticNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let IntegerArithmeticNode {
                    psi_operation,
                    arithmetic,
                    left,
                    right,
                    ..
                } = node;
                let obligation = arithmetic.obligation().ok_or(Error::custody())?;
                let accepted_fact = accepted_fact(unit, optimized, psi_operation, obligation)?;
                if !references_obligation(optimized, psi_operation, obligation) {
                    return Err(Error::custody());
                }
                Ok(LegalizedScalarInstructionKind::ExactBinary {
                    operator: match arithmetic.operator() {
                        IntegerArithmeticOperator::Add => LegalizedExactIntegerOperator::Add,
                        IntegerArithmeticOperator::Subtract => {
                            LegalizedExactIntegerOperator::Subtract
                        }
                        IntegerArithmeticOperator::Multiply => {
                            LegalizedExactIntegerOperator::Multiply
                        }
                        IntegerArithmeticOperator::Divide => LegalizedExactIntegerOperator::Divide,
                        IntegerArithmeticOperator::Remainder => {
                            LegalizedExactIntegerOperator::Remainder
                        }
                    },
                    left,
                    right,
                    obligation,
                    accepted_fact,
                })
            }

            pub(crate) fn project_boolean_equal(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let (AbstractOperation::BooleanEqual { left, right, .. }
                | abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
                | AbstractOperation::IntegerLessOrEqual { left, right, .. }) = &node.operation
                else {
                    unreachable!("dispatched project_boolean_equal")
                };
                let kind = {
                    let predicate = match node.operation {
                        AbstractOperation::BooleanEqual { .. }
                        | AbstractOperation::IntegerEqual { .. } => {
                            LegalizedScalarComparison::Equal
                        }
                        AbstractOperation::IntegerLessThan { .. } => {
                            LegalizedScalarComparison::LessThan
                        }
                        AbstractOperation::IntegerLessOrEqual { .. } => {
                            LegalizedScalarComparison::LessOrEqual
                        }
                        _ => return Err(Error::custody()),
                    };
                    let operand_type = scalar_graph_input::value_type(optimized, *left)
                        .and_then(|scalar| unit.closed_scalar(scalar))
                        .ok_or(Error::custody())?;
                    let right_operand_type = scalar_graph_input::value_type(optimized, *right)
                        .and_then(|scalar| unit.closed_scalar(scalar))
                        .ok_or(Error::custody())?;
                    LegalizedScalarInstructionKind::Compare {
                        predicate,
                        operand_type,
                        right_operand_type,
                        left: *left,
                        right: *right,
                    }
                };
                Ok(kind)
            }
        }
        pub(crate) mod storage_instructions {
            use super::super::Error;
            use crate::LegalizationError;
            use crate::legalization::accepted_fact;
            use crate::legalization::scalar_graph_input;
            use crate::structural_inputs::structural_reference_input::storage_address_geometry;
            use abstract_operations::AbstractOperation;
            use legalized_operations::LegalizedScalarInstructionKind;
            use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

            macro_rules! o {
                ($optimized:expr => $unit:expr => $operation:expr => $elements:expr) => {
                    crate::legalization::runtime_indices::operands(
                        $optimized, $unit, $operation, $elements,
                    )
                };
            }

            /// A tag test reads its source, path, tag geometry and selector
            /// operands from the retained target row. Rejoining them to the
            /// earlier body belongs to the independent correspondence audit.
            pub(crate) fn project_structural_case_membership(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StructuralCaseMembership {
                    psi_operation,
                    result,
                    source,
                    path,
                    case,
                    case_tag,
                    tag_byte_offset,
                    indices,
                    pointer_slots,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                if result.scalar_type != ScalarType::Boolean
                    || pointer_slots.iter().any(|slot| {
                        slot.scalar_descriptor
                            || slot.index_start > slot.index_end
                            || usize::try_from(slot.index_end)
                                .ok()
                                .is_none_or(|end| end > indices.len())
                    })
                {
                    return Err(Error::custody());
                }
                Ok(LegalizedScalarInstructionKind::StructuralCaseMembership {
                    source: *source,
                    path: path.clone(),
                    case: *case,
                    case_tag: *case_tag,
                    tag_byte_offset: *tag_byte_offset,
                    indices: crate::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                    pointer_slots: pointer_slots.clone(),
                })
            }

            /// Every leaf copy (a moved field, a whole value, a transformed
            /// value or an empty-run boundary release) keeps its result home,
            /// source path, offset and selector operands on the target row.
            pub(crate) fn project_structural_leaf_copy(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StructuralLeafCopy {
                    psi_operation,
                    result_home,
                    source,
                    path,
                    byte_offset,
                    indices,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                let Some((operation, result)) = result_home.operation_result() else {
                    return Err(Error::custody());
                };
                if operation != *psi_operation {
                    return Err(Error::custody());
                }
                Ok(LegalizedScalarInstructionKind::StructuralLeafCopy {
                    result: result.clone(),
                    source: *source,
                    path: path.clone(),
                    byte_offset: *byte_offset,
                    shape: result_home.layout.shape(),
                    indices: crate::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                })
            }

            /// A field store keeps its window, offset, value type and selector
            /// operands on the target row; its shape comes from the shared
            /// type catalog, not from the value's earlier definition.
            pub(crate) fn project_structural_field_store(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                types: &crate::native_geometry::NativeGeometry<'_>,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StoreStructuralField {
                    psi_operation,
                    destination,
                    path,
                    field,
                    value,
                    value_type,
                    byte_offset,
                    indices,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(LegalizedScalarInstructionKind::StoreStructuralField {
                    destination: destination.clone(),
                    path: path.clone(),
                    field: *field,
                    value: *value,
                    byte_offset: *byte_offset,
                    shape: crate::structural_inputs::shape(*value_type, types)
                        .ok_or(Error::custody())?,
                    indices: crate::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                })
            }

            /// Length observations use the source, offset and selectors retained
            /// by target lowering. The independent audit reconstructs the carrier
            /// and geometry from the earlier program.
            pub(crate) fn project_sequence_length(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                operation: semantic_vocabulary::OperationId,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                use target_operations::{
                    TargetIntegerExpression, TargetScalarExpression, TargetUnitOperation,
                };

                if let TargetUnitOperation::StructuralByteSequenceFieldLength {
                    psi_operation,
                    source,
                    field,
                    indices,
                    ..
                } = targeted
                {
                    if *psi_operation != operation {
                        return Err(Error::custody());
                    }
                    return Ok(
                        LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                            source: source.clone(),
                            field: *field,
                            indices: crate::legalization::runtime_indices::retained_operands(
                                facts, machine, operation, indices,
                            )?,
                        },
                    );
                }
                let TargetUnitOperation::ScalarDefinition {
                    result_home,
                    expression:
                        TargetScalarExpression::Integer {
                            scalar_type,
                            expression,
                        },
                } = targeted
                else {
                    return Err(Error::custody());
                };
                if result_home.defining_operation != operation
                    || result_home.scalar_type != ScalarType::Integer(*scalar_type)
                    || scalar_type.sign() != IntegerSign::Unsigned
                    || scalar_type.bits() != Some(64)
                {
                    return Err(Error::custody());
                }

                let retained_indices =
                    |indices: &[target_operations::TargetStructuralRuntimeIndex]| {
                        crate::legalization::runtime_indices::retained_operands(
                            facts, machine, operation, indices,
                        )
                    };
                Ok(match expression {
                    TargetIntegerExpression::Immediate {
                        source_value,
                        value: IntegerValue::Unsigned(value),
                    } if *source_value == result_home.source_value
                        && u64::try_from(*value).is_ok() =>
                    {
                        LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(*value))
                    }
                    TargetIntegerExpression::ByteSequenceLength {
                        psi_operation,
                        source_value,
                        source,
                        indices,
                        length_byte_offset,
                    } if *psi_operation == operation
                        && *source_value == result_home.source_value
                        && matches!(*length_byte_offset, 0 | 8) =>
                    {
                        LegalizedScalarInstructionKind::ByteSequenceLength {
                            source: source.clone(),
                            indices: retained_indices(indices)?,
                            length_byte_offset: *length_byte_offset,
                        }
                    }
                    TargetIntegerExpression::ElementViewLength {
                        psi_operation,
                        source_value,
                        source,
                        indices,
                        length_byte_offset,
                    } if *psi_operation == operation
                        && *source_value == result_home.source_value
                        && matches!(*length_byte_offset, 0 | 8) =>
                    {
                        LegalizedScalarInstructionKind::ElementViewLength {
                            source: source.clone(),
                            indices: retained_indices(indices)?,
                            length_byte_offset: *length_byte_offset,
                        }
                    }
                    _ => return Err(Error::custody()),
                })
            }

            pub(crate) fn project_primitive_scalar_read(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::PrimitiveScalarRead {
                    psi_operation,
                    source,
                    path,
                    indices,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                // The producer retained the selector's scalar source, stride,
                // extent and proof key. Source reconstruction belongs only to
                // the independent correspondence audit.
                Ok(LegalizedScalarInstructionKind::PrimitiveScalarRead {
                    source: *source,
                    path: path.clone(),
                    indices: crate::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                })
            }

            pub(crate) fn project_write_only_primitive_store(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
                    psi_operation,
                    destination,
                    path,
                    source,
                    byte_offset,
                    byte_size,
                    indices,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                    destination: destination.clone(),
                    path: path.clone(),
                    value: abstract_operations::AbstractResult {
                        value: source.source_value(),
                        scalar_type: source.scalar_type(),
                    },
                    byte_offset: *byte_offset,
                    byte_size: u8::try_from(*byte_size).map_err(|_| Error::custody())?,
                    indices: crate::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                })
            }

            pub(crate) fn project_structural_scalar_field_store(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StructuralScalarFieldStore {
                    psi_operation,
                    destination,
                    path,
                    field,
                    field_byte_offset,
                    field_byte_size,
                    source,
                    indices,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                    destination: destination.clone(),
                    path: path.clone(),
                    field: *field,
                    value: abstract_operations::AbstractResult {
                        value: source.source_value(),
                        scalar_type: source.scalar_type(),
                    },
                    byte_offset: *field_byte_offset,
                    byte_size: *field_byte_size,
                    indices: crate::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                })
            }

            fn view_indices(
                function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
                unit: &crate::native_geometry::NativeUnit<'_>,
                psi_operation: semantic_vocabulary::OperationId,
                view: &terminal_psi::StructuralArgument,
            ) -> Result<Vec<legalized_operations::LegalizedRuntimeIndexOperand>, LegalizationError>
            {
                let optimized = function_inputs.function();
                o!(optimized => unit => psi_operation => &scalar_graph_input::element_views::descriptor_geometry(
                        function_inputs,
                        view,
                        unit.geometry(),
                    )
                    .ok_or(LegalizationError::custody())?
                    .elements)
            }

            pub(crate) fn project_subslice(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                use target_operations::{TargetByteView, TargetElementView, TargetUnitOperation};
                let (result, psi_operation, place, source, indices, start, end, length, obligation) =
                    match targeted {
                        TargetUnitOperation::ByteSequenceSubslice {
                            result,
                            view:
                                TargetByteView::Subslice {
                                    psi_operation,
                                    place,
                                    source,
                                    indices,
                                    start,
                                    end,
                                    length,
                                    obligation,
                                },
                        }
                        | TargetUnitOperation::ElementViewSubslice {
                            result,
                            view:
                                TargetElementView::Subslice {
                                    psi_operation,
                                    place,
                                    source,
                                    indices,
                                    start,
                                    end,
                                    length,
                                    obligation,
                                },
                        } => (
                            result,
                            *psi_operation,
                            *place,
                            source,
                            indices,
                            start.operand_value().ok_or(Error::custody())?,
                            end.operand_value().ok_or(Error::custody())?,
                            *length,
                            *obligation,
                        ),
                        _ => return Err(Error::custody()),
                    };
                if place != result.place {
                    return Err(Error::custody());
                }
                let indices = crate::legalization::runtime_indices::retained_operands(
                    facts,
                    machine,
                    psi_operation,
                    indices,
                )?;
                let accepted_fact = crate::legalization::obligations::retained_fact(
                    facts,
                    machine,
                    psi_operation,
                    obligation,
                )?;
                let kind = if matches!(targeted, TargetUnitOperation::ElementViewSubslice { .. }) {
                    LegalizedScalarInstructionKind::ElementViewSubslice {
                        result: result.clone(),
                        source: source.clone(),
                        indices,
                        start,
                        end,
                        length,
                        obligation,
                        accepted_fact,
                    }
                } else {
                    LegalizedScalarInstructionKind::ByteSequenceSubslice {
                        result: result.clone(),
                        source: source.clone(),
                        indices,
                        start,
                        end,
                        length,
                        obligation,
                        accepted_fact,
                    }
                };
                Ok(kind)
            }

            pub(crate) fn project_established_view(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                use target_operations::{
                    TargetElementView, TargetElementViewRegion, TargetUnitOperation,
                };
                let TargetUnitOperation::EstablishElementView {
                    result,
                    view:
                        TargetElementView::Established {
                            psi_operation,
                            place,
                            structural_type,
                            source_argument,
                            element,
                            indices,
                            region,
                            ..
                        },
                } = targeted
                else {
                    return Err(Error::custody());
                };
                if *place != result.place || *structural_type != result.structural_type {
                    return Err(Error::custody());
                }
                // The source argument and endpoint identities belong to this
                // view, alongside its physical backing recipe. Never recover
                // them from the body that happened to produce the view.
                let region = match region {
                    TargetElementViewRegion::WholeCollection => {
                        terminal_psi::ElementViewRegion::WholeCollection
                    }
                    TargetElementViewRegion::WholeRepresentation {
                        source_length,
                        obligation,
                    } => terminal_psi::ElementViewRegion::WholeRepresentation {
                        source_length: *source_length,
                        obligation: *obligation,
                    },
                    TargetElementViewRegion::RepresentationBytes {
                        start,
                        end,
                        obligation,
                    } => terminal_psi::ElementViewRegion::RepresentationBytes {
                        start: start.operand_value().ok_or(Error::custody())?,
                        end: end.operand_value().ok_or(Error::custody())?,
                        obligation: *obligation,
                    },
                };
                let accepted_fact = match region {
                    terminal_psi::ElementViewRegion::WholeCollection => None,
                    terminal_psi::ElementViewRegion::WholeRepresentation { obligation, .. }
                    | terminal_psi::ElementViewRegion::RepresentationBytes { obligation, .. } => {
                        Some(crate::legalization::obligations::retained_fact(
                            facts,
                            machine,
                            *psi_operation,
                            obligation,
                        )?)
                    }
                };
                Ok(LegalizedScalarInstructionKind::EstablishElementView {
                    indices: crate::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                    result: result.clone(),
                    destination: *place,
                    source: source_argument.clone(),
                    element: *element,
                    region,
                    accepted_fact,
                })
            }

            /// The target row owns the replacement and each selector used to
            /// reach its destination. Source correspondence is audited separately.
            pub(crate) fn project_byte_view_store(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StructuralByteViewFieldStore {
                    psi_operation,
                    destination,
                    field,
                    source,
                    indices,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(
                    LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
                        destination: destination.clone(),
                        field: *field,
                        source: *source,
                        indices: crate::legalization::runtime_indices::retained_operands(
                            facts,
                            machine,
                            *psi_operation,
                            indices,
                        )?,
                    },
                )
            }

            pub(crate) fn project_structural_byte_sequence_field_byte_store(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::StructuralByteSequenceFieldByteStore {
                    psi_operation,
                    field,
                    index,
                    value,
                    length,
                    obligation,
                    ..
                } = &node.operation
                else {
                    unreachable!("dispatched project_structural_byte_sequence_field_byte_store")
                };
                let target_operations::TargetUnitOperation::StructuralByteSequenceFieldByteStore {
                    destination,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(
                    LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
                        destination: destination.clone(),
                        field: *field,
                        index: *index,
                        value: *value,
                        length: *length,
                        obligation: *obligation,
                        accepted_fact: accepted_fact(unit, optimized, *psi_operation, *obligation)?,
                    },
                )
            }

            pub(crate) fn project_structural_byte_sequence_field_read(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StructuralByteSequenceFieldRead {
                    psi_operation,
                    source,
                    field,
                    indices,
                    index,
                    length,
                    obligation,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(
                    LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
                        source: source.clone(),
                        field: *field,
                        indices: crate::legalization::runtime_indices::retained_operands(
                            facts,
                            machine,
                            *psi_operation,
                            indices,
                        )?,
                        index: index.source_value(),
                        length: *length,
                        obligation: *obligation,
                        accepted_fact: crate::legalization::obligations::retained_fact(
                            facts,
                            machine,
                            *psi_operation,
                            *obligation,
                        )?,
                    },
                )
            }

            pub(crate) fn project_structural_byte_sequence_field_store(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::StructuralByteSequenceFieldStore {
                    psi_operation,
                    field,
                    source,
                    length,
                    obligation,
                    ..
                } = &node.operation
                else {
                    unreachable!("dispatched project_structural_byte_sequence_field_store")
                };
                let target_operations::TargetUnitOperation::StructuralByteSequenceFieldStore {
                    destination,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(
                    LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore {
                        destination: destination.clone(),
                        field: *field,
                        source: *source,
                        length: *length,
                        obligation: *obligation,
                        accepted_fact: accepted_fact(unit, optimized, *psi_operation, *obligation)?,
                    },
                )
            }

            /// Checked byte reads and writes carry their scalar operands and
            /// bounds keys in target lowering. Source and backing
            /// correspondence is checked by the independent audit.
            pub(crate) fn project_byte_access(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                operation: semantic_vocabulary::OperationId,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                use target_operations::{
                    TargetIntegerExpression, TargetScalarExpression, TargetUnitOperation,
                };

                let accepted = |obligation| {
                    crate::legalization::obligations::retained_fact(
                        facts, machine, operation, obligation,
                    )
                };
                match targeted {
                    TargetUnitOperation::ScalarDefinition {
                        result_home,
                        expression:
                            TargetScalarExpression::Integer {
                                scalar_type,
                                expression:
                                    TargetIntegerExpression::ByteSequenceRead {
                                        psi_operation,
                                        source_value,
                                        source,
                                        index,
                                        length,
                                        obligation,
                                        ..
                                    },
                            },
                    } if *psi_operation == operation
                        && result_home.defining_operation == operation
                        && result_home.source_value == *source_value
                        && result_home.scalar_type == ScalarType::Integer(*scalar_type)
                        && scalar_type.sign() == IntegerSign::Unsigned
                        && scalar_type.bits() == Some(8) =>
                    {
                        Ok(LegalizedScalarInstructionKind::ByteSequenceRead {
                            source: *source,
                            index: index.operand_value().ok_or(Error::custody())?,
                            length: *length,
                            obligation: *obligation,
                            accepted_fact: accepted(*obligation)?,
                        })
                    }
                    TargetUnitOperation::ByteSequenceWrite {
                        psi_operation,
                        destination,
                        index,
                        value,
                        length,
                        obligation,
                        ..
                    } if *psi_operation == operation => {
                        Ok(LegalizedScalarInstructionKind::ByteSequenceWrite {
                            destination: *destination,
                            index: index.source_value(),
                            value: value.source_value(),
                            length: *length,
                            obligation: *obligation,
                            accepted_fact: accepted(*obligation)?,
                        })
                    }
                    _ => Err(Error::custody()),
                }
            }

            pub(crate) fn project_element_view_write(
                node: &optimization_unit::OptimizationNode,
                function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
                unit: &crate::native_geometry::NativeUnit<'_>,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let optimized = function_inputs.function();
                let AbstractOperation::ElementViewWrite {
                    path,
                    psi_operation,
                    destination,
                    index,
                    value,
                    length,
                    obligation,
                } = &node.operation
                else {
                    unreachable!("dispatched project_element_view_write")
                };
                Ok(LegalizedScalarInstructionKind::ElementViewWrite {
                    path: path.clone(),
                    destination: destination.clone(),
                    indices: view_indices(function_inputs, unit, *psi_operation, destination)?,
                    index: *index,
                    value: *value,
                    length: *length,
                    obligation: *obligation,
                    accepted_fact: accepted_fact(unit, optimized, *psi_operation, *obligation)?,
                })
            }

            pub(crate) fn project_element_view_read(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                operation: semantic_vocabulary::OperationId,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<Option<LegalizedScalarInstructionKind>, LegalizationError> {
                use target_operations::{
                    TargetBooleanExpression, TargetIntegerExpression, TargetScalarExpression,
                    TargetUnitOperation,
                };
                use terminal_psi::StructuralPathSegment;

                let TargetUnitOperation::ScalarDefinition {
                    result_home,
                    expression,
                } = targeted
                else {
                    return Ok(None);
                };
                let (owner, result, source, indices, index, length, obligation, path) =
                    match expression {
                        TargetScalarExpression::Integer {
                            expression:
                                TargetIntegerExpression::ElementViewRead {
                                    psi_operation,
                                    source_value,
                                    source,
                                    indices,
                                    index,
                                    length,
                                    obligation,
                                    path,
                                    ..
                                },
                            ..
                        }
                        | TargetScalarExpression::Boolean(
                            TargetBooleanExpression::ElementViewRead {
                                psi_operation,
                                source_value,
                                source,
                                indices,
                                index,
                                length,
                                obligation,
                                path,
                                ..
                            },
                        )
                        | TargetScalarExpression::IeeeFloatElementViewRead {
                            psi_operation,
                            source_value,
                            source,
                            indices,
                            index,
                            length,
                            obligation,
                            path,
                            ..
                        } => (
                            psi_operation,
                            source_value,
                            source,
                            indices,
                            index,
                            length,
                            obligation,
                            path,
                        ),
                        _ => return Ok(None),
                    };
                // A checked by-value read may choose a runtime element of the
                // slice, but the scalar leaf inside that element is static.
                if *owner != operation
                    || result_home.defining_operation != operation
                    || result_home.source_value != *result
                    || result_home.scalar_type != expression.scalar_type()
                    || !path.iter().all(|segment| {
                        matches!(
                            segment,
                            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                        )
                    })
                {
                    return Err(Error::custody());
                }
                Ok(Some(LegalizedScalarInstructionKind::ElementViewRead {
                    source: source.clone(),
                    index: index.operand_value().ok_or(Error::custody())?,
                    length: *length,
                    obligation: *obligation,
                    accepted_fact: crate::legalization::obligations::retained_fact(
                        facts,
                        machine,
                        operation,
                        *obligation,
                    )?,
                    path: path.clone(),
                    indices: crate::legalization::runtime_indices::retained_operands(
                        facts, machine, operation, indices,
                    )?,
                }))
            }

            pub(crate) fn project_byte_sequence_concatenation(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::EstablishByteSequenceConcatenation {
                    psi_operation,
                    result,
                    left,
                    right,
                    capacity_obligation,
                } = &node.operation
                else {
                    return Err(Error::custody());
                };
                Ok(
                    LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
                        result: result.clone(),
                        left: left.clone(),
                        right: right.clone(),
                        capacity_obligation: *capacity_obligation,
                        accepted_fact: accepted_fact(
                            unit,
                            optimized,
                            *psi_operation,
                            *capacity_obligation,
                        )?,
                    },
                )
            }

            pub(crate) fn project_byte_sequence_store(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &crate::native_geometry::NativeUnit<'_>,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::ByteSequenceStore {
                    psi_operation,
                    destination,
                    source,
                    length,
                    obligation,
                } = &node.operation
                else {
                    return Err(Error::custody());
                };
                let target_operations::TargetUnitOperation::ByteSequenceStore { .. } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(LegalizedScalarInstructionKind::ByteSequenceStore {
                    destination: *destination,
                    source: *source,
                    length: *length,
                    obligation: *obligation,
                    accepted_fact: accepted_fact(unit, optimized, *psi_operation, *obligation)?,
                })
            }

            pub(crate) fn project_storage_address(
                node: &optimization_unit::OptimizationNode,
                function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
                unit: &crate::native_geometry::NativeUnit<'_>,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let optimized = function_inputs.function();
                let AbstractOperation::StorageAddress {
                    psi_operation,
                    source,
                    path,
                    result,
                } = &node.operation
                else {
                    unreachable!("dispatched storage address")
                };
                if !matches!(result.scalar_type, ScalarType::Integer(integer) if integer.is_address())
                {
                    return Err(Error::custody());
                }
                let root =
                    scalar_graph_input::primitive_locals::read_root_type(function_inputs, *source)
                        .ok_or(Error::custody())?;
                let geometry =
                    storage_address_geometry(root, path, unit.geometry()).ok_or(Error::custody())?;
                let indices = o!(optimized => unit => *psi_operation => &geometry.elements)?;
                Ok(LegalizedScalarInstructionKind::StorageAddress {
                    source: *source,
                    path: path.clone(),
                    indices,
                })
            }

            pub(crate) fn project_global_descriptor_table(
                node: &optimization_unit::OptimizationNode,
                function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
                unit: &crate::native_geometry::NativeUnit<'_>,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let optimized = function_inputs.function();
                let AbstractOperation::GlobalDescriptorTable {
                    psi_operation,
                    service,
                    operation,
                    source,
                } = &node.operation
                else {
                    unreachable!("dispatched descriptor-table operation")
                };
                let root = scalar_graph_input::primitive_locals::read_root_type(
                    function_inputs,
                    source.place,
                )
                .ok_or(Error::custody())?;
                let geometry = storage_address_geometry(root, &source.path, unit.geometry())
                    .ok_or(Error::custody())?;
                let indices = o!(optimized => unit => *psi_operation => &geometry.elements)?;
                Ok(LegalizedScalarInstructionKind::GlobalDescriptorTable {
                    service: *service,
                    operation: *operation,
                    source: source.place,
                    path: source.path.clone(),
                    indices,
                })
            }

            #[cfg(test)]
            mod retained_storage_tests {
                use super::{project_primitive_scalar_read, project_structural_scalar_field_store};
                use semantic_vocabulary::{
                    MachineId, ObligationId, OperationId, PlaceId, StructuralFieldId,
                    StructuralTypeId, ValueId,
                };
                use target_operations::{
                    TargetStructuralRuntimeIndex, TargetUnitOperation,
                    TargetUnitScalarArgumentSource,
                };

                #[test]
                fn scalar_storage_operands_need_only_target_rows_and_admitted_facts() {
                    let machine = MachineId::new(1).unwrap();
                    let operation = OperationId::new(2).unwrap();
                    let obligation = ObligationId::new(3).unwrap();
                    let index_value = ValueId::new(4).unwrap();
                    let scalar_type = target_operations::selector_carrier_type();
                    let index = TargetStructuralRuntimeIndex {
                        operand: TargetUnitScalarArgumentSource::Parameter {
                            parameter_index: 0,
                            source_value: index_value,
                            scalar_type,
                        },
                        stride: 24,
                        extent: Some(7),
                        obligation,
                    };
                    let fact = optimization_unit::AcceptedObligationFact::new(
                        terminal_psi::TerminalPsiIdentity {
                            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes(
                                [5; 32],
                            ),
                        },
                        [6; 32],
                        machine,
                        optimization_unit::PsiProvenance::Operation(operation),
                        obligation,
                        vec![7],
                    );
                    let place = PlaceId::new(1).unwrap();
                    let path = vec![terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: index_value,
                        obligation,
                    }];
                    let read = TargetUnitOperation::PrimitiveScalarRead {
                        psi_operation: operation,
                        result: abstract_operations::AbstractResult {
                            value: ValueId::new(5).unwrap(),
                            scalar_type,
                        },
                        source: place,
                        path: path.clone(),
                        indices: vec![index.clone()],
                    };
                    let parameter = terminal_psi::StructuralParameterDeclaration {
                        place,
                        position: 0,
                        structural_type: StructuralTypeId::new(1).unwrap(),
                        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                        access: terminal_psi::StructuralAccess::MutableBorrow,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                    };
                    let value = TargetUnitScalarArgumentSource::Parameter {
                        parameter_index: 1,
                        source_value: ValueId::new(6).unwrap(),
                        scalar_type,
                    };
                    let store = TargetUnitOperation::StructuralScalarFieldStore {
                        psi_operation: operation,
                        destination:
                            abstract_operations::StructuralScalarStoreDestination::Parameter(
                                parameter,
                            ),
                        path: path.clone(),
                        field: StructuralFieldId::new(1).unwrap(),
                        destination_storage:
                            target_operations::TargetStructuralArgumentSource::BlockParameter {
                                block: semantic_vocabulary::BlockId::new(1).unwrap(),
                                place,
                            },
                        field_byte_offset: 16,
                        field_byte_size: 8,
                        source: value,
                        indices: vec![index],
                    };
                    let lowered_read =
                        project_primitive_scalar_read(machine, std::slice::from_ref(&fact), &read)
                            .unwrap();
                    let legalized_operations::LegalizedScalarInstructionKind::PrimitiveScalarRead {
                        source,
                        path: actual_path,
                        indices,
                    } = lowered_read
                    else {
                        panic!("read");
                    };
                    assert_eq!((source, actual_path), (place, path.clone()));
                    assert_eq!(
                        (
                            indices[0].operand.value,
                            indices[0].stride,
                            indices[0].extent,
                            indices[0].accepted_fact
                        ),
                        (index_value, 24, Some(7), fact.identity)
                    );
                    let lowered_store = project_structural_scalar_field_store(
                        machine,
                        std::slice::from_ref(&fact),
                        &store,
                    )
                    .unwrap();
                    let legalized_operations::LegalizedScalarInstructionKind::StructuralScalarFieldStore { value: actual_value, byte_offset, byte_size, indices, .. } = lowered_store else { panic!("store"); };
                    assert_eq!(
                        (
                            actual_value.value,
                            actual_value.scalar_type,
                            byte_offset,
                            byte_size
                        ),
                        (value.source_value(), scalar_type, 16, 8)
                    );
                    assert_eq!(indices[0].accepted_fact, fact.identity);
                    assert!(project_primitive_scalar_read(machine, &[], &read).is_err());
                    assert!(project_structural_scalar_field_store(machine, &[], &store).is_err());
                    assert!(
                        project_primitive_scalar_read(
                            MachineId::new(9).unwrap(),
                            std::slice::from_ref(&fact),
                            &read
                        )
                        .is_err()
                    );
                    assert!(
                        project_structural_scalar_field_store(
                            MachineId::new(9).unwrap(),
                            &[fact],
                            &store
                        )
                        .is_err()
                    );
                }
            }
        }

        use crate::LegalizationError;
        use crate::legalization::scalar_graph_input;
        use semantic_vocabulary::IntegerValue;

        pub(crate) fn project_reference(
            machine: semantic_vocabulary::MachineId,
            facts: &[optimization_unit::AcceptedObligationFact],
            targeted: &target_operations::TargetUnitOperation,
        ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
            use target_operations::TargetUnitOperation as Target;
            let (operation, home, indices) = match targeted {
                Target::EstablishReference {
                    psi_operation,
                    result_home,
                    indices,
                    ..
                }
                | Target::EstablishAccessReference {
                    psi_operation,
                    result_home,
                    indices,
                    ..
                } => (*psi_operation, result_home, indices),
                _ => return Err(Error::custody()),
            };
            let (_, result) = home
                .operation_result()
                .filter(|(defining, _)| *defining == operation)
                .ok_or(Error::custody())?;
            let indices = crate::legalization::runtime_indices::retained_operands(
                facts, machine, operation, indices,
            )?;
            let shape = home.layout.shape();
            // Formation already owns its result contract and selector geometry.
            // Only the independent audit needs the earlier source body.
            Ok(match targeted {
                Target::EstablishReference {
                    source, storage, ..
                } => LegalizedScalarInstructionKind::EstablishReference {
                    result: result.clone(),
                    source: source.clone(),
                    storage: *storage,
                    indices,
                    shape,
                },
                Target::EstablishAccessReference {
                    source,
                    correspondence,
                    ..
                } => LegalizedScalarInstructionKind::EstablishAccessReference {
                    result: result.clone(),
                    source: source.clone(),
                    correspondence: correspondence.clone(),
                    indices,
                    shape,
                },
                _ => return Err(Error::custody()),
            })
        }

        fn stored_sum_layout(
            home: &target_operations::TargetStructuralHomeRequirement,
        ) -> Result<calling_conventions::ConventionalSumLayout, Error> {
            home.layout.sum().cloned().ok_or(Error::custody())
        }

        pub(crate) use crate::legalization_instruction::project;

        /// Reconstruct the expected execution only for the independent audit.
        pub(crate) fn audit_project<'a>(
            node: &optimization_unit::OptimizationNode,
            function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
            native: &'a TargetOperationPlan,
            plan: &'a crate::native_geometry::NativePlan<'a>,
            unit: &'a crate::native_geometry::NativeUnit<'a>,
            instruction: &target_operations::TargetInstruction,
        ) -> Result<LegalizedScalarInstruction, LegalizationError> {
            let optimized = function_inputs.function();
            let source = instruction.source.as_ref().ok_or_else(|| {
                Error::instruction_metadata(
                    "ordinary target instruction has no retained source metadata",
                )
            })?;
            let operation = source.operation;
            let targeted = &instruction.kind;
            let kind = if matches!(
                targeted,
                target_operations::TargetUnitOperation::StructuralByteSequenceFieldLength { .. }
                    | target_operations::TargetUnitOperation::ScalarDefinition {
                        expression: target_operations::TargetScalarExpression::Integer {
                            expression: target_operations::TargetIntegerExpression::ByteSequenceLength { .. }
                                | target_operations::TargetIntegerExpression::ElementViewLength { .. },
                            ..
                        },
                        ..
                    }
            ) {
                storage_instructions::project_sequence_length(
                    optimized.machine, &unit.accepted_obligation_facts, operation, targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::EstablishElementView { .. }
            ) {
                storage_instructions::project_established_view(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::ByteSequenceSubslice { .. }
                    | target_operations::TargetUnitOperation::ElementViewSubslice { .. }
            ) {
                storage_instructions::project_subslice(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::EstablishReference { .. }
                    | target_operations::TargetUnitOperation::EstablishAccessReference { .. }
            ) {
                project_reference(optimized.machine, &unit.accepted_obligation_facts, targeted)?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::PrimitiveScalarRead { .. }
            ) {
                storage_instructions::project_primitive_scalar_read(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore { .. }
            ) {
                storage_instructions::project_write_only_primitive_store(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::StructuralScalarFieldStore { .. }
            ) {
                storage_instructions::project_structural_scalar_field_store(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::NormalizedForeignCall { .. }
            ) {
                call_instructions::project_normalized_foreign_call(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    native,
                    operation,
                    targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::Call { .. }
            ) {
                call_instructions::project_call(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::StructuralLeafCopy { .. }
            ) {
                storage_instructions::project_structural_leaf_copy(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::StoreStructuralField { .. }
            ) {
                storage_instructions::project_structural_field_store(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    plan.geometry(),
                    targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::StructuralCaseMembership { .. }
            ) {
                storage_instructions::project_structural_case_membership(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::ByteSequenceWrite { .. }
                    | target_operations::TargetUnitOperation::ScalarDefinition {
                        expression: target_operations::TargetScalarExpression::Integer {
                            expression: target_operations::TargetIntegerExpression::ByteSequenceRead { .. },
                            ..
                        },
                        ..
                    }
            ) {
                storage_instructions::project_byte_access(
                    optimized.machine, &unit.accepted_obligation_facts, operation, targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::StructuralByteSequenceFieldRead { .. }
            ) {
                storage_instructions::project_structural_byte_sequence_field_read(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    targeted,
                )?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::StructuralByteViewFieldStore { .. }
            ) {
                storage_instructions::project_byte_view_store(
                    optimized.machine,
                    &unit.accepted_obligation_facts,
                    targeted,
                )?
            } else if let target_operations::TargetUnitOperation::InstallStaticInterrupt {
                boundary,
                arguments,
                calls,
                ..
            } = targeted
            {
                let indices = arguments
                    .iter()
                    .map(|argument| {
                        let root = scalar_graph_input::primitive_locals::read_root_type(
                            function_inputs,
                            argument.place,
                        )
                        .ok_or(Error::custody())?;
                        projection(
                            optimized,
                            unit,
                            operation,
                            root,
                            &argument.path,
                            unit.geometry(),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .try_into()
                    .map_err(|_| Error::custody())?;
                let calls = calls
                    .iter()
                    .enumerate()
                    .map(|(position, call)| {
                        let mut semantic = arguments[if position == 0 { 0 } else { 5 }].clone();
                        semantic.access = terminal_psi::StructuralAccess::SharedBorrow;
                        let indices = projection(
                            optimized,
                            unit,
                            operation,
                            call.argument.root_structural_type,
                            &call.argument.path,
                            unit.geometry(),
                        )?;
                        Ok(legalized_operations::LegalizedScalarCall {
                            source: target_operations::NativeCallOrigin::Authored,
                            target: target_operations::NativeCallTarget::Machine(call.callee),
                            call_plan: call.call_plan.clone(),
                            arguments: vec![
                                legalized_operations::LegalizedScalarArgument::Structural {
                                    semantic,
                                    target: call.argument.clone(),
                                    indices,
                                },
                            ],
                            result_placement: call.call_plan.result.clone(),
                            structural_result: None,
                            descriptor_origins: Vec::new(),
                            access_bindings: Vec::new(),
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        })
                    })
                    .collect::<Result<Vec<_>, LegalizationError>>()?
                    .try_into()
                    .map_err(|_| Error::custody())?;
                LegalizedScalarInstructionKind::InstallStaticInterrupt {
                    boundary: *boundary,
                    arguments: arguments.clone(),
                    indices,
                    calls,
                }
            } else if let target_operations::TargetUnitOperation::StaticInterrupt {
                boundary,
                operation: kind,
                token,
                result_home,
                ..
            } = targeted
            {
                let root =
                    scalar_graph_input::primitive_locals::read_root_type(function_inputs, token.place)
                        .ok_or(Error::custody())?;
                let indices = projection(
                    optimized,
                    unit,
                    operation,
                    root,
                    &token.path,
                    unit.geometry(),
                )?;
                let result = result_home
                    .as_ref()
                    .map(|home| {
                        home.operation_result()
                            .map(|(_, result)| result.clone())
                            .ok_or(Error::custody())
                    })
                    .transpose()?;
                LegalizedScalarInstructionKind::StaticInterrupt {
                    boundary: *boundary,
                    operation: *kind,
                    token: token.clone(),
                    indices,
                    result,
                }
            } else if let target_operations::TargetUnitOperation::AccessLifecycle {
                boundary,
                operands,
                result_home,
                ..
            } = targeted
            {
                let (_, result) = result_home.operation_result().ok_or(Error::custody())?;
                let mut indices = Vec::new();
                for argument in operands.arguments() {
                    let root = scalar_graph_input::primitive_locals::read_root_type(
                        function_inputs,
                        argument.place,
                    )
                    .ok_or(Error::custody())?;
                    indices.extend(projection(
                        optimized,
                        unit,
                        operation,
                        root,
                        &argument.path,
                        unit.geometry(),
                    )?);
                }
                LegalizedScalarInstructionKind::AccessLifecycle {
                    boundary: *boundary,
                    operands: operands.clone(),
                    indices,
                    result: result.clone(),
                }
            } else if let target_operations::TargetUnitOperation::ProgramImageAddress {
                boundary,
                ..
            } = targeted
            {
                LegalizedScalarInstructionKind::ProgramImageAddress {
                    boundary: *boundary,
                }
            } else if let target_operations::TargetUnitOperation::AccessorScalarAccess {
                boundary,
                realization,
                location,
                operand,
                ..
            } = targeted
            {
                let root = optimized
                    .arrival_parameter(location.place)
                    .map(|parameter| parameter.structural_type)
                    .or_else(|| {
                        scalar_graph_input::structural_case::source_result(
                            optimized,
                            location.place,
                        )
                        .ok()
                        .map(|(_, result)| result.structural_type)
                    })
                    .ok_or(Error::custody())?;
                let geometry = reference_source_geometry(root, &location.path, unit.geometry())
                    .ok_or(Error::custody())?;
                let indices = crate::legalization::runtime_indices::operands(
                    optimized,
                    unit,
                    operation,
                    &geometry.elements,
                )?;
                LegalizedScalarInstructionKind::AccessorScalarAccess {
                    boundary: *boundary,
                    realization: *realization,
                    location: location.clone(),
                    indices,
                    operand: operand.map(|source| source.source_value()),
                }
            } else if let Some(read) = storage_instructions::project_element_view_read(
                optimized.machine, &unit.accepted_obligation_facts, operation, targeted,
            )? {
                read
            } else {
                match &node.operation {
                    AbstractOperation::RegisterAccess { service, event, .. } => {
                        LegalizedScalarInstructionKind::RegisterAccess {
                            service: *service,
                            event: *event,
                        }
                    }
                    AbstractOperation::PortWrite {
                        psi_operation,
                        service,
                        port,
                        value,
                    } => {
                        if !matches!(targeted, target_operations::TargetUnitOperation::PortWrite {
                            psi_operation: target_operation, service: target_service, port: target_port, value: target_value,
                        } if target_operation == psi_operation && target_service == service && target_port == port && target_value == value)
                        {
                            return Err(Error::custody());
                        }
                        LegalizedScalarInstructionKind::PortWrite {
                            service: *service,
                            port: *port,
                            value: *value,
                        }
                    }

                    AbstractOperation::SelectMachine { descriptor, .. } => {
                        LegalizedScalarInstructionKind::SelectMachine {
                            descriptor: *descriptor,
                        }
                    }
                    AbstractOperation::SelectImplementation { table, .. } => {
                        LegalizedScalarInstructionKind::SelectImplementation { table: *table }
                    }
                    AbstractOperation::AtomicEvent {
                        event: abstract_operations::AbstractAtomicEvent::Fence { ordering },
                        ..
                    } => LegalizedScalarInstructionKind::AtomicFence {
                        ordering: *ordering,
                    },
                    AbstractOperation::AtomicEvent { event, .. } => {
                        let access =
                            scalar_graph_input::atomic_access::access(event, optimized, unit)
                                .ok_or(Error::custody())?;
                        LegalizedScalarInstructionKind::AtomicAccess {
                            location: access.location,
                            scalar_type: unit
                                .closed_integer(access.scalar_type)
                                .ok_or(Error::custody())?,
                            field_byte_offset: access.field_byte_offset,
                            event: access.event,
                            outcome: access.outcome,
                        }
                    }
                    AbstractOperation::StructuralScalarFieldRead { .. } => {
                        let target_operations::TargetUnitOperation::StructuralScalarFieldRead {
                            source,
                            case,
                            field,
                            ..
                        } = targeted
                        else {
                            return Err(Error::custody());
                        };
                        LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                            source: source.clone(),
                            case: *case,
                            field: *field,
                        }
                    }
                    AbstractOperation::IntegerBitwiseAnd { left, right, .. } => {
                        LegalizedScalarInstructionKind::BitwiseAnd {
                            left: *left,
                            right: *right,
                        }
                    }
                    AbstractOperation::IntegerBitwiseOr { left, right, .. } => {
                        LegalizedScalarInstructionKind::BitwiseOr {
                            left: *left,
                            right: *right,
                        }
                    }
                    AbstractOperation::IntegerBitwiseXor { left, right, .. } => {
                        LegalizedScalarInstructionKind::BitwiseXor {
                            left: *left,
                            right: *right,
                        }
                    }
                    AbstractOperation::IntegerBitwiseNot { operand, .. } => {
                        LegalizedScalarInstructionKind::BitwiseNot { operand: *operand }
                    }
                    AbstractOperation::FloatResultPolicy { format, policy, .. } => {
                        LegalizedScalarInstructionKind::FloatResultPolicy {
                            format: *format,
                            policy: *policy,
                        }
                    }
                    AbstractOperation::ScalarRepresentationView {
                        operand,
                        source_type,
                        ..
                    } => LegalizedScalarInstructionKind::ScalarRepresentationView {
                        operand: *operand,
                        source_type: unit.closed_scalar(*source_type).ok_or(Error::custody())?,
                    },
                    AbstractOperation::IeeeFloatCompare {
                        comparison,
                        format,
                        left,
                        right,
                        ..
                    } => LegalizedScalarInstructionKind::IeeeFloatCompare {
                        comparison: *comparison,
                        format: *format,
                        left: *left,
                        right: *right,
                    },
                    AbstractOperation::ReleaseReference { source, path, .. } => {
                        LegalizedScalarInstructionKind::ReleaseReference {
                            source: *source,
                            path: path.clone(),
                        }
                    }
                    AbstractOperation::EstablishZeroedStructural { result, .. } => {
                        LegalizedScalarInstructionKind::EstablishZeroedStructural {
                            result: result.clone(),
                            shape: home_layout(result, plan)?.shape(),
                        }
                    }
                    AbstractOperation::EstablishStructuralLocal { result, .. } => {
                        LegalizedScalarInstructionKind::EstablishStructuralLocal {
                            result: result.clone(),
                            shape: home_layout(result, plan)?.shape(),
                        }
                    }
                    AbstractOperation::EstablishStructuralArray {
                        result, elements, ..
                    } => LegalizedScalarInstructionKind::EstablishStructuralArray {
                        result: result.clone(),
                        elements: elements.clone(),
                        shape: home_layout(result, plan)?.shape(),
                    },
                    AbstractOperation::EstablishActivationClaim {
                        result,
                        bound,
                        committed,
                        backing_type,
                        established_case,
                        rejected_case,
                        ..
                    } => {
                        let target_operations::TargetUnitOperation::EstablishActivationClaim {
                            result_home,
                            ..
                        } = targeted
                        else {
                            return Err(Error::custody());
                        };
                        LegalizedScalarInstructionKind::EstablishActivationClaim {
                            result: result.clone(),
                            bound: *bound,
                            committed: *committed,
                            backing_type: *backing_type,
                            established_case: *established_case,
                            rejected_case: *rejected_case,
                            layout: stored_sum_layout(result_home)?,
                        }
                    }
                    AbstractOperation::ReleaseActivationClaim { source, claim, .. } => {
                        LegalizedScalarInstructionKind::ReleaseActivationClaim {
                            source: source.clone(),
                            claim: *claim,
                        }
                    }
                    AbstractOperation::SplitActivationClaim {
                        result,
                        source,
                        claim,
                        bound_at,
                        committed_at,
                        covered_case,
                        rejected_case,
                        ..
                    } => {
                        let target_operations::TargetUnitOperation::SplitActivationClaim {
                            result_home,
                            ..
                        } = targeted
                        else {
                            return Err(Error::custody());
                        };
                        LegalizedScalarInstructionKind::SplitActivationClaim {
                            result: result.clone(),
                            source: source.clone(),
                            claim: *claim,
                            bound_at: *bound_at,
                            committed_at: *committed_at,
                            covered_case: *covered_case,
                            rejected_case: *rejected_case,
                            layout: stored_sum_layout(result_home)?,
                        }
                    }
                    AbstractOperation::MergeActivationClaim {
                        result,
                        lower,
                        upper,
                        lower_claim,
                        upper_claim,
                        covered_case,
                        rejected_case,
                        ..
                    } => {
                        let target_operations::TargetUnitOperation::MergeActivationClaim {
                            result_home,
                            ..
                        } = targeted
                        else {
                            return Err(Error::custody());
                        };
                        LegalizedScalarInstructionKind::MergeActivationClaim {
                            result: result.clone(),
                            lower: lower.clone(),
                            upper: upper.clone(),
                            lower_claim: *lower_claim,
                            upper_claim: *upper_claim,
                            covered_case: *covered_case,
                            rejected_case: *rejected_case,
                            layout: stored_sum_layout(result_home)?,
                        }
                    }
                    AbstractOperation::EstablishNativeCodeLayout {
                        result, machine, ..
                    } => LegalizedScalarInstructionKind::EstablishNativeCodeLayout {
                        result: result.clone(),
                        machine: *machine,
                        shape: home_layout(result, plan)?.shape(),
                    },
                    AbstractOperation::EstablishRecord { result, fields, .. } => {
                        LegalizedScalarInstructionKind::EstablishRecord {
                            result: result.clone(),
                            fields: fields.clone(),
                            shape: home_layout(result, plan)?.shape(),
                        }
                    }
                    AbstractOperation::EstablishScalarArray {
                        result, elements, ..
                    } => LegalizedScalarInstructionKind::EstablishScalarArray {
                        result: result.clone(),
                        elements: elements.clone(),
                        shape: scalar_graph_input::scalar_arrays::shape(result, plan)?.2,
                    },
                    AbstractOperation::EstablishCase {
                        result,
                        result_case,
                        fields,
                        ..
                    } => {
                        let target_operations::TargetUnitOperation::EstablishCase {
                            result_home,
                            ..
                        } = targeted
                        else {
                            return Err(Error::custody());
                        };
                        LegalizedScalarInstructionKind::EstablishCase {
                            result: result.clone(),
                            result_case: *result_case,
                            fields: fields.clone(),
                            layout: stored_sum_layout(result_home)?,
                        }
                    }
                    AbstractOperation::EstablishPrimitiveLocal { result, value, .. } => {
                        LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
                            result: result.clone(),
                            value: *value,
                            shape: scalar_graph_input::scalar_shape(value.scalar_type)
                                .ok_or(Error::custody())?,
                        }
                    }
                    AbstractOperation::PrimitiveLocalStore {
                        destination, value, ..
                    } => LegalizedScalarInstructionKind::PrimitiveLocalStore {
                        destination: *destination,
                        value: *value,
                    },
                    AbstractOperation::DecomposeStructuralValue { subject, .. } => {
                        LegalizedScalarInstructionKind::DecomposeStructuralValue {
                            subject: subject.clone(),
                        }
                    }
                    AbstractOperation::EstablishTrivialAffineLocal {
                        place,
                        structural_type,
                        ..
                    } => LegalizedScalarInstructionKind::EstablishTrivialAffineLocal {
                        place: *place,
                        structural_type: structural_type.clone(),
                    },
                    AbstractOperation::ActivateTaskRegister {
                        psi_operation,
                        service,
                        entries,
                        gdt,
                        task_state,
                        selector,
                        scratch,
                        ..
                    } => {
                        let mut indices = [Vec::new(), Vec::new(), Vec::new()];
                        for (argument, collected) in
                            [gdt, task_state, scratch].into_iter().zip(&mut indices)
                        {
                            let root = scalar_graph_input::primitive_locals::read_root_type(
                                function_inputs,
                                argument.place,
                            )
                            .ok_or(Error::custody())?;
                            *collected = projection(
                                optimized,
                                unit,
                                *psi_operation,
                                root,
                                &argument.path,
                                unit.geometry(),
                            )?;
                        }
                        LegalizedScalarInstructionKind::ActivateTaskRegister {
                            service: *service,
                            entries: *entries,
                            gdt: gdt.clone(),
                            task_state: task_state.clone(),
                            selector: *selector,
                            scratch: scratch.clone(),
                            indices,
                        }
                    }
                    AbstractOperation::ReadTaskRegister { service, .. } => {
                        LegalizedScalarInstructionKind::ReadTaskRegister { service: *service }
                    }
                    AbstractOperation::ReloadSegmentDescriptors {
                        psi_operation,
                        service,
                        entries,
                        gdt,
                        kind,
                        selector,
                        scratch,
                        ..
                    } => {
                        let mut indices = [Vec::new(), Vec::new()];
                        for (argument, collected) in [gdt, scratch].into_iter().zip(&mut indices) {
                            let root = scalar_graph_input::primitive_locals::read_root_type(
                                function_inputs,
                                argument.place,
                            )
                            .ok_or(Error::custody())?;
                            *collected = projection(
                                optimized,
                                unit,
                                *psi_operation,
                                root,
                                &argument.path,
                                unit.geometry(),
                            )?;
                        }
                        LegalizedScalarInstructionKind::ReloadSegmentDescriptors {
                            service: *service,
                            entries: *entries,
                            gdt: gdt.clone(),
                            kind: *kind,
                            selector: *selector,
                            scratch: scratch.clone(),
                            indices,
                        }
                    }
                    AbstractOperation::ReadSegmentSelector {
                        service, register, ..
                    } => LegalizedScalarInstructionKind::ReadSegmentSelector {
                        service: *service,
                        register: *register,
                    },
                    AbstractOperation::GlobalDescriptorTable { .. } => {
                        storage_instructions::project_global_descriptor_table(
                            node, function_inputs, unit,
                        )?
                    }
                    AbstractOperation::StorageAddress { .. } => {
                        storage_instructions::project_storage_address(node, function_inputs, unit)?
                    }
                    AbstractOperation::BoundaryCall {
                        boundary,
                        result,
                        arguments,
                        ..
                    } if scalar_graph_input::ieee_float_arithmetic::row(
                        native,
                        optimized.machine,
                        operation,
                    )?
                    .is_some() =>
                    {
                        let Some(row) = scalar_graph_input::ieee_float_arithmetic::row(
                            native,
                            optimized.machine,
                            operation,
                        )?
                        else {
                            return Err(Error::custody());
                        };
                        match row {
                            target_operations::TargetUnitOperation::IeeeFloatClassify {
                                boundary: row_boundary,
                                realization,
                                result_home,
                                ..
                            } => {
                                let abstract_operations::AbstractBoundaryResult::Structural(result) =
                                    result
                                else {
                                    return Err(Error::custody());
                                };
                                if *row_boundary != *boundary || arguments.len() != 1 {
                                    return Err(Error::custody());
                                }
                                LegalizedScalarInstructionKind::IeeeFloatClassify {
                                    boundary: *boundary,
                                    format: realization.format,
                                    argument: arguments[0],
                                    result: result.clone(),
                                    layout: result_home.layout.sum().ok_or(Error::custody())?.clone(),
                                }
                            }
                            target_operations::TargetUnitOperation::NearestIeeeFloatArithmetic {
                                boundary: row_boundary,
                                realization,
                                ..
                            } => {
                                if arguments.len() != realization.operation.operand_count()
                                    || row_boundary != boundary
                                {
                                    return Err(Error::custody());
                                }
                                LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic {
                                    boundary: *boundary,
                                    realization: *realization,
                                    arguments: arguments.clone(),
                                }
                            }
                            _ => return Err(Error::custody()),
                        }
                    }
                    AbstractOperation::BoundaryCall {
                        boundary,
                        result: abstract_operations::AbstractBoundaryResult::Structural(result),
                        ..
                    } if matches!(
                        scalar_graph_input::hosted_realization(
                            native,
                            optimized.machine,
                            operation
                        )?,
                        (_, target_operations::BoundaryRealization::HostedReadByte(_))
                    ) =>
                    {
                        LegalizedScalarInstructionKind::HostedReadByte {
                            boundary: *boundary,
                            result: result.clone(),
                            layout: scalar_graph_input::read_byte::layout(result, plan)?,
                        }
                    }
                    AbstractOperation::BoundaryCall { .. } => {
                        call_instructions::project_boundary_call(
                            node, optimized, native, operation,
                        )?
                    }
                    AbstractOperation::EstablishByteSequenceLiteral {
                        place,
                        structural_type,
                        bytes,
                        ..
                    } => LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                        destination: *place,
                        structural_type: structural_type.clone(),
                        bytes: bytes.clone(),
                    },
                    AbstractOperation::StructuralByteSequenceFieldByteStore { .. } => {
                        storage_instructions::project_structural_byte_sequence_field_byte_store(
                            node, optimized, unit, targeted,
                        )?
                    }
                    AbstractOperation::EstablishByteSequenceConcatenation { .. } => {
                        storage_instructions::project_byte_sequence_concatenation(
                            node, optimized, unit,
                        )?
                    }
                    AbstractOperation::ByteSequenceStore { .. } => {
                        storage_instructions::project_byte_sequence_store(
                            node, optimized, unit, targeted,
                        )?
                    }
                    AbstractOperation::StructuralByteSequenceFieldStore { .. } => {
                        storage_instructions::project_structural_byte_sequence_field_store(
                            node, optimized, unit, targeted,
                        )?
                    }
                    AbstractOperation::ByteSequencePredicate {
                        operand, predicate, ..
                    } => LegalizedScalarInstructionKind::ByteSequencePredicate {
                        operand: operand.clone(),
                        predicate: *predicate,
                    },
                    AbstractOperation::ByteSequenceEqual { left, right, .. } => {
                        LegalizedScalarInstructionKind::ByteSequenceEqual {
                            left: left.clone(),
                            right: right.clone(),
                        }
                    }
                    // A folded byte length uses the same immediate row as
                    // other scalar observations. The opcode only dispatches it.
                    AbstractOperation::ByteSequenceLength { .. } => {
                        storage_instructions::project_sequence_length(
                            optimized.machine, &unit.accepted_obligation_facts, operation, targeted,
                        )?
                    }
                    AbstractOperation::ElementViewWrite { .. } => {
                        storage_instructions::project_element_view_write(node, function_inputs, unit)?
                    }
                    AbstractOperation::BooleanNot { operand, .. } => {
                        LegalizedScalarInstructionKind::BooleanNot { operand: *operand }
                    }
                    AbstractOperation::IntegerWiden {
                        operand,
                        source_type,
                        ..
                    } => LegalizedScalarInstructionKind::IntegerWiden {
                        operand: *operand,
                        source_type: unit.closed_integer(*source_type).ok_or(Error::custody())?,
                    },
                    AbstractOperation::IntegerExactCast { .. } => {
                        scalar_instructions::project_integer_exact_cast(node, optimized, unit)?
                    }
                    AbstractOperation::IntegerConstant { value, .. } => {
                        LegalizedScalarInstructionKind::Constant(*value)
                    }
                    AbstractOperation::IeeeFloatConstant { value, .. } => {
                        let bits = match value {
                            semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                                u128::from(*bits)
                            }
                            semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                                u128::from(*bits)
                            }
                        };
                        LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(bits))
                    }
                    AbstractOperation::BooleanConstant { value, .. } => {
                        LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(
                            u128::from(*value),
                        ))
                    }
                    AbstractOperation::TrappingInteger { .. } => {
                        scalar_instructions::project_trapping_integer(node, optimized)?
                    }
                    AbstractOperation::IntegerArithmetic {
                        psi_operation,
                        scalar_type,
                        arithmetic,
                        left,
                        right,
                        ..
                    } => scalar_instructions::project_integer_arithmetic(
                        scalar_instructions::IntegerArithmeticNode {
                            psi_operation: *psi_operation,
                            scalar_type: *scalar_type,
                            arithmetic: *arithmetic,
                            left: *left,
                            right: *right,
                        },
                        optimized,
                        unit,
                    )?,
                    AbstractOperation::IntegerShift { .. } => {
                        scalar_instructions::project_shift(node, optimized, unit)?
                    }
                    AbstractOperation::BooleanEqual { .. }
                    | abstract_operations::integer_equal_less_pair_pattern!(..)
                    | AbstractOperation::IntegerLessOrEqual { .. } => {
                        scalar_instructions::project_boolean_equal(node, optimized, unit)?
                    }
                    _ => return Err(Error::custody()),
                }
            };
            with_metadata(source, kind, unit.closed_types())
        }

        /// The instruction owns its already-retained execution and custody annotations.
        pub(crate) fn with_metadata(
            source: &target_operations::TargetInstructionSource,
            kind: LegalizedScalarInstructionKind,
            types: &crate::native_geometry::ModuleTypes,
        ) -> Result<LegalizedScalarInstruction, Error> {
            Ok(LegalizedScalarInstruction {
                operation: source.operation,
                result: source
                    .result
                    .map(|result| {
                        Ok::<_, Error>(LegalizedValueDefinition {
                            value: result.value,
                            scalar_type: types
                                .scalar(result.scalar_type)
                                .ok_or_else(Error::custody)?,
                            definition_site: result.site,
                        })
                    })
                    .transpose()?,
                kind,
                work: source.metadata.work.clone(),
                effect: source.metadata.effect,
                ownership: source.metadata.ownership.clone(),
            })
        }
    }
    pub(crate) mod structural_case {
        use super::{Error, scalar_graph_input};
        use crate::LegalizationError;
        use abstract_operations::AbstractOperation;
        use legalized_operations::{
            LegalizedScalarTerminator, LegalizedStructuralCasePayload,
            LegalizedStructuralCaseSuccessor, LegalizedValueDefinition,
        };

        pub(crate) fn project(
            node: &optimization_unit::OptimizationNode,
            function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
            plan: &crate::native_geometry::NativePlan<'_>,
            unit: &crate::native_geometry::NativeUnit<'_>,
            targeted: &target_operations::TargetControlTerminator,
        ) -> Result<LegalizedScalarTerminator, LegalizationError> {
            let function = function_inputs.function();
            let AbstractOperation::StructuralCase {
                psi_block,
                source,
                path,
                cases,
            } = &node.operation
            else {
                return Err(Error::custody());
            };
            let target_operations::TargetControlTerminator::StructuralCase {
                psi_block: stored_block,
                indices: stored_indices,
                source: stored_source,
                path: stored_path,
                byte_offset,
                layout,
                pointer_slots,
                region_byte_size,
                cases: stored_cases,
            } = targeted
            else {
                return Err(Error::custody());
            };
            if stored_block != psi_block || stored_source.place() != *source || stored_path != path
            {
                return Err(Error::custody());
            }
            let byte_offset = *byte_offset;
            if pointer_slots.iter().any(|slot| slot.scalar_descriptor)
                || byte_offset
                    .checked_add(layout.shape.byte_size)
                    .is_none_or(|end| end > *region_byte_size)
            {
                return Err(Error::custody());
            }
            let root_shape = stored_source.layout().shape();
            let indices = crate::legalization::runtime_indices::retained_operands(
                &unit.accepted_obligation_facts,
                function.machine,
                terminal_psi::ExecutionSite::Block(*psi_block),
                stored_indices,
            )?;
            let layout = layout.clone();
            let source = scalar_graph_input::structural_case::case_source(function, *source)?;
            if cases.len() != node.successors.len()
                || cases.len() != layout.cases.len()
                || cases.len() != stored_cases.len()
            {
                return Err(Error::custody());
            }
            let mut successors = Vec::with_capacity(cases.len());
            for (case_position, ((case, edge), stored)) in cases
                .iter()
                .zip(&node.successors)
                .zip(stored_cases)
                .enumerate()
            {
                let destination = function.block(case.target).ok_or(Error::custody())?;
                let case_tag = i32::try_from(case_position).map_err(|_| Error::custody())?;
                if stored.psi_edge != case.psi_edge
                    || stored.target != case.target
                    || stored.case != case.case
                    || stored.case_tag != case_tag
                    || stored.payloads.len() != case.payloads.len()
                    || case.payloads.len() != destination.parameters.len()
                    || stored.structural_bindings != case.structural_bindings
                    || stored.cleanup_actions != case.cleanup_actions
                {
                    return Err(Error::custody());
                }
                let mut payloads = Vec::with_capacity(case.payloads.len());
                for ((payload, parameter), stored_payload) in case
                    .payloads
                    .iter()
                    .zip(&destination.parameters)
                    .zip(&stored.payloads)
                {
                    if stored_payload.field != payload.field
                        || stored_payload.parameter.block != case.target
                        || stored_payload.parameter.value != payload.parameter
                        || stored_payload.parameter.value != parameter.value
                        || stored_payload.parameter.scalar_type != payload.scalar_type
                        || stored_payload.parameter.scalar_type != parameter.scalar_type
                    {
                        return Err(Error::custody());
                    }
                    payloads.push(LegalizedStructuralCasePayload {
                        field: payload.field,
                        field_byte_offset: stored_payload.field_byte_offset,
                        parameter: LegalizedValueDefinition {
                            value: parameter.value,
                            scalar_type: unit
                                .closed_scalar(parameter.scalar_type)
                                .ok_or(Error::custody())?,
                            definition_site: parameter.site,
                        },
                    });
                }
                successors.push(LegalizedStructuralCaseSuccessor {
                    edge: case.psi_edge,
                    target: case.target,
                    case: case.case,
                    case_tag,
                    payloads,
                    structural_bindings: super::terminator::successor(
                        edge,
                        function_inputs,
                        plan,
                        unit,
                    )?
                    .structural_bindings,
                    cleanup_actions: case.cleanup_actions.clone(),
                    cleanup_calls: Vec::new(),
                    work: edge.work.clone(),
                });
            }
            Ok(LegalizedScalarTerminator::StructuralCase {
                psi_block: *psi_block,
                indices,
                source,
                path: path.clone(),
                root_shape,
                byte_offset,
                layout,
                pointer_slots: pointer_slots.clone(),
                region_byte_size: *region_byte_size,
                cases: successors,
                effect: node.effect,
                ownership: node.ownership.clone(),
            })
        }
    }
    pub(crate) mod terminator {
        use super::Error;
        use crate::LegalizationError;
        use crate::legalization::scalar_graph_input;
        use abstract_operations::AbstractOperation;
        use legalized_operations::{
            LegalizedScalarReturn, LegalizedScalarReturnValue, LegalizedScalarSuccessor,
            LegalizedScalarTerminator,
        };
        /// Source oracle for the independent correspondence audit only.
        pub(crate) fn audit_project(
            node: &optimization_unit::OptimizationNode,
            function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
            plan: &crate::native_geometry::NativePlan<'_>,
            native: &target_operations::TargetOperationPlan,
            unit: &crate::native_geometry::NativeUnit<'_>,
            targeted: &target_operations::TargetControlTerminator,
        ) -> Result<LegalizedScalarTerminator, LegalizationError> {
            let function = function_inputs.function();
            let cleanup_calls = scalar_graph_input::cleanup_calls::expected(
                &node.operation,
                function_inputs,
                native,
                plan,
                unit,
            )?;
            let mut result = match &node.operation {
                AbstractOperation::InvokeNoNormal {
                    psi_edge,
                    callee,
                    arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                } => {
                    let mut candidates = native
                        .functions
                        .iter()
                        .filter(|row| row.machine == function.machine)
                        .flat_map(|row| &row.graph.blocks)
                        .filter_map(|block| match &block.terminator {
                            target_operations::TargetControlTerminator::InvokeNoNormal {
                                psi_edge: edge,
                                callee: target,
                                call_plan,
                                scalar_arguments,
                                arguments: structural,
                                access_bindings,
                                claim_transfers: transfers,
                                requirement_obligations: requires,
                                crash_continuations: crashes,
                                declared_result,
                            } if edge == psi_edge && target == callee => Some((
                                call_plan,
                                scalar_arguments,
                                structural,
                                access_bindings,
                                transfers,
                                requires,
                                crashes,
                                declared_result,
                            )),
                            _ => None,
                        });
                    let (
                        call_plan,
                        scalars,
                        structural,
                        placed,
                        transfers,
                        requires,
                        crashes,
                        declared_result,
                    ) = candidates.next().ok_or(Error::custody())?;
                    if candidates.next().is_some()
                        || transfers != claim_transfers
                        || requires != requirement_obligations
                        || crashes != crash_continuations
                        || scalars.len() != arguments.len()
                        || structural.len() != structural_arguments.len()
                        || scalars
                            .iter()
                            .zip(arguments)
                            .any(|(actual, expected)| actual.source.source_value() != *expected)
                        || structural
                            .iter()
                            .zip(structural_arguments)
                            .any(|(actual, expected)| {
                                actual.place != expected.place
                                    || actual.path != expected.path
                                    || actual.access != expected.access
                            })
                        || unit
                            .function(*callee)
                            .is_none_or(|function| &function.result != declared_result)
                    {
                        return Err(Error::custody());
                    }
                    let mut actuals = scalars
                        .iter()
                        .map(
                            |argument| legalized_operations::LegalizedScalarArgument::Scalar {
                                source: argument.source.source_value(),
                                placement: argument.placement.clone(),
                            },
                        )
                        .collect::<Vec<_>>();
                    for (argument, semantic) in structural.iter().zip(structural_arguments) {
                        actuals.push(legalized_operations::LegalizedScalarArgument::Structural {
                            indices: crate::legalization::runtime_indices::retained_operands(
                                &unit.accepted_obligation_facts,
                                function.machine,
                                optimization_unit::PsiProvenance::Edge(*psi_edge),
                                &argument.indices,
                            )?,
                            semantic: semantic.clone(),
                            target: argument.clone(),
                        });
                    }
                    if call_plan != &scalar_graph_input::callee_plan(*callee, native, plan, unit)? {
                        return Err(Error::custody());
                    }
                    let call = legalized_operations::LegalizedScalarCall {
                        descriptor_origins: Vec::new(),
                        access_bindings: placed.clone(),
                        source: target_operations::NativeCallOrigin::Authored,
                        target: target_operations::NativeCallTarget::Machine(*callee),
                        call_plan: call_plan.clone(),
                        arguments: actuals,
                        result_placement: call_plan.result.clone(),
                        structural_result: None,
                        claim_transfers: transfers.clone(),
                        requirement_obligations: requires.clone(),
                        crash_continuations: crashes.clone(),
                    };
                    call.validate_no_normal_shape()
                        .map_err(|_| Error::custody())?;
                    call.validate_source(&node.ownership)
                        .map_err(|_| Error::custody())?;
                    Ok(LegalizedScalarTerminator::InvokeNoNormal {
                        psi_edge: *psi_edge,
                        call,
                        declared_result: declared_result.clone(),
                        work: node.work.clone(),
                        effect: node.effect,
                        ownership: node.ownership.clone(),
                    })
                }
                AbstractOperation::ExternalCompletion {
                    psi_edge,
                    boundary,
                    kind,
                    structural_arguments,
                    requirement_obligations,
                    frontier_lower_bound,
                    ..
                } => {
                    let mut matches = native
                        .functions
                        .iter()
                        .filter(|row| row.machine == function.machine)
                        .flat_map(|row| &row.graph.blocks)
                        .filter_map(|block| match &block.terminator {
                            target_operations::TargetControlTerminator::ExternalCompletion {
                                psi_edge: edge,
                                arguments,
                                binding,
                                ..
                            } if edge == psi_edge => Some((arguments, binding)),
                            _ => None,
                        });
                    let (arguments, binding) = matches.next().ok_or(Error::custody())?;
                    if matches.next().is_some() {
                        return Err(Error::custody());
                    }
                    Ok(LegalizedScalarTerminator::ExternalCompletion {
                        psi_edge: *psi_edge,
                        boundary: *boundary,
                        kind: *kind,
                        arguments: arguments.clone(),
                        binding: binding.clone(),
                        structural_arguments: structural_arguments.clone(),
                        requirement_obligations: requirement_obligations.clone(),
                        frontier_lower_bound: frontier_lower_bound.clone(),
                        work: node.work.clone(),
                        effect: node.effect,
                        ownership: node.ownership.clone(),
                    })
                }
                AbstractOperation::EliminateEmpty {
                    psi_edge,
                    source,
                    path,
                } => {
                    if !matches!(targeted,
                        target_operations::TargetControlTerminator::EliminateEmpty {
                            psi_edge: target_edge, source: target_source, path: target_path
                        } if target_edge == psi_edge && target_source == source && target_path == path)
                    {
                        return Err(Error::custody());
                    }
                    Ok(LegalizedScalarTerminator::EliminateEmpty {
                        psi_edge: *psi_edge,
                        source: *source,
                        path: path.clone(),
                        work: node.work.clone(),
                        effect: node.effect,
                        ownership: node.ownership.clone(),
                    })
                }
                AbstractOperation::Crash {
                    psi_edge,
                    cause,
                    site_guard,
                    frontier_lower_bound,
                } => Ok(LegalizedScalarTerminator::Crash {
                    psi_edge: *psi_edge,
                    cause: *cause,
                    site_guard: site_guard.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                    work: node.work.clone(),
                    effect: node.effect,
                    ownership: node.ownership.clone(),
                }),
                AbstractOperation::ReturnStructural {
                    psi_edge, source, ..
                } => {
                    let value = if scalar_graph_input::byte_views::descriptor_return(
                        function_inputs,
                        *source,
                        plan.geometry(),
                    ) {
                        LegalizedScalarReturnValue::SharedDescriptor { place: *source }
                    } else if function.structural_parameter(*source).is_some() {
                        LegalizedScalarReturnValue::StructuralParameter { place: *source }
                    } else {
                        LegalizedScalarReturnValue::Structural {
                            source: scalar_graph_input::structural_case::source_owner(
                                function, *source,
                            )?,
                        }
                    };
                    Ok(LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                        edge: *psi_edge,
                        value,
                        work: node.work.clone(),
                        effect: node.effect,
                        ownership: node.ownership.clone(),
                    }))
                }
                AbstractOperation::StructuralCase { .. } => {
                    super::structural_case::project(node, function_inputs, plan, unit, targeted)
                }
                AbstractOperation::ReturnUnit { psi_edge, .. } => {
                    Ok(LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                        edge: *psi_edge,
                        value: LegalizedScalarReturnValue::Unit,
                        work: node.work.clone(),
                        effect: node.effect,
                        ownership: node.ownership.clone(),
                    }))
                }
                AbstractOperation::Return {
                    psi_edge,
                    value,
                    scalar_type,
                    ..
                } if scalar_graph_input::scalar_shape(*scalar_type).is_some() => {
                    Ok(LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                        edge: *psi_edge,
                        value: LegalizedScalarReturnValue::Value {
                            value: *value,
                            scalar_type: unit.closed_scalar(*scalar_type).ok_or(Error::custody())?,
                        },
                        work: node.work.clone(),
                        effect: node.effect,
                        ownership: node.ownership.clone(),
                    }))
                }
                AbstractOperation::Jump { .. } => {
                    let [edge] = node.successors.as_slice() else {
                        return Err(Error::custody());
                    };
                    Ok(LegalizedScalarTerminator::Jump {
                        successor: successor(edge, function_inputs, plan, unit)?,
                        effect: node.effect,
                        ownership: node.ownership.clone(),
                    })
                }
                AbstractOperation::Conditional { condition, .. } => {
                    let [when_true, when_false] = node.successors.as_slice() else {
                        return Err(Error::custody());
                    };
                    Ok(LegalizedScalarTerminator::Conditional {
                        condition: *condition,
                        when_true: successor(when_true, function_inputs, plan, unit)?,
                        when_false: successor(when_false, function_inputs, plan, unit)?,
                        effect: node.effect,
                        ownership: node.ownership.clone(),
                    })
                }
                _ => Err(Error::custody()),
            }?;
            if cleanup_calls.is_empty() {
                return Ok(result);
            }
            if let LegalizedScalarTerminator::Return(returned) = result {
                return Ok(LegalizedScalarTerminator::ReturnCleanup {
                    returned,
                    calls: cleanup_calls,
                });
            }
            for (owner, call) in cleanup_calls {
                let calls = match &mut result {
                    LegalizedScalarTerminator::Jump { successor, .. }
                        if owner.edge() == Some(successor.edge) =>
                    {
                        &mut successor.cleanup_calls
                    }
                    LegalizedScalarTerminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        if owner.edge() == Some(when_true.edge) {
                            &mut when_true.cleanup_calls
                        } else if owner.edge() == Some(when_false.edge) {
                            &mut when_false.cleanup_calls
                        } else {
                            return Err(Error::custody());
                        }
                    }
                    LegalizedScalarTerminator::StructuralCase { cases, .. } => {
                        &mut cases
                            .iter_mut()
                            .find(|case| owner.edge() == Some(case.edge))
                            .ok_or(Error::custody())?
                            .cleanup_calls
                    }
                    _ => return Err(Error::custody()),
                };
                calls.push((owner, call));
            }
            Ok(result)
        }
        pub(crate) fn successor(
            edge: &optimization_unit::OptimizationEdge,
            function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
            plan: &crate::native_geometry::NativePlan<'_>,
            unit: &crate::native_geometry::NativeUnit<'_>,
        ) -> Result<LegalizedScalarSuccessor, LegalizationError> {
            let function = function_inputs.function();
            let structural_bindings = edge.structural_bindings.iter().map(|binding| {
                let indices = if binding.argument.path.iter().any(|segment| segment.runtime_index().is_some()) {
                    let root = scalar_graph_input::primitive_locals::read_root_type(function_inputs, binding.argument.place)
                        .ok_or(Error::custody())?;
                    let geometry = crate::structural_inputs::structural_reference_input::reference_formation_geometry(
                        root, &binding.argument.path, plan.geometry(),
                    ).ok_or(Error::custody())?;
                    crate::legalization::runtime_indices::operands(
                        function, unit, optimization_unit::PsiProvenance::Edge(edge.psi_edge), &geometry.elements,
                    )?
                } else { Vec::new() };
                Ok(legalized_operations::LegalizedStructuralBinding { semantic: binding.clone(), indices })
            }).collect::<Result<Vec<_>, LegalizationError>>()?;
            Ok(LegalizedScalarSuccessor {
                edge: edge.psi_edge,
                target: edge.target,
                bindings: edge.bindings.clone(),
                structural_bindings,
                work: edge.work.clone(),
                cleanup_calls: Vec::new(),
            })
        }
    }

    /// Parameter sites are ordinal in the retained function ABI, not body-derived.
    pub(crate) fn parameters(
        target: &target_operations::TargetFunction,
        types: &crate::native_geometry::ModuleTypes,
    ) -> Result<Vec<LegalizedScalarParameter>, LegalizationError> {
        target
            .graph
            .scalar_parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                Ok(LegalizedScalarParameter {
                    value: parameter.value,
                    scalar_type: types
                        .scalar(parameter.scalar_type)
                        .ok_or_else(Error::custody)?,
                    definition_site: optimization_unit::ValueDefinitionSite::FunctionParameter(
                        u32::try_from(position).map_err(|_| Error::custody())?,
                    ),
                    placement: parameter.placement.clone(),
                })
            })
            .collect()
    }

    pub(crate) fn derive<'a>(
        target: &target_operations::TargetFunction,
        optimized: &optimization_unit::PsiOptimizationFunction,
        native: &'a TargetOperationPlan,
        plan: &'a crate::native_geometry::NativePlan<'a>,
        unit: &'a crate::native_geometry::NativeUnit<'a>,
        current_operation: &mut Option<semantic_vocabulary::OperationId>,
    ) -> Result<LegalizedScalarFunction, LegalizationError> {
        let function_inputs = crate::legalization_function_inputs::FunctionInputs::new(optimized);
        let call_plan = target.graph.call_plan.clone();
        let parameters = parameters(target, unit.closed_types())?;
        if optimized.blocks.len() != target.graph.blocks.len() {
            return Err(Error::custody());
        }
        let blocks = optimized
            .blocks
            .iter()
            .zip(&target.graph.blocks)
            .map(|(block, targeted)| {
                *current_operation = None;
                let (_, body) = block.nodes.split_last().ok_or(Error::custody())?;
                let rows = targeted
                    .operations
                    .split_at(
                        targeted
                            .operations
                            .iter()
                            .position(|row| {
                                matches!(
                                    &row.kind,
                                    target_operations::TargetUnitOperation::CleanupCall { .. }
                                )
                            })
                            .unwrap_or(targeted.operations.len()),
                    )
                    .0;
                if block.id != targeted.block || body.len() != rows.len() {
                    return Err(Error::custody());
                }
                let mut instructions = Vec::with_capacity(body.len());
                for (node, row) in body.iter().zip(rows) {
                    *current_operation = row.source.as_ref().map(|source| source.operation);
                    instructions.push(instruction::audit_project(
                        node,
                        &function_inputs,
                        native,
                        plan,
                        unit,
                        row,
                    )?);
                }
                *current_operation = None;
                let mut parameters = Vec::with_capacity(targeted.parameters.len());
                for (position, parameter) in targeted.parameters.iter().enumerate() {
                    parameters.push(legalized_operations::LegalizedValueDefinition {
                        value: parameter.value,
                        scalar_type: unit
                            .closed_scalar(parameter.scalar_type)
                            .ok_or_else(Error::custody)?,
                        definition_site: optimization_unit::ValueDefinitionSite::BlockParameter {
                            block: targeted.block,
                            position: u32::try_from(position).map_err(|_| Error::custody())?,
                        },
                    });
                }
                Ok(LegalizedScalarBlock {
                    id: targeted.block,
                    parameters,
                    structural_parameters: targeted.structural_parameters.clone().into(),
                    instructions,
                    terminator: crate::legalization_control::project(
                        target.machine,
                        targeted,
                        target
                            .structural_contract
                            .as_ref()
                            .map(|contract| contract.parameters.as_slice())
                            .unwrap_or(&[]),
                        &unit.accepted_obligation_facts,
                        unit.closed_types(),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, LegalizationError>>()?;
        Ok(LegalizedScalarFunction {
            machine: target.machine,
            attachment: target.attachment,
            provenance: target.provenance.clone(),
            call_plan,
            parameters,
            structural: scalar_graph_input::structural_contract(target)?,
            entry_block: target.graph.entry,
            blocks,
            ownership_frontier_facts: unit
                .ownership_frontier_facts
                .iter()
                .filter(|fact| fact.machine == target.machine)
                .cloned()
                .collect(),
        })
    }
}

pub(crate) struct SourceFunctionRosters {
    pub scalar_functions: Vec<legalized_operations::LegalizedScalarFunction>,
}

pub(crate) fn derive_source_function_rosters(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    closed_types: &crate::native_geometry::ModuleTypes,
) -> Result<SourceFunctionRosters, LegalizationError> {
    let plan_geometry = crate::native_geometry::NativeGeometry::with_owned_sequence_bounds(
        &abstract_plan.structural_types,
        abstract_plan.structural_types.owned_sequence_bounds(),
    )
    .map_err(|cause| LegalizationError::custody_with_cause(cause))?;
    let unit_geometry = crate::native_geometry::NativeGeometry::with_owned_sequence_bounds(
        &unit.structural_types,
        unit.structural_types.owned_sequence_bounds(),
    )
    .map_err(|cause| LegalizationError::custody_with_cause(cause))?;
    let abstract_plan = &crate::native_geometry::NativeInput::with_types(
        abstract_plan,
        &plan_geometry,
        closed_types,
    );
    let unit = &crate::native_geometry::NativeInput::with_types(unit, &unit_geometry, closed_types);
    validate_source_custody(target, abstract_plan, unit)?;
    crate::legalization::scalar_graph_input::normalized_foreign::validate_native_callback_roster(
        target,
    )?;

    let mut rosters = SourceFunctionRosters {
        scalar_functions: Vec::new(),
    };
    audit_roster::derive_remaining(&mut rosters, target, abstract_plan, unit)?;

    Ok(rosters)
}
pub(crate) use custody::validate_source_custody;

/// Ordinary legalization consumes only target execution and immutable judgments.
pub(crate) fn derive_target_function_rosters(
    target: &TargetOperationPlan,
    evidence: crate::PreparationEvidence<'_>,
    types: &crate::native_geometry::ModuleTypes,
) -> Result<SourceFunctionRosters, LegalizationError> {
    let geometries =
        crate::native_geometry::NativeGeometries::new(target.functions.iter().map(|function| {
            (
                function.graph.structural_types.as_ref(),
                function.graph.structural_types.owned_sequence_bounds(),
            )
        }))
        .map_err(|cause| LegalizationError::custody_with_cause(cause))?;
    let mut scalar_functions = Vec::with_capacity(target.functions.len());
    for function in &target.functions {
        let geometry = geometries
            .get(
                &function.graph.structural_types,
                function.graph.structural_types.owned_sequence_bounds(),
            )
            .ok_or_else(LegalizationError::custody)?;
        let mut blocks = Vec::with_capacity(function.graph.blocks.len());
        for block in &function.graph.blocks {
            let mut instructions = Vec::with_capacity(block.operations.len());
            for row in &block.operations {
                if matches!(
                    row.kind,
                    target_operations::TargetUnitOperation::CleanupCall { .. }
                ) {
                    continue;
                }
                instructions.push(
                    scalar_graph::instruction::project(
                        function.machine,
                        evidence.accepted_obligation_facts,
                        types,
                        target,
                        geometry,
                        row,
                    )
                    .map_err(|error| {
                        error.attributed(
                            function.machine,
                            row.source.as_ref().map(|source| source.operation),
                        )
                    })?,
                );
            }
            let parameters = block
                .parameters
                .iter()
                .enumerate()
                .map(|(position, parameter)| {
                    Ok(legalized_operations::LegalizedValueDefinition {
                        value: parameter.value,
                        scalar_type: types
                            .scalar(parameter.scalar_type)
                            .ok_or_else(LegalizationError::custody)?,
                        definition_site: optimization_unit::ValueDefinitionSite::BlockParameter {
                            block: block.block,
                            position: u32::try_from(position)
                                .map_err(|cause| LegalizationError::custody_with_cause(cause))?,
                        },
                    })
                })
                .collect::<Result<_, LegalizationError>>()?;
            blocks.push(legalized_operations::LegalizedScalarBlock {
                id: block.block,
                parameters,
                structural_parameters: block.structural_parameters.clone().into(),
                instructions,
                terminator: crate::legalization_control::project(
                    function.machine,
                    block,
                    function
                        .structural_contract
                        .as_ref()
                        .map(|contract| contract.parameters.as_slice())
                        .unwrap_or(&[]),
                    evidence.accepted_obligation_facts,
                    types,
                )?,
            });
        }
        scalar_functions.push(legalized_operations::LegalizedScalarFunction {
            machine: function.machine,
            attachment: function.attachment,
            provenance: function.provenance.clone(),
            call_plan: function.graph.call_plan.clone(),
            parameters: scalar_graph::parameters(function, types)?,
            structural: crate::legalization::scalar_graph_input::structural_contract(function)?,
            entry_block: function.graph.entry,
            blocks,
            ownership_frontier_facts: evidence
                .ownership_frontier_facts
                .iter()
                .filter(|fact| fact.machine == function.machine)
                .cloned()
                .collect(),
        });
    }
    Ok(SourceFunctionRosters { scalar_functions })
}
