#![forbid(unsafe_code)]

//! Resolve source-independent terminal Omega requirements into the first
//! target operation slice.

use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::{
    CallSignature, CallingPolicy, PlanDiagnostic, ValueLocation, ValuePlacement, ValueShape,
    evaluate_call_plan,
};
use omega_target::{Architecture, NativeTarget};
use omega_terminal_abstract_operations::{
    TerminalAbstractFunction, TerminalAbstractFunctionResult, TerminalAbstractOperation,
    TerminalAbstractOperationPlan, TerminalAbstractParameter, TerminalAbstractResult,
};
use omega_terminal_target_operations::{
    TerminalBoundaryRealization, TerminalBoundarySettlementBinding, TerminalPsiProvenance,
    TerminalScalarParameterLocation, TerminalTargetBooleanControl, TerminalTargetBooleanExpression,
    TerminalTargetCallArgument, TerminalTargetConditionalBooleanArm,
    TerminalTargetConditionalIntegerArm, TerminalTargetFunction, TerminalTargetIntegerControl,
    TerminalTargetIntegerExpression, TerminalTargetOperation, TerminalTargetOperationPlan,
    TerminalTargetScalarExpression, TerminalTargetStructuralArgument,
    TerminalTargetStructuralParameter, TerminalTargetUnitBody, TerminalTargetUnitOperation,
};
use psi_core::{
    BlockId, BoundaryMachineId, EdgeId, IeeeFloatFormat, IntegerSign, IntegerType, IntegerValue,
    MachineId, OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

/// One boundary realization sourced from a validated, admitted provider
/// execution. Callers supply the exact target mechanism but cannot substitute
/// a secondary provider-plan identity.
#[derive(Debug, Clone, Copy)]
pub struct AdmittedTerminalBoundarySettlement<'execution> {
    pub boundary: BoundaryMachineId,
    pub provider_execution:
        &'execution dyn omega_terminal_installation_evidence::TerminalProviderExecutionEvidence,
    pub realization: TerminalBoundaryRealization,
}

pub fn lower_to_target_operations(
    plan: &TerminalAbstractOperationPlan,
    target: NativeTarget,
) -> Result<TerminalTargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_settlements(plan, target, &[])
}

/// Lower an effectful terminal plan using the exact provider executions
/// already admitted by the external-root ledger.
pub fn lower_to_target_operations_with_provider_executions(
    plan: &TerminalAbstractOperationPlan,
    target: NativeTarget,
    settlements: &[AdmittedTerminalBoundarySettlement<'_>],
) -> Result<TerminalTargetOperationPlan, LoweringError> {
    let bindings = settlements
        .iter()
        .map(|settlement| {
            let provider_plan = omega_terminal_target_operations::TerminalProviderPlanIdentity::new(
                settlement.provider_execution.provider_plan(),
            )
            .ok_or_else(|| LoweringError::ProviderExecutionBinding("zero provider plan".into()))?;
            let provider_execution =
                omega_terminal_target_operations::TerminalProviderExecutionBinding::from_execution_record(
                    provider_plan,
                    settlement.provider_execution.provider_execution_identity(),
                    settlement.provider_execution.provider_execution_fingerprint(),
                    settlement.provider_execution.normalized_root_identity(),
                    settlement.provider_execution.boundary_contract_fingerprint(),
                )
                .ok_or_else(|| {
                    LoweringError::ProviderExecutionBinding(
                        "admitted provider execution contains a zero identity".into(),
                    )
                })?;
            Ok(TerminalBoundarySettlementBinding {
                boundary: settlement.boundary,
                provider_execution,
                realization: settlement.realization,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    lower_to_target_operations_with_settlements(plan, target, &bindings)
}

fn lower_to_target_operations_with_settlements(
    plan: &TerminalAbstractOperationPlan,
    target: NativeTarget,
    settlement_bindings: &[TerminalBoundarySettlementBinding],
) -> Result<TerminalTargetOperationPlan, LoweringError> {
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(LoweringError::EntryFunctionMissing(plan.entry));
    }
    let functions_by_machine = plan
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<BTreeMap<_, _>>();
    let structural_types = plan
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    let mut settlements_by_boundary = BTreeMap::new();
    for binding in settlement_bindings {
        if settlements_by_boundary
            .insert(binding.boundary, *binding)
            .is_some()
        {
            return Err(LoweringError::DuplicateBoundarySettlement(binding.boundary));
        }
        if !plan
            .boundary_machines
            .iter()
            .any(|boundary| boundary.id == binding.boundary)
        {
            return Err(LoweringError::UnknownBoundarySettlement(binding.boundary));
        }
    }
    let required_settlements = plan
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .filter_map(|operation| match operation {
            TerminalAbstractOperation::BoundaryCall { boundary, .. } => Some(*boundary),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for boundary in &required_settlements {
        if !settlements_by_boundary.contains_key(boundary) {
            return Err(LoweringError::MissingBoundarySettlement(*boundary));
        }
    }
    if let Some(extra) = settlements_by_boundary
        .keys()
        .find(|boundary| !required_settlements.contains(boundary))
    {
        return Err(LoweringError::UnusedBoundarySettlement(*extra));
    }
    Ok(TerminalTargetOperationPlan {
        terminal_psi: plan.terminal_psi,
        target,
        entry: plan.entry,
        functions: plan
            .functions
            .iter()
            .map(|function| {
                lower_function(
                    function,
                    target,
                    &functions_by_machine,
                    &structural_types,
                    &settlements_by_boundary,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_function(
    function: &TerminalAbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, TerminalBoundarySettlementBinding>,
) -> Result<TerminalTargetFunction, LoweringError> {
    if let Some(result) = function.result.structural() {
        return lower_structural_return_function(function, result, target, structural_types);
    }
    let Some(function_result) = function.result.scalar() else {
        return lower_unit_function(function, target, functions, structural_types, settlements);
    };
    let mut values = BTreeMap::new();
    let mut provenance = TerminalPsiProvenance::default();
    let mut returned = None;
    let scalar_parameter_shapes = function
        .parameters
        .iter()
        .map(|parameter| scalar_shape(parameter.value, parameter.scalar_type, true))
        .collect::<Result<Vec<_>, _>>()?;
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let structural_parameter_shapes = function
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            if usize::try_from(parameter.position) != Ok(position)
                || parameter.is_self
                || !parameter.qualifications.is_empty()
                || parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine
            {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
            structural_shape(
                parameter.structural_type,
                structural_types,
                &mut shape_cache,
                &mut active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = CallSignature {
        parameters: scalar_parameter_shapes
            .iter()
            .copied()
            .chain(structural_parameter_shapes.iter().copied())
            .collect(),
        result: Some(scalar_shape(
            function_result.value,
            function_result.scalar_type,
            false,
        )?),
    };
    let call_plan = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
        .map_err(LoweringError::AbiPlan)?;
    if call_plan.parameters.len()
        != function.parameters.len() + function.structural_parameters.len()
    {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: function.parameters.len() + function.structural_parameters.len(),
            actual: call_plan.parameters.len(),
        });
    }
    for (parameter_index, (parameter, placement)) in function
        .parameters
        .iter()
        .zip(&call_plan.parameters[..function.parameters.len()])
        .enumerate()
    {
        let location = scalar_parameter_location(parameter, placement)?;
        let value = match parameter.scalar_type {
            ScalarType::Boolean => {
                KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::Parameter {
                    source_value: parameter.value,
                    parameter_index,
                    location,
                })
            }
            ScalarType::Integer(scalar_type) => KnownScalar::Integer {
                scalar_type,
                value: KnownInteger::Runtime(TerminalTargetIntegerExpression::Parameter {
                    source_value: parameter.value,
                    parameter_index,
                    location,
                }),
            },
        };
        insert_value(&mut values, parameter.value, value)?;
    }
    let target_structural_parameters = function
        .structural_parameters
        .iter()
        .zip(structural_parameter_shapes)
        .zip(&call_plan.parameters[function.parameters.len()..])
        .map(
            |((parameter, shape), placement)| TerminalTargetStructuralParameter {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                shape,
                placement: placement.clone(),
            },
        )
        .collect::<Vec<_>>();

    if let [
        TerminalAbstractOperation::BoundaryCall {
            psi_operation,
            result: Some(boundary_result),
            boundary,
            structural_arguments,
            completion_receipts,
        },
        TerminalAbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
        && *result == function_result.value
        && *value == boundary_result.value
        && *scalar_type == boundary_result.scalar_type
        && boundary_result.scalar_type
            == ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is a valid integer type"),
            )
        && cleanup_actions.is_empty()
        && structural_arguments
            .iter()
            .all(|argument| argument.path.is_empty())
    {
        let binding = settlements.get(boundary).copied().ok_or(
            LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                machine: function.machine,
                operation: *psi_operation,
                boundary: *boundary,
            },
        )?;
        let TerminalBoundaryRealization::DirectPortReadU8(realization) = binding.realization else {
            return Err(
                LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                    machine: function.machine,
                    operation: *psi_operation,
                    boundary: *boundary,
                },
            );
        };
        if target.architecture != Architecture::X86_64 {
            return Err(
                LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                    machine: function.machine,
                    operation: *psi_operation,
                    boundary: *boundary,
                },
            );
        }
        return Ok(TerminalTargetFunction {
            machine: function.machine,
            attachment: function.attachment,
            provenance: TerminalPsiProvenance {
                operations: vec![*psi_operation],
                edges: vec![*psi_edge],
            },
            operation: TerminalTargetOperation::ReturnBoundaryPortReadU8 {
                psi_edge: *psi_edge,
                psi_operation: *psi_operation,
                source_value: boundary_result.value,
                boundary: *boundary,
                provider_execution: binding.provider_execution,
                realization,
                arguments: structural_arguments.clone(),
                completion_receipts: completion_receipts.clone(),
                call_plan,
                structural_parameters: target_structural_parameters,
            },
        });
    }

    if function
        .operations
        .iter()
        .any(|operation| matches!(operation, TerminalAbstractOperation::Conditional { .. }))
    {
        if function.structural_parameters.is_empty() {
            if function.operations.iter().any(|operation| {
                matches!(operation,
                    TerminalAbstractOperation::Return { cleanup_actions, .. }
                        if !cleanup_actions.is_empty())
            }) {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
            return match function_result.scalar_type {
                ScalarType::Integer(_) => {
                    lower_integer_conditional(function, &values, target, functions)
                }
                ScalarType::Boolean => {
                    lower_boolean_conditional(function, &values, target, functions)
                }
            };
        }
        if function_result.scalar_type != ScalarType::Boolean {
            return Err(LoweringError::UnsupportedOperationInScalarFunction(
                function.machine,
            ));
        }
        let lowered = lower_boolean_block(
            function,
            values,
            function.entry,
            BTreeSet::new(),
            target,
            functions,
            &target_structural_parameters,
            structural_types,
        )?;
        if let Some(shared_return_edge) = shared_boolean_cleanup_convergence_return_edge(function) {
            if shared_boolean_control_return_edge(&lowered.control) != Some(shared_return_edge) {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
            let cleanup_actions = uniform_conditional_cleanup(
                function,
                &[shared_return_edge],
                &target_structural_parameters,
                functions,
                structural_types,
            )?;
            return Ok(TerminalTargetFunction {
                machine: function.machine,
                attachment: function.attachment,
                provenance: conditional_provenance(function, lowered.operations, lowered.edges),
                operation: TerminalTargetOperation::ScalarReturnWithCleanup {
                    scalar: Box::new(TerminalTargetOperation::ReturnBooleanSharedConvergence {
                        psi_edge: shared_return_edge,
                        control: lowered.control,
                    }),
                    structural_types: structural_types
                        .values()
                        .map(|declaration| (*declaration).clone())
                        .collect(),
                    call_plan,
                    structural_parameters: target_structural_parameters,
                    cleanup_actions,
                    psi_edge: shared_return_edge,
                },
            });
        }
        let return_edges = finite_boolean_cleanup_return_edges(&lowered.control).ok_or(
            LoweringError::UnsupportedOperationInScalarFunction(function.machine),
        )?;
        let cleanup_actions = uniform_conditional_cleanup(
            function,
            &return_edges,
            &target_structural_parameters,
            functions,
            structural_types,
        )?;
        return Ok(TerminalTargetFunction {
            machine: function.machine,
            attachment: function.attachment,
            provenance: conditional_provenance(function, lowered.operations, lowered.edges),
            operation: TerminalTargetOperation::BooleanControlWithCleanup {
                control: lowered.control,
                structural_types: structural_types
                    .values()
                    .map(|declaration| (*declaration).clone())
                    .collect(),
                call_plan,
                structural_parameters: target_structural_parameters,
                cleanup_actions,
            },
        });
    }

    for operation in &function.operations {
        if returned.is_some() {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        match operation {
            TerminalAbstractOperation::BoundaryCall {
                psi_operation,
                result,
                boundary,
                ..
            } => {
                if result.is_some() {
                    return Err(
                        LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        },
                    );
                }
                return Err(LoweringError::UnitOperationInScalarFunction {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            TerminalAbstractOperation::EstablishTrivialAffineLocal { psi_operation, .. }
            | TerminalAbstractOperation::CallUnit { psi_operation, .. }
            | TerminalAbstractOperation::PortWrite { psi_operation, .. } => {
                return Err(LoweringError::UnitOperationInScalarFunction {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            TerminalAbstractOperation::CallStructuralScalar { .. } => {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
            TerminalAbstractOperation::Call {
                psi_operation,
                result,
                scalar_type,
                callee,
                arguments,
            } => {
                let value = lower_call(
                    *psi_operation,
                    *result,
                    *scalar_type,
                    *callee,
                    arguments,
                    &values,
                    target,
                    functions,
                )?;
                insert_value(&mut values, *result, value)?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } => {
                let ScalarType::Integer(integer_type) = scalar_type else {
                    return Err(LoweringError::IntegerConstantHasNonIntegerType(*result));
                };
                if !integer_type.admits(*value) {
                    return Err(LoweringError::IntegerConstantOutsideType(*result));
                }
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *integer_type,
                        value: KnownInteger::Immediate(*value),
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value,
            } => {
                insert_value(&mut values, *result, KnownScalar::Boolean(*value))?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::BooleanStructuralField { psi_operation, .. } => {
                return Err(LoweringError::UnitOperationInScalarFunction {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            TerminalAbstractOperation::BooleanNot {
                psi_operation,
                result,
                operand,
            } => {
                let operand = values
                    .get(operand)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*operand))?;
                insert_value(
                    &mut values,
                    *result,
                    negate_boolean(operand, *psi_operation, *result)?,
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::BooleanEqual {
                psi_operation,
                result,
                left,
                right,
            } => {
                let left = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                insert_value(
                    &mut values,
                    *result,
                    equal_boolean(left, right, *psi_operation, *result)?,
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::IntegerEqual {
                psi_operation,
                result,
                left,
                right,
            } => {
                let left_value = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right_value = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                insert_value(
                    &mut values,
                    *result,
                    equal_integer(
                        *left,
                        left_value,
                        *right,
                        right_value,
                        *psi_operation,
                        *result,
                    )?,
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::IntegerLessThan {
                psi_operation,
                result,
                left,
                right,
            }
            | TerminalAbstractOperation::IntegerLessOrEqual {
                psi_operation,
                result,
                left,
                right,
            } => {
                let left_value = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right_value = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                let inclusive = matches!(
                    operation,
                    TerminalAbstractOperation::IntegerLessOrEqual { .. }
                );
                insert_value(
                    &mut values,
                    *result,
                    order_integer(
                        *left,
                        left_value,
                        *right,
                        right_value,
                        *psi_operation,
                        *result,
                        inclusive,
                    )?,
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
            | TerminalAbstractOperation::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
            | TerminalAbstractOperation::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let kind = match operation {
                    TerminalAbstractOperation::IntegerBitwiseAnd { .. } => {
                        IntegerBinaryKind::BitwiseAnd
                    }
                    TerminalAbstractOperation::IntegerBitwiseOr { .. } => {
                        IntegerBinaryKind::BitwiseOr
                    }
                    TerminalAbstractOperation::IntegerBitwiseXor { .. } => {
                        IntegerBinaryKind::BitwiseXor
                    }
                    _ => unreachable!(),
                };
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    kind,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::IntegerBitwiseNot {
                psi_operation,
                result,
                scalar_type,
                operand,
            } => {
                let operand_value = match values.get(operand).cloned() {
                    Some(KnownScalar::Integer {
                        scalar_type: operand_type,
                        value,
                    }) if operand_type == *scalar_type => value,
                    Some(_) => {
                        return Err(LoweringError::IntegerBitwiseOperandTypeMismatch(*result));
                    }
                    None => return Err(LoweringError::UnknownValue(*operand)),
                };
                let value = match operand_value {
                    KnownInteger::Immediate(value) => KnownInteger::Immediate(
                        scalar_type
                            .bitwise_not(value)
                            .ok_or(LoweringError::IntegerBitwiseOperandTypeMismatch(*result))?,
                    ),
                    KnownInteger::Runtime(expression) => {
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::BitwiseNot {
                            psi_operation: *psi_operation,
                            operand: Box::new(expression),
                        })
                    }
                };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::IntegerWiden {
                psi_operation,
                result,
                source_type,
                target_type,
                operand,
            } => {
                let operand_value = match values.get(operand).cloned() {
                    Some(KnownScalar::Integer {
                        scalar_type: operand_type,
                        value,
                    }) if operand_type == *source_type
                        && source_type.can_widen_to(*target_type) =>
                    {
                        value
                    }
                    Some(_) => return Err(LoweringError::IntegerWidenTypeMismatch(*result)),
                    None => return Err(LoweringError::UnknownValue(*operand)),
                };
                let value = match operand_value {
                    KnownInteger::Immediate(value) => KnownInteger::Immediate(
                        source_type
                            .widen_value_to(*target_type, value)
                            .ok_or(LoweringError::IntegerWidenTypeMismatch(*result))?,
                    ),
                    KnownInteger::Runtime(expression) => {
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::IntegerWiden {
                            psi_operation: *psi_operation,
                            source_type: *source_type,
                            operand: Box::new(expression),
                        })
                    }
                };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *target_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::IntegerExactCast {
                psi_operation,
                result,
                source_type,
                target_type,
                operand,
            } => {
                let operand_value = match values.get(operand).cloned() {
                    Some(KnownScalar::Integer {
                        scalar_type: operand_type,
                        value,
                    }) if operand_type == *source_type
                        && source_type.can_exact_cast_to(*target_type) =>
                    {
                        value
                    }
                    Some(_) => return Err(LoweringError::IntegerExactCastTypeMismatch(*result)),
                    None => return Err(LoweringError::UnknownValue(*operand)),
                };
                let value = match operand_value {
                    KnownInteger::Immediate(value) => KnownInteger::Immediate(
                        source_type
                            .exact_cast_value_to(*target_type, value)
                            .ok_or(LoweringError::IntegerExactCastTypeMismatch(*result))?,
                    ),
                    KnownInteger::Runtime(expression) => {
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::IntegerExactCast {
                            psi_operation: *psi_operation,
                            source_type: *source_type,
                            operand: Box::new(expression),
                        })
                    }
                };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *target_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::WrappingIntegerShiftLeft {
                psi_operation,
                result,
                value_type,
                count_type,
                value,
                count,
            }
            | TerminalAbstractOperation::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                count_type,
                value,
                count,
            } => {
                let kind = if matches!(
                    operation,
                    TerminalAbstractOperation::WrappingIntegerShiftLeft { .. }
                ) {
                    WrappingShiftKind::Left
                } else {
                    WrappingShiftKind::Right
                };
                let shifted = lower_wrapping_shift(
                    &values,
                    *result,
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                    kind,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *value_type,
                        value: shifted,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::ExactIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                count_type,
                value,
                count,
            } => {
                let shifted = lower_exact_shift_right(
                    &values,
                    *result,
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *value_type,
                        value: shifted,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::ExactIntegerShiftLeft {
                psi_operation,
                result,
                value_type,
                count_type,
                value,
                count,
            } => {
                let shifted = lower_exact_shift_left(
                    &values,
                    *result,
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *value_type,
                        value: shifted,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let left_id = *left;
                let right_id = *right;
                let left = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                let (
                    KnownScalar::Integer {
                        scalar_type: left_type,
                        value: left,
                    },
                    KnownScalar::Integer {
                        scalar_type: right_type,
                        value: right,
                    },
                ) = (left, right)
                else {
                    return Err(LoweringError::WrappingAddOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::WrappingAddOperandTypeMismatch(*result));
                }
                let value = match (left, right) {
                    (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                        KnownInteger::Immediate(
                            scalar_type
                                .wrapping_add(left, right)
                                .ok_or(LoweringError::WrappingAddOperandTypeMismatch(*result))?,
                        )
                    }
                    (left, right) => {
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::WrappingAdd {
                            psi_operation: *psi_operation,
                            left: Box::new(left.into_expression(left_id)),
                            right: Box::new(right.into_expression(right_id)),
                        })
                    }
                };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let left_id = *left;
                let right_id = *right;
                let left = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                let (
                    KnownScalar::Integer {
                        scalar_type: left_type,
                        value: left,
                    },
                    KnownScalar::Integer {
                        scalar_type: right_type,
                        value: right,
                    },
                ) = (left, right)
                else {
                    return Err(LoweringError::SaturatingAddOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::SaturatingAddOperandTypeMismatch(*result));
                }
                let value = match (left, right) {
                    (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                        KnownInteger::Immediate(
                            scalar_type
                                .saturating_add(left, right)
                                .ok_or(LoweringError::SaturatingAddOperandTypeMismatch(*result))?,
                        )
                    }
                    (left, right) => {
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::SaturatingAdd {
                            psi_operation: *psi_operation,
                            left: Box::new(left.into_expression(left_id)),
                            right: Box::new(right.into_expression(right_id)),
                        })
                    }
                };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::WrappingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let left_id = *left;
                let right_id = *right;
                let left = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                let (
                    KnownScalar::Integer {
                        scalar_type: left_type,
                        value: left,
                    },
                    KnownScalar::Integer {
                        scalar_type: right_type,
                        value: right,
                    },
                ) = (left, right)
                else {
                    return Err(LoweringError::WrappingSubtractOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::WrappingSubtractOperandTypeMismatch(*result));
                }
                let value =
                    match (left, right) {
                        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                            KnownInteger::Immediate(scalar_type.wrapping_sub(left, right).ok_or(
                                LoweringError::WrappingSubtractOperandTypeMismatch(*result),
                            )?)
                        }
                        (left, right) => KnownInteger::Runtime(
                            TerminalTargetIntegerExpression::WrappingSubtract {
                                psi_operation: *psi_operation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            },
                        ),
                    };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::SaturatingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let left_id = *left;
                let right_id = *right;
                let left = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                let (
                    KnownScalar::Integer {
                        scalar_type: left_type,
                        value: left,
                    },
                    KnownScalar::Integer {
                        scalar_type: right_type,
                        value: right,
                    },
                ) = (left, right)
                else {
                    return Err(LoweringError::SaturatingSubtractOperandTypeMismatch(
                        *result,
                    ));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::SaturatingSubtractOperandTypeMismatch(
                        *result,
                    ));
                }
                let value = match (left, right) {
                    (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                        KnownInteger::Immediate(scalar_type.saturating_sub(left, right).ok_or(
                            LoweringError::SaturatingSubtractOperandTypeMismatch(*result),
                        )?)
                    }
                    (left, right) => {
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::SaturatingSubtract {
                            psi_operation: *psi_operation,
                            left: Box::new(left.into_expression(left_id)),
                            right: Box::new(right.into_expression(right_id)),
                        })
                    }
                };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let left_id = *left;
                let right_id = *right;
                let left = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                let (
                    KnownScalar::Integer {
                        scalar_type: left_type,
                        value: left,
                    },
                    KnownScalar::Integer {
                        scalar_type: right_type,
                        value: right,
                    },
                ) = (left, right)
                else {
                    return Err(LoweringError::WrappingMultiplyOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::WrappingMultiplyOperandTypeMismatch(*result));
                }
                let value =
                    match (left, right) {
                        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                            KnownInteger::Immediate(scalar_type.wrapping_mul(left, right).ok_or(
                                LoweringError::WrappingMultiplyOperandTypeMismatch(*result),
                            )?)
                        }
                        (left, right) => KnownInteger::Runtime(
                            TerminalTargetIntegerExpression::WrappingMultiply {
                                psi_operation: *psi_operation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            },
                        ),
                    };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let left_id = *left;
                let right_id = *right;
                let left = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                let (
                    KnownScalar::Integer {
                        scalar_type: left_type,
                        value: left,
                    },
                    KnownScalar::Integer {
                        scalar_type: right_type,
                        value: right,
                    },
                ) = (left, right)
                else {
                    return Err(LoweringError::SaturatingMultiplyOperandTypeMismatch(
                        *result,
                    ));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::SaturatingMultiplyOperandTypeMismatch(
                        *result,
                    ));
                }
                let value = match (left, right) {
                    (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                        KnownInteger::Immediate(scalar_type.saturating_mul(left, right).ok_or(
                            LoweringError::SaturatingMultiplyOperandTypeMismatch(*result),
                        )?)
                    }
                    (left, right) => {
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::SaturatingMultiply {
                            psi_operation: *psi_operation,
                            left: Box::new(left.into_expression(left_id)),
                            right: Box::new(right.into_expression(right_id)),
                        })
                    }
                };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::ExactIntegerDivide {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::ExactDivide,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::ExactIntegerRemainder {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::ExactRemainder,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::WrappingIntegerDivide {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::WrappingDivide,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::WrappingIntegerRemainder {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::WrappingRemainder,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::SaturatingIntegerDivide {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::SaturatingDivide,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::SaturatingIntegerRemainder {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::SaturatingRemainder,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::Jump {
                psi_edge, bindings, ..
            } => {
                let transferred = bindings
                    .iter()
                    .map(|binding| {
                        let value = values
                            .get(&binding.argument)
                            .cloned()
                            .ok_or(LoweringError::UnknownValue(binding.argument))?;
                        if binding.scalar_type != value.scalar_type() {
                            return Err(LoweringError::ValueTypeMismatch(binding.parameter));
                        }
                        Ok((binding.parameter, value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                for (parameter, value) in transferred {
                    insert_value(&mut values, parameter, value)?;
                }
                provenance.edges.push(*psi_edge);
            }
            TerminalAbstractOperation::Conditional { .. } => {
                return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                    function.machine,
                ));
            }
            TerminalAbstractOperation::Crash {
                psi_edge,
                cause,
                site_guard,
                frontier_lower_bound,
            } => {
                provenance.edges.push(*psi_edge);
                returned = Some(TerminalTargetOperation::Crash {
                    psi_edge: *psi_edge,
                    cause: *cause,
                    site_guard: site_guard.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                });
            }
            TerminalAbstractOperation::Return {
                psi_edge,
                result,
                value,
                scalar_type,
                cleanup_actions,
            } => {
                if *result != function_result.value || *scalar_type != function_result.scalar_type {
                    return Err(LoweringError::FunctionResultMismatch(function.machine));
                }
                let returned_value = values
                    .get(value)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*value))?;
                if *scalar_type != returned_value.scalar_type() {
                    return Err(LoweringError::ValueTypeMismatch(*result));
                }
                provenance.edges.push(*psi_edge);
                let scalar = match returned_value {
                    KnownScalar::Boolean(boolean) => {
                        TerminalTargetOperation::ReturnBooleanImmediate {
                            psi_edge: *psi_edge,
                            source_value: *value,
                            value: boolean,
                        }
                    }
                    KnownScalar::Integer {
                        scalar_type,
                        value: KnownInteger::Immediate(integer),
                    } => TerminalTargetOperation::ReturnIntegerImmediate {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        value: integer,
                    },
                    KnownScalar::Integer {
                        scalar_type,
                        value:
                            KnownInteger::Runtime(TerminalTargetIntegerExpression::Parameter {
                                parameter_index,
                                location,
                                ..
                            }),
                    } => TerminalTargetOperation::ReturnIntegerParameter {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        parameter_index,
                        location,
                    },
                    KnownScalar::Integer {
                        scalar_type,
                        value: KnownInteger::Runtime(expression),
                    } => TerminalTargetOperation::ReturnIntegerExpression {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        expression,
                    },
                    KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::Parameter {
                        parameter_index,
                        location,
                        ..
                    }) => TerminalTargetOperation::ReturnBooleanParameter {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        parameter_index,
                        location,
                    },
                    KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::Not {
                        operand,
                        ..
                    }) if matches!(*operand, TerminalTargetBooleanExpression::Parameter { .. }) => {
                        let TerminalTargetBooleanExpression::Parameter {
                            parameter_index,
                            location,
                            ..
                        } = *operand
                        else {
                            unreachable!("guard requires a parameter operand")
                        };
                        TerminalTargetOperation::ReturnBooleanNotParameter {
                            psi_edge: *psi_edge,
                            source_value: *value,
                            parameter_index,
                            location,
                        }
                    }
                    KnownScalar::BooleanRuntime(expression) => {
                        TerminalTargetOperation::ReturnBooleanExpression {
                            psi_edge: *psi_edge,
                            source_value: *value,
                            expression,
                        }
                    }
                };
                if cleanup_actions.is_empty() {
                    returned = Some(scalar);
                } else {
                    validate_scalar_cleanup_frontier(
                        function.machine,
                        cleanup_actions,
                        &target_structural_parameters,
                        functions,
                        structural_types,
                    )?;
                    returned = Some(TerminalTargetOperation::ScalarReturnWithCleanup {
                        scalar: Box::new(scalar),
                        structural_types: structural_types
                            .values()
                            .map(|declaration| (*declaration).clone())
                            .collect(),
                        call_plan: call_plan.clone(),
                        structural_parameters: target_structural_parameters.clone(),
                        cleanup_actions: cleanup_actions.clone(),
                        psi_edge: *psi_edge,
                    });
                }
            }
            TerminalAbstractOperation::ReturnUnit { .. }
            | TerminalAbstractOperation::ReturnStructural { .. } => {
                return Err(LoweringError::FunctionResultKindMismatch(function.machine));
            }
        }
    }

    Ok(TerminalTargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance,
        operation: returned.ok_or(LoweringError::FunctionHasNoReturn(function.machine))?,
    })
}

fn shared_boolean_cleanup_convergence_return_edge(
    function: &TerminalAbstractFunction,
) -> Option<EdgeId> {
    let mut conditional_count = 0_usize;
    let mut jump_target = None;
    let mut jump_bindings = Vec::new();
    let mut return_edge = None;
    for operation in &function.operations {
        match operation {
            TerminalAbstractOperation::Conditional { .. } => conditional_count += 1,
            TerminalAbstractOperation::Jump {
                target, bindings, ..
            } => {
                if bindings.len() != 1 || jump_target.is_some_and(|existing| existing != *target) {
                    return None;
                }
                jump_target = Some(*target);
                jump_bindings.push(bindings[0]);
            }
            TerminalAbstractOperation::Return {
                psi_edge,
                value,
                scalar_type: ScalarType::Boolean,
                cleanup_actions,
                ..
            } if !cleanup_actions.is_empty() => {
                if return_edge.replace((*psi_edge, *value)).is_some() {
                    return None;
                }
            }
            TerminalAbstractOperation::BooleanConstant { .. }
            | TerminalAbstractOperation::BooleanStructuralField { .. }
            | TerminalAbstractOperation::BooleanNot { .. }
            | TerminalAbstractOperation::IntegerConstant { .. }
            | TerminalAbstractOperation::IntegerEqual { .. }
            | TerminalAbstractOperation::IntegerLessThan { .. }
            | TerminalAbstractOperation::IntegerLessOrEqual { .. }
            | TerminalAbstractOperation::IntegerBitwiseNot { .. }
            | TerminalAbstractOperation::IntegerWiden { .. }
            | TerminalAbstractOperation::IntegerExactCast { .. }
            | TerminalAbstractOperation::IntegerBitwiseAnd { .. }
            | TerminalAbstractOperation::IntegerBitwiseOr { .. }
            | TerminalAbstractOperation::IntegerBitwiseXor { .. }
            | TerminalAbstractOperation::WrappingIntegerShiftLeft { .. }
            | TerminalAbstractOperation::WrappingIntegerShiftRight { .. }
            | TerminalAbstractOperation::ExactIntegerShiftLeft { .. }
            | TerminalAbstractOperation::ExactIntegerShiftRight { .. }
            | TerminalAbstractOperation::WrappingIntegerAdd { .. }
            | TerminalAbstractOperation::SaturatingIntegerAdd { .. }
            | TerminalAbstractOperation::WrappingIntegerSubtract { .. }
            | TerminalAbstractOperation::SaturatingIntegerSubtract { .. }
            | TerminalAbstractOperation::WrappingIntegerMultiply { .. }
            | TerminalAbstractOperation::SaturatingIntegerMultiply { .. }
            | TerminalAbstractOperation::ExactIntegerDivide { .. }
            | TerminalAbstractOperation::ExactIntegerRemainder { .. } => {}
            _ => return None,
        }
    }
    let target = jump_target?;
    let (edge, returned_value) = return_edge?;
    if conditional_count == 0
        || Some(jump_bindings.len()) != conditional_count.checked_add(1)
        || jump_bindings.iter().any(|binding| {
            binding.parameter != returned_value || binding.scalar_type != ScalarType::Boolean
        })
    {
        return None;
    }
    let target_entry = function
        .block_entries
        .iter()
        .position(|entry| entry.block == target)?;
    let start = function.block_entries[target_entry].operation_offset;
    let end = function
        .block_entries
        .get(target_entry + 1)
        .map_or(function.operations.len(), |entry| entry.operation_offset);
    matches!(
        function.operations.get(start..end),
        Some([TerminalAbstractOperation::Return { psi_edge, .. }]) if *psi_edge == edge
    )
    .then_some(edge)
}

fn shared_boolean_control_return_edge(control: &TerminalTargetBooleanControl) -> Option<EdgeId> {
    match control {
        TerminalTargetBooleanControl::ReturnImmediate {
            psi_return_edge, ..
        } => Some(*psi_return_edge),
        TerminalTargetBooleanControl::Conditional {
            when_true,
            when_false,
            ..
        }
        | TerminalTargetBooleanControl::ConditionalExpression {
            when_true,
            when_false,
            ..
        } => {
            let when_true = shared_boolean_control_return_edge(&when_true.control)?;
            let when_false = shared_boolean_control_return_edge(&when_false.control)?;
            (when_true == when_false).then_some(when_true)
        }
        TerminalTargetBooleanControl::Crash { .. }
        | TerminalTargetBooleanControl::ReturnParameter { .. }
        | TerminalTargetBooleanControl::ReturnNotParameter { .. }
        | TerminalTargetBooleanControl::ReturnExpression { .. } => None,
    }
}

fn lower_structural_return_function(
    function: &TerminalAbstractFunction,
    result: &psi_terminal::StructuralResultDeclaration,
    target: NativeTarget,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<TerminalTargetFunction, LoweringError> {
    if function.structural_parameters.is_empty() {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    }
    let [entry_claim] = function.entry_claims.as_slice() else {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    };
    let [block_entry] = function.block_entries.as_slice() else {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    };
    let [
        TerminalAbstractOperation::ReturnStructural {
            psi_edge,
            source: returned_source,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
        },
    ] = function.operations.as_slice()
    else {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    };
    if !function.parameters.is_empty()
        || !function.published_service_ceiling.is_empty()
        || block_entry.block != function.entry
        || block_entry.operation_offset != 0
        || result.multiplicity != psi_terminal::StructuralMultiplicity::Linear
        || function
            .structural_parameters
            .iter()
            .enumerate()
            .any(|(index, parameter)| {
                parameter.is_self || usize::try_from(parameter.position) != Ok(index)
            })
        || function
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<BTreeSet<_>>()
            .len()
            != function.structural_parameters.len()
        || !entry_claim.path.is_empty()
        || returned_claims.as_slice() != [entry_claim.claim]
    {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    }
    let source_index = 0;
    let source = &function.structural_parameters[source_index];
    if source.place != *returned_source {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    }
    if source.multiplicity != psi_terminal::StructuralMultiplicity::Linear
        || source.structural_type != result.structural_type
        || source.qualifications != result.qualifications
        || source.place == result.place
        || entry_claim.input != source.place
    {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    }
    let expected_cleanup = trivial_affine_locals
        .iter()
        .rev()
        .map(|(_, local, _)| local.id)
        .chain(
            function
                .structural_parameters
                .iter()
                .skip(1)
                .rev()
                .map(|parameter| parameter.place),
        )
        .collect::<Vec<_>>();
    if trivial_affine_discards != &expected_cleanup
        || trivial_affine_locals
            .iter()
            .enumerate()
            .any(|(index, (_, local, local_type))| {
            let psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
            } = local.kind
            else {
                return true;
            };
            usize::try_from(declaration_ordinal) != Ok(index)
                || local.id == source.place
                || local.id == result.place
                || function
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == local.id)
                || structural_types.get(&structural_type).is_none_or(|declaration| {
                    *declaration != local_type
                        || declaration.identity.is_empty()
                        || !matches!(
                        declaration.shape,
                        psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty()
                    )
                })
        })
        || trivial_affine_locals
            .iter()
            .map(|(_, local, _)| local.id)
            .collect::<BTreeSet<_>>()
            .len()
            != trivial_affine_locals.len()
        || function
            .structural_parameters
            .iter()
            .skip(1)
            .any(|cleanup| {
                cleanup.multiplicity != psi_terminal::StructuralMultiplicity::Affine
                    || !cleanup.qualifications.is_empty()
                    || cleanup.place == result.place
            })
    {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    }
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let parameter_shapes = function
        .structural_parameters
        .iter()
        .map(|parameter| {
            structural_shape(
                parameter.structural_type,
                structural_types,
                &mut cache,
                &mut active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape = parameter_shapes[source_index];
    if shape.byte_size != 8 || shape.alignment != 8 {
        return Err(LoweringError::UnsupportedStructuralReturnShape {
            machine: function.machine,
            byte_size: shape.byte_size,
        });
    }
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: parameter_shapes,
            result: Some(shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let Some(source_placement) = call_plan.parameters.get(source_index) else {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: function.structural_parameters.len(),
            actual: call_plan.parameters.len(),
        });
    };
    let Some(result_placement) = call_plan.result.as_ref() else {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    };
    require_one_direct_structural_fragment(function.machine, source_placement)?;
    require_one_direct_structural_fragment(function.machine, result_placement)?;
    Ok(TerminalTargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: TerminalPsiProvenance {
            operations: trivial_affine_locals
                .iter()
                .map(|(operation, _, _)| *operation)
                .collect(),
            edges: vec![*psi_edge],
        },
        operation: TerminalTargetOperation::ReturnStructuralParameter {
            call_plan: call_plan.clone(),
            parameters: function.structural_parameters.clone(),
            source: source.clone(),
            result: result.clone(),
            shape,
            source_placement: source_placement.clone(),
            result_placement: result_placement.clone(),
            psi_edge: *psi_edge,
            returned_claims: returned_claims.clone(),
            trivial_affine_locals: trivial_affine_locals.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
        },
    })
}

fn require_one_direct_structural_fragment(
    machine: MachineId,
    placement: &ValuePlacement,
) -> Result<(), LoweringError> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                value_byte_offset: 0,
                byte_size: 8,
                ..
            },
        ] if placement.shape.byte_size == 8 => Ok(()),
        _ => Err(LoweringError::UnsupportedStructuralReturnPlacement(machine)),
    }
}

fn lower_unit_function(
    function: &TerminalAbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, TerminalBoundarySettlementBinding>,
) -> Result<TerminalTargetFunction, LoweringError> {
    if !function.parameters.is_empty() {
        return Err(LoweringError::UnitFunctionHasScalarParameters(
            function.machine,
        ));
    }
    if function.block_entries.len() != 1 || function.block_entries[0].block != function.entry {
        return Err(LoweringError::UnitFunctionNotStraightLine(function.machine));
    }

    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let parameter_shapes = function
        .structural_parameters
        .iter()
        .map(|parameter| {
            structural_shape(
                parameter.structural_type,
                structural_types,
                &mut shape_cache,
                &mut active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = CallSignature {
        parameters: parameter_shapes.clone(),
        result: None,
    };
    let call_plan = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
        .map_err(LoweringError::AbiPlan)?;
    if call_plan.parameters.len() != function.structural_parameters.len() {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: function.structural_parameters.len(),
            actual: call_plan.parameters.len(),
        });
    }
    let parameters = function
        .structural_parameters
        .iter()
        .zip(parameter_shapes)
        .zip(&call_plan.parameters)
        .map(
            |((parameter, shape), placement)| TerminalTargetStructuralParameter {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                shape,
                placement: placement.clone(),
            },
        )
        .collect::<Vec<_>>();
    let parameters_by_place = parameters
        .iter()
        .map(|parameter| (parameter.place, parameter))
        .collect::<BTreeMap<_, _>>();

    let mut operations = Vec::with_capacity(function.operations.len());
    let mut provenance = TerminalPsiProvenance::default();
    let mut returned = false;
    for operation in &function.operations {
        if returned {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        match operation {
            TerminalAbstractOperation::EstablishTrivialAffineLocal {
                psi_operation,
                place,
                structural_type,
            } => {
                operations.push(TerminalTargetUnitOperation::EstablishTrivialAffineLocal {
                    psi_operation: *psi_operation,
                    place: place.clone(),
                    structural_type: structural_type.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                claim_transfers,
            } => {
                let callee_function = functions
                    .get(callee)
                    .copied()
                    .ok_or(LoweringError::UnknownCallTarget(*callee))?;
                if callee_function.result != TerminalAbstractFunctionResult::Unit
                    || !callee_function.parameters.is_empty()
                {
                    return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
                }
                if structural_arguments.len() != callee_function.structural_parameters.len() {
                    return Err(LoweringError::StructuralCallArgumentCountMismatch {
                        callee: *callee,
                        expected: callee_function.structural_parameters.len(),
                        actual: structural_arguments.len(),
                    });
                }
                let callee_shapes = callee_function
                    .structural_parameters
                    .iter()
                    .map(|parameter| {
                        structural_shape(
                            parameter.structural_type,
                            structural_types,
                            &mut shape_cache,
                            &mut active,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let callee_plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(target),
                    &CallSignature {
                        parameters: callee_shapes.clone(),
                        result: None,
                    },
                )
                .map_err(LoweringError::AbiPlan)?;
                let arguments = structural_arguments
                    .iter()
                    .zip(&callee_function.structural_parameters)
                    .zip(callee_shapes)
                    .zip(&callee_plan.parameters)
                    .map(|(((argument, callee_parameter), shape), destination)| {
                        let source = parameters_by_place.get(&argument.place).copied().ok_or(
                            LoweringError::UnknownStructuralArgumentPlace {
                                machine: function.machine,
                                place: argument.place,
                            },
                        )?;
                        let (
                            projected_type,
                            projected_shape,
                            source_byte_offset,
                            fixed_array_length,
                            element_stride,
                        ) = match argument.path.as_slice() {
                            [] => (source.structural_type, source.shape, 0, None, None),
                            [StructuralPathSegment::FixedIndex(index)] => {
                                let declaration = structural_types
                                    .get(&source.structural_type)
                                    .copied()
                                    .ok_or(LoweringError::UnknownStructuralType(
                                        source.structural_type,
                                    ))?;
                                let StructuralTypeShape::FixedArray { element, length } =
                                    declaration.shape
                                else {
                                    return Err(
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        },
                                    );
                                };
                                if *index >= length {
                                    return Err(
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        },
                                    );
                                }
                                let element_shape = structural_shape(
                                    element,
                                    structural_types,
                                    &mut shape_cache,
                                    &mut active,
                                )?;
                                let stride = checked_align_up_u32(
                                    u32::from(element_shape.byte_size),
                                    u32::from(element_shape.alignment),
                                )
                                .ok_or(
                                    LoweringError::StructuralTypeTooLarge(source.structural_type),
                                )?;
                                let offset = u64::from(stride)
                                    .checked_mul(*index)
                                    .and_then(|offset| u32::try_from(offset).ok())
                                    .ok_or(LoweringError::StructuralTypeTooLarge(
                                        source.structural_type,
                                    ))?;
                                (element, element_shape, offset, Some(length), Some(stride))
                            }
                            path @ [StructuralPathSegment::Field(_), ..]
                                if path.iter().all(|segment| {
                                    matches!(segment, StructuralPathSegment::Field(_))
                                }) =>
                            {
                                let (field_type, field_shape, offset) =
                                    resolve_structural_field_path(
                                        source.structural_type,
                                        path,
                                        structural_types,
                                        &mut shape_cache,
                                        &mut active,
                                    )
                                    .map_err(|_| {
                                        LoweringError::StructuralCallArgumentTypeMismatch {
                                            callee: *callee,
                                            place: argument.place,
                                        }
                                    })?;
                                (field_type, field_shape, offset, None, None)
                            }
                            _ => {
                                return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                    callee: *callee,
                                    place: argument.place,
                                });
                            }
                        };
                        if projected_type != callee_parameter.structural_type
                            || projected_shape != shape
                            || u32::from(shape.byte_size)
                                .checked_add(source_byte_offset)
                                .is_none_or(|end| end > u32::from(source.shape.byte_size))
                        {
                            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                callee: *callee,
                                place: argument.place,
                            });
                        }
                        Ok(TerminalTargetStructuralArgument {
                            place: argument.place,
                            path: argument.path.clone(),
                            root_structural_type: source.structural_type,
                            structural_type: projected_type,
                            shape,
                            source_byte_offset,
                            fixed_array_length,
                            element_stride,
                            source: source.placement.clone(),
                            destination: destination.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                operations.push(TerminalTargetUnitOperation::Call {
                    psi_operation: *psi_operation,
                    callee: *callee,
                    arguments,
                    claim_transfers: claim_transfers.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::PortWrite {
                psi_operation,
                service,
                port,
                value,
            } => {
                operations.push(TerminalTargetUnitOperation::PortWrite {
                    psi_operation: *psi_operation,
                    service: *service,
                    port: *port,
                    value: *value,
                });
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::BoundaryCall {
                psi_operation,
                result,
                boundary,
                structural_arguments,
                completion_receipts,
            } => {
                if result.is_some() {
                    return Err(
                        LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        },
                    );
                }
                let binding = settlements
                    .get(boundary)
                    .copied()
                    .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
                let TerminalBoundaryRealization::MetadataOnlyPort(realization) =
                    binding.realization
                else {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                };
                if !matches!(
                    operations.last(),
                    Some(TerminalTargetUnitOperation::PortWrite {
                        psi_operation,
                        service,
                        port,
                        value,
                    }) if *psi_operation == realization.effect_operation
                        && *service == realization.service
                        && *port == realization.port
                        && *value == realization.value
                ) {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                }
                for argument in structural_arguments {
                    if !parameters_by_place.contains_key(&argument.place) {
                        return Err(LoweringError::UnknownStructuralArgumentPlace {
                            machine: function.machine,
                            place: argument.place,
                        });
                    }
                }
                operations.push(TerminalTargetUnitOperation::BoundarySettlement {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    provider_execution: binding.provider_execution,
                    realization: realization.into(),
                    arguments: structural_arguments.clone(),
                    completion_receipts: completion_receipts.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::ReturnUnit {
                psi_edge,
                cleanup_actions,
            } => {
                let local_places = operations
                    .iter()
                    .filter_map(|operation| match operation {
                        TerminalTargetUnitOperation::EstablishTrivialAffineLocal {
                            place, ..
                        } => Some(place.id),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let expected_roots = local_places
                    .iter()
                    .rev()
                    .copied()
                    .chain(
                        function
                            .structural_parameters
                            .iter()
                            .rev()
                            .filter(|parameter| {
                                parameter.multiplicity
                                    == psi_terminal::StructuralMultiplicity::Affine
                            })
                            .map(|parameter| parameter.place),
                    )
                    .collect::<Vec<_>>();
                let root_discards = cleanup_actions
                    .iter()
                    .filter_map(|action| match action {
                        psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
                            Some(*place)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let residual_discards = cleanup_actions
                    .iter()
                    .filter_map(|action| match action {
                        psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                            Some(discard)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let nominal_cleanups = cleanup_actions
                    .iter()
                    .filter_map(|action| match action {
                        psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                            Some(cleanup.clone())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if root_discards.len() + residual_discards.len() + nominal_cleanups.len()
                    != cleanup_actions.len()
                {
                    unreachable!("every cleanup action has one exact kind")
                }
                if residual_discards.is_empty()
                    && nominal_cleanups.is_empty()
                    && root_discards != expected_roots
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                if !residual_discards.is_empty() {
                    let Some(residual_root) =
                        residual_discards.first().map(|discard| discard.place)
                    else {
                        unreachable!("nonempty residual cleanup has a root")
                    };
                    let Some(parameter) = function
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == residual_root)
                    else {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    };
                    let moved_arguments = operations
                        .iter()
                        .filter_map(|operation| match operation {
                            TerminalTargetUnitOperation::Call { arguments, .. } => Some(arguments),
                            _ => None,
                        })
                        .flatten()
                        .filter(|argument| argument.place == residual_root)
                        .collect::<Vec<_>>();
                    let mut moved_subtrees = Vec::with_capacity(moved_arguments.len());
                    if moved_arguments.is_empty()
                        || moved_arguments.iter().any(|argument| {
                            argument.root_structural_type != parameter.structural_type
                                || argument.path.is_empty()
                                || argument.path.iter().any(|segment| {
                                    !matches!(segment, StructuralPathSegment::Field(identity)
                                        if !identity.is_empty())
                                })
                                || moved_subtrees
                                    .iter()
                                    .any(|(path, _)| path == &argument.path)
                                || {
                                    moved_subtrees
                                        .push((argument.path.clone(), argument.structural_type));
                                    false
                                }
                        })
                    {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    }
                    let Some(expected_residuals) = expected_maximal_residual_subtrees(
                        parameter.structural_type,
                        &moved_subtrees,
                        structural_types,
                    ) else {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    };
                    if parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine
                        || root_discards != local_places.iter().rev().copied().collect::<Vec<_>>()
                        || expected_roots.get(local_places.len()..) != Some(&[residual_root][..])
                        || expected_residuals.len() != residual_discards.len()
                        || cleanup_actions.get(..root_discards.len()).is_none_or(|prefix| {
                            !prefix.iter().zip(&root_discards).all(|(action, place)| {
                                matches!(action,
                                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(actual)
                                        if actual == place)
                            })
                        })
                        || cleanup_actions.get(root_discards.len()..).is_none_or(|suffix| {
                            suffix.iter().zip(&expected_residuals).any(
                                |(action, (path, structural_type))| {
                                    !matches!(action,
                                        psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard)
                                            if discard.place == residual_root
                                                && discard.path == *path
                                                && discard.structural_type == *structural_type)
                                },
                            )
                        })
                    {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    }
                }
                if !nominal_cleanups.is_empty() {
                    if !local_places.is_empty()
                        || !root_discards.is_empty()
                        || !residual_discards.is_empty()
                        || nominal_cleanups.is_empty()
                        || function.structural_parameters.len() != nominal_cleanups.len()
                        || function
                            .structural_parameters
                            .iter()
                            .rev()
                            .zip(&nominal_cleanups)
                            .any(|(parameter, cleanup)| {
                                parameter.place != cleanup.place
                                    || parameter.structural_type != cleanup.structural_type
                                    || parameter.multiplicity
                                        != psi_terminal::StructuralMultiplicity::Affine
                            })
                    {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    }
                    for cleanup in &nominal_cleanups {
                        let Some(cleanup_function) =
                            functions.get(&cleanup.cleanup_machine).copied()
                        else {
                            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                                function.machine,
                            ));
                        };
                        if cleanup_function.attachment != Some(cleanup.structural_type)
                            || cleanup_function.result != TerminalAbstractFunctionResult::Unit
                            || !cleanup_function.parameters.is_empty()
                            || !cleanup_function.structural_parameters.is_empty()
                            || !cleanup_function.entry_claims.is_empty()
                            || !cleanup_function.published_service_ceiling.is_empty()
                            || cleanup_function.block_entries.as_slice()
                                != [omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                                    block: cleanup_function.entry,
                                    operation_offset: 0,
                                }]
                        {
                            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                                function.machine,
                            ));
                        }
                        validate_bounded_nominal_cleanup_body(
                            function.machine,
                            cleanup,
                            cleanup_function,
                            functions,
                            structural_types,
                        )?;
                    }
                }
                if !nominal_cleanups.is_empty()
                    && nominal_cleanups.len() + root_discards.len() + residual_discards.len()
                        != cleanup_actions.len()
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                operations.push(TerminalTargetUnitOperation::Return {
                    psi_edge: *psi_edge,
                    cleanup_actions: cleanup_actions.clone(),
                });
                provenance.edges.push(*psi_edge);
                returned = true;
            }
            TerminalAbstractOperation::Crash { .. }
            | TerminalAbstractOperation::Call { .. }
            | TerminalAbstractOperation::CallStructuralScalar { .. }
            | TerminalAbstractOperation::IntegerConstant { .. }
            | TerminalAbstractOperation::BooleanConstant { .. }
            | TerminalAbstractOperation::BooleanStructuralField { .. }
            | TerminalAbstractOperation::BooleanNot { .. }
            | TerminalAbstractOperation::BooleanEqual { .. }
            | TerminalAbstractOperation::IntegerEqual { .. }
            | TerminalAbstractOperation::IntegerLessThan { .. }
            | TerminalAbstractOperation::IntegerLessOrEqual { .. }
            | TerminalAbstractOperation::IntegerBitwiseNot { .. }
            | TerminalAbstractOperation::IntegerWiden { .. }
            | TerminalAbstractOperation::IntegerExactCast { .. }
            | TerminalAbstractOperation::IntegerBitwiseAnd { .. }
            | TerminalAbstractOperation::IntegerBitwiseOr { .. }
            | TerminalAbstractOperation::IntegerBitwiseXor { .. }
            | TerminalAbstractOperation::WrappingIntegerShiftLeft { .. }
            | TerminalAbstractOperation::WrappingIntegerShiftRight { .. }
            | TerminalAbstractOperation::ExactIntegerShiftLeft { .. }
            | TerminalAbstractOperation::ExactIntegerShiftRight { .. }
            | TerminalAbstractOperation::WrappingIntegerAdd { .. }
            | TerminalAbstractOperation::SaturatingIntegerAdd { .. }
            | TerminalAbstractOperation::WrappingIntegerSubtract { .. }
            | TerminalAbstractOperation::SaturatingIntegerSubtract { .. }
            | TerminalAbstractOperation::WrappingIntegerMultiply { .. }
            | TerminalAbstractOperation::SaturatingIntegerMultiply { .. }
            | TerminalAbstractOperation::ExactIntegerDivide { .. }
            | TerminalAbstractOperation::ExactIntegerRemainder { .. }
            | TerminalAbstractOperation::WrappingIntegerDivide { .. }
            | TerminalAbstractOperation::WrappingIntegerRemainder { .. }
            | TerminalAbstractOperation::SaturatingIntegerDivide { .. }
            | TerminalAbstractOperation::SaturatingIntegerRemainder { .. }
            | TerminalAbstractOperation::Jump { .. }
            | TerminalAbstractOperation::Conditional { .. }
            | TerminalAbstractOperation::Return { .. }
            | TerminalAbstractOperation::ReturnStructural { .. } => {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
        }
    }
    if !returned {
        return Err(LoweringError::FunctionHasNoReturn(function.machine));
    }
    Ok(TerminalTargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance,
        operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
            structural_types: structural_types
                .values()
                .map(|declaration| (*declaration).clone())
                .collect(),
            call_plan,
            parameters,
            operations,
        }),
    })
}

fn validate_scalar_cleanup_frontier(
    caller: MachineId,
    cleanup_actions: &[psi_terminal::TerminalAffineCleanupAction],
    structural_parameters: &[TerminalTargetStructuralParameter],
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(caller);
    if cleanup_actions.is_empty()
        || cleanup_actions.len() != structural_parameters.len()
        || structural_parameters
            .iter()
            .rev()
            .zip(cleanup_actions)
            .any(|(parameter, action)| match action {
                psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
                    *place != parameter.place
                }
                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    cleanup.place != parameter.place
                        || cleanup.structural_type != parameter.structural_type
                }
                psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => true,
            })
    {
        return Err(invalid());
    }
    for action in cleanup_actions {
        let psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
            continue;
        };
        let cleanup_function = functions
            .get(&cleanup.cleanup_machine)
            .copied()
            .ok_or_else(invalid)?;
        validate_bounded_nominal_cleanup_body(
            caller,
            cleanup,
            cleanup_function,
            functions,
            structural_types,
        )?;
    }
    Ok(())
}

fn finite_boolean_cleanup_return_edges(
    control: &TerminalTargetBooleanControl,
) -> Option<Vec<EdgeId>> {
    fn collect(
        control: &TerminalTargetBooleanControl,
        decision_count: &mut usize,
        return_edges: &mut Vec<EdgeId>,
    ) -> Option<()> {
        match control {
            TerminalTargetBooleanControl::ReturnImmediate {
                psi_return_edge, ..
            }
            | TerminalTargetBooleanControl::ReturnParameter {
                psi_return_edge, ..
            }
            | TerminalTargetBooleanControl::ReturnNotParameter {
                psi_return_edge, ..
            }
            | TerminalTargetBooleanControl::ReturnExpression {
                psi_return_edge, ..
            } => return_edges.push(*psi_return_edge),
            TerminalTargetBooleanControl::Conditional {
                when_true,
                when_false,
                ..
            }
            | TerminalTargetBooleanControl::ConditionalExpression {
                when_true,
                when_false,
                ..
            } => {
                *decision_count = decision_count.checked_add(1)?;
                collect(&when_true.control, decision_count, return_edges)?;
                collect(&when_false.control, decision_count, return_edges)?;
            }
            TerminalTargetBooleanControl::Crash { .. } => return None,
        }
        Some(())
    }

    let mut decision_count = 0;
    let mut return_edges = Vec::new();
    collect(control, &mut decision_count, &mut return_edges)?;
    if decision_count == 0
        || return_edges.len() < 2
        || return_edges.iter().copied().collect::<BTreeSet<_>>().len() != return_edges.len()
    {
        return None;
    }
    Some(return_edges)
}

fn uniform_conditional_cleanup(
    function: &TerminalAbstractFunction,
    return_edges: &[EdgeId],
    structural_parameters: &[TerminalTargetStructuralParameter],
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<Vec<psi_terminal::TerminalAffineCleanupAction>, LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(function.machine);
    let mut returns = BTreeMap::new();
    for operation in &function.operations {
        let TerminalAbstractOperation::Return {
            psi_edge,
            cleanup_actions,
            ..
        } = operation
        else {
            continue;
        };
        if returns.insert(*psi_edge, cleanup_actions).is_some() {
            return Err(invalid());
        }
    }
    let first = return_edges
        .first()
        .and_then(|edge| returns.get(edge))
        .copied()
        .ok_or_else(invalid)?;
    if first.is_empty()
        || return_edges
            .iter()
            .any(|edge| returns.get(edge).copied() != Some(first))
    {
        return Err(invalid());
    }
    validate_scalar_cleanup_frontier(
        function.machine,
        first,
        structural_parameters,
        functions,
        structural_types,
    )?;
    Ok(first.to_vec())
}

fn validate_bounded_nominal_cleanup_body(
    caller: MachineId,
    cleanup: &psi_terminal::NominalAffineCleanup,
    cleanup_function: &TerminalAbstractFunction,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInUnitFunction(caller);
    if cleanup.cleanup_receiver.is_some() || !cleanup.requirement_obligations.is_empty() {
        // Contextual cleanup premises are verified terminal-Psi evidence. The
        // verified Psi-to-Omega boundary projects them away; accepting them in
        // an Omega plan would create a second, unverified proof authority.
        return Err(invalid());
    }
    let Some((cleanup_return, helper_calls)) = cleanup_function.operations.split_last() else {
        return Err(invalid());
    };
    if !matches!(cleanup_return,
            TerminalAbstractOperation::ReturnUnit { cleanup_actions, .. }
                if cleanup_actions.is_empty())
    {
        return Err(invalid());
    }
    let helper_sites = helper_calls
        .iter()
        .map(|operation| match operation {
            TerminalAbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                claim_transfers,
                ..
            } if structural_arguments.is_empty() && claim_transfers.is_empty() => {
                Ok((*psi_operation, *callee))
            }
            _ => Err(invalid()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if helper_sites
        .iter()
        .map(|(operation, _)| *operation)
        .collect::<BTreeSet<_>>()
        .len()
        != helper_sites.len()
        || helper_sites
            .iter()
            .map(|(_, callee)| *callee)
            .collect::<BTreeSet<_>>()
            .len()
            != helper_sites.len()
    {
        return Err(invalid());
    }
    for (_, helper_machine) in helper_sites {
        let helper = functions
            .get(&helper_machine)
            .copied()
            .ok_or_else(invalid)?;
        let Some(helper_type) = helper.attachment else {
            return Err(invalid());
        };
        let Some(helper_declaration) = structural_types.get(&helper_type) else {
            return Err(invalid());
        };
        if helper.machine == cleanup.cleanup_machine
            || helper.result != TerminalAbstractFunctionResult::Unit
            || !helper.parameters.is_empty()
            || !helper.structural_parameters.is_empty()
            || !helper.entry_claims.is_empty()
            || !helper.published_service_ceiling.is_empty()
            || helper.block_entries.as_slice()
                != [
                    omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                        block: helper.entry,
                        operation_offset: 0,
                    },
                ]
            || !matches!(helper_declaration.shape,
                psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty())
            || !matches!(helper.operations.as_slice(),
                [TerminalAbstractOperation::ReturnUnit { cleanup_actions, .. }]
                    if cleanup_actions.is_empty())
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn structural_shape(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, LoweringError> {
    if let Some(shape) = cache.get(&structural_type) {
        return Ok(*shape);
    }
    if !active.insert(structural_type) {
        return Err(LoweringError::RecursiveStructuralType(structural_type));
    }
    let result = (|| {
        let declaration = declarations
            .get(&structural_type)
            .copied()
            .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        match &declaration.shape {
            StructuralTypeShape::Record { fields } => {
                if fields.is_empty() {
                    return Ok(ValueShape::integer(0, 1));
                }
                let mut byte_size = 0_u32;
                let mut alignment = 1_u16;
                for field in fields {
                    if field.relevance.is_erased() {
                        continue;
                    }
                    let field_shape = match &field.field_type {
                        StructuralFieldType::Scalar(ScalarType::Boolean) => {
                            ValueShape::integer(1, 1)
                        }
                        StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                            let size = integer.bits().div_ceil(8);
                            let field_alignment = size.next_power_of_two().min(16);
                            ValueShape::integer(size, field_alignment)
                        }
                        StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => {
                            ValueShape::float(4)
                        }
                        StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => {
                            ValueShape::float(8)
                        }
                        StructuralFieldType::ByteSequence(carrier) => {
                            byte_sequence_shape(*carrier, structural_type)?
                        }
                        StructuralFieldType::Structural(nested) => {
                            structural_shape(*nested, declarations, cache, active)?
                        }
                        StructuralFieldType::Erased { .. } => {
                            return Err(LoweringError::RelevantOpaqueStructuralField(
                                structural_type,
                            ));
                        }
                    };
                    alignment = alignment.max(field_shape.alignment);
                    byte_size = checked_align_up_u32(byte_size, u32::from(field_shape.alignment))
                        .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                    byte_size = byte_size
                        .checked_add(u32::from(field_shape.byte_size))
                        .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                }
                byte_size = checked_align_up_u32(byte_size, u32::from(alignment))
                    .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                if byte_size == 0 {
                    return Err(LoweringError::EmptyStructuralType(structural_type));
                }
                let byte_size = u16::try_from(byte_size)
                    .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?;
                Ok(ValueShape::integer(byte_size, alignment))
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if *length == 0 {
                    return Err(LoweringError::EmptyStructuralType(structural_type));
                }
                let element = structural_shape(*element, declarations, cache, active)?;
                let stride = checked_align_up_u32(
                    u32::from(element.byte_size),
                    u32::from(element.alignment),
                )
                .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                let byte_size = u64::from(stride)
                    .checked_mul(*length)
                    .and_then(|size| u16::try_from(size).ok())
                    .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                Ok(ValueShape::integer(byte_size, element.alignment))
            }
            StructuralTypeShape::Sum { .. } => {
                Err(LoweringError::UnsupportedStructuralSum(structural_type))
            }
        }
    })();
    active.remove(&structural_type);
    let shape = result?;
    cache.insert(structural_type, shape);
    Ok(shape)
}

fn direct_boolean_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<u32, LoweringError> {
    let declaration = declarations
        .get(&structural_type)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(LoweringError::UnknownStructuralType(structural_type));
    };
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut offset = 0_u32;
    for candidate in fields
        .iter()
        .filter(|candidate| !candidate.relevance.is_erased())
    {
        let shape = match candidate.field_type {
            StructuralFieldType::Scalar(ScalarType::Boolean) => ValueShape::integer(1, 1),
            StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                let size = integer.bits().div_ceil(8);
                ValueShape::integer(size, size.next_power_of_two().min(16))
            }
            StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => ValueShape::float(4),
            StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => ValueShape::float(8),
            StructuralFieldType::ByteSequence(carrier) => {
                byte_sequence_shape(carrier, structural_type)?
            }
            StructuralFieldType::Structural(nested) => {
                structural_shape(nested, declarations, &mut cache, &mut active)?
            }
            StructuralFieldType::Erased { .. } => {
                return Err(LoweringError::RelevantOpaqueStructuralField(
                    structural_type,
                ));
            }
        };
        offset = checked_align_up_u32(offset, u32::from(shape.alignment))
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
        if candidate.id == field {
            return (candidate.field_type == StructuralFieldType::Scalar(ScalarType::Boolean))
                .then_some(offset)
                .ok_or(LoweringError::UnknownStructuralType(structural_type));
        }
        offset = offset
            .checked_add(u32::from(shape.byte_size))
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
    }
    Err(LoweringError::UnknownStructuralType(structural_type))
}

fn resolve_structural_field_path(
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<(StructuralTypeId, ValueShape, u32), LoweringError> {
    let root_type = structural_type;
    let mut total_offset = 0_u32;
    let mut selected_shape = None;
    for segment in path {
        let StructuralPathSegment::Field(identity) = segment else {
            return Err(LoweringError::UnknownStructuralType(structural_type));
        };
        let declaration = declarations
            .get(&structural_type)
            .copied()
            .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return Err(LoweringError::UnknownStructuralType(structural_type));
        };
        let mut local_offset = 0_u32;
        let mut selected = None;
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            let field_shape = match field.field_type {
                StructuralFieldType::Scalar(ScalarType::Boolean) => ValueShape::integer(1, 1),
                StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                    let size = integer.bits().div_ceil(8);
                    ValueShape::integer(size, size.next_power_of_two().min(16))
                }
                StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => ValueShape::float(4),
                StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => ValueShape::float(8),
                StructuralFieldType::ByteSequence(carrier) => {
                    byte_sequence_shape(carrier, structural_type)?
                }
                StructuralFieldType::Structural(nested) => {
                    structural_shape(nested, declarations, cache, active)?
                }
                StructuralFieldType::Erased { .. } => {
                    return Err(LoweringError::RelevantOpaqueStructuralField(
                        structural_type,
                    ));
                }
            };
            local_offset = checked_align_up_u32(local_offset, u32::from(field_shape.alignment))
                .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
            if field.identity == *identity {
                let StructuralFieldType::Structural(field_type) = field.field_type else {
                    return Err(LoweringError::UnknownStructuralType(structural_type));
                };
                selected = Some((field_type, field_shape, local_offset));
                break;
            }
            local_offset = local_offset
                .checked_add(u32::from(field_shape.byte_size))
                .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
        }
        let (field_type, field_shape, field_offset) =
            selected.ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        total_offset = total_offset
            .checked_add(field_offset)
            .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
        structural_type = field_type;
        selected_shape = Some(field_shape);
    }
    selected_shape
        .map(|shape| (structural_type, shape, total_offset))
        .ok_or(LoweringError::UnknownStructuralType(root_type))
}

fn byte_sequence_shape(
    carrier: psi_terminal::ByteSequenceCarrier,
    structural_type: StructuralTypeId,
) -> Result<ValueShape, LoweringError> {
    let byte_size = match carrier {
        // Current terminal-native targets are 64-bit. The semantic carrier
        // deliberately does not retain the physical descriptor fields.
        psi_terminal::ByteSequenceCarrier::BorrowedView => 16_u64,
        psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity } => capacity
            .checked_add(8)
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?,
    };
    Ok(ValueShape::integer(
        u16::try_from(byte_size)
            .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?,
        8,
    ))
}

fn expected_maximal_residual_subtrees(
    root_type: StructuralTypeId,
    moved: &[(Vec<StructuralPathSegment>, StructuralTypeId)],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    if moved.is_empty() {
        return None;
    }
    let borrowed = moved
        .iter()
        .map(|(path, structural_type)| (path.as_slice(), *structural_type))
        .collect::<Vec<_>>();
    let mut residuals = Vec::new();
    append_maximal_residual_subtrees(root_type, &[], &borrowed, declarations, &mut residuals)?;
    (!residuals.is_empty()).then_some(residuals)
}

fn append_maximal_residual_subtrees(
    structural_type: StructuralTypeId,
    prefix: &[StructuralPathSegment],
    moved: &[(&[StructuralPathSegment], StructuralTypeId)],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    let declaration = declarations.get(&structural_type).copied()?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    if fields.is_empty()
        || fields.iter().any(|field| {
            field.relevance.is_erased()
                || !matches!(field.field_type, StructuralFieldType::Structural(_))
        })
        || moved
            .iter()
            .any(|(path, _)| !matches!(path.first(), Some(StructuralPathSegment::Field(_))))
    {
        return None;
    }
    for field in fields.iter().rev() {
        let StructuralFieldType::Structural(field_type) = field.field_type else {
            return None;
        };
        let matching = moved
            .iter()
            .filter(|(path, _)| {
                matches!(path.first(), Some(StructuralPathSegment::Field(identity))
                    if identity == &field.identity)
            })
            .copied()
            .collect::<Vec<_>>();
        let mut field_path = prefix.to_vec();
        field_path.push(StructuralPathSegment::Field(field.identity.clone()));
        if matching.is_empty() {
            residuals.push((field_path, field_type));
            continue;
        }
        let whole_moves = matching
            .iter()
            .filter(|(path, _)| path.len() == 1)
            .collect::<Vec<_>>();
        if !whole_moves.is_empty() {
            if whole_moves.len() != 1 || matching.len() != 1 || whole_moves[0].1 != field_type {
                return None;
            }
            continue;
        }
        let nested = matching
            .iter()
            .map(|(path, leaf_type)| (&path[1..], *leaf_type))
            .collect::<Vec<_>>();
        append_maximal_residual_subtrees(
            field_type,
            &field_path,
            &nested,
            declarations,
            residuals,
        )?;
    }
    Some(())
}

fn checked_align_up_u32(value: u32, alignment: u32) -> Option<u32> {
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_call(
    psi_operation: psi_core::OperationId,
    result: ValueId,
    scalar_type: ScalarType,
    callee: MachineId,
    arguments: &[ValueId],
    values: &BTreeMap<ValueId, KnownScalar>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
) -> Result<KnownScalar, LoweringError> {
    let callee_function = functions
        .get(&callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(callee))?;
    let callee_result = scalar_function_result(callee_function)?;
    let callee_signature = CallSignature {
        parameters: callee_function
            .parameters
            .iter()
            .map(|parameter| scalar_shape(parameter.value, parameter.scalar_type, true))
            .collect::<Result<Vec<_>, _>>()?,
        result: Some(scalar_shape(
            callee_result.value,
            callee_result.scalar_type,
            false,
        )?),
    };
    let callee_call_plan =
        evaluate_call_plan(CallingPolicy::native_for_target(target), &callee_signature)
            .map_err(LoweringError::AbiPlan)?;
    if arguments.len() != callee_function.parameters.len()
        || arguments.len() != callee_call_plan.parameters.len()
    {
        return Err(LoweringError::CallArgumentCountMismatch {
            callee,
            expected: callee_function.parameters.len(),
            actual: arguments.len(),
        });
    }
    let arguments = arguments
        .iter()
        .zip(&callee_function.parameters)
        .zip(&callee_call_plan.parameters)
        .map(|((argument, parameter), placement)| {
            let expression = values
                .get(argument)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*argument))?
                .into_expression(*argument)?;
            if expression.scalar_type() != parameter.scalar_type {
                return Err(LoweringError::CallArgumentTypeMismatch {
                    callee,
                    argument: *argument,
                });
            }
            Ok(TerminalTargetCallArgument {
                scalar_type: parameter.scalar_type,
                location: scalar_parameter_location(parameter, placement)?,
                expression,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(match scalar_type {
        ScalarType::Boolean => KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::Call {
            psi_operation,
            source_value: result,
            callee,
            arguments,
        }),
        ScalarType::Integer(scalar_type) => KnownScalar::Integer {
            scalar_type,
            value: KnownInteger::Runtime(TerminalTargetIntegerExpression::Call {
                psi_operation,
                source_value: result,
                callee,
                arguments,
            }),
        },
    })
}

fn lower_integer_conditional(
    function: &TerminalAbstractFunction,
    values: &BTreeMap<ValueId, KnownScalar>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
) -> Result<TerminalTargetFunction, LoweringError> {
    let function_result = scalar_function_result(function)?;
    let ScalarType::Integer(result_type) = function_result.scalar_type else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    let lowered = lower_conditional_block(
        function,
        result_type,
        values.clone(),
        function.entry,
        BTreeSet::new(),
        target,
        functions,
    )?;
    Ok(TerminalTargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: conditional_provenance(function, lowered.operations, lowered.edges),
        operation: target_operation_from_integer_control(lowered.control, result_type),
    })
}

fn lower_boolean_conditional(
    function: &TerminalAbstractFunction,
    values: &BTreeMap<ValueId, KnownScalar>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
) -> Result<TerminalTargetFunction, LoweringError> {
    let lowered = lower_boolean_block(
        function,
        values.clone(),
        function.entry,
        BTreeSet::new(),
        target,
        functions,
        &[],
        &BTreeMap::new(),
    )?;
    Ok(TerminalTargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: conditional_provenance(function, lowered.operations, lowered.edges),
        operation: target_operation_from_boolean_control(lowered.control),
    })
}

struct LoweredBooleanArm {
    arm: TerminalTargetConditionalBooleanArm,
    operations: Vec<OperationId>,
    edges: Vec<EdgeId>,
}

fn lower_boolean_arm(
    function: &TerminalAbstractFunction,
    values: &BTreeMap<ValueId, KnownScalar>,
    successor: &omega_terminal_abstract_operations::TerminalAbstractSuccessor,
    visited: &BTreeSet<BlockId>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_parameters: &[TerminalTargetStructuralParameter],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<LoweredBooleanArm, LoweringError> {
    let mut values = values.clone();
    bind_conditional_values(&mut values, &successor.bindings, successor.psi_edge)?;
    let mut lowered = lower_boolean_block(
        function,
        values,
        successor.target,
        visited.clone(),
        target,
        functions,
        structural_parameters,
        structural_types,
    )?;
    lowered.edges.insert(0, successor.psi_edge);
    Ok(LoweredBooleanArm {
        arm: TerminalTargetConditionalBooleanArm {
            psi_edge: successor.psi_edge,
            control: Box::new(lowered.control),
        },
        operations: lowered.operations,
        edges: lowered.edges,
    })
}

struct LoweredBooleanControl {
    control: TerminalTargetBooleanControl,
    operations: Vec<OperationId>,
    edges: Vec<EdgeId>,
}

fn lower_boolean_block(
    function: &TerminalAbstractFunction,
    mut values: BTreeMap<ValueId, KnownScalar>,
    block: BlockId,
    mut visited: BTreeSet<BlockId>,
    native_target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_parameters: &[TerminalTargetStructuralParameter],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<LoweredBooleanControl, LoweringError> {
    if !visited.insert(block) {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    }
    let Some((block_index, block_entry)) = function
        .block_entries
        .iter()
        .enumerate()
        .find(|(_, block_entry)| block_entry.block == block)
    else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    let block_end = function
        .block_entries
        .get(block_index + 1)
        .map_or(function.operations.len(), |next| next.operation_offset);
    let Some((terminator, body)) = function
        .operations
        .get(block_entry.operation_offset..block_end)
        .and_then(|operations| operations.split_last())
    else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    let mut operations = Vec::new();
    for operation in body {
        if !lower_conditional_scalar_operation(
            operation,
            function.machine,
            &mut values,
            &mut operations,
            native_target,
            functions,
            structural_parameters,
            structural_types,
        )? {
            return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                function.machine,
            ));
        }
    }
    match terminator {
        TerminalAbstractOperation::Jump {
            psi_edge,
            target,
            bindings,
        } => {
            bind_conditional_values(&mut values, bindings, *psi_edge)?;
            let mut lowered = lower_boolean_block(
                function,
                values,
                *target,
                visited,
                native_target,
                functions,
                structural_parameters,
                structural_types,
            )?;
            operations.append(&mut lowered.operations);
            lowered.operations = operations;
            lowered.edges.insert(0, *psi_edge);
            Ok(lowered)
        }
        TerminalAbstractOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => match values
            .get(condition)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*condition))?
        {
            KnownScalar::Boolean(selected_true_arm) => {
                let selected = if selected_true_arm {
                    when_true
                } else {
                    when_false
                };
                let mut lowered = lower_boolean_arm(
                    function,
                    &values,
                    selected,
                    &visited,
                    native_target,
                    functions,
                    structural_parameters,
                    structural_types,
                )?;
                operations.append(&mut lowered.operations);
                Ok(LoweredBooleanControl {
                    control: *lowered.arm.control,
                    operations,
                    edges: lowered.edges,
                })
            }
            KnownScalar::BooleanRuntime(expression) => {
                let direct = direct_boolean_condition(expression.clone(), *condition);
                let invert = matches!(direct, Ok((_, _, true)));
                let (selected_true, selected_false) = if invert {
                    (when_false, when_true)
                } else {
                    (when_true, when_false)
                };
                let lowered_true = lower_boolean_arm(
                    function,
                    &values,
                    selected_true,
                    &visited,
                    native_target,
                    functions,
                    structural_parameters,
                    structural_types,
                )?;
                let lowered_false = lower_boolean_arm(
                    function,
                    &values,
                    selected_false,
                    &visited,
                    native_target,
                    functions,
                    structural_parameters,
                    structural_types,
                )?;
                operations.extend(lowered_true.operations);
                operations.extend(lowered_false.operations);
                let mut edges = lowered_true.edges;
                edges.extend(lowered_false.edges);
                let control = match direct {
                    Ok((parameter_index, location, _)) => {
                        TerminalTargetBooleanControl::Conditional {
                            condition_source: *condition,
                            condition_parameter_index: parameter_index,
                            condition_location: location,
                            when_true: lowered_true.arm,
                            when_false: lowered_false.arm,
                        }
                    }
                    Err(LoweringError::UnsupportedRuntimeBooleanCondition(_)) => {
                        TerminalTargetBooleanControl::ConditionalExpression {
                            condition_source: *condition,
                            condition: expression,
                            when_true: lowered_true.arm,
                            when_false: lowered_false.arm,
                        }
                    }
                    Err(error) => return Err(error),
                };
                Ok(LoweredBooleanControl {
                    control,
                    operations,
                    edges,
                })
            }
            KnownScalar::Integer { .. } => {
                Err(LoweringError::ConditionalConditionMustBeBoolean(*condition))
            }
        },
        TerminalAbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            ..
        } => {
            if *result != scalar_function_result(function)?.value
                || *scalar_type != ScalarType::Boolean
            {
                return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                    function.machine,
                ));
            }
            let returned = values
                .get(value)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*value))?;
            let control = match returned {
                KnownScalar::Boolean(returned_value) => {
                    TerminalTargetBooleanControl::ReturnImmediate {
                        psi_return_edge: *psi_edge,
                        source_value: *value,
                        value: returned_value,
                    }
                }
                KnownScalar::BooleanRuntime(expression) => {
                    match direct_boolean_condition(expression.clone(), *value) {
                        Ok((parameter_index, location, invert)) if invert => {
                            TerminalTargetBooleanControl::ReturnNotParameter {
                                psi_return_edge: *psi_edge,
                                source_value: *value,
                                parameter_index,
                                location,
                            }
                        }
                        Ok((parameter_index, location, _)) => {
                            TerminalTargetBooleanControl::ReturnParameter {
                                psi_return_edge: *psi_edge,
                                source_value: *value,
                                parameter_index,
                                location,
                            }
                        }
                        Err(LoweringError::UnsupportedRuntimeBooleanCondition(_)) => {
                            TerminalTargetBooleanControl::ReturnExpression {
                                psi_return_edge: *psi_edge,
                                source_value: *value,
                                expression,
                            }
                        }
                        Err(error) => return Err(error),
                    }
                }
                KnownScalar::Integer { .. } => {
                    return Err(LoweringError::ValueTypeMismatch(*value));
                }
            };
            Ok(LoweredBooleanControl {
                control,
                operations,
                edges: vec![*psi_edge],
            })
        }
        TerminalAbstractOperation::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => Ok(LoweredBooleanControl {
            control: TerminalTargetBooleanControl::Crash {
                psi_crash_edge: *psi_edge,
                cause: *cause,
                site_guard: site_guard.clone(),
                frontier_lower_bound: frontier_lower_bound.clone(),
            },
            operations,
            edges: vec![*psi_edge],
        }),
        _ => Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        )),
    }
}

fn target_operation_from_boolean_control(
    control: TerminalTargetBooleanControl,
) -> TerminalTargetOperation {
    match control {
        TerminalTargetBooleanControl::Crash {
            psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => TerminalTargetOperation::Crash {
            psi_edge: psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        },
        TerminalTargetBooleanControl::ReturnImmediate {
            psi_return_edge,
            source_value,
            value,
        } => TerminalTargetOperation::ReturnBooleanImmediate {
            psi_edge: psi_return_edge,
            source_value,
            value,
        },
        TerminalTargetBooleanControl::ReturnParameter {
            psi_return_edge,
            source_value,
            parameter_index,
            location,
        } => TerminalTargetOperation::ReturnBooleanParameter {
            psi_edge: psi_return_edge,
            source_value,
            parameter_index,
            location,
        },
        TerminalTargetBooleanControl::ReturnNotParameter {
            psi_return_edge,
            source_value,
            parameter_index,
            location,
        } => TerminalTargetOperation::ReturnBooleanNotParameter {
            psi_edge: psi_return_edge,
            source_value,
            parameter_index,
            location,
        },
        TerminalTargetBooleanControl::ReturnExpression {
            psi_return_edge,
            source_value,
            expression,
        } => TerminalTargetOperation::ReturnBooleanExpression {
            psi_edge: psi_return_edge,
            source_value,
            expression,
        },
        TerminalTargetBooleanControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => TerminalTargetOperation::ReturnBooleanConditionalControl {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        },
        TerminalTargetBooleanControl::ConditionalExpression {
            condition_source,
            condition,
            when_true,
            when_false,
        } => TerminalTargetOperation::ReturnBooleanExpressionConditionalControl {
            condition_source,
            condition,
            when_true,
            when_false,
        },
    }
}

struct LoweredConditionalArm {
    arm: TerminalTargetConditionalIntegerArm,
    operations: Vec<OperationId>,
    edges: Vec<EdgeId>,
}

fn lower_conditional_arm(
    function: &TerminalAbstractFunction,
    result_type: IntegerType,
    values: &BTreeMap<ValueId, KnownScalar>,
    successor: &omega_terminal_abstract_operations::TerminalAbstractSuccessor,
    visited: &BTreeSet<BlockId>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
) -> Result<LoweredConditionalArm, LoweringError> {
    let mut values = values.clone();
    bind_conditional_values(&mut values, &successor.bindings, successor.psi_edge)?;
    let mut lowered = lower_conditional_block(
        function,
        result_type,
        values,
        successor.target,
        visited.clone(),
        target,
        functions,
    )?;
    lowered.edges.insert(0, successor.psi_edge);
    Ok(LoweredConditionalArm {
        arm: TerminalTargetConditionalIntegerArm {
            psi_edge: successor.psi_edge,
            control: Box::new(lowered.control),
        },
        operations: lowered.operations,
        edges: lowered.edges,
    })
}

struct LoweredIntegerControl {
    control: TerminalTargetIntegerControl,
    operations: Vec<OperationId>,
    edges: Vec<EdgeId>,
}

fn lower_conditional_block(
    function: &TerminalAbstractFunction,
    result_type: IntegerType,
    mut values: BTreeMap<ValueId, KnownScalar>,
    block: BlockId,
    mut visited: BTreeSet<BlockId>,
    native_target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
) -> Result<LoweredIntegerControl, LoweringError> {
    if !visited.insert(block) {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    }
    let Some((block_index, block_entry)) = function
        .block_entries
        .iter()
        .enumerate()
        .find(|(_, block_entry)| block_entry.block == block)
    else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    let block_end = function
        .block_entries
        .get(block_index + 1)
        .map_or(function.operations.len(), |next| next.operation_offset);
    let Some((terminator, body)) = function
        .operations
        .get(block_entry.operation_offset..block_end)
        .and_then(|operations| operations.split_last())
    else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    let mut operations = Vec::new();
    for operation in body {
        if !lower_conditional_scalar_operation(
            operation,
            function.machine,
            &mut values,
            &mut operations,
            native_target,
            functions,
            &[],
            &BTreeMap::new(),
        )? {
            return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                function.machine,
            ));
        }
    }
    match terminator {
        TerminalAbstractOperation::Jump {
            psi_edge,
            target,
            bindings,
        } => {
            bind_conditional_values(&mut values, bindings, *psi_edge)?;
            let mut lowered = lower_conditional_block(
                function,
                result_type,
                values,
                *target,
                visited,
                native_target,
                functions,
            )?;
            operations.append(&mut lowered.operations);
            lowered.operations = operations;
            lowered.edges.insert(0, *psi_edge);
            Ok(lowered)
        }
        TerminalAbstractOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => match values
            .get(condition)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*condition))?
        {
            KnownScalar::Boolean(selected_true_arm) => {
                let selected = if selected_true_arm {
                    when_true
                } else {
                    when_false
                };
                let mut lowered = lower_conditional_arm(
                    function,
                    result_type,
                    &values,
                    selected,
                    &visited,
                    native_target,
                    functions,
                )?;
                operations.append(&mut lowered.operations);
                Ok(LoweredIntegerControl {
                    control: *lowered.arm.control,
                    operations,
                    edges: lowered.edges,
                })
            }
            KnownScalar::BooleanRuntime(expression) => {
                let direct = direct_boolean_condition(expression.clone(), *condition);
                let invert = matches!(direct, Ok((_, _, true)));
                let (selected_true, selected_false) = if invert {
                    (when_false, when_true)
                } else {
                    (when_true, when_false)
                };
                let lowered_true = lower_conditional_arm(
                    function,
                    result_type,
                    &values,
                    selected_true,
                    &visited,
                    native_target,
                    functions,
                )?;
                let lowered_false = lower_conditional_arm(
                    function,
                    result_type,
                    &values,
                    selected_false,
                    &visited,
                    native_target,
                    functions,
                )?;
                operations.extend(lowered_true.operations);
                operations.extend(lowered_false.operations);
                let mut edges = lowered_true.edges;
                edges.extend(lowered_false.edges);
                let control = match direct {
                    Ok((parameter_index, location, _)) => {
                        TerminalTargetIntegerControl::Conditional {
                            condition_source: *condition,
                            condition_parameter_index: parameter_index,
                            condition_location: location,
                            when_true: lowered_true.arm,
                            when_false: lowered_false.arm,
                        }
                    }
                    Err(LoweringError::UnsupportedRuntimeBooleanCondition(_)) => {
                        TerminalTargetIntegerControl::ConditionalExpression {
                            condition_source: *condition,
                            condition: expression,
                            when_true: lowered_true.arm,
                            when_false: lowered_false.arm,
                        }
                    }
                    Err(error) => return Err(error),
                };
                Ok(LoweredIntegerControl {
                    control,
                    operations,
                    edges,
                })
            }
            KnownScalar::Integer { .. } => {
                Err(LoweringError::ConditionalConditionMustBeBoolean(*condition))
            }
        },
        TerminalAbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            ..
        } => {
            let function_result = scalar_function_result(function)?;
            if *result != function_result.value || *scalar_type != function_result.scalar_type {
                return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                    function.machine,
                ));
            }
            let KnownScalar::Integer {
                scalar_type: returned_type,
                value: returned,
            } = values
                .get(value)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*value))?
            else {
                return Err(LoweringError::ValueTypeMismatch(*value));
            };
            if returned_type != result_type {
                return Err(LoweringError::ValueTypeMismatch(*value));
            }
            Ok(LoweredIntegerControl {
                control: TerminalTargetIntegerControl::Return {
                    psi_return_edge: *psi_edge,
                    source_value: *value,
                    expression: returned.into_expression(*value),
                },
                operations,
                edges: vec![*psi_edge],
            })
        }
        TerminalAbstractOperation::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => Ok(LoweredIntegerControl {
            control: TerminalTargetIntegerControl::Crash {
                psi_crash_edge: *psi_edge,
                cause: *cause,
                site_guard: site_guard.clone(),
                frontier_lower_bound: frontier_lower_bound.clone(),
            },
            operations,
            edges: vec![*psi_edge],
        }),
        _ => Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        )),
    }
}

fn bind_conditional_values(
    values: &mut BTreeMap<ValueId, KnownScalar>,
    bindings: &[omega_terminal_abstract_operations::TerminalValueBinding],
    edge: EdgeId,
) -> Result<(), LoweringError> {
    let pending = bindings
        .iter()
        .map(|binding| {
            let value = values
                .get(&binding.argument)
                .cloned()
                .ok_or(LoweringError::UnknownValue(binding.argument))?;
            if binding.scalar_type != value.scalar_type() {
                return Err(LoweringError::ConditionalArmBindingTypeMismatch(edge));
            }
            Ok((
                binding.parameter,
                value.rebind_direct_parameter(binding.parameter),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (parameter, value) in pending {
        insert_value(values, parameter, value)?;
    }
    Ok(())
}

fn target_operation_from_integer_control(
    control: TerminalTargetIntegerControl,
    scalar_type: IntegerType,
) -> TerminalTargetOperation {
    match control {
        TerminalTargetIntegerControl::Crash {
            psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => TerminalTargetOperation::Crash {
            psi_edge: psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        },
        TerminalTargetIntegerControl::Return {
            psi_return_edge,
            source_value,
            expression,
        } => match expression {
            TerminalTargetIntegerExpression::Immediate { value, .. } => {
                TerminalTargetOperation::ReturnIntegerImmediate {
                    psi_edge: psi_return_edge,
                    source_value,
                    scalar_type,
                    value,
                }
            }
            TerminalTargetIntegerExpression::Parameter {
                parameter_index,
                location,
                ..
            } => TerminalTargetOperation::ReturnIntegerParameter {
                psi_edge: psi_return_edge,
                source_value,
                scalar_type,
                parameter_index,
                location,
            },
            expression => TerminalTargetOperation::ReturnIntegerExpression {
                psi_edge: psi_return_edge,
                source_value,
                scalar_type,
                expression,
            },
        },
        TerminalTargetIntegerControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => TerminalTargetOperation::ReturnIntegerConditionalControl {
            condition_source,
            condition_parameter_index,
            condition_location,
            scalar_type,
            when_true,
            when_false,
        },
        TerminalTargetIntegerControl::ConditionalExpression {
            condition_source,
            condition,
            when_true,
            when_false,
        } => TerminalTargetOperation::ReturnIntegerExpressionConditionalControl {
            condition_source,
            condition,
            scalar_type,
            when_true,
            when_false,
        },
    }
}

fn lower_conditional_scalar_operation(
    operation: &TerminalAbstractOperation,
    machine: MachineId,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut Vec<psi_core::OperationId>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_parameters: &[TerminalTargetStructuralParameter],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<bool, LoweringError> {
    if let TerminalAbstractOperation::Call {
        psi_operation,
        result,
        scalar_type,
        callee,
        arguments,
    } = operation
    {
        let value = lower_call(
            *psi_operation,
            *result,
            *scalar_type,
            *callee,
            arguments,
            values,
            target,
            functions,
        )?;
        insert_value(values, *result, value)?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value,
    } = operation
    {
        insert_value(values, *result, KnownScalar::Boolean(*value))?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::BooleanStructuralField {
        psi_operation,
        result,
        source,
        field,
    } = operation
    {
        let parameter = structural_parameters
            .iter()
            .find(|parameter| parameter.place == *source)
            .ok_or(LoweringError::UnsupportedOperationInScalarFunction(machine))?;
        let field_byte_offset =
            direct_boolean_field_offset(parameter.structural_type, *field, structural_types)?;
        insert_value(
            values,
            *result,
            KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::StructuralField {
                psi_operation: *psi_operation,
                source_value: *result,
                source: *source,
                field: *field,
                source_placement: parameter.placement.clone(),
                field_byte_offset,
            }),
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::BooleanNot {
        psi_operation,
        result,
        operand,
    } = operation
    {
        let operand = values
            .get(operand)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*operand))?;
        insert_value(
            values,
            *result,
            negate_boolean(operand, *psi_operation, *result)?,
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::BooleanEqual {
        psi_operation,
        result,
        left,
        right,
    } = operation
    {
        let left_value = values
            .get(left)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*left))?;
        let right_value = values
            .get(right)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*right))?;
        insert_value(
            values,
            *result,
            equal_boolean(left_value, right_value, *psi_operation, *result)?,
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::IntegerEqual {
        psi_operation,
        result,
        left,
        right,
    } = operation
    {
        let left_value = values
            .get(left)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*left))?;
        let right_value = values
            .get(right)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*right))?;
        insert_value(
            values,
            *result,
            equal_integer(
                *left,
                left_value,
                *right,
                right_value,
                *psi_operation,
                *result,
            )?,
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::IntegerLessThan {
        psi_operation,
        result,
        left,
        right,
    }
    | TerminalAbstractOperation::IntegerLessOrEqual {
        psi_operation,
        result,
        left,
        right,
    } = operation
    {
        let left_value = values
            .get(left)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*left))?;
        let right_value = values
            .get(right)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*right))?;
        let inclusive = matches!(
            operation,
            TerminalAbstractOperation::IntegerLessOrEqual { .. }
        );
        insert_value(
            values,
            *result,
            order_integer(
                *left,
                left_value,
                *right,
                right_value,
                *psi_operation,
                *result,
                inclusive,
            )?,
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    let (psi_operation, result, scalar_type, value) = match operation {
        TerminalAbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return Err(LoweringError::IntegerConstantHasNonIntegerType(*result));
            };
            if !integer_type.admits(*value) {
                return Err(LoweringError::IntegerConstantOutsideType(*result));
            }
            (
                *psi_operation,
                *result,
                *integer_type,
                KnownInteger::Immediate(*value),
            )
        }
        TerminalAbstractOperation::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingAdd,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingAdd,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingSubtract,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingSubtract,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingMultiply,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingMultiply,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::ExactDivide,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::ExactRemainder,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingDivide,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingRemainder,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingDivide,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingRemainder,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => {
            let operand_value = match values.get(operand).cloned() {
                Some(KnownScalar::Integer {
                    scalar_type: operand_type,
                    value,
                }) if operand_type == *scalar_type => value,
                Some(_) => {
                    return Err(LoweringError::IntegerBitwiseOperandTypeMismatch(*result));
                }
                None => return Err(LoweringError::UnknownValue(*operand)),
            };
            let value = match operand_value {
                KnownInteger::Immediate(value) => KnownInteger::Immediate(
                    scalar_type
                        .bitwise_not(value)
                        .ok_or(LoweringError::IntegerBitwiseOperandTypeMismatch(*result))?,
                ),
                KnownInteger::Runtime(expression) => {
                    KnownInteger::Runtime(TerminalTargetIntegerExpression::BitwiseNot {
                        psi_operation: *psi_operation,
                        operand: Box::new(expression),
                    })
                }
            };
            (*psi_operation, *result, *scalar_type, value)
        }
        TerminalAbstractOperation::IntegerWiden {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => {
            let operand_value = match values.get(operand).cloned() {
                Some(KnownScalar::Integer {
                    scalar_type: operand_type,
                    value,
                }) if operand_type == *source_type && source_type.can_widen_to(*target_type) => {
                    value
                }
                Some(_) => return Err(LoweringError::IntegerWidenTypeMismatch(*result)),
                None => return Err(LoweringError::UnknownValue(*operand)),
            };
            let value = match operand_value {
                KnownInteger::Immediate(value) => KnownInteger::Immediate(
                    source_type
                        .widen_value_to(*target_type, value)
                        .ok_or(LoweringError::IntegerWidenTypeMismatch(*result))?,
                ),
                KnownInteger::Runtime(expression) => {
                    KnownInteger::Runtime(TerminalTargetIntegerExpression::IntegerWiden {
                        psi_operation: *psi_operation,
                        source_type: *source_type,
                        operand: Box::new(expression),
                    })
                }
            };
            (*psi_operation, *result, *target_type, value)
        }
        TerminalAbstractOperation::IntegerExactCast {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => {
            let operand_value = match values.get(operand).cloned() {
                Some(KnownScalar::Integer {
                    scalar_type: operand_type,
                    value,
                }) if operand_type == *source_type
                    && source_type.can_exact_cast_to(*target_type) =>
                {
                    value
                }
                Some(_) => return Err(LoweringError::IntegerExactCastTypeMismatch(*result)),
                None => return Err(LoweringError::UnknownValue(*operand)),
            };
            let value = match operand_value {
                KnownInteger::Immediate(value) => KnownInteger::Immediate(
                    source_type
                        .exact_cast_value_to(*target_type, value)
                        .ok_or(LoweringError::IntegerExactCastTypeMismatch(*result))?,
                ),
                KnownInteger::Runtime(expression) => {
                    KnownInteger::Runtime(TerminalTargetIntegerExpression::IntegerExactCast {
                        psi_operation: *psi_operation,
                        source_type: *source_type,
                        operand: Box::new(expression),
                    })
                }
            };
            (*psi_operation, *result, *target_type, value)
        }
        TerminalAbstractOperation::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | TerminalAbstractOperation::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | TerminalAbstractOperation::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let kind = match operation {
                TerminalAbstractOperation::IntegerBitwiseAnd { .. } => {
                    IntegerBinaryKind::BitwiseAnd
                }
                TerminalAbstractOperation::IntegerBitwiseOr { .. } => IntegerBinaryKind::BitwiseOr,
                TerminalAbstractOperation::IntegerBitwiseXor { .. } => {
                    IntegerBinaryKind::BitwiseXor
                }
                _ => unreachable!(),
            };
            (
                *psi_operation,
                *result,
                *scalar_type,
                lower_conditional_integer_binary(
                    values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    kind,
                    *psi_operation,
                )?,
            )
        }
        TerminalAbstractOperation::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        }
        | TerminalAbstractOperation::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => {
            let kind = if matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerShiftLeft { .. }
            ) {
                WrappingShiftKind::Left
            } else {
                WrappingShiftKind::Right
            };
            (
                *psi_operation,
                *result,
                *value_type,
                lower_wrapping_shift(
                    values,
                    *result,
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                    kind,
                    *psi_operation,
                )?,
            )
        }
        TerminalAbstractOperation::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            *psi_operation,
            *result,
            *value_type,
            lower_exact_shift_right(
                values,
                *result,
                *value_type,
                *count_type,
                *value,
                *count,
                *psi_operation,
            )?,
        ),
        TerminalAbstractOperation::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            *psi_operation,
            *result,
            *value_type,
            lower_exact_shift_left(
                values,
                *result,
                *value_type,
                *count_type,
                *value,
                *count,
                *psi_operation,
            )?,
        ),
        _ => return Ok(false),
    };
    insert_value(values, result, KnownScalar::Integer { scalar_type, value })?;
    provenance.push(psi_operation);
    Ok(true)
}

#[derive(Clone, Copy)]
enum IntegerBinaryKind {
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
}

#[derive(Clone, Copy)]
enum WrappingShiftKind {
    Left,
    Right,
}

#[allow(clippy::too_many_arguments)]
fn lower_wrapping_shift(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value_id: ValueId,
    count_id: ValueId,
    kind: WrappingShiftKind,
    psi_operation: psi_core::OperationId,
) -> Result<KnownInteger, LoweringError> {
    let operand = |id, expected_type| match values.get(&id).cloned() {
        Some(KnownScalar::Integer { scalar_type, value }) if scalar_type == expected_type => {
            Ok(value)
        }
        Some(_) => Err(LoweringError::WrappingShiftOperandTypeMismatch(result)),
        None => Err(LoweringError::UnknownValue(id)),
    };
    let value = operand(value_id, value_type)?;
    let count = operand(count_id, count_type)?;
    Ok(match (value, count) {
        (KnownInteger::Immediate(value), KnownInteger::Immediate(count)) => {
            let shifted = match kind {
                WrappingShiftKind::Left => value_type.wrapping_shift_left(value, count_type, count),
                WrappingShiftKind::Right => {
                    value_type.wrapping_shift_right(value, count_type, count)
                }
            }
            .ok_or(LoweringError::WrappingShiftOperandTypeMismatch(result))?;
            KnownInteger::Immediate(shifted)
        }
        (value, count) => {
            let value = Box::new(value.into_expression(value_id));
            let count = Box::new(count.into_expression(count_id));
            KnownInteger::Runtime(match kind {
                WrappingShiftKind::Left => TerminalTargetIntegerExpression::WrappingShiftLeft {
                    psi_operation,
                    count_type,
                    value,
                    count,
                },
                WrappingShiftKind::Right => TerminalTargetIntegerExpression::WrappingShiftRight {
                    psi_operation,
                    count_type,
                    value,
                    count,
                },
            })
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_exact_shift_right(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value_id: ValueId,
    count_id: ValueId,
    psi_operation: psi_core::OperationId,
) -> Result<KnownInteger, LoweringError> {
    let operand = |id, expected_type| match values.get(&id).cloned() {
        Some(KnownScalar::Integer { scalar_type, value }) if scalar_type == expected_type => {
            Ok(value)
        }
        Some(_) => Err(LoweringError::ExactShiftOperandTypeMismatch(result)),
        None => Err(LoweringError::UnknownValue(id)),
    };
    let value = operand(value_id, value_type)?;
    let count = operand(count_id, count_type)?;
    Ok(match (value, count) {
        (KnownInteger::Immediate(value), KnownInteger::Immediate(count)) => {
            KnownInteger::Immediate(
                value_type
                    .exact_shift_right(value, count_type, count)
                    .ok_or(LoweringError::ExactShiftOperandTypeMismatch(result))?,
            )
        }
        (value, count) => KnownInteger::Runtime(TerminalTargetIntegerExpression::ExactShiftRight {
            psi_operation,
            count_type,
            value: Box::new(value.into_expression(value_id)),
            count: Box::new(count.into_expression(count_id)),
        }),
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_exact_shift_left(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value_id: ValueId,
    count_id: ValueId,
    psi_operation: psi_core::OperationId,
) -> Result<KnownInteger, LoweringError> {
    let operand = |id, expected_type| match values.get(&id).cloned() {
        Some(KnownScalar::Integer { scalar_type, value }) if scalar_type == expected_type => {
            Ok(value)
        }
        Some(_) => Err(LoweringError::ExactShiftOperandTypeMismatch(result)),
        None => Err(LoweringError::UnknownValue(id)),
    };
    let value = operand(value_id, value_type)?;
    let count = operand(count_id, count_type)?;
    Ok(match (value, count) {
        (KnownInteger::Immediate(value), KnownInteger::Immediate(count)) => {
            KnownInteger::Immediate(
                value_type
                    .exact_shift_left(value, count_type, count)
                    .ok_or(LoweringError::ExactShiftOperandTypeMismatch(result))?,
            )
        }
        (value, count) => KnownInteger::Runtime(TerminalTargetIntegerExpression::ExactShiftLeft {
            psi_operation,
            count_type,
            value: Box::new(value.into_expression(value_id)),
            count: Box::new(count.into_expression(count_id)),
        }),
    })
}

fn lower_conditional_integer_binary(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    scalar_type: IntegerType,
    left_id: ValueId,
    right_id: ValueId,
    kind: IntegerBinaryKind,
    psi_operation: psi_core::OperationId,
) -> Result<KnownInteger, LoweringError> {
    let operand = |id| match values.get(&id).cloned() {
        Some(KnownScalar::Integer {
            scalar_type: operand_type,
            value,
        }) if operand_type == scalar_type => Ok(value),
        Some(_) => Err(kind.mismatch(result)),
        None => Err(LoweringError::UnknownValue(id)),
    };
    let left = operand(left_id)?;
    let right = operand(right_id)?;
    Ok(match (left, right) {
        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => KnownInteger::Immediate(
            kind.fold(scalar_type, left, right)
                .ok_or(kind.mismatch(result))?,
        ),
        (left, right) => KnownInteger::Runtime(kind.expression(
            psi_operation,
            left.into_expression(left_id),
            right.into_expression(right_id),
        )),
    })
}

impl IntegerBinaryKind {
    fn mismatch(self, result: ValueId) -> LoweringError {
        match self {
            Self::BitwiseAnd | Self::BitwiseOr | Self::BitwiseXor => {
                LoweringError::IntegerBitwiseOperandTypeMismatch(result)
            }
            Self::WrappingAdd => LoweringError::WrappingAddOperandTypeMismatch(result),
            Self::SaturatingAdd => LoweringError::SaturatingAddOperandTypeMismatch(result),
            Self::WrappingSubtract => LoweringError::WrappingSubtractOperandTypeMismatch(result),
            Self::SaturatingSubtract => {
                LoweringError::SaturatingSubtractOperandTypeMismatch(result)
            }
            Self::WrappingMultiply => LoweringError::WrappingMultiplyOperandTypeMismatch(result),
            Self::SaturatingMultiply => {
                LoweringError::SaturatingMultiplyOperandTypeMismatch(result)
            }
            Self::ExactDivide => LoweringError::ExactDivideOperandTypeMismatch(result),
            Self::ExactRemainder => LoweringError::ExactRemainderOperandTypeMismatch(result),
            Self::WrappingDivide => LoweringError::WrappingDivideOperandTypeMismatch(result),
            Self::WrappingRemainder => LoweringError::WrappingRemainderOperandTypeMismatch(result),
            Self::SaturatingDivide => LoweringError::SaturatingDivideOperandTypeMismatch(result),
            Self::SaturatingRemainder => {
                LoweringError::SaturatingRemainderOperandTypeMismatch(result)
            }
        }
    }

    fn fold(
        self,
        scalar_type: IntegerType,
        left: IntegerValue,
        right: IntegerValue,
    ) -> Option<IntegerValue> {
        match self {
            Self::BitwiseAnd => scalar_type.bitwise_and(left, right),
            Self::BitwiseOr => scalar_type.bitwise_or(left, right),
            Self::BitwiseXor => scalar_type.bitwise_xor(left, right),
            Self::WrappingAdd => scalar_type.wrapping_add(left, right),
            Self::SaturatingAdd => scalar_type.saturating_add(left, right),
            Self::WrappingSubtract => scalar_type.wrapping_sub(left, right),
            Self::SaturatingSubtract => scalar_type.saturating_sub(left, right),
            Self::WrappingMultiply => scalar_type.wrapping_mul(left, right),
            Self::SaturatingMultiply => scalar_type.saturating_mul(left, right),
            Self::ExactDivide => scalar_type.exact_div(left, right),
            Self::ExactRemainder => scalar_type.exact_rem(left, right),
            Self::WrappingDivide => scalar_type.wrapping_div(left, right),
            Self::WrappingRemainder => scalar_type.wrapping_rem(left, right),
            Self::SaturatingDivide => scalar_type.saturating_div(left, right),
            Self::SaturatingRemainder => scalar_type.saturating_rem(left, right),
        }
    }

    fn expression(
        self,
        psi_operation: psi_core::OperationId,
        left: TerminalTargetIntegerExpression,
        right: TerminalTargetIntegerExpression,
    ) -> TerminalTargetIntegerExpression {
        let left = Box::new(left);
        let right = Box::new(right);
        match self {
            Self::BitwiseAnd => TerminalTargetIntegerExpression::BitwiseAnd {
                psi_operation,
                left,
                right,
            },
            Self::BitwiseOr => TerminalTargetIntegerExpression::BitwiseOr {
                psi_operation,
                left,
                right,
            },
            Self::BitwiseXor => TerminalTargetIntegerExpression::BitwiseXor {
                psi_operation,
                left,
                right,
            },
            Self::WrappingAdd => TerminalTargetIntegerExpression::WrappingAdd {
                psi_operation,
                left,
                right,
            },
            Self::SaturatingAdd => TerminalTargetIntegerExpression::SaturatingAdd {
                psi_operation,
                left,
                right,
            },
            Self::WrappingSubtract => TerminalTargetIntegerExpression::WrappingSubtract {
                psi_operation,
                left,
                right,
            },
            Self::SaturatingSubtract => TerminalTargetIntegerExpression::SaturatingSubtract {
                psi_operation,
                left,
                right,
            },
            Self::WrappingMultiply => TerminalTargetIntegerExpression::WrappingMultiply {
                psi_operation,
                left,
                right,
            },
            Self::SaturatingMultiply => TerminalTargetIntegerExpression::SaturatingMultiply {
                psi_operation,
                left,
                right,
            },
            Self::ExactDivide => TerminalTargetIntegerExpression::ExactDivide {
                psi_operation,
                left,
                right,
            },
            Self::ExactRemainder => TerminalTargetIntegerExpression::ExactRemainder {
                psi_operation,
                left,
                right,
            },
            Self::WrappingDivide => TerminalTargetIntegerExpression::WrappingDivide {
                psi_operation,
                left,
                right,
            },
            Self::WrappingRemainder => TerminalTargetIntegerExpression::WrappingRemainder {
                psi_operation,
                left,
                right,
            },
            Self::SaturatingDivide => TerminalTargetIntegerExpression::SaturatingDivide {
                psi_operation,
                left,
                right,
            },
            Self::SaturatingRemainder => TerminalTargetIntegerExpression::SaturatingRemainder {
                psi_operation,
                left,
                right,
            },
        }
    }
}

fn scalar_function_result(
    function: &TerminalAbstractFunction,
) -> Result<TerminalAbstractResult, LoweringError> {
    function
        .result
        .scalar()
        .ok_or(LoweringError::FunctionResultKindMismatch(function.machine))
}

fn scalar_shape(
    value: ValueId,
    scalar_type: ScalarType,
    require_native_parameter: bool,
) -> Result<ValueShape, LoweringError> {
    let bytes = match scalar_type {
        ScalarType::Boolean => 1,
        ScalarType::Integer(integer_type) => {
            let bits = integer_type.bits();
            if require_native_parameter && !matches!(bits, 8 | 16 | 32 | 64) {
                return Err(LoweringError::ParameterWidthNotNativelySupported { value, bits });
            }
            bits.div_ceil(8)
        }
    };
    Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}

fn scalar_parameter_location(
    parameter: &TerminalAbstractParameter,
    placement: &ValuePlacement,
) -> Result<TerminalScalarParameterLocation, LoweringError> {
    let expected_bytes = scalar_shape(parameter.value, parameter.scalar_type, true)?.byte_size;
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == expected_bytes => {
            Ok(TerminalScalarParameterLocation::Register(*register))
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == expected_bytes => Ok(TerminalScalarParameterLocation::IncomingStack {
            byte_offset: *stack_byte_offset,
        }),
        _ => Err(LoweringError::UnsupportedScalarParameterPlacement(
            parameter.value,
        )),
    }
}

fn insert_value(
    values: &mut BTreeMap<ValueId, KnownScalar>,
    id: ValueId,
    value: KnownScalar,
) -> Result<(), LoweringError> {
    if values.insert(id, value).is_some() {
        return Err(LoweringError::DuplicateValue(id));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KnownScalar {
    Boolean(bool),
    BooleanRuntime(TerminalTargetBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        value: KnownInteger,
    },
}

impl KnownScalar {
    const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::BooleanRuntime(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(*scalar_type),
        }
    }

    fn rebind_direct_parameter(self, source_value: ValueId) -> Self {
        match self {
            Self::Integer { scalar_type, value } => Self::Integer {
                scalar_type,
                value: value.rebind_direct_parameter(source_value),
            },
            Self::BooleanRuntime(TerminalTargetBooleanExpression::Parameter {
                parameter_index,
                location,
                ..
            }) => Self::BooleanRuntime(TerminalTargetBooleanExpression::Parameter {
                source_value,
                parameter_index,
                location,
            }),
            value @ (Self::Boolean(_) | Self::BooleanRuntime(_)) => value,
        }
    }

    fn into_expression(
        self,
        source_value: ValueId,
    ) -> Result<TerminalTargetScalarExpression, LoweringError> {
        Ok(match self {
            Self::Boolean(value) => TerminalTargetScalarExpression::Boolean(
                TerminalTargetBooleanExpression::Immediate {
                    source_value,
                    value,
                },
            ),
            Self::BooleanRuntime(expression) => TerminalTargetScalarExpression::Boolean(expression),
            Self::Integer { scalar_type, value } => TerminalTargetScalarExpression::Integer {
                scalar_type,
                expression: value.into_expression(source_value),
            },
        })
    }
}

fn negate_boolean(
    value: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
) -> Result<KnownScalar, LoweringError> {
    match value {
        KnownScalar::Boolean(value) => Ok(KnownScalar::Boolean(!value)),
        KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::Not { operand, .. }) => {
            Ok(KnownScalar::BooleanRuntime(*operand))
        }
        KnownScalar::BooleanRuntime(expression) => Ok(KnownScalar::BooleanRuntime(
            TerminalTargetBooleanExpression::Not {
                psi_operation,
                operand: Box::new(expression),
            },
        )),
        KnownScalar::Integer { .. } => Err(LoweringError::ValueTypeMismatch(result)),
    }
}

fn equal_boolean(
    left: KnownScalar,
    right: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
) -> Result<KnownScalar, LoweringError> {
    match (left, right) {
        (KnownScalar::Boolean(left), KnownScalar::Boolean(right)) => {
            Ok(KnownScalar::Boolean(left == right))
        }
        (value, KnownScalar::Boolean(true)) | (KnownScalar::Boolean(true), value) => Ok(value),
        (value, KnownScalar::Boolean(false)) | (KnownScalar::Boolean(false), value) => {
            negate_boolean(value, psi_operation, result)
        }
        (KnownScalar::BooleanRuntime(left), KnownScalar::BooleanRuntime(right)) => Ok(
            KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::Equal {
                psi_operation,
                left: Box::new(left),
                right: Box::new(right),
            }),
        ),
        _ => Err(LoweringError::ValueTypeMismatch(result)),
    }
}

fn equal_integer(
    left_id: ValueId,
    left: KnownScalar,
    right_id: ValueId,
    right: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
) -> Result<KnownScalar, LoweringError> {
    let (
        KnownScalar::Integer {
            scalar_type: left_type,
            value: left,
        },
        KnownScalar::Integer {
            scalar_type: right_type,
            value: right,
        },
    ) = (left, right)
    else {
        return Err(LoweringError::ValueTypeMismatch(result));
    };
    if left_type != right_type {
        return Err(LoweringError::ValueTypeMismatch(result));
    }
    match (left, right) {
        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
            Ok(KnownScalar::Boolean(left == right))
        }
        (left, right) => Ok(KnownScalar::BooleanRuntime(
            TerminalTargetBooleanExpression::IntegerEqual {
                psi_operation,
                scalar_type: left_type,
                left: Box::new(left.into_expression(left_id)),
                right: Box::new(right.into_expression(right_id)),
            },
        )),
    }
}

fn order_integer(
    left_id: ValueId,
    left: KnownScalar,
    right_id: ValueId,
    right: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
    inclusive: bool,
) -> Result<KnownScalar, LoweringError> {
    let (
        KnownScalar::Integer {
            scalar_type: left_type,
            value: left,
        },
        KnownScalar::Integer {
            scalar_type: right_type,
            value: right,
        },
    ) = (left, right)
    else {
        return Err(LoweringError::ValueTypeMismatch(result));
    };
    if left_type != right_type {
        return Err(LoweringError::ValueTypeMismatch(result));
    }
    match (left, right) {
        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
            let ordering = left_type
                .compare(left, right)
                .ok_or(LoweringError::ValueTypeMismatch(result))?;
            Ok(KnownScalar::Boolean(if inclusive {
                !ordering.is_gt()
            } else {
                ordering.is_lt()
            }))
        }
        (left, right) => {
            let left = Box::new(left.into_expression(left_id));
            let right = Box::new(right.into_expression(right_id));
            Ok(KnownScalar::BooleanRuntime(if inclusive {
                TerminalTargetBooleanExpression::IntegerLessOrEqual {
                    psi_operation,
                    scalar_type: left_type,
                    left,
                    right,
                }
            } else {
                TerminalTargetBooleanExpression::IntegerLessThan {
                    psi_operation,
                    scalar_type: left_type,
                    left,
                    right,
                }
            }))
        }
    }
}

fn direct_boolean_condition(
    expression: TerminalTargetBooleanExpression,
    value: ValueId,
) -> Result<(usize, TerminalScalarParameterLocation, bool), LoweringError> {
    match expression {
        TerminalTargetBooleanExpression::Parameter {
            parameter_index,
            location,
            ..
        } => Ok((parameter_index, location, false)),
        TerminalTargetBooleanExpression::Not { operand, .. } => match *operand {
            TerminalTargetBooleanExpression::Parameter {
                parameter_index,
                location,
                ..
            } => Ok((parameter_index, location, true)),
            _ => Err(LoweringError::UnsupportedRuntimeBooleanCondition(value)),
        },
        _ => Err(LoweringError::UnsupportedRuntimeBooleanCondition(value)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KnownInteger {
    Immediate(IntegerValue),
    Runtime(TerminalTargetIntegerExpression),
}

impl KnownInteger {
    fn into_expression(self, source_value: ValueId) -> TerminalTargetIntegerExpression {
        match self {
            Self::Immediate(value) => TerminalTargetIntegerExpression::Immediate {
                source_value,
                value,
            },
            Self::Runtime(expression) => expression,
        }
    }

    fn rebind_direct_parameter(self, source_value: ValueId) -> Self {
        match self {
            Self::Runtime(TerminalTargetIntegerExpression::Parameter {
                parameter_index,
                location,
                ..
            }) => Self::Runtime(TerminalTargetIntegerExpression::Parameter {
                source_value,
                parameter_index,
                location,
            }),
            value => value,
        }
    }
}

fn conditional_provenance(
    function: &TerminalAbstractFunction,
    operations: Vec<psi_core::OperationId>,
    edges: Vec<psi_core::EdgeId>,
) -> TerminalPsiProvenance {
    let mut operations = operations.into_iter().collect::<BTreeSet<_>>();
    let mut edges = edges.into_iter().collect::<BTreeSet<_>>();
    let mut provenance = TerminalPsiProvenance::default();
    for operation in &function.operations {
        let psi_operation = match operation {
            TerminalAbstractOperation::EstablishTrivialAffineLocal { psi_operation, .. }
            | TerminalAbstractOperation::CallUnit { psi_operation, .. }
            | TerminalAbstractOperation::CallStructuralScalar { psi_operation, .. }
            | TerminalAbstractOperation::BoundaryCall { psi_operation, .. }
            | TerminalAbstractOperation::PortWrite { psi_operation, .. }
            | TerminalAbstractOperation::Call { psi_operation, .. }
            | TerminalAbstractOperation::IntegerConstant { psi_operation, .. }
            | TerminalAbstractOperation::BooleanConstant { psi_operation, .. }
            | TerminalAbstractOperation::BooleanStructuralField { psi_operation, .. }
            | TerminalAbstractOperation::BooleanNot { psi_operation, .. }
            | TerminalAbstractOperation::BooleanEqual { psi_operation, .. }
            | TerminalAbstractOperation::IntegerEqual { psi_operation, .. }
            | TerminalAbstractOperation::IntegerLessThan { psi_operation, .. }
            | TerminalAbstractOperation::IntegerLessOrEqual { psi_operation, .. }
            | TerminalAbstractOperation::IntegerBitwiseNot { psi_operation, .. }
            | TerminalAbstractOperation::IntegerWiden { psi_operation, .. }
            | TerminalAbstractOperation::IntegerExactCast { psi_operation, .. }
            | TerminalAbstractOperation::IntegerBitwiseAnd { psi_operation, .. }
            | TerminalAbstractOperation::IntegerBitwiseOr { psi_operation, .. }
            | TerminalAbstractOperation::IntegerBitwiseXor { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerShiftLeft { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerShiftRight { psi_operation, .. }
            | TerminalAbstractOperation::ExactIntegerShiftLeft { psi_operation, .. }
            | TerminalAbstractOperation::ExactIntegerShiftRight { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerAdd { psi_operation, .. }
            | TerminalAbstractOperation::SaturatingIntegerAdd { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerSubtract { psi_operation, .. }
            | TerminalAbstractOperation::SaturatingIntegerSubtract { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerMultiply { psi_operation, .. }
            | TerminalAbstractOperation::SaturatingIntegerMultiply { psi_operation, .. } => {
                Some(*psi_operation)
            }
            TerminalAbstractOperation::ExactIntegerDivide { psi_operation, .. } => {
                Some(*psi_operation)
            }
            TerminalAbstractOperation::ExactIntegerRemainder { psi_operation, .. } => {
                Some(*psi_operation)
            }
            TerminalAbstractOperation::WrappingIntegerDivide { psi_operation, .. } => {
                Some(*psi_operation)
            }
            TerminalAbstractOperation::WrappingIntegerRemainder { psi_operation, .. } => {
                Some(*psi_operation)
            }
            TerminalAbstractOperation::SaturatingIntegerDivide { psi_operation, .. } => {
                Some(*psi_operation)
            }
            TerminalAbstractOperation::SaturatingIntegerRemainder { psi_operation, .. } => {
                Some(*psi_operation)
            }
            TerminalAbstractOperation::Jump { .. }
            | TerminalAbstractOperation::Conditional { .. }
            | TerminalAbstractOperation::Return { .. }
            | TerminalAbstractOperation::ReturnUnit { .. }
            | TerminalAbstractOperation::ReturnStructural { .. }
            | TerminalAbstractOperation::Crash { .. } => None,
        };
        if let Some(psi_operation) = psi_operation
            && operations.remove(&psi_operation)
        {
            provenance.operations.push(psi_operation);
        }
        match operation {
            TerminalAbstractOperation::Jump { psi_edge, .. }
            | TerminalAbstractOperation::Return { psi_edge, .. }
            | TerminalAbstractOperation::ReturnUnit { psi_edge, .. }
            | TerminalAbstractOperation::ReturnStructural { psi_edge, .. }
            | TerminalAbstractOperation::Crash { psi_edge, .. } => {
                if edges.remove(psi_edge) {
                    provenance.edges.push(*psi_edge);
                }
            }
            TerminalAbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for psi_edge in [when_true.psi_edge, when_false.psi_edge] {
                    if edges.remove(&psi_edge) {
                        provenance.edges.push(psi_edge);
                    }
                }
            }
            _ => {}
        }
    }
    debug_assert!(operations.is_empty());
    debug_assert!(edges.is_empty());
    provenance
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    EntryFunctionMissing(MachineId),
    DuplicateBoundarySettlement(BoundaryMachineId),
    UnknownBoundarySettlement(BoundaryMachineId),
    MissingBoundarySettlement(BoundaryMachineId),
    UnusedBoundarySettlement(BoundaryMachineId),
    BoundaryRealizationMismatch(BoundaryMachineId),
    ProviderExecutionBinding(String),
    OperationAfterReturn(MachineId),
    FunctionHasNoReturn(MachineId),
    FunctionResultMismatch(MachineId),
    FunctionResultKindMismatch(MachineId),
    UnitFunctionHasScalarParameters(MachineId),
    UnitFunctionNotStraightLine(MachineId),
    UnitOperationInScalarFunction {
        machine: MachineId,
        operation: OperationId,
    },
    ResultBearingBoundarySettlementRequiresNativeRealization {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    UnsupportedOperationInScalarFunction(MachineId),
    UnsupportedOperationInUnitFunction(MachineId),
    UnsupportedStructuralReturn(MachineId),
    UnsupportedStructuralReturnShape {
        machine: MachineId,
        byte_size: u16,
    },
    UnsupportedStructuralReturnPlacement(MachineId),
    UnitCallTargetKindMismatch(MachineId),
    StructuralCallArgumentCountMismatch {
        callee: MachineId,
        expected: usize,
        actual: usize,
    },
    UnknownStructuralArgumentPlace {
        machine: MachineId,
        place: PlaceId,
    },
    StructuralCallArgumentTypeMismatch {
        callee: MachineId,
        place: PlaceId,
    },
    UnknownStructuralType(StructuralTypeId),
    RecursiveStructuralType(StructuralTypeId),
    EmptyStructuralType(StructuralTypeId),
    RelevantOpaqueStructuralField(StructuralTypeId),
    UnsupportedStructuralSum(StructuralTypeId),
    StructuralTypeTooLarge(StructuralTypeId),
    ConditionalControlFlowRequiresBlockLowering(MachineId),
    ConditionalConditionMustBeBoolean(ValueId),
    ConditionalArmBindingTypeMismatch(psi_core::EdgeId),
    DuplicateValue(ValueId),
    UnknownCallTarget(MachineId),
    CallArgumentCountMismatch {
        callee: MachineId,
        expected: usize,
        actual: usize,
    },
    CallArgumentTypeMismatch {
        callee: MachineId,
        argument: ValueId,
    },
    UnknownValue(ValueId),
    ValueTypeMismatch(ValueId),
    UnsupportedRuntimeBooleanCondition(ValueId),
    IntegerConstantHasNonIntegerType(ValueId),
    IntegerConstantOutsideType(ValueId),
    IntegerBitwiseOperandTypeMismatch(ValueId),
    IntegerWidenTypeMismatch(ValueId),
    IntegerExactCastTypeMismatch(ValueId),
    WrappingShiftOperandTypeMismatch(ValueId),
    ExactShiftOperandTypeMismatch(ValueId),
    WrappingAddOperandTypeMismatch(ValueId),
    SaturatingAddOperandTypeMismatch(ValueId),
    WrappingSubtractOperandTypeMismatch(ValueId),
    SaturatingSubtractOperandTypeMismatch(ValueId),
    WrappingMultiplyOperandTypeMismatch(ValueId),
    SaturatingMultiplyOperandTypeMismatch(ValueId),
    ExactDivideOperandTypeMismatch(ValueId),
    ExactRemainderOperandTypeMismatch(ValueId),
    WrappingDivideOperandTypeMismatch(ValueId),
    WrappingRemainderOperandTypeMismatch(ValueId),
    SaturatingDivideOperandTypeMismatch(ValueId),
    SaturatingRemainderOperandTypeMismatch(ValueId),
    ParameterWidthNotNativelySupported {
        value: ValueId,
        bits: u16,
    },
    UnsupportedScalarParameterPlacement(ValueId),
    AbiPlan(PlanDiagnostic),
    AbiParameterCountMismatch {
        expected: usize,
        actual: usize,
    },
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_terminal_abstract_operations::{
        TerminalAbstractFunction, TerminalAbstractOperation, TerminalAbstractOperationPlan,
        TerminalAbstractParameter, TerminalAbstractResult, TerminalAbstractSuccessor,
        TerminalValueBinding,
    };
    use omega_terminal_target_operations::MachineRegister;
    use psi_core::{BlockId, EdgeId, PlaceId, StructuralFieldId, StructuralTypeId};
    use psi_terminal::{
        BoundaryMachineDeclaration, SemanticFingerprint, StructuralFieldDeclaration,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
        TerminalPsiIdentity, VocabularyMarker,
    };

    fn bounded_boolean_cleanup_plan() -> TerminalAbstractOperationPlan {
        let caller = MachineId::new(40).unwrap();
        let cleanup = MachineId::new(41).unwrap();
        let helper = MachineId::new(42).unwrap();
        let token_type = StructuralTypeId::new(40).unwrap();
        let plain_type = StructuralTypeId::new(41).unwrap();
        let helper_type = StructuralTypeId::new(42).unwrap();
        let token = PlaceId::new(40).unwrap();
        let plain = PlaceId::new(41).unwrap();
        let left = ValueId::new(40).unwrap();
        let right = ValueId::new(41).unwrap();
        let false_value = ValueId::new(42).unwrap();
        let true_value = ValueId::new(43).unwrap();
        let second_false_value = ValueId::new(44).unwrap();
        let result = ValueId::new(45).unwrap();
        let cleanup_actions = vec![
            TerminalAffineCleanupAction::DiscardRoot(plain),
            TerminalAffineCleanupAction::InvokeNominal(psi_terminal::NominalAffineCleanup {
                place: token,
                structural_type: token_type,
                cleanup_machine: cleanup,
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            }),
        ];
        let leaf_return = |edge, value| TerminalAbstractOperation::Return {
            psi_edge: EdgeId::new(edge).unwrap(),
            result,
            value,
            scalar_type: ScalarType::Boolean,
            cleanup_actions: cleanup_actions.clone(),
        };
        let block_entry = |block, operation_offset| {
            omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                block: BlockId::new(block).unwrap(),
                operation_offset,
            }
        };
        let return_unit = |edge| TerminalAbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(edge).unwrap(),
            cleanup_actions: Vec::new(),
        };
        let unit_function = |machine, attachment, operations| TerminalAbstractFunction {
            machine,
            attachment,
            entry: BlockId::new(machine.get()).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalAbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![block_entry(machine.get(), 0)],
            operations,
        };
        TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: caller,
            structural_types: vec![
                StructuralTypeDeclaration {
                    id: token_type,
                    identity: "Token".into(),
                    shape: StructuralTypeShape::Record { fields: Vec::new() },
                },
                StructuralTypeDeclaration {
                    id: plain_type,
                    identity: "Plain".into(),
                    shape: StructuralTypeShape::Record { fields: Vec::new() },
                },
                StructuralTypeDeclaration {
                    id: helper_type,
                    identity: "Helper".into(),
                    shape: StructuralTypeShape::Record { fields: Vec::new() },
                },
            ],
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                TerminalAbstractFunction {
                    machine: caller,
                    attachment: None,
                    entry: BlockId::new(1).unwrap(),
                    parameters: vec![
                        TerminalAbstractParameter {
                            value: left,
                            scalar_type: ScalarType::Boolean,
                        },
                        TerminalAbstractParameter {
                            value: right,
                            scalar_type: ScalarType::Boolean,
                        },
                    ],
                    structural_parameters: vec![
                        StructuralParameterDeclaration {
                            place: token,
                            position: 0,
                            is_self: false,
                            structural_type: token_type,
                            multiplicity: StructuralMultiplicity::Affine,
                            qualifications: Vec::new(),
                        },
                        StructuralParameterDeclaration {
                            place: plain,
                            position: 1,
                            is_self: false,
                            structural_type: plain_type,
                            multiplicity: StructuralMultiplicity::Affine,
                            qualifications: Vec::new(),
                        },
                    ],
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Boolean,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        block_entry(1, 0),
                        block_entry(2, 1),
                        block_entry(3, 2),
                        block_entry(4, 4),
                        block_entry(5, 6),
                    ],
                    operations: vec![
                        TerminalAbstractOperation::Conditional {
                            condition: left,
                            when_true: TerminalAbstractSuccessor {
                                psi_edge: EdgeId::new(1).unwrap(),
                                target: BlockId::new(2).unwrap(),
                                bindings: Vec::new(),
                            },
                            when_false: TerminalAbstractSuccessor {
                                psi_edge: EdgeId::new(2).unwrap(),
                                target: BlockId::new(3).unwrap(),
                                bindings: Vec::new(),
                            },
                        },
                        TerminalAbstractOperation::Conditional {
                            condition: right,
                            when_true: TerminalAbstractSuccessor {
                                psi_edge: EdgeId::new(3).unwrap(),
                                target: BlockId::new(4).unwrap(),
                                bindings: Vec::new(),
                            },
                            when_false: TerminalAbstractSuccessor {
                                psi_edge: EdgeId::new(4).unwrap(),
                                target: BlockId::new(5).unwrap(),
                                bindings: Vec::new(),
                            },
                        },
                        TerminalAbstractOperation::BooleanConstant {
                            psi_operation: OperationId::new(40).unwrap(),
                            result: false_value,
                            value: false,
                        },
                        leaf_return(5, false_value),
                        TerminalAbstractOperation::BooleanConstant {
                            psi_operation: OperationId::new(41).unwrap(),
                            result: true_value,
                            value: true,
                        },
                        leaf_return(6, true_value),
                        TerminalAbstractOperation::BooleanConstant {
                            psi_operation: OperationId::new(42).unwrap(),
                            result: second_false_value,
                            value: false,
                        },
                        leaf_return(7, second_false_value),
                    ],
                },
                unit_function(
                    cleanup,
                    Some(token_type),
                    vec![
                        TerminalAbstractOperation::CallUnit {
                            psi_operation: OperationId::new(43).unwrap(),
                            callee: helper,
                            structural_arguments: Vec::new(),
                            claim_transfers: Vec::new(),
                        },
                        return_unit(8),
                    ],
                ),
                unit_function(helper, Some(helper_type), vec![return_unit(9)]),
            ],
        }
    }

    #[test]
    fn bounded_boolean_control_retains_one_uniform_mixed_cleanup_frontier() {
        let plan = bounded_boolean_cleanup_plan();
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::uefi_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let lowered = lower_to_target_operations(&plan, target)
                .expect("bounded Boolean control and mixed cleanup lower");
            let TerminalTargetOperation::BooleanControlWithCleanup {
                control,
                structural_parameters,
                cleanup_actions,
                ..
            } = &lowered.functions[0].operation
            else {
                panic!("bounded Boolean cleanup retains its target carrier")
            };
            assert_eq!(structural_parameters.len(), 2);
            assert!(matches!(
                cleanup_actions.as_slice(),
                [
                    TerminalAffineCleanupAction::DiscardRoot(discarded),
                    TerminalAffineCleanupAction::InvokeNominal(cleanup),
                ] if *discarded == PlaceId::new(41).unwrap()
                    && cleanup.place == PlaceId::new(40).unwrap()
                    && cleanup.cleanup_machine == MachineId::new(41).unwrap()
                    && cleanup.cleanup_receiver.is_none()
                    && cleanup.requirement_obligations.is_empty()
            ));
            let TerminalTargetBooleanControl::Conditional {
                when_true,
                when_false,
                ..
            } = control
            else {
                panic!("outer runtime input remains the root decision")
            };
            let TerminalTargetBooleanControl::Conditional {
                when_true: nested_true,
                when_false: nested_false,
                ..
            } = when_true.control.as_ref()
            else {
                panic!("true arm retains the second decision")
            };
            let leaf_edge = |control: &TerminalTargetBooleanControl| match control {
                TerminalTargetBooleanControl::ReturnImmediate {
                    psi_return_edge, ..
                } => *psi_return_edge,
                _ => panic!("bounded decision leaf returns one immediate Boolean"),
            };
            assert_eq!(
                [
                    leaf_edge(&nested_true.control),
                    leaf_edge(&nested_false.control),
                    leaf_edge(&when_false.control),
                ],
                [
                    EdgeId::new(6).unwrap(),
                    EdgeId::new(7).unwrap(),
                    EdgeId::new(5).unwrap(),
                ],
            );
        }
    }

    #[test]
    fn bounded_boolean_cleanup_rejects_nonuniform_or_hidden_frontiers() {
        let mut plan = bounded_boolean_cleanup_plan();
        let TerminalAbstractOperation::Return {
            cleanup_actions, ..
        } = &mut plan.functions[0].operations[3]
        else {
            unreachable!("first leaf returns")
        };
        cleanup_actions.clear();
        assert!(matches!(
            lower_to_target_operations(&plan, NativeTarget::linux_x64()),
            Err(LoweringError::UnsupportedOperationInScalarFunction(_))
        ));

        let mut ordinary = constant_conditional_plan(false);
        let place = PlaceId::new(90).unwrap();
        let TerminalAbstractOperation::Return {
            cleanup_actions, ..
        } = &mut ordinary.functions[0].operations[3]
        else {
            unreachable!("constant fixture true arm returns")
        };
        cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(place));
        assert!(matches!(
            lower_to_target_operations(&ordinary, NativeTarget::linux_x64()),
            Err(LoweringError::UnsupportedOperationInScalarFunction(_))
        ));
    }

    #[test]
    fn two_nominal_cleanups_admit_zero_one_distinct_or_shared_bounded_executable_bodies() {
        let caller = MachineId::new(1).unwrap();
        let executable_cleanup = MachineId::new(2).unwrap();
        let empty_cleanup = MachineId::new(3).unwrap();
        let helper = MachineId::new(4).unwrap();
        let receiver_type = StructuralTypeId::new(1).unwrap();
        let helper_type = StructuralTypeId::new(2).unwrap();
        let first_place = PlaceId::new(1).unwrap();
        let second_place = PlaceId::new(2).unwrap();
        let block = |machine: MachineId| BlockId::new(machine.get()).unwrap();
        let return_unit = |edge| TerminalAbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(edge).unwrap(),
            cleanup_actions: Vec::new(),
        };
        let unit_function = |machine, attachment, operations| TerminalAbstractFunction {
            machine,
            attachment,
            entry: block(machine),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalAbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![
                omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                    block: block(machine),
                    operation_offset: 0,
                },
            ],
            operations,
        };
        let cleanup = |place, cleanup_machine| psi_terminal::NominalAffineCleanup {
            place,
            structural_type: receiver_type,
            cleanup_machine,
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        };
        let caller_parameters = [first_place, second_place]
            .into_iter()
            .enumerate()
            .map(|(position, place)| StructuralParameterDeclaration {
                place,
                position: u32::try_from(position).unwrap(),
                is_self: false,
                structural_type: receiver_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            })
            .collect::<Vec<_>>();
        let executable_call = TerminalAbstractOperation::CallUnit {
            psi_operation: OperationId::new(1).unwrap(),
            callee: helper,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
        };
        let mut plan = TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: caller,
            structural_types: vec![
                StructuralTypeDeclaration {
                    id: receiver_type,
                    identity: "Receiver".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "value".into(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
                            )),
                        }],
                    },
                },
                StructuralTypeDeclaration {
                    id: helper_type,
                    identity: "Helper".into(),
                    shape: StructuralTypeShape::Record { fields: Vec::new() },
                },
            ],
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                TerminalAbstractFunction {
                    structural_parameters: caller_parameters,
                    ..unit_function(
                        caller,
                        None,
                        vec![TerminalAbstractOperation::ReturnUnit {
                            psi_edge: EdgeId::new(1).unwrap(),
                            cleanup_actions: vec![
                                TerminalAffineCleanupAction::InvokeNominal(cleanup(
                                    second_place,
                                    executable_cleanup,
                                )),
                                TerminalAffineCleanupAction::InvokeNominal(cleanup(
                                    first_place,
                                    empty_cleanup,
                                )),
                            ],
                        }],
                    )
                },
                unit_function(
                    executable_cleanup,
                    Some(receiver_type),
                    vec![executable_call.clone(), return_unit(2)],
                ),
                unit_function(empty_cleanup, Some(receiver_type), vec![return_unit(3)]),
                unit_function(helper, Some(helper_type), vec![return_unit(4)]),
            ],
        };

        lower_to_target_operations(&plan, NativeTarget::linux_x64())
            .expect("one executable and one empty cleanup lower");

        plan.functions[1].operations.remove(0);
        lower_to_target_operations(&plan, NativeTarget::linux_x64())
            .expect("two empty cleanup bodies remain accepted");

        plan.functions[1]
            .operations
            .insert(0, executable_call.clone());
        plan.functions[2].operations.insert(
            0,
            TerminalAbstractOperation::CallUnit {
                psi_operation: OperationId::new(2).unwrap(),
                callee: helper,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
            },
        );
        lower_to_target_operations(&plan, NativeTarget::linux_x64())
            .expect("two distinct executable cleanup bodies lower");

        let TerminalAbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } = &mut plan.functions[0].operations[0]
        else {
            unreachable!("caller remains a direct return")
        };
        let TerminalAffineCleanupAction::InvokeNominal(second) = &mut cleanup_actions[1] else {
            unreachable!("second action remains nominal")
        };
        second.cleanup_machine = executable_cleanup;
        let scalar_cleanup_actions = cleanup_actions.clone();
        lower_to_target_operations(&plan, NativeTarget::linux_x64())
            .expect("two actions sharing one executable cleanup body lower");

        let scalar_value = ValueId::new(1).unwrap();
        let scalar_result = ValueId::new(2).unwrap();
        plan.functions[0].result = TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
            value: scalar_result,
            scalar_type: ScalarType::Boolean,
        });
        plan.functions[0].operations = vec![
            TerminalAbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(3).unwrap(),
                result: scalar_value,
                value: true,
            },
            TerminalAbstractOperation::Return {
                psi_edge: EdgeId::new(1).unwrap(),
                result: scalar_result,
                value: scalar_value,
                scalar_type: ScalarType::Boolean,
                cleanup_actions: scalar_cleanup_actions.clone(),
            },
        ];
        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64())
            .expect("scalar result composes the same ordered cleanup frontier");
        assert!(matches!(
            &lowered.functions[0].operation,
            TerminalTargetOperation::ScalarReturnWithCleanup {
                scalar,
                structural_parameters,
                cleanup_actions: lowered_actions,
                ..
            } if matches!(scalar.as_ref(), TerminalTargetOperation::ReturnBooleanImmediate {
                value: true,
                ..
            })
                && structural_parameters.len() == 2
                && lowered_actions == &scalar_cleanup_actions
        ));
    }

    #[test]
    fn refuses_a_return_whose_value_was_never_materialized() {
        let machine = MachineId::new(1).expect("machine");
        let unknown = ValueId::new(1).expect("unknown value");
        let result = ValueId::new(2).expect("result");
        let i32_type = IntegerType::new(psi_core::IntegerSign::Signed, 32).expect("i32");
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![TerminalAbstractFunction {
                machine,
                attachment: None,
                entry: BlockId::new(1).expect("block"),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: result,
                    scalar_type: ScalarType::Integer(i32_type),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![TerminalAbstractOperation::Return {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    result,
                    value: unknown,
                    scalar_type: ScalarType::Integer(i32_type),
                    cleanup_actions: Vec::new(),
                }],
            }],
        };

        assert_eq!(
            lower_to_target_operations(&plan, NativeTarget::linux_x64()),
            Err(LoweringError::UnknownValue(unknown))
        );
    }

    #[test]
    fn unit_fixed_array_call_selects_exact_forty_byte_native_placements() {
        let root = MachineId::new(1).unwrap();
        let callee = MachineId::new(2).unwrap();
        let element_type = StructuralTypeId::new(1).unwrap();
        let structural_type = StructuralTypeId::new(2).unwrap();
        let root_place = PlaceId::new(1).unwrap();
        let callee_place = PlaceId::new(2).unwrap();
        let u64_type =
            ScalarType::Integer(IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap());
        let structural_types = vec![
            StructuralTypeDeclaration {
                id: element_type,
                identity: "Acknowledgement".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "value".into(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(u64_type),
                        },
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(2).unwrap(),
                            identity: "proof".into(),
                            relevance: psi_terminal::BindingRelevance::Erased,
                            field_type: StructuralFieldType::Erased {
                                type_identity: "named(name(example::Evidence))".into(),
                            },
                        },
                    ],
                },
            },
            StructuralTypeDeclaration {
                id: structural_type,
                identity: "[Acknowledgement; 5]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: element_type,
                    length: 5,
                },
            },
        ];
        let parameter = |place| StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: Vec::new(),
        };
        let unit_function = |machine, place, operations| TerminalAbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(machine.get()).unwrap(),
            parameters: Vec::new(),
            structural_parameters: vec![parameter(place)],
            result: TerminalAbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![
                omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                    block: BlockId::new(machine.get()).unwrap(),
                    operation_offset: 0,
                },
            ],
            operations,
        };
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: root,
            structural_types,
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                unit_function(
                    root,
                    root_place,
                    vec![
                        TerminalAbstractOperation::CallUnit {
                            psi_operation: OperationId::new(1).unwrap(),
                            callee,
                            structural_arguments: vec![psi_terminal::StructuralArgument {
                                place: root_place,
                                path: Vec::new(),
                            }],
                            claim_transfers: Vec::new(),
                        },
                        TerminalAbstractOperation::ReturnUnit {
                            psi_edge: EdgeId::new(1).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                ),
                unit_function(
                    callee,
                    callee_place,
                    vec![TerminalAbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(2).unwrap(),
                        cleanup_actions: Vec::new(),
                    }],
                ),
            ],
        };

        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::uefi_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let lowered = lower_to_target_operations(&plan, target).unwrap();
            let TerminalTargetOperation::UnitBody(root) = &lowered.functions[0].operation else {
                panic!("root must remain Unit")
            };
            assert_eq!(root.parameters[0].shape, ValueShape::integer(40, 8));
            let TerminalTargetUnitOperation::Call { arguments, .. } = &root.operations[0] else {
                panic!("root must call helper")
            };
            assert!(arguments[0].path.is_empty());
            assert_eq!(arguments[0].shape, ValueShape::integer(40, 8));
        }

        let linux = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        let TerminalTargetOperation::UnitBody(linux_root) = &linux.functions[0].operation else {
            panic!("root must remain Unit")
        };
        assert_eq!(linux_root.parameters[0].shape, ValueShape::integer(40, 8));
        assert_eq!(linux_root.parameters[0].placement.locations.len(), 5);
        assert!(
            linux_root.parameters[0]
                .placement
                .locations
                .iter()
                .enumerate()
                .all(|(index, location)| matches!(
                    location,
                    ValueLocation::Stack {
                        stack_byte_offset,
                        value_byte_offset,
                        byte_size: 8,
                        alignment: 8,
                    } if *stack_byte_offset == index as u32 * 8
                        && *value_byte_offset == index as u16 * 8
                ))
        );
        let TerminalTargetUnitOperation::Call { arguments, .. } = &linux_root.operations[0] else {
            panic!("root must call helper")
        };
        assert_eq!(arguments[0].source, arguments[0].destination);

        let windows = lower_to_target_operations(&plan, NativeTarget::windows_x64()).unwrap();
        let TerminalTargetOperation::UnitBody(windows_root) = &windows.functions[0].operation
        else {
            panic!("root must remain Unit")
        };
        assert!(matches!(
            windows_root.parameters[0].placement.locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: omega_calling_conventions::IndirectPointerLocation::Register(
                    MachineRegister::X86Rcx
                ),
                byte_size: 40,
                alignment: 8,
                ..
            }]
        ));
        let TerminalTargetUnitOperation::Call { arguments, .. } = &windows_root.operations[0]
        else {
            panic!("root must call helper")
        };
        assert_eq!(arguments[0].source, arguments[0].destination);
    }

    #[test]
    fn fixed_array_layout_repeats_padded_nested_elements_and_rejects_overflow() {
        let element_type = StructuralTypeId::new(1).unwrap();
        let inner_array_type = StructuralTypeId::new(2).unwrap();
        let outer_array_type = StructuralTypeId::new(3).unwrap();
        let oversized_array_type = StructuralTypeId::new(4).unwrap();
        let u64_type =
            ScalarType::Integer(IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap());
        let declarations = vec![
            StructuralTypeDeclaration {
                id: element_type,
                identity: "PaddedElement".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "tag".into(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                        },
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(2).unwrap(),
                            identity: "value".into(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(u64_type),
                        },
                    ],
                },
            },
            StructuralTypeDeclaration {
                id: inner_array_type,
                identity: "[PaddedElement; 2]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: element_type,
                    length: 2,
                },
            },
            StructuralTypeDeclaration {
                id: outer_array_type,
                identity: "[[PaddedElement; 2]; 3]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: inner_array_type,
                    length: 3,
                },
            },
            StructuralTypeDeclaration {
                id: oversized_array_type,
                identity: "[PaddedElement; 4096]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: element_type,
                    length: 4096,
                },
            },
        ];
        let declarations = declarations
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect::<BTreeMap<_, _>>();

        let shape = structural_shape(
            outer_array_type,
            &declarations,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(shape, ValueShape::integer(96, 8));
        assert_eq!(
            structural_shape(
                oversized_array_type,
                &declarations,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            ),
            Err(LoweringError::StructuralTypeTooLarge(oversized_array_type))
        );
    }

    #[test]
    fn metadata_only_boundary_requires_the_exact_preceding_port_realization() {
        use omega_terminal_target_operations::{
            TerminalMetadataOnlyPortRealization, TerminalProviderExecutionBinding,
            TerminalProviderPlanIdentity,
        };

        let machine = MachineId::new(1).unwrap();
        let boundary = BoundaryMachineId::new(1).unwrap();
        let port_operation = OperationId::new(1).unwrap();
        let settlement_operation = OperationId::new(2).unwrap();
        let service = psi_core::ServiceId::new(1).unwrap();
        let element_type = StructuralTypeId::new(1).unwrap();
        let array_type = StructuralTypeId::new(2).unwrap();
        let argument_place = PlaceId::new(1).unwrap();
        let boundary_place = PlaceId::new(2).unwrap();
        let u64_type =
            ScalarType::Integer(IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap());
        let provider_execution = TerminalProviderExecutionBinding::from_execution_record(
            TerminalProviderPlanIdentity::new(7).unwrap(),
            8,
            9,
            10,
            11,
        )
        .unwrap();
        let realization = TerminalMetadataOnlyPortRealization {
            effect_operation: port_operation,
            service,
            port: 0x20,
            value: 0x20,
        };
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: machine,
            structural_types: vec![
                StructuralTypeDeclaration {
                    id: element_type,
                    identity: "Acknowledgement".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "value".into(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(u64_type),
                        }],
                    },
                },
                StructuralTypeDeclaration {
                    id: array_type,
                    identity: "[Acknowledgement; 2]".into(),
                    shape: StructuralTypeShape::FixedArray {
                        element: element_type,
                        length: 2,
                    },
                },
            ],
            boundary_machines: vec![BoundaryMachineDeclaration {
                id: boundary,
                identity: "InterruptAcknowledgement::complete".into(),
                attachment: None,
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: boundary_place,
                    position: 0,
                    is_self: false,
                    structural_type: element_type,
                    multiplicity: StructuralMultiplicity::Linear,
                    qualifications: Vec::new(),
                }],
                result: None,
                requires: Vec::new(),
                published_service_ceiling: vec![service],
            }],
            provider_candidates: Vec::new(),
            functions: vec![TerminalAbstractFunction {
                machine,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: argument_place,
                    position: 0,
                    is_self: false,
                    structural_type: array_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                }],
                result: TerminalAbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: vec![service],
                block_entries: vec![
                    omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                        block: BlockId::new(1).unwrap(),
                        operation_offset: 0,
                    },
                ],
                operations: vec![
                    TerminalAbstractOperation::PortWrite {
                        psi_operation: port_operation,
                        service,
                        port: 0x20,
                        value: 0x20,
                    },
                    TerminalAbstractOperation::BoundaryCall {
                        psi_operation: settlement_operation,
                        result: None,
                        boundary,
                        structural_arguments: vec![psi_terminal::StructuralArgument {
                            place: argument_place,
                            path: vec![StructuralPathSegment::FixedIndex(1)],
                        }],
                        completion_receipts: Vec::new(),
                    },
                    TerminalAbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(1).unwrap(),
                        cleanup_actions: vec![
                            psi_terminal::TerminalAffineCleanupAction::DiscardRoot(argument_place),
                        ],
                    },
                ],
            }],
        };
        let binding = TerminalBoundarySettlementBinding {
            boundary,
            provider_execution,
            realization: realization.into(),
        };
        let lowered = lower_to_target_operations_with_settlements(
            &plan,
            NativeTarget::linux_x64(),
            &[binding],
        )
        .expect("exact effect evidence");
        let TerminalTargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
            panic!("Unit body")
        };
        let TerminalTargetUnitOperation::BoundarySettlement {
            provider_execution: actual,
            realization: actual_realization,
            arguments,
            ..
        } = &body.operations[1]
        else {
            panic!("boundary settlement")
        };
        assert_eq!(*actual, provider_execution);
        assert_eq!(*actual_realization, realization);
        assert_eq!(
            arguments,
            &[psi_terminal::StructuralArgument {
                place: argument_place,
                path: vec![StructuralPathSegment::FixedIndex(1)],
            }]
        );

        let wrong = TerminalBoundarySettlementBinding {
            realization: TerminalMetadataOnlyPortRealization {
                value: 0x21,
                ..realization
            }
            .into(),
            ..binding
        };
        assert_eq!(
            lower_to_target_operations_with_settlements(&plan, NativeTarget::linux_x64(), &[wrong],),
            Err(LoweringError::BoundaryRealizationMismatch(boundary))
        );

        let mut result_bearing = plan.clone();
        let result = TerminalAbstractResult {
            value: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Boolean,
        };
        result_bearing.boundary_machines[0].result = Some(result.scalar_type);
        let TerminalAbstractOperation::BoundaryCall {
            result: operation_result,
            ..
        } = &mut result_bearing.functions[0].operations[1]
        else {
            unreachable!("fixture contains a boundary call")
        };
        *operation_result = Some(result);
        assert_eq!(
            lower_to_target_operations_with_settlements(
                &result_bearing,
                NativeTarget::linux_x64(),
                &[binding],
            ),
            Err(
                LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                    machine,
                    operation: settlement_operation,
                    boundary,
                }
            )
        );
    }

    #[test]
    fn selects_native_register_and_stack_locations_for_runtime_parameters() {
        let register_cases = [
            (
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ),
            (
                NativeTarget::windows_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rcx),
            ),
            (
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ),
        ];
        for (target, expected) in register_cases {
            let lowered = lower_to_target_operations(&parameter_return_plan(1), target).unwrap();
            assert!(matches!(
                lowered.functions[0].operation,
                TerminalTargetOperation::ReturnIntegerParameter {
                    parameter_index: 0,
                    location,
                    ..
                } if location == expected
            ));
        }

        let stack_cases = [
            (
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
            ),
            (
                NativeTarget::windows_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 64 },
            ),
            (
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
            ),
        ];
        for (target, expected) in stack_cases {
            let lowered = lower_to_target_operations(&parameter_return_plan(9), target).unwrap();
            assert!(matches!(
                lowered.functions[0].operation,
                TerminalTargetOperation::ReturnIntegerParameter {
                    parameter_index: 8,
                    location,
                    ..
                } if location == expected
            ));
        }
    }

    #[test]
    fn direct_calls_retain_stack_locations_from_the_callee_call_plan() {
        let stack_cases = [
            (
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
            ),
            (
                NativeTarget::windows_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 64 },
            ),
            (
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
            ),
        ];
        for (target, expected) in stack_cases {
            let lowered = lower_to_target_operations(&direct_call_plan(9), target).unwrap();
            let TerminalTargetOperation::ReturnIntegerExpression { expression, .. } =
                &lowered.functions[0].operation
            else {
                panic!("caller must return its call result")
            };
            let TerminalTargetIntegerExpression::Call { arguments, .. } = expression else {
                panic!("caller result must remain a direct call")
            };
            assert_eq!(arguments[8].location, expected);
        }
    }

    #[test]
    fn lowers_runtime_parameter_arithmetic_to_a_typed_target_expression() {
        let mut plan = parameter_return_plan(2);
        let function = &mut plan.functions[0];
        let sum = ValueId::new(50).expect("sum");
        let scalar_type = match scalar_result(function).scalar_type {
            ScalarType::Integer(integer) => integer,
            ScalarType::Boolean => unreachable!("fixture is integer"),
        };
        function.operations.insert(
            0,
            TerminalAbstractOperation::WrappingIntegerAdd {
                psi_operation: psi_core::OperationId::new(50).expect("operation"),
                result: sum,
                scalar_type,
                left: function.parameters[0].value,
                right: function.parameters[1].value,
            },
        );
        let TerminalAbstractOperation::Return { value, .. } = &mut function.operations[1] else {
            unreachable!("fixture ends in return")
        };
        *value = sum;

        let lowered = lower_to_target_operations(&plan, NativeTarget::host()).unwrap();
        assert!(matches!(
            &lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerExpression {
                source_value,
                scalar_type: result_type,
                expression: TerminalTargetIntegerExpression::WrappingAdd {
                    psi_operation,
                    left,
                    right,
                },
                ..
            } if *source_value == sum
                && *result_type == scalar_type
                && *psi_operation == psi_core::OperationId::new(50).expect("operation")
                && matches!(
                    left.as_ref(),
                    TerminalTargetIntegerExpression::Parameter {
                        parameter_index: 0,
                        ..
                    }
                )
                && matches!(
                    right.as_ref(),
                    TerminalTargetIntegerExpression::Parameter {
                        parameter_index: 1,
                        ..
                    }
                )
        ));
    }

    #[test]
    fn folds_closed_wrapping_subtraction_at_the_declared_width() {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        let left = ValueId::new(50).expect("left");
        let right = ValueId::new(51).expect("right");
        let difference = ValueId::new(52).expect("difference");
        let scalar_type = match scalar_result(function).scalar_type {
            ScalarType::Integer(integer) => integer,
            ScalarType::Boolean => unreachable!("fixture is integer"),
        };
        function.operations.splice(
            0..0,
            [
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                    result: left,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(5),
                },
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                    result: right,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(10),
                },
                TerminalAbstractOperation::WrappingIntegerSubtract {
                    psi_operation: psi_core::OperationId::new(52).expect("subtract operation"),
                    result: difference,
                    scalar_type,
                    left,
                    right,
                },
            ],
        );
        let TerminalAbstractOperation::Return { value, .. } =
            function.operations.last_mut().expect("return")
        else {
            unreachable!("fixture ends in return")
        };
        *value = difference;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerImmediate {
                source_value,
                scalar_type: result_type,
                value: IntegerValue::Unsigned(251),
                ..
            } if source_value == difference && result_type == scalar_type
        ));
    }

    #[test]
    fn folds_closed_saturating_subtraction_at_zero() {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        let left = ValueId::new(50).expect("left");
        let right = ValueId::new(51).expect("right");
        let difference = ValueId::new(52).expect("difference");
        let scalar_type = match scalar_result(function).scalar_type {
            ScalarType::Integer(integer) => integer,
            ScalarType::Boolean => unreachable!("fixture is integer"),
        };
        function.operations.splice(
            0..0,
            [
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                    result: left,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(5),
                },
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                    result: right,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(10),
                },
                TerminalAbstractOperation::SaturatingIntegerSubtract {
                    psi_operation: psi_core::OperationId::new(52).expect("subtract operation"),
                    result: difference,
                    scalar_type,
                    left,
                    right,
                },
            ],
        );
        let TerminalAbstractOperation::Return { value, .. } =
            function.operations.last_mut().expect("return")
        else {
            unreachable!("fixture ends in return")
        };
        *value = difference;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerImmediate {
                source_value,
                scalar_type: result_type,
                value: IntegerValue::Unsigned(0),
                ..
            } if source_value == difference && result_type == scalar_type
        ));
    }

    #[test]
    fn folds_closed_wrapping_multiplication_at_the_declared_width() {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        let left = ValueId::new(50).expect("left");
        let right = ValueId::new(51).expect("right");
        let product = ValueId::new(52).expect("product");
        let scalar_type = match scalar_result(function).scalar_type {
            ScalarType::Integer(integer) => integer,
            ScalarType::Boolean => unreachable!("fixture is integer"),
        };
        function.operations.splice(
            0..0,
            [
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                    result: left,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(20),
                },
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                    result: right,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(13),
                },
                TerminalAbstractOperation::WrappingIntegerMultiply {
                    psi_operation: psi_core::OperationId::new(52).expect("multiply operation"),
                    result: product,
                    scalar_type,
                    left,
                    right,
                },
            ],
        );
        let TerminalAbstractOperation::Return { value, .. } =
            function.operations.last_mut().expect("return")
        else {
            unreachable!("fixture ends in return")
        };
        *value = product;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerImmediate {
                source_value,
                scalar_type: result_type,
                value: IntegerValue::Unsigned(4),
                ..
            } if source_value == product && result_type == scalar_type
        ));
    }

    #[test]
    fn folds_closed_saturating_multiplication_at_the_declared_width() {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        let left = ValueId::new(50).expect("left");
        let right = ValueId::new(51).expect("right");
        let product = ValueId::new(52).expect("product");
        let scalar_type = match scalar_result(function).scalar_type {
            ScalarType::Integer(integer) => integer,
            ScalarType::Boolean => unreachable!("fixture is integer"),
        };
        function.operations.splice(
            0..0,
            [
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                    result: left,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(20),
                },
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                    result: right,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(13),
                },
                TerminalAbstractOperation::SaturatingIntegerMultiply {
                    psi_operation: psi_core::OperationId::new(52).expect("multiply operation"),
                    result: product,
                    scalar_type,
                    left,
                    right,
                },
            ],
        );
        let TerminalAbstractOperation::Return { value, .. } =
            function.operations.last_mut().expect("return")
        else {
            unreachable!("fixture ends in return")
        };
        *value = product;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerImmediate {
                source_value,
                scalar_type: result_type,
                value: IntegerValue::Unsigned(255),
                ..
            } if source_value == product && result_type == scalar_type
        ));
    }

    #[test]
    fn lowers_a_boolean_runtime_parameter_with_its_selected_abi_location() {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        function.parameters[0].scalar_type = ScalarType::Boolean;
        scalar_result_mut(function).scalar_type = ScalarType::Boolean;
        let TerminalAbstractOperation::Return { scalar_type, .. } = &mut function.operations[0]
        else {
            unreachable!("fixture ends in return")
        };
        *scalar_type = ScalarType::Boolean;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanParameter {
                parameter_index: 0,
                location: TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ..
            }
        ));
    }

    #[test]
    fn lowers_runtime_boolean_equality_to_a_target_expression() {
        let mut plan = parameter_return_plan(2);
        let function = &mut plan.functions[0];
        for parameter in &mut function.parameters {
            parameter.scalar_type = ScalarType::Boolean;
        }
        scalar_result_mut(function).scalar_type = ScalarType::Boolean;
        let result = ValueId::new(50).expect("equality result");
        function.operations.insert(
            0,
            TerminalAbstractOperation::BooleanEqual {
                psi_operation: OperationId::new(50).expect("equality operation"),
                result,
                left: function.parameters[0].value,
                right: function.parameters[1].value,
            },
        );
        let TerminalAbstractOperation::Return {
            value, scalar_type, ..
        } = &mut function.operations[1]
        else {
            unreachable!("fixture ends in return")
        };
        *value = result;
        *scalar_type = ScalarType::Boolean;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            &lowered.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanExpression {
                source_value,
                expression: TerminalTargetBooleanExpression::Equal {
                    psi_operation,
                    left,
                    right,
                },
                ..
            } if *source_value == result
                && *psi_operation == OperationId::new(50).expect("equality operation")
                && matches!(
                    left.as_ref(),
                    TerminalTargetBooleanExpression::Parameter { parameter_index: 0, .. }
                )
                && matches!(
                    right.as_ref(),
                    TerminalTargetBooleanExpression::Parameter { parameter_index: 1, .. }
                )
        ));
    }

    #[test]
    fn lowers_runtime_integer_equality_to_a_typed_target_expression() {
        let mut plan = parameter_return_plan(2);
        let function = &mut plan.functions[0];
        let integer_type = match function.parameters[0].scalar_type {
            ScalarType::Integer(integer_type) => integer_type,
            ScalarType::Boolean => unreachable!("fixture has integer parameters"),
        };
        scalar_result_mut(function).scalar_type = ScalarType::Boolean;
        let result = ValueId::new(51).expect("integer-equality result");
        function.operations.insert(
            0,
            TerminalAbstractOperation::IntegerEqual {
                psi_operation: OperationId::new(51).expect("integer-equality operation"),
                result,
                left: function.parameters[0].value,
                right: function.parameters[1].value,
            },
        );
        let TerminalAbstractOperation::Return {
            value, scalar_type, ..
        } = &mut function.operations[1]
        else {
            unreachable!("fixture ends in return")
        };
        *value = result;
        *scalar_type = ScalarType::Boolean;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            &lowered.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanExpression {
                source_value,
                expression: TerminalTargetBooleanExpression::IntegerEqual {
                    psi_operation,
                    scalar_type,
                    left,
                    right,
                },
                ..
            } if *source_value == result
                && *psi_operation == OperationId::new(51).expect("integer-equality operation")
                && *scalar_type == integer_type
                && matches!(
                    left.as_ref(),
                    TerminalTargetIntegerExpression::Parameter { parameter_index: 0, .. }
                )
                && matches!(
                    right.as_ref(),
                    TerminalTargetIntegerExpression::Parameter { parameter_index: 1, .. }
                )
        ));
    }

    #[test]
    fn folds_a_compile_known_conditional_to_only_the_selected_arm() {
        let condition_operation = psi_core::OperationId::new(20).expect("condition operation");
        let true_operation = psi_core::OperationId::new(21).expect("true operation");
        let false_operation = psi_core::OperationId::new(22).expect("false operation");
        let true_edge = EdgeId::new(1).expect("true edge");
        let false_edge = EdgeId::new(2).expect("false edge");
        let true_return = EdgeId::new(3).expect("true return");
        let false_return = EdgeId::new(4).expect("false return");

        for (select_true, selected_operation, selected_edges) in [
            (true, true_operation, [true_edge, true_return]),
            (false, false_operation, [false_edge, false_return]),
        ] {
            let plan = constant_conditional_plan(select_true);
            let lowered =
                lower_to_target_operations(&plan, NativeTarget::linux_x64()).expect("lower");
            let function = &lowered.functions[0];
            assert_eq!(
                function.provenance.operations,
                [condition_operation, selected_operation]
            );
            assert_eq!(function.provenance.edges, selected_edges);
            assert!(
                matches!(
                    &function.operation,
                    TerminalTargetOperation::ReturnIntegerExpression {
                        psi_edge,
                        expression:
                            TerminalTargetIntegerExpression::WrappingAdd { psi_operation, .. },
                        ..
                    } if select_true && *psi_edge == true_return && *psi_operation == true_operation
                ) || matches!(
                    &function.operation,
                    TerminalTargetOperation::ReturnIntegerExpression {
                        psi_edge,
                        expression:
                            TerminalTargetIntegerExpression::SaturatingMultiply {
                                psi_operation,
                                ..
                            },
                        ..
                    } if !select_true && *psi_edge == false_return && *psi_operation == false_operation
                )
            );
        }
    }

    fn constant_conditional_plan(select_true: bool) -> TerminalAbstractOperationPlan {
        let machine = MachineId::new(20).expect("machine");
        let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
        let scalar_type = ScalarType::Integer(integer);
        let argument = ValueId::new(1).expect("argument");
        let condition = ValueId::new(2).expect("condition");
        let true_parameter = ValueId::new(3).expect("true parameter");
        let false_parameter = ValueId::new(4).expect("false parameter");
        let true_value = ValueId::new(5).expect("true value");
        let false_value = ValueId::new(6).expect("false value");
        let result = ValueId::new(7).expect("result");
        let true_edge = EdgeId::new(1).expect("true edge");
        let false_edge = EdgeId::new(2).expect("false edge");
        let true_return = EdgeId::new(3).expect("true return");
        let false_return = EdgeId::new(4).expect("false return");
        TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![TerminalAbstractFunction {
                machine,
                attachment: None,
                entry: BlockId::new(1).expect("entry block"),
                parameters: vec![TerminalAbstractParameter {
                    value: argument,
                    scalar_type,
                }],
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                        block: BlockId::new(1).expect("entry block"),
                        operation_offset: 0,
                    },
                    omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                        block: BlockId::new(2).expect("true block"),
                        operation_offset: 2,
                    },
                    omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                        block: BlockId::new(3).expect("false block"),
                        operation_offset: 4,
                    },
                ],
                operations: vec![
                    TerminalAbstractOperation::BooleanConstant {
                        psi_operation: psi_core::OperationId::new(20).expect("condition operation"),
                        result: condition,
                        value: select_true,
                    },
                    TerminalAbstractOperation::Conditional {
                        condition,
                        when_true: TerminalAbstractSuccessor {
                            psi_edge: true_edge,
                            target: BlockId::new(2).expect("true block"),
                            bindings: vec![TerminalValueBinding {
                                parameter: true_parameter,
                                argument,
                                scalar_type,
                            }],
                        },
                        when_false: TerminalAbstractSuccessor {
                            psi_edge: false_edge,
                            target: BlockId::new(3).expect("false block"),
                            bindings: vec![TerminalValueBinding {
                                parameter: false_parameter,
                                argument,
                                scalar_type,
                            }],
                        },
                    },
                    TerminalAbstractOperation::WrappingIntegerAdd {
                        psi_operation: psi_core::OperationId::new(21).expect("true operation"),
                        result: true_value,
                        scalar_type: integer,
                        left: true_parameter,
                        right: true_parameter,
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: true_return,
                        result,
                        value: true_value,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                    TerminalAbstractOperation::SaturatingIntegerMultiply {
                        psi_operation: psi_core::OperationId::new(22).expect("false operation"),
                        result: false_value,
                        scalar_type: integer,
                        left: false_parameter,
                        right: false_parameter,
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: false_return,
                        result,
                        value: false_value,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        }
    }

    fn parameter_return_plan(parameter_count: usize) -> TerminalAbstractOperationPlan {
        let machine = MachineId::new(10).expect("machine");
        let result = ValueId::new(100).expect("result");
        let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
        let scalar_type = ScalarType::Integer(integer);
        let parameters = (0..parameter_count)
            .map(|index| TerminalAbstractParameter {
                value: ValueId::new(10 + index as u64).expect("parameter"),
                scalar_type,
            })
            .collect::<Vec<_>>();
        let returned = parameters.last().expect("fixture has parameters").value;
        TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![TerminalAbstractFunction {
                machine,
                attachment: None,
                entry: BlockId::new(10).expect("block"),
                parameters,
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![TerminalAbstractOperation::Return {
                    psi_edge: EdgeId::new(10).expect("edge"),
                    result,
                    value: returned,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                }],
            }],
        }
    }

    fn direct_call_plan(parameter_count: usize) -> TerminalAbstractOperationPlan {
        let caller = MachineId::new(1).expect("caller");
        let callee = MachineId::new(2).expect("callee");
        let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
        let scalar_type = ScalarType::Integer(integer);
        let caller_parameters = (0..parameter_count)
            .map(|index| TerminalAbstractParameter {
                value: ValueId::new(10 + index as u64).expect("caller parameter"),
                scalar_type,
            })
            .collect::<Vec<_>>();
        let callee_parameters = (0..parameter_count)
            .map(|index| TerminalAbstractParameter {
                value: ValueId::new(30 + index as u64).expect("callee parameter"),
                scalar_type,
            })
            .collect::<Vec<_>>();
        let caller_result = ValueId::new(100).expect("caller result");
        let callee_result = ValueId::new(101).expect("callee result");
        TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: caller,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                TerminalAbstractFunction {
                    machine: caller,
                    attachment: None,
                    entry: BlockId::new(1).expect("caller block"),
                    parameters: caller_parameters.clone(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: caller_result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: Vec::new(),
                    operations: vec![
                        TerminalAbstractOperation::Call {
                            psi_operation: OperationId::new(1).expect("call"),
                            result: caller_result,
                            scalar_type,
                            callee,
                            arguments: caller_parameters
                                .iter()
                                .map(|parameter| parameter.value)
                                .collect(),
                        },
                        TerminalAbstractOperation::Return {
                            psi_edge: EdgeId::new(1).expect("caller return"),
                            result: caller_result,
                            value: caller_result,
                            scalar_type,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                },
                TerminalAbstractFunction {
                    machine: callee,
                    attachment: None,
                    entry: BlockId::new(2).expect("callee block"),
                    parameters: callee_parameters.clone(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: callee_result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: Vec::new(),
                    operations: vec![TerminalAbstractOperation::Return {
                        psi_edge: EdgeId::new(2).expect("callee return"),
                        result: callee_result,
                        value: callee_parameters.last().expect("parameter").value,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    }],
                },
            ],
        }
    }

    fn identity() -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        }
    }

    fn scalar_result(function: &TerminalAbstractFunction) -> TerminalAbstractResult {
        function.result.scalar().expect("fixture is scalar")
    }

    fn scalar_result_mut(function: &mut TerminalAbstractFunction) -> &mut TerminalAbstractResult {
        let TerminalAbstractFunctionResult::Scalar(result) = &mut function.result else {
            panic!("fixture is scalar")
        };
        result
    }
}
