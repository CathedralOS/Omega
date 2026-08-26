#![forbid(unsafe_code)]

//! Resolve source-independent terminal Omega requirements into the first
//! target operation slice.

use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::{
    CallSignature, CallingPolicy, PlanDiagnostic, ValueClass, ValueLocation, ValuePlacement,
    ValueShape, evaluate_call_plan,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_abstract_operations::{
    TerminalAbstractFunction, TerminalAbstractFunctionResult, TerminalAbstractOperation,
    TerminalAbstractOperationPlan, TerminalAbstractParameter, TerminalAbstractResult,
};
use omega_terminal_target_operations::{
    MachineRegister, TerminalBoundaryByteSequenceArgument, TerminalBoundaryRealization,
    TerminalBoundaryScalarArgument, TerminalBoundarySettlementBinding, TerminalPsiProvenance,
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

mod conditional_control;
mod conditional_scalar;
mod structural_result;
mod structural_scalar;

use conditional_control::{
    lower_boolean_block, lower_boolean_conditional, lower_integer_conditional,
};
use conditional_scalar::{
    IntegerBinaryKind, WrappingShiftKind, lower_conditional_integer_binary,
    lower_conditional_scalar_operation, lower_exact_shift_left, lower_exact_shift_right,
    lower_wrapping_shift,
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
            let declaration = plan
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == settlement.boundary)
                .ok_or(LoweringError::UnknownBoundarySettlement(
                    settlement.boundary,
                ))?;
            if settlement.provider_execution.requirement_identity() != declaration.identity {
                return Err(LoweringError::ProviderExecutionRequirementMismatch {
                    boundary: settlement.boundary,
                    expected: declaration.identity.clone(),
                    actual: settlement
                        .provider_execution
                        .requirement_identity()
                        .to_owned(),
                });
            }
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
    let boundary_machines = plan
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary))
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
                    &boundary_machines,
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
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, TerminalBoundarySettlementBinding>,
) -> Result<TerminalTargetFunction, LoweringError> {
    if let Some(lowered) =
        lower_linux_exit_group_i32(function, target, boundary_machines, settlements)?
    {
        return Ok(lowered);
    }
    if let Some(TerminalAbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        ..
    }) = function.operations.iter().find(|operation| {
        matches!(
            operation,
            TerminalAbstractOperation::BoundaryCall {
                boundary,
                arguments,
                ..
            } if !arguments.is_empty()
                && !matches!(
                    settlements.get(boundary).map(|binding| binding.realization),
                    Some(TerminalBoundaryRealization::LinuxExitGroupI32(_))
                )
        )
    }) {
        return Err(
            LoweringError::ScalarBoundaryArgumentsRequireNativeRealization {
                machine: function.machine,
                operation: *psi_operation,
                boundary: *boundary,
            },
        );
    }
    if let Some(result) = function.result.structural() {
        if let Some(lowered) =
            structural_result::lower_direct_return(function, target, functions, structural_types)?
        {
            return Ok(lowered);
        }
        return lower_structural_return_function(function, result, target, structural_types);
    }
    let Some(function_result) = function.result.scalar() else {
        return lower_unit_function(
            function,
            target,
            functions,
            structural_types,
            boundary_machines,
            settlements,
        );
    };
    let mut values = BTreeMap::new();
    let mut provenance = TerminalPsiProvenance::default();
    let mut returned = None;
    let scalar_parameter_shapes = function
        .parameters
        .iter()
        .map(|parameter| scalar_shape(parameter.value, parameter.scalar_type, true))
        .collect::<Result<Vec<_>, _>>()?;
    let boundary_custody_places = function
        .operations
        .iter()
        .filter_map(|operation| {
            let TerminalAbstractOperation::BoundaryCall {
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
                ..
            } = operation
            else {
                return None;
            };
            Some(structural_arguments.iter().enumerate().filter_map(
                |(argument_index, argument)| {
                    let receipt = completion_receipts.iter().find(|receipt| {
                        usize::try_from(receipt.argument_index) == Ok(argument_index)
                    })?;
                    completion_claim_sources
                        .iter()
                        .any(|source| {
                            source.claim == receipt.claim
                                && source.entry.as_ref().is_some_and(|entry| {
                                    entry.input == argument.place && entry.path == argument.path
                                })
                        })
                        .then_some(argument.place)
                },
            ))
        })
        .flatten()
        .collect::<BTreeSet<_>>();
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let structural_parameter_shapes = function
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            // Verified qualifications remain in the exact completion-custody
            // source and do not alter the structural ABI shape. Linear inputs
            // enter this scalar lane only when that same boundary call carries
            // their claim toward provider custody.
            let carries_boundary_custody = boundary_custody_places.contains(&parameter.place);
            if usize::try_from(parameter.position) != Ok(position)
                || parameter.is_self
                || !matches!(
                    parameter.multiplicity,
                    psi_terminal::StructuralMultiplicity::Affine
                        | psi_terminal::StructuralMultiplicity::Linear
                )
                || ((!parameter.qualifications.is_empty()
                    || parameter.multiplicity == psi_terminal::StructuralMultiplicity::Linear)
                    && !carries_boundary_custody)
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
                access: parameter.access,
                shape,
                placement: placement.clone(),
            },
        )
        .collect::<Vec<_>>();

    if let Some(lowered) = structural_scalar::lower_direct_return(
        function,
        function_result,
        target,
        functions,
        structural_types,
        &call_plan,
        &target_structural_parameters,
        &mut shape_cache,
        &mut active,
    )? {
        return Ok(lowered);
    }

    if let [
        TerminalAbstractOperation::BoundaryCall {
            psi_operation,
            result: Some(boundary_result),
            boundary,
            arguments: _,
            structural_arguments,
            completion_claim_sources,
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
                completion_claim_sources: completion_claim_sources.clone(),
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
            TerminalAbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. } => {
                return Err(LoweringError::UnitOperationInScalarFunction {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
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
            TerminalAbstractOperation::CallStructuralScalar { .. }
            | TerminalAbstractOperation::CallStructural { .. } => {
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
                obligation,
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
                let value =
                    KnownInteger::Runtime(TerminalTargetIntegerExpression::IntegerExactCast {
                        psi_operation: *psi_operation,
                        obligation: *obligation,
                        source_type: *source_type,
                        operand: Box::new(operand_value.into_expression(*operand)),
                    });
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
                obligation,
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
                    *obligation,
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
                obligation,
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
                    *obligation,
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
            }
            | TerminalAbstractOperation::ExactIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
                ..
            } => {
                let exact_obligation = match operation {
                    TerminalAbstractOperation::ExactIntegerAdd { obligation, .. } => {
                        Some(*obligation)
                    }
                    _ => None,
                };
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
                let value = match (exact_obligation, left, right) {
                    (None, KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                        KnownInteger::Immediate(
                            scalar_type
                                .wrapping_add(left, right)
                                .ok_or(LoweringError::WrappingAddOperandTypeMismatch(*result))?,
                        )
                    }
                    (Some(obligation), left, right) => {
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::ExactAdd {
                            psi_operation: *psi_operation,
                            obligation,
                            left: Box::new(left.into_expression(left_id)),
                            right: Box::new(right.into_expression(right_id)),
                        })
                    }
                    (None, left, right) => {
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
            }
            | TerminalAbstractOperation::ExactIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
                ..
            } => {
                let exact_obligation = match operation {
                    TerminalAbstractOperation::ExactIntegerSubtract { obligation, .. } => {
                        Some(*obligation)
                    }
                    _ => None,
                };
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
                    match (exact_obligation, left, right) {
                        (None, KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                            KnownInteger::Immediate(scalar_type.wrapping_sub(left, right).ok_or(
                                LoweringError::WrappingSubtractOperandTypeMismatch(*result),
                            )?)
                        }
                        (Some(obligation), left, right) => {
                            KnownInteger::Runtime(TerminalTargetIntegerExpression::ExactSubtract {
                                psi_operation: *psi_operation,
                                obligation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            })
                        }
                        (None, left, right) => KnownInteger::Runtime(
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
            }
            | TerminalAbstractOperation::ExactIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
                ..
            } => {
                let exact_obligation = match operation {
                    TerminalAbstractOperation::ExactIntegerMultiply { obligation, .. } => {
                        Some(*obligation)
                    }
                    _ => None,
                };
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
                    match (exact_obligation, left, right) {
                        (None, KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                            KnownInteger::Immediate(scalar_type.wrapping_mul(left, right).ok_or(
                                LoweringError::WrappingMultiplyOperandTypeMismatch(*result),
                            )?)
                        }
                        (Some(obligation), left, right) => {
                            KnownInteger::Runtime(TerminalTargetIntegerExpression::ExactMultiply {
                                psi_operation: *psi_operation,
                                obligation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            })
                        }
                        (None, left, right) => KnownInteger::Runtime(
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
                obligation,
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
                    IntegerBinaryKind::ExactDivide(*obligation),
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
                obligation,
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
                    IntegerBinaryKind::ExactRemainder(*obligation),
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
                obligation,
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
                    IntegerBinaryKind::WrappingDivide(*obligation),
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
                obligation,
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
                    IntegerBinaryKind::WrappingRemainder(*obligation),
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
                obligation,
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
                    IntegerBinaryKind::SaturatingDivide(*obligation),
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
                obligation,
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
                    IntegerBinaryKind::SaturatingRemainder(*obligation),
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

fn lower_linux_exit_group_i32(
    function: &TerminalAbstractFunction,
    target: NativeTarget,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, TerminalBoundarySettlementBinding>,
) -> Result<Option<TerminalTargetFunction>, LoweringError> {
    let Some(TerminalAbstractOperation::BoundaryCall { boundary, arguments, .. }) = function
        .operations
        .iter()
        .find(|operation| matches!(operation, TerminalAbstractOperation::BoundaryCall { arguments, .. } if !arguments.is_empty()))
    else {
        return Ok(None);
    };
    let Some(binding) = settlements.get(boundary).copied() else {
        return Err(LoweringError::MissingBoundarySettlement(*boundary));
    };
    let TerminalBoundaryRealization::LinuxExitGroupI32(realization) = binding.realization else {
        return Ok(None);
    };
    if target.object_format != ObjectFormat::Elf
        || !matches!(
            target.architecture,
            Architecture::X86_64 | Architecture::Aarch64
        )
    {
        return Err(LoweringError::LinuxExitGroupUnsupportedTarget {
            machine: function.machine,
            target,
        });
    }
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
    let expected_scalar_type = ScalarType::Integer(i32_type);
    let Some(declaration) = boundary_machines.get(boundary).copied() else {
        return Err(LoweringError::UnknownBoundarySettlement(*boundary));
    };
    let [
        TerminalAbstractOperation::IntegerConstant {
            psi_operation: constant_operation,
            result: constant_result,
            scalar_type,
            value,
        },
        TerminalAbstractOperation::BoundaryCall {
            psi_operation,
            result: None,
            boundary: called_boundary,
            arguments: call_arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        },
        TerminalAbstractOperation::ReturnUnit {
            psi_edge: nominal_return_edge,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
    else {
        // A Linux exit may be the nonreturning tail of a larger straight-line
        // Unit effect body (notably write_line -> exit_process). Let the Unit
        // lowering validate that composition; retain the directed error for a
        // malformed isolated exit shape.
        return if function.operations.iter().any(|operation| {
            matches!(
                operation,
                TerminalAbstractOperation::EstablishByteSequenceLiteral { .. }
            )
        }) || function
            .operations
            .iter()
            .filter(|operation| matches!(operation, TerminalAbstractOperation::BoundaryCall { .. }))
            .count()
            > 1
        {
            Ok(None)
        } else {
            Err(LoweringError::InvalidLinuxExitGroupShape(function.machine))
        };
    };
    if function.result != TerminalAbstractFunctionResult::Unit
        || !function.parameters.is_empty()
        || !function.structural_parameters.is_empty()
        || function.block_entries.len() != 1
        || function.block_entries[0].block != function.entry
        || declaration.scalar_parameters.as_slice() != [expected_scalar_type]
        || !declaration.structural_parameters.is_empty()
        || declaration.result.is_some()
        || *called_boundary != *boundary
        || arguments.as_slice() != [*constant_result]
        || call_arguments.as_slice() != [*constant_result]
        || *scalar_type != expected_scalar_type
        || !i32_type.admits(*value)
        || !structural_arguments.is_empty()
        || !cleanup_actions.is_empty()
    {
        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
    }
    let destination = match target.architecture {
        Architecture::X86_64 => MachineRegister::X86Rdi,
        Architecture::Aarch64 => MachineRegister::Aarch64X(0),
    };
    Ok(Some(TerminalTargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: TerminalPsiProvenance {
            operations: vec![*constant_operation, *psi_operation],
            edges: vec![*nominal_return_edge],
        },
        operation: TerminalTargetOperation::ExitProcessI32 {
            constant_operation: *constant_operation,
            psi_operation: *psi_operation,
            nominal_return_edge: *nominal_return_edge,
            boundary: *boundary,
            provider_execution: binding.provider_execution,
            realization,
            argument: TerminalBoundaryScalarArgument {
                source_value: *constant_result,
                scalar_type: *scalar_type,
                immediate: *value,
                destination,
            },
            completion_claim_sources: completion_claim_sources.clone(),
            completion_receipts: completion_receipts.clone(),
        },
    }))
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
            | TerminalAbstractOperation::ExactIntegerAdd { .. }
            | TerminalAbstractOperation::SaturatingIntegerAdd { .. }
            | TerminalAbstractOperation::WrappingIntegerSubtract { .. }
            | TerminalAbstractOperation::ExactIntegerSubtract { .. }
            | TerminalAbstractOperation::SaturatingIntegerSubtract { .. }
            | TerminalAbstractOperation::WrappingIntegerMultiply { .. }
            | TerminalAbstractOperation::ExactIntegerMultiply { .. }
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
    if shape.class != ValueClass::Integer
        || !((shape.byte_size == 8 && shape.alignment == 8) || (9..=16).contains(&shape.byte_size))
    {
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
    require_direct_structural_fragments(function.machine, source_placement)?;
    require_direct_structural_fragments(function.machine, result_placement)?;
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

fn require_direct_structural_fragments(
    machine: MachineId,
    placement: &ValuePlacement,
) -> Result<(), LoweringError> {
    if placement.shape.class != ValueClass::Integer
        || !((placement.shape.byte_size == 8 && placement.shape.alignment == 8)
            || (9..=16).contains(&placement.shape.byte_size))
        || !(1..=2).contains(&placement.locations.len())
    {
        return Err(LoweringError::UnsupportedStructuralReturnPlacement(machine));
    }
    let mut expected_offset = 0_u16;
    for location in &placement.locations {
        let ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        } = *location
        else {
            return Err(LoweringError::UnsupportedStructuralReturnPlacement(machine));
        };
        let expected_size = (placement.shape.byte_size - expected_offset).min(8);
        if value_byte_offset != expected_offset || byte_size != expected_size {
            return Err(LoweringError::UnsupportedStructuralReturnPlacement(machine));
        }
        expected_offset = expected_offset
            .checked_add(byte_size)
            .ok_or(LoweringError::UnsupportedStructuralReturnPlacement(machine))?;
    }
    if expected_offset != placement.shape.byte_size {
        return Err(LoweringError::UnsupportedStructuralReturnPlacement(machine));
    }
    Ok(())
}

fn lower_unit_function(
    function: &TerminalAbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
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
                access: parameter.access,
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
    let mut established_byte_sequences =
        BTreeMap::<PlaceId, (OperationId, StructuralTypeDeclaration, Vec<u8>)>::new();
    let mut integer_constants =
        BTreeMap::<ValueId, (OperationId, IntegerType, IntegerValue)>::new();
    let mut nonreturning_boundary = false;
    for operation in &function.operations {
        if returned {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        match operation {
            TerminalAbstractOperation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                structural_type,
                bytes,
            } => {
                if nonreturning_boundary
                    || !matches!(
                        (&place.kind, &structural_type.shape),
                        (
                            psi_core::StructuralPlaceKind::ByteSequenceLiteral {
                                structural_type: place_type,
                                ..
                            },
                            StructuralTypeShape::ByteSequence(
                                psi_terminal::ByteSequenceCarrier::BorrowedView
                            )
                        ) if *place_type == structural_type.id
                    )
                    || established_byte_sequences
                        .insert(
                            place.id,
                            (*psi_operation, structural_type.clone(), bytes.clone()),
                        )
                        .is_some()
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                operations.push(TerminalTargetUnitOperation::EstablishByteSequenceLiteral {
                    psi_operation: *psi_operation,
                    place: place.clone(),
                    structural_type: structural_type.clone(),
                    bytes: bytes.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
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
                            access: argument.access,
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
                arguments,
                structural_arguments,
                completion_claim_sources,
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
                let declaration = boundary_machines
                    .get(boundary)
                    .copied()
                    .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
                let mut scalar_arguments = Vec::new();
                let mut byte_sequence_arguments = Vec::new();
                match binding.realization {
                    TerminalBoundaryRealization::MetadataOnlyPort(realization) => {
                        if !arguments.is_empty()
                            || !matches!(
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
                            )
                        {
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
                    }
                    TerminalBoundaryRealization::LinuxWriteLine(_) => {
                        if target.object_format != ObjectFormat::Elf
                            || !matches!(
                                target.architecture,
                                Architecture::X86_64 | Architecture::Aarch64
                            )
                            || !arguments.is_empty()
                            || declaration.result.is_some()
                            || !declaration.scalar_parameters.is_empty()
                            || structural_arguments.len() != 1
                            || declaration.structural_parameters.len() != 1
                        {
                            return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                                machine: function.machine,
                                boundary: *boundary,
                                target,
                            });
                        }
                        let argument = &structural_arguments[0];
                        let parameter = &declaration.structural_parameters[0];
                        let Some((literal_operation, structural_type, bytes)) =
                            established_byte_sequences.get(&argument.place)
                        else {
                            return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                                machine: function.machine,
                                boundary: *boundary,
                                target,
                            });
                        };
                        if !argument.path.is_empty()
                            || parameter.position != 0
                            || parameter.is_self
                            || parameter.structural_type != structural_type.id
                            || parameter.multiplicity
                                != psi_terminal::StructuralMultiplicity::Unrestricted
                            || !parameter.qualifications.is_empty()
                        {
                            return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                                machine: function.machine,
                                boundary: *boundary,
                                target,
                            });
                        }
                        byte_sequence_arguments.push(TerminalBoundaryByteSequenceArgument {
                            argument: argument.clone(),
                            literal_operation: *literal_operation,
                            structural_type: structural_type.clone(),
                            bytes: bytes.clone(),
                        });
                    }
                    TerminalBoundaryRealization::LinuxExitGroupI32(_) => {
                        let i32_type =
                            IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
                        let [argument] = arguments.as_slice() else {
                            return Err(LoweringError::InvalidLinuxExitGroupShape(
                                function.machine,
                            ));
                        };
                        let Some((_, actual_type, value)) = integer_constants.get(argument) else {
                            return Err(LoweringError::InvalidLinuxExitGroupShape(
                                function.machine,
                            ));
                        };
                        if target.object_format != ObjectFormat::Elf
                            || !matches!(
                                target.architecture,
                                Architecture::X86_64 | Architecture::Aarch64
                            )
                            || declaration.scalar_parameters.as_slice()
                                != [ScalarType::Integer(i32_type)]
                            || !declaration.structural_parameters.is_empty()
                            || declaration.result.is_some()
                            || *actual_type != i32_type
                            || !i32_type.admits(*value)
                            || !structural_arguments.is_empty()
                        {
                            return Err(LoweringError::InvalidLinuxExitGroupShape(
                                function.machine,
                            ));
                        }
                        scalar_arguments.push(TerminalBoundaryScalarArgument {
                            source_value: *argument,
                            scalar_type: ScalarType::Integer(*actual_type),
                            immediate: *value,
                            destination: match target.architecture {
                                Architecture::X86_64 => MachineRegister::X86Rdi,
                                Architecture::Aarch64 => MachineRegister::Aarch64X(0),
                            },
                        });
                        nonreturning_boundary = true;
                    }
                    TerminalBoundaryRealization::DirectPortReadU8(_) => {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    }
                }
                operations.push(TerminalTargetUnitOperation::BoundarySettlement {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    provider_execution: binding.provider_execution,
                    realization: binding.realization,
                    scalar_arguments,
                    arguments: structural_arguments.clone(),
                    byte_sequence_arguments,
                    completion_claim_sources: completion_claim_sources.clone(),
                    completion_receipts: completion_receipts.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::ReturnUnit {
                psi_edge,
                cleanup_actions,
            } => {
                if nonreturning_boundary && !cleanup_actions.is_empty() {
                    return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
                }
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
                                    parameters: Vec::new(),
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
            TerminalAbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type: ScalarType::Integer(scalar_type),
                value,
            } => {
                if nonreturning_boundary
                    || integer_constants
                        .insert(*result, (*psi_operation, *scalar_type, *value))
                        .is_some()
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                operations.push(TerminalTargetUnitOperation::IntegerConstant {
                    psi_operation: *psi_operation,
                    result: *result,
                    scalar_type: *scalar_type,
                    value: *value,
                });
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::Crash { .. }
            | TerminalAbstractOperation::Call { .. }
            | TerminalAbstractOperation::CallStructuralScalar { .. }
            | TerminalAbstractOperation::CallStructural { .. }
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
            | TerminalAbstractOperation::ExactIntegerAdd { .. }
            | TerminalAbstractOperation::SaturatingIntegerAdd { .. }
            | TerminalAbstractOperation::WrappingIntegerSubtract { .. }
            | TerminalAbstractOperation::ExactIntegerSubtract { .. }
            | TerminalAbstractOperation::SaturatingIntegerSubtract { .. }
            | TerminalAbstractOperation::WrappingIntegerMultiply { .. }
            | TerminalAbstractOperation::ExactIntegerMultiply { .. }
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
                        parameters: Vec::new(),
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
            StructuralTypeShape::ByteSequence(_) => Err(
                LoweringError::UnsupportedStructuralByteSequence(structural_type),
            ),
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
                || !matches!(
                    field.field_type,
                    StructuralFieldType::Structural(_)
                        | StructuralFieldType::Scalar(_)
                        | StructuralFieldType::IeeeFloat(_)
                )
        })
        || moved
            .iter()
            .any(|(path, _)| !matches!(path.first(), Some(StructuralPathSegment::Field(_))))
    {
        return None;
    }
    let mut matched = 0_usize;
    for field in fields.iter().rev() {
        let matching = moved
            .iter()
            .filter(|(path, _)| {
                matches!(path.first(), Some(StructuralPathSegment::Field(identity))
                    if identity == &field.identity)
            })
            .copied()
            .collect::<Vec<_>>();
        matched += matching.len();
        let mut field_path = prefix.to_vec();
        field_path.push(StructuralPathSegment::Field(field.identity.clone()));
        let StructuralFieldType::Structural(field_type) = field.field_type else {
            if !matching.is_empty() {
                return None;
            }
            continue;
        };
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
    (matched == moved.len()).then_some(())
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
            TerminalAbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. }
            | TerminalAbstractOperation::EstablishTrivialAffineLocal { psi_operation, .. }
            | TerminalAbstractOperation::CallUnit { psi_operation, .. }
            | TerminalAbstractOperation::CallStructuralScalar { psi_operation, .. }
            | TerminalAbstractOperation::CallStructural { psi_operation, .. }
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
            | TerminalAbstractOperation::ExactIntegerAdd { psi_operation, .. }
            | TerminalAbstractOperation::SaturatingIntegerAdd { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerSubtract { psi_operation, .. }
            | TerminalAbstractOperation::ExactIntegerSubtract { psi_operation, .. }
            | TerminalAbstractOperation::SaturatingIntegerSubtract { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerMultiply { psi_operation, .. }
            | TerminalAbstractOperation::ExactIntegerMultiply { psi_operation, .. }
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
    ProviderExecutionRequirementMismatch {
        boundary: BoundaryMachineId,
        expected: String,
        actual: String,
    },
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
    ScalarBoundaryArgumentsRequireNativeRealization {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    LinuxExitGroupUnsupportedTarget {
        machine: MachineId,
        target: NativeTarget,
    },
    LinuxWriteLineUnsupportedOrInvalid {
        machine: MachineId,
        boundary: BoundaryMachineId,
        target: NativeTarget,
    },
    InvalidLinuxExitGroupShape(MachineId),
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
    UnsupportedStructuralByteSequence(StructuralTypeId),
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
mod tests;
