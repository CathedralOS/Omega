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
    use abstract_operations::AbstractOperationPlan;
    use optimization_unit::PsiOptimizationUnit;
    use target_operations::TargetOperationPlan;

    pub(crate) fn validate_source_custody(
        target: &TargetOperationPlan,
        abstract_plan: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
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
pub(crate) mod ordinary_roster {
    use super::{AbstractOperationPlan, PsiOptimizationUnit, TargetOperationPlan};
    use crate::LegalizationError as Error;
    use crate::LegalizationError;
    use crate::legalization::source::SourceFunctionRosters;

    pub(crate) fn derive_remaining(
        rosters: &mut SourceFunctionRosters,
        target: &TargetOperationPlan,
        abstract_plan: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
    ) -> Result<(), LegalizationError> {
        let call_descriptor_graph = std::cell::OnceCell::new();
        let mut call_descriptor_origins =
            crate::legalization::scalar_graph_input::byte_views::CallDescriptorOrigins::new(
                unit,
                target,
                abstract_plan,
                &call_descriptor_graph,
            );
        for target_function in &target.functions {
            let mut current_operation = None;
            let function = derive_one(
                target_function,
                target,
                abstract_plan,
                unit,
                &mut current_operation,
                &mut call_descriptor_origins,
            )
            .map_err(|error| error.attributed(target_function.machine, current_operation))?;
            rosters.scalar_functions.push(function);
        }
        Ok(())
    }

    fn derive_one<'a, 'g>(
        target_function: &target_operations::TargetFunction,
        target: &'a TargetOperationPlan,
        abstract_plan: &'a AbstractOperationPlan,
        unit: &'a PsiOptimizationUnit,
        current_operation: &mut Option<semantic_vocabulary::OperationId>,
        call_descriptor_origins: &mut crate::legalization::scalar_graph_input::byte_views::CallDescriptorOrigins<'a, 'g>,
    ) -> Result<legalized_operations::LegalizedScalarFunction, LegalizationError>
    where
        'a: 'g,
    {
        let abstract_matches = abstract_plan
            .functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let optimized_matches = unit
            .functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let ([abstracted], [optimized]) =
            (abstract_matches.as_slice(), optimized_matches.as_slice())
        else {
            return Err(Error::custody());
        };
        super::scalar_graph::derive(
            target_function,
            abstracted,
            optimized,
            target,
            abstract_plan,
            unit,
            current_operation,
            call_descriptor_origins,
        )
    }
}
pub(crate) mod scalar_graph {
    use crate::legalization::scalar_graph_input;
    use crate::{LegalizationError, LegalizationError as Error};
    use abstract_operations::AbstractOperationPlan;
    use legalized_operations::{
        LegalizedScalarBlock, LegalizedScalarFunction, LegalizedScalarParameter,
    };
    use optimization_unit::PsiOptimizationUnit;
    use target_operations::TargetOperationPlan;
    pub(crate) mod instruction {
        use super::{AbstractOperationPlan, Error, PsiOptimizationUnit, TargetOperationPlan};
        use crate::legalization::runtime_indices::{collection_view, projection, reference_source};
        use crate::structural_inputs::structural_reference_input::reference_source_geometry;
        use abstract_operations::AbstractOperation;
        use legalized_operations::{
            LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedValueDefinition,
        };
        use scalar_graph_input::aggregate_results::{home_layout, sum_layout};
        use scalar_graph_input::structural_fields::{
            byte_index_operands, view_slot_index_operands,
        };
        pub(crate) mod call_instructions {
            use super::super::{
                AbstractOperationPlan, Error, PsiOptimizationUnit, TargetOperationPlan,
            };
            use crate::LegalizationError;
            use crate::legalization::scalar_graph_input;
            use abstract_operations::AbstractOperation;
            use legalized_operations::{
                LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarInstructionKind,
            };
            use scalar_graph_input::normalized_foreign::native_callbacks_at;
            use semantic_vocabulary::OperationId;

            pub(crate) fn project_call<'a, 'g>(
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &'a PsiOptimizationUnit,
                targeted: &target_operations::TargetUnitOperation,
                call_descriptor_origins: &mut scalar_graph_input::byte_views::CallDescriptorOrigins<
                    'a,
                    'g,
                >,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError>
            where
                'a: 'g,
            {
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
                        indices: crate::legalization::runtime_indices::projection(
                            optimized,
                            unit,
                            *psi_operation,
                            argument.root_structural_type,
                            &argument.path,
                            &unit.structural_types,
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
                let mut descriptor_origins = match &structural_result {
                    Some(result) => call_descriptor_origins.resolve(optimized.machine, result)?,
                    None => Vec::new(),
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
                        target_operations::BoundaryRealization::HostedWriteByteI32(_) => {
                            LegalizedScalarInstructionKind::HostedWriteByteI32 {
                                boundary: *boundary,
                                source: *source,
                            }
                        }
                        _ => return Err(Error::custody()),
                    }
                };
                Ok(kind)
            }

            pub(crate) fn project_normalized_foreign_call(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                native: &TargetOperationPlan,
                plan: &AbstractOperationPlan,
                unit: &PsiOptimizationUnit,
                operation: OperationId,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::BoundaryCall { boundary, .. } = &node.operation else {
                    unreachable!("dispatched project_normalized_foreign_call")
                };
                let Some(target_operations::TargetUnitOperation::NormalizedForeignCall {
                    psi_operation,
                    boundary: row_boundary,
                    provider_execution,
                    binding,
                    scalar_arguments,
                    structural_arguments,
                    result,
                    descriptor_input,
                }) = scalar_graph_input::normalized_foreign::row(
                    native,
                    optimized.machine,
                    operation,
                )?
                else {
                    return Err(Error::custody());
                };
                if *psi_operation != operation || *row_boundary != *boundary {
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
                                crate::legalization::runtime_indices::projection(
                                    optimized,
                                    unit,
                                    operation,
                                    argument.root_structural_type,
                                    &argument.path,
                                    &plan.structural_types,
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
            use super::super::{Error, PsiOptimizationUnit};
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
                unit: &PsiOptimizationUnit,
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
                        source_type: *source_type,
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
                unit: &PsiOptimizationUnit,
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
                unit: &PsiOptimizationUnit,
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
                unit: &PsiOptimizationUnit,
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
                unit: &PsiOptimizationUnit,
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
                unit: &PsiOptimizationUnit,
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
                unit: &PsiOptimizationUnit,
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
                    let operand_type =
                        scalar_graph_input::value_type(optimized, *left).ok_or(Error::custody())?;
                    let right_operand_type =
                        scalar_graph_input::value_type(optimized, *right).ok_or(Error::custody())?;
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
            use super::super::{AbstractOperationPlan, Error, PsiOptimizationUnit};
            use crate::LegalizationError;
            use crate::legalization::accepted_fact;
            use crate::legalization::scalar_graph_input;
            use crate::structural_inputs::structural_reference_input::{
                element_leaf_projection, primitive_read_geometry, primitive_store,
                storage_address_geometry,
            };
            use abstract_operations::AbstractOperation;
            use legalized_operations::LegalizedScalarInstructionKind;
            use scalar_graph_input::structural_case::{
                membership_layout, source_result, structural_transport_layout,
            };
            use semantic_vocabulary::ScalarType;

            macro_rules! o {
                ($optimized:expr => $unit:expr => $operation:expr => $elements:expr) => {
                    crate::legalization::runtime_indices::operands(
                        $optimized, $unit, $operation, $elements,
                    )
                };
            }

            pub(crate) fn project_structural_case_membership(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                plan: &AbstractOperationPlan,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::StructuralCaseMembership {
                    psi_operation,
                    source,
                    path,
                    case,
                    result,
                } = &node.operation
                else {
                    unreachable!("dispatched project_structural_case_membership")
                };
                if result.scalar_type != ScalarType::Boolean {
                    return Err(Error::custody());
                }
                let (case_tag, tag_byte_offset, elements) = membership_layout(
                    optimized,
                    *source,
                    path,
                    *case,
                    plan,
                    source_result(optimized, *source),
                )?;
                let indices = o!(optimized => unit => *psi_operation => &elements)?;
                Ok(LegalizedScalarInstructionKind::StructuralCaseMembership {
                    source: *source,
                    path: path.clone(),
                    case: *case,
                    case_tag,
                    tag_byte_offset,
                    indices,
                })
            }

            pub(crate) fn project_structural_leaf_copy(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                plan: &AbstractOperationPlan,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let owned_path;
                let (psi_operation, result, source, path) = match &node.operation {
                    AbstractOperation::StructuralLeafCopy {
                        psi_operation,
                        result,
                        source,
                        path,
                    } => (*psi_operation, result, *source, path),
                    AbstractOperation::MoveStructuralValue {
                        psi_operation,
                        result,
                        source,
                    }
                    | AbstractOperation::TransformContent {
                        psi_operation,
                        result,
                        source,
                        ..
                    } => {
                        owned_path = source.path.clone().into();
                        (*psi_operation, result, source.place, &owned_path)
                    }
                    AbstractOperation::BoundaryCall {
                        psi_operation,
                        boundary,
                        arguments,
                        structural_arguments,
                        result: abstract_operations::AbstractBoundaryResult::Structural(result),
                        ..
                    } => {
                        let declaration = plan
                            .boundary_machines
                            .iter()
                            .find(|row| row.id == *boundary)
                            .ok_or(Error::custody())?;
                        let contract = terminal_semantics::empty_run_release_contract(
                            plan.structural_types.as_slice(),
                            declaration,
                        )
                        .map_err(|_| Error::custody())?;
                        let [source] = structural_arguments.as_slice() else {
                            return Err(Error::custody());
                        };
                        if !arguments.is_empty()
                            || source.access != terminal_psi::StructuralAccess::Owned
                        {
                            return Err(Error::custody());
                        }
                        let mut projection = source.path.clone();
                        projection
                            .push(terminal_psi::StructuralPathSegment::Field(contract.storage));
                        owned_path = projection.into();
                        (*psi_operation, result, source.place, &owned_path)
                    }
                    _ => return Err(Error::custody()),
                };
                let (byte_offset, shape, elements) = structural_transport_layout(
                    optimized,
                    psi_operation,
                    source,
                    path,
                    result,
                    plan,
                )?;
                let indices = o!(optimized => unit => psi_operation => &elements)?;
                Ok(LegalizedScalarInstructionKind::StructuralLeafCopy {
                    result: result.clone(),
                    source,
                    path: path.clone(),
                    byte_offset,
                    shape,
                    indices,
                })
            }

            pub(crate) fn project_borrowed_window(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                plan: &AbstractOperationPlan,
                unit: &PsiOptimizationUnit,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                match (&node.operation, targeted) {
                    (
                        AbstractOperation::MoveStructuralField {
                            psi_operation,
                            result,
                            source,
                            ..
                        },
                        target_operations::TargetUnitOperation::StructuralLeafCopy {
                            path,
                            byte_offset,
                            result_home,
                            ..
                        },
                    ) => {
                        let abstract_operations::StructuralCopyPath::Ordinary(selected) = path
                        else {
                            return Err(Error::custody());
                        };
                        Ok(LegalizedScalarInstructionKind::StructuralLeafCopy {
                            result: result.clone(),
                            source: source.place(),
                            path: path.clone(),
                            byte_offset: *byte_offset,
                            shape: result_home.layout.shape(),
                            indices: crate::legalization::runtime_indices::projection(
                                optimized,
                                unit,
                                *psi_operation,
                                source.structural_type(),
                                selected,
                                &plan.structural_types,
                            )?,
                        })
                    }
                    (
                        AbstractOperation::StoreStructuralField { psi_operation, .. },
                        target_operations::TargetUnitOperation::StoreStructuralField {
                            destination,
                            path,
                            field,
                            value,
                            byte_offset,
                            ..
                        },
                    ) => {
                        let value_type =
                            scalar_graph_input::primitive_locals::read_root_type(optimized, *value)
                                .ok_or(Error::custody())?;
                        let shape =
                            crate::structural_inputs::shape(value_type, &plan.structural_types)
                                .ok_or(Error::custody())?;
                        Ok(LegalizedScalarInstructionKind::StoreStructuralField {
                            destination: destination.clone(),
                            path: path.clone(),
                            field: *field,
                            value: *value,
                            byte_offset: *byte_offset,
                            shape,
                            indices: crate::legalization::runtime_indices::projection(
                                optimized,
                                unit,
                                *psi_operation,
                                destination.structural_type(),
                                path,
                                &plan.structural_types,
                            )?,
                        })
                    }
                    _ => Err(Error::custody()),
                }
            }

            pub(crate) fn project_primitive_scalar_read(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::PrimitiveScalarRead {
                    psi_operation,
                    result,
                    source,
                    path,
                } = &node.operation
                else {
                    unreachable!("dispatched project_primitive_scalar_read")
                };
                let indices = if terminal_psi::is_static_structural_path(path) {
                    Vec::new()
                } else {
                    let root_type =
                        scalar_graph_input::primitive_locals::read_root_type(optimized, *source)
                            .ok_or(Error::custody())?;
                    let geometry = primitive_read_geometry(
                        root_type,
                        path,
                        result.scalar_type,
                        &unit.structural_types,
                    )
                    .ok_or(Error::custody())?;
                    o!(optimized => unit => *psi_operation => &geometry.elements)?
                };
                Ok(LegalizedScalarInstructionKind::PrimitiveScalarRead {
                    source: *source,
                    path: path.clone(),
                    indices,
                })
            }

            pub(crate) fn project_write_only_primitive_store(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::WriteOnlyPrimitiveStore {
                    psi_operation,
                    destination,
                    path,
                    value,
                    ..
                } = &node.operation
                else {
                    unreachable!("dispatched project_write_only_primitive_store")
                };
                let geometry =
                    primitive_store(destination, path, value.scalar_type, &unit.structural_types)
                        .ok_or(Error::custody())?;
                Ok(LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                    destination: destination.clone(),
                    path: path.clone(),
                    value: *value,
                    byte_offset: geometry.byte_offset,
                    byte_size: u8::try_from(geometry.byte_size).map_err(|_| Error::custody())?,
                    indices: o!(optimized => unit => *psi_operation => &geometry.elements)?,
                })
            }

            pub(crate) fn project_structural_scalar_field_store(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::StructuralScalarFieldStore {
                    psi_operation,
                    field,
                    value,
                    ..
                } = &node.operation
                else {
                    unreachable!("dispatched project_structural_scalar_field_store")
                };
                let target_operations::TargetUnitOperation::StructuralScalarFieldStore {
                    destination,
                    path,
                    field_byte_offset,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                let (_, byte_size, elements) =
                    crate::structural_inputs::structural_reference_input::store(
                        destination.structural_type(),
                        path,
                        *field,
                        value.scalar_type,
                        &unit.structural_types,
                    )
                    .ok_or(Error::custody())?;
                Ok(LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                    destination: destination.clone(),
                    path: path.clone(),
                    field: *field,
                    value: *value,
                    byte_offset: *field_byte_offset,
                    byte_size,
                    indices: o!(optimized => unit => *psi_operation => &elements)?,
                })
            }

            fn view_indices(
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
                psi_operation: semantic_vocabulary::OperationId,
                view: &terminal_psi::StructuralArgument,
            ) -> Result<Vec<legalized_operations::LegalizedRuntimeIndexOperand>, LegalizationError>
            {
                o!(optimized => unit => psi_operation => &scalar_graph_input::element_views::descriptor_geometry(
                        optimized,
                        view,
                        &unit.structural_types,
                    )
                    .ok_or(LegalizationError::custody())?
                    .elements)
            }

            pub(crate) fn project_subslice(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let (AbstractOperation::ByteSequenceSubslice {
                    psi_operation,
                    result,
                    source,
                    start,
                    end,
                    length,
                    obligation,
                }
                | AbstractOperation::ElementViewSubslice {
                    psi_operation,
                    result,
                    source,
                    start,
                    end,
                    length,
                    obligation,
                }) = &node.operation
                else {
                    return Err(Error::custody());
                };
                let indices = view_indices(optimized, unit, *psi_operation, source)?;
                let accepted_fact = accepted_fact(unit, optimized, *psi_operation, *obligation)?;
                let kind = if matches!(
                    node.operation,
                    AbstractOperation::ElementViewSubslice { .. }
                ) {
                    LegalizedScalarInstructionKind::ElementViewSubslice {
                        result: result.clone(),
                        source: source.clone(),
                        indices,
                        start: *start,
                        end: *end,
                        length: *length,
                        obligation: *obligation,
                        accepted_fact,
                    }
                } else {
                    LegalizedScalarInstructionKind::ByteSequenceSubslice {
                        result: result.clone(),
                        source: source.clone(),
                        indices,
                        start: *start,
                        end: *end,
                        length: *length,
                        obligation: *obligation,
                        accepted_fact,
                    }
                };
                Ok(kind)
            }

            pub(crate) fn project_structural_byte_sequence_field_byte_store(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
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
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::StructuralByteSequenceFieldRead {
                    psi_operation,
                    field,
                    index,
                    length,
                    obligation,
                    ..
                } = &node.operation
                else {
                    unreachable!("dispatched project_structural_byte_sequence_field_read")
                };
                let source = crate::legalization::scalar_graph_input::structural_fields::byte_read(
                    optimized,
                    &node.operation,
                    &unit.structural_types,
                )
                .ok_or(Error::custody())?;
                Ok(
                    LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
                        indices: scalar_graph_input::structural_fields::byte_index_operands(
                            optimized,
                            unit,
                            *psi_operation,
                            &source,
                            *field,
                        )?,
                        source,
                        field: *field,
                        index: *index,
                        length: *length,
                        obligation: *obligation,
                        accepted_fact: accepted_fact(unit, optimized, *psi_operation, *obligation)?,
                    },
                )
            }

            pub(crate) fn project_structural_byte_sequence_field_store(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
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

            pub(crate) fn project_byte_sequence_write(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::ByteSequenceWrite {
                    psi_operation,
                    destination,
                    index,
                    value,
                    length,
                    obligation,
                } = &node.operation
                else {
                    unreachable!("dispatched project_byte_sequence_write")
                };
                Ok(LegalizedScalarInstructionKind::ByteSequenceWrite {
                    destination: *destination,
                    index: *index,
                    value: *value,
                    length: *length,
                    obligation: *obligation,
                    accepted_fact: accepted_fact(unit, optimized, *psi_operation, *obligation)?,
                })
            }

            pub(crate) fn project_byte_sequence_read(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::ByteSequenceRead {
                    psi_operation,
                    source,
                    index,
                    length,
                    obligation,
                    ..
                } = &node.operation
                else {
                    unreachable!("dispatched project_byte_sequence_read")
                };
                Ok(LegalizedScalarInstructionKind::ByteSequenceRead {
                    source: *source,
                    index: *index,
                    length: *length,
                    obligation: *obligation,
                    accepted_fact: accepted_fact(unit, optimized, *psi_operation, *obligation)?,
                })
            }

            pub(crate) fn project_element_view_write(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
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
                    indices: view_indices(optimized, unit, *psi_operation, destination)?,
                    index: *index,
                    value: *value,
                    length: *length,
                    obligation: *obligation,
                    accepted_fact: accepted_fact(unit, optimized, *psi_operation, *obligation)?,
                })
            }

            pub(crate) fn project_element_view_read(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::ElementViewRead {
                    psi_operation,
                    source,
                    index,
                    length,
                    obligation,
                    path,
                    result,
                    ..
                } = &node.operation
                else {
                    unreachable!("dispatched project_element_view_read")
                };
                Ok(LegalizedScalarInstructionKind::ElementViewRead {
                    source: *source,
                    index: *index,
                    length: *length,
                    obligation: *obligation,
                    accepted_fact: accepted_fact(unit, optimized, *psi_operation, *obligation)?,
                    path: path.clone(),
                    indices: {
                        let element = scalar_graph_input::byte_views::element_type(
                            optimized,
                            *source,
                            &unit.structural_types,
                        )
                        .ok_or(Error::custody())?;
                        let (_, _, elements) = element_leaf_projection(
                            element,
                            path,
                            result.scalar_type,
                            &unit.structural_types,
                        )
                        .ok_or(Error::custody())?;
                        o!(optimized => unit => *psi_operation => &elements)?
                    },
                })
            }

            pub(crate) fn project_byte_sequence_concatenation(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
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
                unit: &PsiOptimizationUnit,
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
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
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
                let root = scalar_graph_input::primitive_locals::read_root_type(optimized, *source)
                    .ok_or(Error::custody())?;
                let geometry = storage_address_geometry(root, path, &unit.structural_types)
                    .ok_or(Error::custody())?;
                let indices = o!(optimized => unit => *psi_operation => &geometry.elements)?;
                Ok(LegalizedScalarInstructionKind::StorageAddress {
                    source: *source,
                    path: path.clone(),
                    indices,
                })
            }

            pub(crate) fn project_global_descriptor_table(
                node: &optimization_unit::OptimizationNode,
                optimized: &optimization_unit::PsiOptimizationFunction,
                unit: &PsiOptimizationUnit,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let AbstractOperation::GlobalDescriptorTable {
                    psi_operation,
                    service,
                    operation,
                    source,
                } = &node.operation
                else {
                    unreachable!("dispatched descriptor-table operation")
                };
                let root =
                    scalar_graph_input::primitive_locals::read_root_type(optimized, source.place)
                        .ok_or(Error::custody())?;
                let geometry = storage_address_geometry(root, &source.path, &unit.structural_types)
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
        }

        use crate::LegalizationError;
        use crate::legalization::accepted_fact;
        use crate::legalization::scalar_graph_input;
        use semantic_vocabulary::IntegerValue;
        pub(crate) fn project<'a, 'g>(
            node: &optimization_unit::OptimizationNode,
            optimized: &optimization_unit::PsiOptimizationFunction,
            native: &'a TargetOperationPlan,
            plan: &'a AbstractOperationPlan,
            unit: &'a PsiOptimizationUnit,
            targeted: &target_operations::TargetUnitOperation,
            call_descriptor_origins: &mut scalar_graph_input::byte_views::CallDescriptorOrigins<
                'a,
                'g,
            >,
        ) -> Result<LegalizedScalarInstruction, LegalizationError>
        where
            'a: 'g,
        {
            let (operation, result) =
                scalar_graph_input::instruction(node).ok_or(Error::custody())?;
            let kind = if matches!(
                targeted,
                target_operations::TargetUnitOperation::Call { .. }
            ) {
                call_instructions::project_call(optimized, unit, targeted, call_descriptor_origins)?
            } else if matches!(
                targeted,
                target_operations::TargetUnitOperation::StructuralLeafCopy { .. }
            ) && matches!(node.operation, AbstractOperation::BoundaryCall { .. })
            {
                storage_instructions::project_structural_leaf_copy(node, optimized, plan, unit)?
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
                            optimized,
                            argument.place,
                        )
                        .ok_or(Error::custody())?;
                        projection(
                            optimized,
                            unit,
                            operation,
                            root,
                            &argument.path,
                            &unit.structural_types,
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
                            &unit.structural_types,
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
                    scalar_graph_input::primitive_locals::read_root_type(optimized, token.place)
                        .ok_or(Error::custody())?;
                let indices = projection(
                    optimized,
                    unit,
                    operation,
                    root,
                    &token.path,
                    &unit.structural_types,
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
                        optimized,
                        argument.place,
                    )
                    .ok_or(Error::custody())?;
                    indices.extend(projection(
                        optimized,
                        unit,
                        operation,
                        root,
                        &argument.path,
                        &unit.structural_types,
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
                let geometry = reference_source_geometry(
                    root,
                    &location.path,
                    unit.structural_types.as_slice(),
                )
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
                        let access = scalar_graph_input::atomic_access::access(
                            event,
                            optimized,
                            &unit.structural_types,
                        )
                        .ok_or(Error::custody())?;
                        LegalizedScalarInstructionKind::AtomicAccess {
                            location: access.location,
                            scalar_type: access.scalar_type,
                            field_byte_offset: access.field_byte_offset,
                            event: access.event,
                            outcome: access.outcome,
                        }
                    }
                    AbstractOperation::StructuralByteSequenceFieldLength { .. } => {
                        let target_operations::TargetUnitOperation::StructuralByteSequenceFieldLength {
                            source,
                            field,
                            ..
                        } = targeted
                        else {
                            return Err(Error::custody());
                        };
                        LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                            indices: byte_index_operands(
                                optimized, unit, operation, source, *field,
                            )?,
                            source: source.clone(),
                            field: *field,
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
                        source_type: *source_type,
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
                    AbstractOperation::EstablishAccessReference {
                        psi_operation,
                        result,
                        source,
                        correspondence,
                    } => LegalizedScalarInstructionKind::EstablishAccessReference {
                        result: result.clone(),
                        source: source.clone(),
                        correspondence: correspondence.clone(),
                        indices: reference_source(optimized, unit, *psi_operation, source)?,
                        shape: home_layout(result, plan)?.shape(),
                    },
                    AbstractOperation::EstablishReference {
                        psi_operation,
                        result,
                        source,
                        storage,
                        ..
                    } => LegalizedScalarInstructionKind::EstablishReference {
                        result: result.clone(),
                        source: source.clone(),
                        storage: *storage,
                        indices: reference_source(optimized, unit, *psi_operation, source)?,
                        shape: home_layout(result, plan)?.shape(),
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
                    } => LegalizedScalarInstructionKind::EstablishActivationClaim {
                        result: result.clone(),
                        bound: *bound,
                        committed: *committed,
                        backing_type: *backing_type,
                        established_case: *established_case,
                        rejected_case: *rejected_case,
                        layout: sum_layout(result, plan)?,
                    },
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
                    } => LegalizedScalarInstructionKind::SplitActivationClaim {
                        result: result.clone(),
                        source: source.clone(),
                        claim: *claim,
                        bound_at: *bound_at,
                        committed_at: *committed_at,
                        covered_case: *covered_case,
                        rejected_case: *rejected_case,
                        layout: sum_layout(result, plan)?,
                    },
                    AbstractOperation::MergeActivationClaim {
                        result,
                        lower,
                        upper,
                        lower_claim,
                        upper_claim,
                        covered_case,
                        rejected_case,
                        ..
                    } => LegalizedScalarInstructionKind::MergeActivationClaim {
                        result: result.clone(),
                        lower: lower.clone(),
                        upper: upper.clone(),
                        lower_claim: *lower_claim,
                        upper_claim: *upper_claim,
                        covered_case: *covered_case,
                        rejected_case: *rejected_case,
                        layout: sum_layout(result, plan)?,
                    },
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
                    } => LegalizedScalarInstructionKind::EstablishCase {
                        result: result.clone(),
                        result_case: *result_case,
                        fields: fields.clone(),
                        layout: sum_layout(result, plan)?,
                    },
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
                    AbstractOperation::StructuralCaseMembership { .. } => {
                        storage_instructions::project_structural_case_membership(
                            node, optimized, plan, unit,
                        )?
                    }
                    AbstractOperation::StructuralLeafCopy { .. }
                    | AbstractOperation::MoveStructuralValue { .. }
                    | AbstractOperation::TransformContent { .. } => {
                        storage_instructions::project_structural_leaf_copy(
                            node, optimized, plan, unit,
                        )?
                    }
                    abstract_operations::move_or_store_structural_field_pattern!() => {
                        storage_instructions::project_borrowed_window(
                            node, optimized, plan, unit, targeted,
                        )?
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
                                optimized,
                                argument.place,
                            )
                            .ok_or(Error::custody())?;
                            *collected = projection(
                                optimized,
                                unit,
                                *psi_operation,
                                root,
                                &argument.path,
                                &unit.structural_types,
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
                                optimized,
                                argument.place,
                            )
                            .ok_or(Error::custody())?;
                            *collected = projection(
                                optimized,
                                unit,
                                *psi_operation,
                                root,
                                &argument.path,
                                &unit.structural_types,
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
                            node, optimized, unit,
                        )?
                    }
                    AbstractOperation::StorageAddress { .. } => {
                        storage_instructions::project_storage_address(node, optimized, unit)?
                    }
                    AbstractOperation::PrimitiveScalarRead { .. } => {
                        storage_instructions::project_primitive_scalar_read(node, optimized, unit)?
                    }
                    AbstractOperation::BoundaryCall { .. }
                        if scalar_graph_input::normalized_foreign::row(
                            native,
                            optimized.machine,
                            operation,
                        )?
                        .is_some() =>
                    {
                        call_instructions::project_normalized_foreign_call(
                            node, optimized, native, plan, unit, operation,
                        )?
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
                        target_operations::BoundaryRealization::HostedReadByte(_)
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
                    AbstractOperation::WriteOnlyPrimitiveStore { .. } => {
                        storage_instructions::project_write_only_primitive_store(
                            node, optimized, unit,
                        )?
                    }
                    AbstractOperation::StructuralScalarFieldStore { .. } => {
                        storage_instructions::project_structural_scalar_field_store(
                            node, optimized, unit, targeted,
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
                    AbstractOperation::ByteSequenceSubslice { .. } => {
                        storage_instructions::project_subslice(node, optimized, unit)?
                    }
                    AbstractOperation::StructuralByteSequenceFieldByteStore { .. } => {
                        storage_instructions::project_structural_byte_sequence_field_byte_store(
                            node, optimized, unit, targeted,
                        )?
                    }
                    AbstractOperation::StructuralByteSequenceFieldRead { .. } => {
                        storage_instructions::project_structural_byte_sequence_field_read(
                            node, optimized, unit,
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
                    AbstractOperation::StructuralByteViewFieldStore { field, source, .. } => {
                        let target_operations::TargetUnitOperation::StructuralByteViewFieldStore {
                            destination,
                            ..
                        } = targeted
                        else {
                            return Err(Error::custody());
                        };
                        LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
                            indices: view_slot_index_operands(
                                optimized,
                                unit,
                                operation,
                                destination,
                                *field,
                            )?,
                            destination: destination.clone(),
                            field: *field,
                            source: *source,
                        }
                    }
                    AbstractOperation::ByteSequenceWrite { .. } => {
                        storage_instructions::project_byte_sequence_write(node, optimized, unit)?
                    }
                    AbstractOperation::ByteSequenceRead { .. } => {
                        storage_instructions::project_byte_sequence_read(node, optimized, unit)?
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
                    AbstractOperation::ByteSequenceLength { source, .. } => {
                        if let Some(length) = scalar_graph_input::element_views::fixed_byte_length(
                            optimized,
                            source,
                            &unit.structural_types,
                        ) {
                            LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(
                                u128::from(length),
                            ))
                        } else {
                            let (geometry, carrier) =
                            crate::legalization::scalar_graph_input::element_views::byte_length_geometry(
                                optimized,
                                source,
                                &unit.structural_types,
                            )
                            .ok_or(LegalizationError::custody())?;
                            LegalizedScalarInstructionKind::ByteSequenceLength {
                                source: source.clone(),
                                indices: crate::legalization::runtime_indices::operands(
                                    optimized,
                                    unit,
                                    node.operation
                                        .psi_operation()
                                        .ok_or(LegalizationError::custody())?,
                                    &geometry.elements,
                                )?,
                                length_byte_offset: if matches!(
                                    carrier,
                                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                ) {
                                    8
                                } else {
                                    0
                                },
                            }
                        }
                    }
                    AbstractOperation::ElementViewLength { source, .. } => {
                        let geometry =
                            crate::legalization::scalar_graph_input::element_views::length_geometry(
                                optimized,
                                source,
                                &unit.structural_types,
                            )
                            .ok_or(LegalizationError::custody())?;
                        LegalizedScalarInstructionKind::ElementViewLength {
                            source: source.clone(),
                            indices: crate::legalization::runtime_indices::operands(
                                optimized,
                                unit,
                                node.operation
                                    .psi_operation()
                                    .ok_or(LegalizationError::custody())?,
                                &geometry.elements,
                            )?,
                            length_byte_offset: 8,
                        }
                    }
                    AbstractOperation::ElementViewRead { .. } => {
                        storage_instructions::project_element_view_read(node, optimized, unit)?
                    }
                    AbstractOperation::ElementViewWrite { .. } => {
                        storage_instructions::project_element_view_write(node, optimized, unit)?
                    }
                    AbstractOperation::ElementViewSubslice { .. } => {
                        storage_instructions::project_subslice(node, optimized, unit)?
                    }
                    AbstractOperation::EstablishElementView {
                        psi_operation,
                        result,
                        destination,
                        source,
                        element,
                        region,
                    } => {
                        let accepted_fact = match region {
                            terminal_psi::ElementViewRegion::WholeCollection => None,
                            terminal_psi::ElementViewRegion::WholeRepresentation {
                                obligation,
                                ..
                            }
                            | terminal_psi::ElementViewRegion::RepresentationBytes {
                                obligation,
                                ..
                            } => Some(accepted_fact(unit, optimized, *psi_operation, *obligation)?),
                        };
                        LegalizedScalarInstructionKind::EstablishElementView {
                            indices: collection_view(optimized, unit, *psi_operation, source)?,
                            result: result.clone(),
                            destination: *destination,
                            source: source.clone(),
                            element: *element,
                            region: *region,
                            accepted_fact,
                        }
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
                        source_type: *source_type,
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
                        scalar_instructions::project_boolean_equal(node, optimized)?
                    }
                    _ => return Err(Error::custody()),
                }
            };
            Ok(LegalizedScalarInstruction {
                operation,
                result: result.map(|value| LegalizedValueDefinition {
                    value,
                    scalar_type: node.definitions[0].scalar_type,
                    definition_site: node.definitions[0].site,
                }),
                kind,
                work: node.work.clone(),
                effect: node.effect,
                ownership: node.ownership.clone(),
            })
        }
    }
    pub(crate) mod structural_case {
        use super::{AbstractOperationPlan, Error, scalar_graph_input};
        use crate::LegalizationError;
        use abstract_operations::AbstractOperation;
        use legalized_operations::{
            LegalizedScalarTerminator, LegalizedStructuralCasePayload,
            LegalizedStructuralCaseSuccessor, LegalizedValueDefinition,
        };

        pub(crate) fn project(
            node: &optimization_unit::OptimizationNode,
            function: &optimization_unit::PsiOptimizationFunction,
            plan: &AbstractOperationPlan,
            unit: &optimization_unit::PsiOptimizationUnit,
        ) -> Result<LegalizedScalarTerminator, LegalizationError> {
            let AbstractOperation::StructuralCase {
                source,
                path,
                cases,
            } = &node.operation
            else {
                return Err(Error::custody());
            };
            let (selected, byte_offset, root_shape, layout) =
                scalar_graph_input::structural_case::dispatch_projection(
                    function, *source, path, plan,
                )?;
            let source = scalar_graph_input::structural_case::case_source(function, *source)?;
            let declaration = plan.structural_type(selected).ok_or(Error::custody())?;
            let declared: &[terminal_psi::StructuralCaseDeclaration] = match &declaration.shape {
                terminal_psi::StructuralTypeShape::Sum { cases }
                | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => cases,
                _ => return Err(Error::custody()),
            };
            if cases.len() != declared.len()
                || cases.len() != node.successors.len()
                || cases.len() != layout.cases.len()
            {
                return Err(Error::custody());
            }
            let mut successors = Vec::with_capacity(cases.len());
            for (case_position, ((case, edge), declared)) in
                cases.iter().zip(&node.successors).zip(declared).enumerate()
            {
                let destination = function.block(case.target).ok_or(Error::custody())?;
                let fields = declared
                    .fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .collect::<Vec<_>>();
                if case.payloads.len() != destination.parameters.len()
                    || fields.len() != layout.cases[case_position].fields.len()
                {
                    return Err(Error::custody());
                }
                let mut payloads = Vec::with_capacity(case.payloads.len());
                for (payload, parameter) in case.payloads.iter().zip(&destination.parameters) {
                    let field_position = fields
                        .iter()
                        .position(|field| field.id == payload.field)
                        .ok_or(Error::custody())?;
                    payloads.push(LegalizedStructuralCasePayload {
                        field: payload.field,
                        field_byte_offset: byte_offset
                            .checked_add(
                                layout.cases[case_position].fields[field_position].byte_offset,
                            )
                            .ok_or(Error::custody())?,
                        parameter: LegalizedValueDefinition {
                            value: parameter.value,
                            scalar_type: parameter.scalar_type,
                            definition_site: parameter.site,
                        },
                    });
                }
                successors.push(LegalizedStructuralCaseSuccessor {
                    edge: case.psi_edge,
                    target: case.target,
                    case: case.case,
                    case_tag: i32::try_from(case_position).map_err(|_| Error::custody())?,
                    payloads,
                    structural_bindings: super::terminator::successor(edge, function, plan, unit)?
                        .structural_bindings,
                    cleanup_actions: case.cleanup_actions.clone(),
                    cleanup_calls: Vec::new(),
                    work: edge.work.clone(),
                });
            }
            Ok(LegalizedScalarTerminator::StructuralCase {
                source,
                path: path.clone(),
                root_shape,
                byte_offset,
                layout,
                cases: successors,
                effect: node.effect,
                ownership: node.ownership.clone(),
            })
        }
    }
    pub(crate) mod terminator {
        use super::{AbstractOperationPlan, Error};
        use crate::LegalizationError;
        use crate::legalization::scalar_graph_input;
        use abstract_operations::AbstractOperation;
        use legalized_operations::{
            LegalizedScalarReturn, LegalizedScalarReturnValue, LegalizedScalarSuccessor,
            LegalizedScalarTerminator,
        };
        pub(crate) fn project(
            node: &optimization_unit::OptimizationNode,
            function_inputs: &crate::legalization_function_inputs::FunctionInputs<'_>,
            plan: &AbstractOperationPlan,
            native: &target_operations::TargetOperationPlan,
            unit: &optimization_unit::PsiOptimizationUnit,
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
                            indices: crate::legalization::runtime_indices::projection(
                                function,
                                unit,
                                optimization_unit::PsiProvenance::Edge(*psi_edge),
                                argument.root_structural_type,
                                &argument.path,
                                &unit.structural_types,
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
                        function,
                        *source,
                        &plan.structural_types,
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
                    super::structural_case::project(node, function, plan, unit)
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
                            scalar_type: *scalar_type,
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
                        successor: successor(edge, function, plan, unit)?,
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
                        when_true: successor(when_true, function, plan, unit)?,
                        when_false: successor(when_false, function, plan, unit)?,
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
            function: &optimization_unit::PsiOptimizationFunction,
            plan: &AbstractOperationPlan,
            unit: &optimization_unit::PsiOptimizationUnit,
        ) -> Result<LegalizedScalarSuccessor, LegalizationError> {
            let structural_bindings = edge.structural_bindings.iter().map(|binding| {
                let indices = if binding.argument.path.iter().any(|segment| segment.runtime_index().is_some()) {
                    let root = scalar_graph_input::primitive_locals::read_root_type(function, binding.argument.place)
                        .ok_or(Error::custody())?;
                    let geometry = crate::structural_inputs::structural_reference_input::reference_formation_geometry(
                        root, &binding.argument.path, &plan.structural_types,
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
    pub(crate) fn derive<'a, 'g>(
        target: &target_operations::TargetFunction,
        abstracted: &abstract_operations::AbstractFunction,
        optimized: &optimization_unit::PsiOptimizationFunction,
        native: &'a TargetOperationPlan,
        plan: &'a AbstractOperationPlan,
        unit: &'a PsiOptimizationUnit,
        current_operation: &mut Option<semantic_vocabulary::OperationId>,
        call_descriptor_origins: &mut scalar_graph_input::byte_views::CallDescriptorOrigins<'a, 'g>,
    ) -> Result<LegalizedScalarFunction, LegalizationError>
    where
        'a: 'g,
    {
        let call_plan = target.graph.call_plan.clone();
        let parameters = optimized
            .parameters
            .iter()
            .zip(&call_plan.parameters)
            .map(|(parameter, placement)| {
                Ok(LegalizedScalarParameter {
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                    definition_site: parameter.site,
                    placement: placement.clone(),
                })
            })
            .collect::<Result<Vec<_>, LegalizationError>>()?;
        if optimized.blocks.len() != target.graph.blocks.len() {
            return Err(Error::custody());
        }
        let function_inputs = crate::legalization_function_inputs::FunctionInputs::new(optimized);
        let blocks = optimized
            .blocks
            .iter()
            .zip(&target.graph.blocks)
            .map(|(block, targeted)| {
                *current_operation = None;
                let (last, body) = block.nodes.split_last().ok_or(Error::custody())?;
                let rows = targeted
                    .operations
                    .split_at(
                        targeted
                            .operations
                            .iter()
                            .position(|row| {
                                matches!(
                                    row,
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
                    *current_operation =
                        scalar_graph_input::instruction(node).map(|(operation, _)| operation);
                    instructions.push(instruction::project(
                        node,
                        optimized,
                        native,
                        plan,
                        unit,
                        row,
                        call_descriptor_origins,
                    )?);
                }
                *current_operation = None;
                Ok(LegalizedScalarBlock {
                    id: block.id,
                    parameters: block.parameters.clone(),
                    structural_parameters: block.structural_parameters.clone(),
                    instructions,
                    terminator: terminator::project(last, &function_inputs, plan, native, unit)?,
                })
            })
            .collect::<Result<Vec<_>, LegalizationError>>()?;
        Ok(LegalizedScalarFunction {
            machine: target.machine,
            attachment: target.attachment,
            nominal_cleanup_subject: target.nominal_cleanup_subject,
            provenance: target.provenance.clone(),
            call_plan,
            parameters,
            structural: scalar_graph_input::structural_contract(
                target, abstracted, optimized, plan,
            ),
            entry_block: optimized.entry,
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
) -> Result<SourceFunctionRosters, LegalizationError> {
    validate_source_custody(target, abstract_plan, unit)?;
    crate::legalization::scalar_graph_input::normalized_foreign::validate_native_callback_roster(
        target,
    )?;

    let mut rosters = SourceFunctionRosters {
        scalar_functions: Vec::new(),
    };
    ordinary_roster::derive_remaining(&mut rosters, target, abstract_plan, unit)?;

    Ok(rosters)
}
pub(crate) use custody::validate_source_custody;
