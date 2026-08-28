#![forbid(unsafe_code)]

//! Resolve source-independent terminal Omega requirements into the first
//! target operation slice.

use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractParameter, AbstractResult, CompletionClaimSource,
};
use omega_calling_conventions::{
    CallSignature, CallingPolicy, PlanDiagnostic, ValueClass, ValueLocation, ValuePlacement,
    ValueShape, evaluate_call_plan,
};
use omega_installation_evidence::{
    InstalledProviderCompletionClaimSource, InstalledProviderUnitCallEvidence,
    ProviderInstallationEvidence,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::{
    BoundaryByteSequenceArgument, BoundaryRealization, BoundaryScalarArgument,
    BoundarySettlementBinding, MachineRegister, ScalarParameterLocation, TargetBooleanControl,
    TargetBooleanExpression, TargetCallArgument, TargetConditionalBooleanArm,
    TargetConditionalIntegerArm, TargetFunction, TargetIntegerControl, TargetIntegerExpression,
    TargetOperation, TargetOperationPlan, TargetScalarExpression, TargetStructuralArgument,
    TargetStructuralParameter, TargetUnitBody, TargetUnitOperation, TerminalPsiProvenance,
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
pub struct AdmittedBoundarySettlement<'execution> {
    pub boundary: BoundaryMachineId,
    pub provider_execution: &'execution dyn omega_installation_evidence::ProviderExecutionEvidence,
    pub realization: BoundaryRealization,
}

pub fn lower_to_target_operations(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
) -> Result<TargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_settlements(plan, target, &[])
}

/// Lower an effectful terminal plan using the exact provider executions
/// already admitted by the external-root ledger.
pub fn lower_to_target_operations_with_provider_executions(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<TargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_provider_executions_and_installation(
        plan,
        target,
        settlements,
        None,
    )
}

/// Lower with optional checked-provider installation evidence and any
/// remaining external boundary settlements. Installed provider occurrences
/// are exact in-module Unit calls and cannot also consume an external
/// settlement for the same boundary.
pub fn lower_to_target_operations_with_provider_executions_and_installation(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: Option<&dyn ProviderInstallationEvidence>,
) -> Result<TargetOperationPlan, LoweringError> {
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
            let provider_plan = omega_target_operations::ProviderPlanIdentity::new(
                settlement.provider_execution.provider_plan(),
            )
            .ok_or_else(|| LoweringError::ProviderExecutionBinding("zero provider plan".into()))?;
            let provider_execution =
                omega_target_operations::ProviderExecutionBinding::from_execution_record(
                    provider_plan,
                    settlement.provider_execution.provider_execution_identity(),
                    settlement
                        .provider_execution
                        .provider_execution_fingerprint(),
                    settlement.provider_execution.normalized_root_identity(),
                    settlement
                        .provider_execution
                        .boundary_contract_fingerprint(),
                )
                .ok_or_else(|| {
                    LoweringError::ProviderExecutionBinding(
                        "admitted provider execution contains a zero identity".into(),
                    )
                })?;
            Ok(BoundarySettlementBinding {
                boundary: settlement.boundary,
                provider_execution,
                realization: settlement.realization,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    lower_to_target_operations_with_settlements_and_installation(
        plan,
        target,
        &bindings,
        installation,
    )
}

fn lower_to_target_operations_with_settlements(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlement_bindings: &[BoundarySettlementBinding],
) -> Result<TargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_settlements_and_installation(
        plan,
        target,
        settlement_bindings,
        None,
    )
}

fn lower_to_target_operations_with_settlements_and_installation(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlement_bindings: &[BoundarySettlementBinding],
    installation: Option<&dyn ProviderInstallationEvidence>,
) -> Result<TargetOperationPlan, LoweringError> {
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
    let installed_calls = installation
        .map(|installation| {
            if installation.psi() != plan.psi {
                return Err(LoweringError::ProviderInstallationIdentityMismatch);
            }
            Ok(installation.installed_provider_unit_calls())
        })
        .transpose()?
        .unwrap_or_default();
    let mut installed_by_call = BTreeMap::new();
    for installed in installed_calls {
        let key = (
            installed.caller,
            installed.psi_operation,
            installed.boundary,
        );
        if installed_by_call.insert(key, installed).is_some() {
            return Err(LoweringError::DuplicateInstalledProviderCall {
                machine: key.0,
                operation: key.1,
                boundary: key.2,
            });
        }
    }
    let boundary_calls = plan
        .functions
        .iter()
        .flat_map(|function| {
            function
                .operations
                .iter()
                .filter_map(move |operation| match operation {
                    AbstractOperation::BoundaryCall {
                        psi_operation,
                        boundary,
                        ..
                    } => Some(((function.machine, *psi_operation, *boundary), operation)),
                    _ => None,
                })
        })
        .collect::<BTreeMap<_, _>>();
    for (key, installed) in &installed_by_call {
        let Some(AbstractOperation::BoundaryCall {
            result,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
            ..
        }) = boundary_calls.get(key).copied()
        else {
            return Err(LoweringError::UnknownInstalledProviderCall {
                machine: key.0,
                operation: key.1,
                boundary: key.2,
            });
        };
        let exact_sources = completion_claim_sources
            .iter()
            .map(|source| InstalledProviderCompletionClaimSource {
                claim: source.claim,
                entry: source.entry.clone(),
                content: source.content.clone(),
            })
            .collect::<Vec<_>>();
        if result.is_some()
            || !arguments.is_empty()
            || installed.structural_arguments != *structural_arguments
            || installed.completion_claim_sources != exact_sources
            || installed.completion_receipts != *completion_receipts
            || installed.provider.boundary != key.2
            || !plan
                .provider_candidates
                .iter()
                .any(|candidate| candidate == &installed.provider)
        {
            return Err(LoweringError::InstalledProviderCallEvidenceMismatch {
                machine: key.0,
                operation: key.1,
                boundary: key.2,
            });
        }
    }
    let installed_boundaries = installed_by_call
        .keys()
        .map(|(_, _, boundary)| *boundary)
        .collect::<BTreeSet<_>>();
    if let Some(boundary) = settlements_by_boundary
        .keys()
        .find(|boundary| installed_boundaries.contains(boundary))
    {
        return Err(LoweringError::BoundarySettlementOverlapsInstalledProvider(
            *boundary,
        ));
    }
    if let Some((machine, operation, boundary)) = boundary_calls
        .keys()
        .find(|key| installed_boundaries.contains(&key.2) && !installed_by_call.contains_key(key))
        .copied()
    {
        return Err(LoweringError::PartialInstalledProviderBoundary {
            machine,
            operation,
            boundary,
        });
    }
    let required_settlements = boundary_calls
        .keys()
        .filter_map(|key| (!installed_by_call.contains_key(key)).then_some(key.2))
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
    Ok(TargetOperationPlan {
        psi: plan.psi,
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
                    &installed_by_call,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_function(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
) -> Result<TargetFunction, LoweringError> {
    if let Some(lowered) =
        lower_linux_exit_group_i32(function, target, boundary_machines, settlements)?
    {
        return Ok(lowered);
    }
    if let Some(AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        ..
    }) = function.operations.iter().find(|operation| {
        matches!(
            operation,
            AbstractOperation::BoundaryCall {
                boundary,
                arguments,
                ..
            } if !arguments.is_empty()
                && !matches!(
                    settlements.get(boundary).map(|binding| binding.realization),
                    Some(BoundaryRealization::LinuxExitGroupI32(_))
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
            installed_calls,
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
            let AbstractOperation::BoundaryCall {
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
                KnownScalar::BooleanRuntime(TargetBooleanExpression::Parameter {
                    source_value: parameter.value,
                    parameter_index,
                    location,
                })
            }
            ScalarType::Integer(scalar_type) => KnownScalar::Integer {
                scalar_type,
                value: KnownInteger::Runtime(TargetIntegerExpression::Parameter {
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
            |((parameter, shape), placement)| TargetStructuralParameter {
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
        AbstractOperation::BoundaryCall {
            psi_operation,
            result: Some(boundary_result),
            boundary,
            arguments: _,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        },
        AbstractOperation::Return {
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
        let BoundaryRealization::DirectPortReadU8(realization) = binding.realization else {
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
        return Ok(TargetFunction {
            machine: function.machine,
            attachment: function.attachment,
            provenance: TerminalPsiProvenance {
                operations: vec![*psi_operation],
                edges: vec![*psi_edge],
            },
            operation: TargetOperation::ReturnBoundaryPortReadU8 {
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
        .any(|operation| matches!(operation, AbstractOperation::Conditional { .. }))
    {
        if function.structural_parameters.is_empty() {
            if function.operations.iter().any(|operation| {
                matches!(operation,
                    AbstractOperation::Return { cleanup_actions, .. }
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
            return Ok(TargetFunction {
                machine: function.machine,
                attachment: function.attachment,
                provenance: conditional_provenance(function, lowered.operations, lowered.edges),
                operation: TargetOperation::ScalarReturnWithCleanup {
                    scalar: Box::new(TargetOperation::ReturnBooleanSharedConvergence {
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
        return Ok(TargetFunction {
            machine: function.machine,
            attachment: function.attachment,
            provenance: conditional_provenance(function, lowered.operations, lowered.edges),
            operation: TargetOperation::BooleanControlWithCleanup {
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
            AbstractOperation::EstablishPayloadlessCase { psi_operation, .. }
            | AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. } => {
                return Err(LoweringError::UnitOperationInScalarFunction {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            AbstractOperation::BoundaryCall {
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
            AbstractOperation::EstablishTrivialAffineLocal { psi_operation, .. }
            | AbstractOperation::CallUnit { psi_operation, .. }
            | AbstractOperation::PortWrite { psi_operation, .. } => {
                return Err(LoweringError::UnitOperationInScalarFunction {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            AbstractOperation::CallStructuralScalar { .. }
            | AbstractOperation::CallStructural { .. } => {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
            AbstractOperation::Call {
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
            AbstractOperation::IntegerConstant {
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
            AbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value,
            } => {
                insert_value(&mut values, *result, KnownScalar::Boolean(*value))?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::BooleanStructuralField { psi_operation, .. } => {
                return Err(LoweringError::UnitOperationInScalarFunction {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            AbstractOperation::BooleanNot {
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
            AbstractOperation::BooleanEqual {
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
            AbstractOperation::IntegerEqual {
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
            AbstractOperation::IntegerLessThan {
                psi_operation,
                result,
                left,
                right,
            }
            | AbstractOperation::IntegerLessOrEqual {
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
                let inclusive = matches!(operation, AbstractOperation::IntegerLessOrEqual { .. });
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
            AbstractOperation::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
            | AbstractOperation::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
            | AbstractOperation::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let kind = match operation {
                    AbstractOperation::IntegerBitwiseAnd { .. } => IntegerBinaryKind::BitwiseAnd,
                    AbstractOperation::IntegerBitwiseOr { .. } => IntegerBinaryKind::BitwiseOr,
                    AbstractOperation::IntegerBitwiseXor { .. } => IntegerBinaryKind::BitwiseXor,
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
            AbstractOperation::IntegerBitwiseNot {
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
                        KnownInteger::Runtime(TargetIntegerExpression::BitwiseNot {
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
            AbstractOperation::IntegerWiden {
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
                        KnownInteger::Runtime(TargetIntegerExpression::IntegerWiden {
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
            AbstractOperation::IntegerExactCast {
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
                let value = KnownInteger::Runtime(TargetIntegerExpression::IntegerExactCast {
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
            AbstractOperation::WrappingIntegerShiftLeft {
                psi_operation,
                result,
                value_type,
                count_type,
                value,
                count,
            }
            | AbstractOperation::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                count_type,
                value,
                count,
            } => {
                let kind = if matches!(
                    operation,
                    AbstractOperation::WrappingIntegerShiftLeft { .. }
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
            AbstractOperation::ExactIntegerShiftRight {
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
            AbstractOperation::ExactIntegerShiftLeft {
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
            AbstractOperation::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
            | AbstractOperation::ExactIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
                ..
            } => {
                let exact_obligation = match operation {
                    AbstractOperation::ExactIntegerAdd { obligation, .. } => Some(*obligation),
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
                        KnownInteger::Runtime(TargetIntegerExpression::ExactAdd {
                            psi_operation: *psi_operation,
                            obligation,
                            left: Box::new(left.into_expression(left_id)),
                            right: Box::new(right.into_expression(right_id)),
                        })
                    }
                    (None, left, right) => {
                        KnownInteger::Runtime(TargetIntegerExpression::WrappingAdd {
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
            AbstractOperation::SaturatingIntegerAdd {
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
                        KnownInteger::Runtime(TargetIntegerExpression::SaturatingAdd {
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
            AbstractOperation::WrappingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
            | AbstractOperation::ExactIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
                ..
            } => {
                let exact_obligation = match operation {
                    AbstractOperation::ExactIntegerSubtract { obligation, .. } => Some(*obligation),
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
                            KnownInteger::Runtime(TargetIntegerExpression::ExactSubtract {
                                psi_operation: *psi_operation,
                                obligation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            })
                        }
                        (None, left, right) => {
                            KnownInteger::Runtime(TargetIntegerExpression::WrappingSubtract {
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
            AbstractOperation::SaturatingIntegerSubtract {
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
                        KnownInteger::Runtime(TargetIntegerExpression::SaturatingSubtract {
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
            AbstractOperation::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
            | AbstractOperation::ExactIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
                ..
            } => {
                let exact_obligation = match operation {
                    AbstractOperation::ExactIntegerMultiply { obligation, .. } => Some(*obligation),
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
                            KnownInteger::Runtime(TargetIntegerExpression::ExactMultiply {
                                psi_operation: *psi_operation,
                                obligation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            })
                        }
                        (None, left, right) => {
                            KnownInteger::Runtime(TargetIntegerExpression::WrappingMultiply {
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
            AbstractOperation::SaturatingIntegerMultiply {
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
                        KnownInteger::Runtime(TargetIntegerExpression::SaturatingMultiply {
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
            AbstractOperation::ExactIntegerDivide {
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
            AbstractOperation::ExactIntegerRemainder {
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
            AbstractOperation::WrappingIntegerDivide {
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
            AbstractOperation::WrappingIntegerRemainder {
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
            AbstractOperation::SaturatingIntegerDivide {
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
            AbstractOperation::SaturatingIntegerRemainder {
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
            AbstractOperation::Jump {
                psi_edge,
                bindings,
                trivial_affine_discards,
                ..
            } => {
                // This ownership-only edge work is deliberately erased after
                // Terminal verification (and optimizer admission when
                // selected); it has no target instruction.
                let _ = trivial_affine_discards;
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
            AbstractOperation::Conditional { .. } => {
                return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                    function.machine,
                ));
            }
            AbstractOperation::Crash {
                psi_edge,
                cause,
                site_guard,
                frontier_lower_bound,
            } => {
                provenance.edges.push(*psi_edge);
                returned = Some(TargetOperation::Crash {
                    psi_edge: *psi_edge,
                    cause: *cause,
                    site_guard: site_guard.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                });
            }
            AbstractOperation::Return {
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
                    KnownScalar::Boolean(boolean) => TargetOperation::ReturnBooleanImmediate {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        value: boolean,
                    },
                    KnownScalar::Integer {
                        scalar_type,
                        value: KnownInteger::Immediate(integer),
                    } => TargetOperation::ReturnIntegerImmediate {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        value: integer,
                    },
                    KnownScalar::Integer {
                        scalar_type,
                        value:
                            KnownInteger::Runtime(TargetIntegerExpression::Parameter {
                                parameter_index,
                                location,
                                ..
                            }),
                    } => TargetOperation::ReturnIntegerParameter {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        parameter_index,
                        location,
                    },
                    KnownScalar::Integer {
                        scalar_type,
                        value: KnownInteger::Runtime(expression),
                    } => TargetOperation::ReturnIntegerExpression {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        expression,
                    },
                    KnownScalar::BooleanRuntime(TargetBooleanExpression::Parameter {
                        parameter_index,
                        location,
                        ..
                    }) => TargetOperation::ReturnBooleanParameter {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        parameter_index,
                        location,
                    },
                    KnownScalar::BooleanRuntime(TargetBooleanExpression::Not {
                        operand, ..
                    }) if matches!(*operand, TargetBooleanExpression::Parameter { .. }) => {
                        let TargetBooleanExpression::Parameter {
                            parameter_index,
                            location,
                            ..
                        } = *operand
                        else {
                            unreachable!("guard requires a parameter operand")
                        };
                        TargetOperation::ReturnBooleanNotParameter {
                            psi_edge: *psi_edge,
                            source_value: *value,
                            parameter_index,
                            location,
                        }
                    }
                    KnownScalar::BooleanRuntime(expression) => {
                        TargetOperation::ReturnBooleanExpression {
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
                    returned = Some(TargetOperation::ScalarReturnWithCleanup {
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
            AbstractOperation::ReturnUnit { .. } | AbstractOperation::ReturnStructural { .. } => {
                return Err(LoweringError::FunctionResultKindMismatch(function.machine));
            }
        }
    }

    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance,
        operation: returned.ok_or(LoweringError::FunctionHasNoReturn(function.machine))?,
    })
}

fn lower_linux_exit_group_i32(
    function: &AbstractFunction,
    target: NativeTarget,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
) -> Result<Option<TargetFunction>, LoweringError> {
    let Some(AbstractOperation::BoundaryCall { boundary, arguments, .. }) = function
        .operations
        .iter()
        .find(|operation| matches!(operation, AbstractOperation::BoundaryCall { arguments, .. } if !arguments.is_empty()))
    else {
        return Ok(None);
    };
    let Some(binding) = settlements.get(boundary).copied() else {
        return Err(LoweringError::MissingBoundarySettlement(*boundary));
    };
    let BoundaryRealization::LinuxExitGroupI32(realization) = binding.realization else {
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
        AbstractOperation::IntegerConstant {
            psi_operation: constant_operation,
            result: constant_result,
            scalar_type,
            value,
        },
        AbstractOperation::BoundaryCall {
            psi_operation,
            result: None,
            boundary: called_boundary,
            arguments: call_arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        },
        AbstractOperation::ReturnUnit {
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
                AbstractOperation::EstablishByteSequenceLiteral { .. }
            )
        }) || function
            .operations
            .iter()
            .filter(|operation| matches!(operation, AbstractOperation::BoundaryCall { .. }))
            .count()
            > 1
        {
            Ok(None)
        } else {
            Err(LoweringError::InvalidLinuxExitGroupShape(function.machine))
        };
    };
    if function.result != AbstractFunctionResult::Unit
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
    Ok(Some(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: TerminalPsiProvenance {
            operations: vec![*constant_operation, *psi_operation],
            edges: vec![*nominal_return_edge],
        },
        operation: TargetOperation::ExitProcessI32 {
            constant_operation: *constant_operation,
            psi_operation: *psi_operation,
            nominal_return_edge: *nominal_return_edge,
            boundary: *boundary,
            provider_execution: binding.provider_execution,
            realization,
            argument: BoundaryScalarArgument {
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

fn shared_boolean_cleanup_convergence_return_edge(function: &AbstractFunction) -> Option<EdgeId> {
    let mut conditional_count = 0_usize;
    let mut jump_target = None;
    let mut jump_bindings = Vec::new();
    let mut return_edge = None;
    for operation in &function.operations {
        match operation {
            AbstractOperation::Conditional { .. } => conditional_count += 1,
            AbstractOperation::Jump {
                target, bindings, ..
            } => {
                if bindings.len() != 1 || jump_target.is_some_and(|existing| existing != *target) {
                    return None;
                }
                jump_target = Some(*target);
                jump_bindings.push(bindings[0]);
            }
            AbstractOperation::Return {
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
            AbstractOperation::BooleanConstant { .. }
            | AbstractOperation::BooleanStructuralField { .. }
            | AbstractOperation::BooleanNot { .. }
            | AbstractOperation::IntegerConstant { .. }
            | AbstractOperation::IntegerEqual { .. }
            | AbstractOperation::IntegerLessThan { .. }
            | AbstractOperation::IntegerLessOrEqual { .. }
            | AbstractOperation::IntegerBitwiseNot { .. }
            | AbstractOperation::IntegerWiden { .. }
            | AbstractOperation::IntegerExactCast { .. }
            | AbstractOperation::IntegerBitwiseAnd { .. }
            | AbstractOperation::IntegerBitwiseOr { .. }
            | AbstractOperation::IntegerBitwiseXor { .. }
            | AbstractOperation::WrappingIntegerShiftLeft { .. }
            | AbstractOperation::WrappingIntegerShiftRight { .. }
            | AbstractOperation::ExactIntegerShiftLeft { .. }
            | AbstractOperation::ExactIntegerShiftRight { .. }
            | AbstractOperation::WrappingIntegerAdd { .. }
            | AbstractOperation::ExactIntegerAdd { .. }
            | AbstractOperation::SaturatingIntegerAdd { .. }
            | AbstractOperation::WrappingIntegerSubtract { .. }
            | AbstractOperation::ExactIntegerSubtract { .. }
            | AbstractOperation::SaturatingIntegerSubtract { .. }
            | AbstractOperation::WrappingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerMultiply { .. }
            | AbstractOperation::SaturatingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerDivide { .. }
            | AbstractOperation::ExactIntegerRemainder { .. } => {}
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
        Some([AbstractOperation::Return { psi_edge, .. }]) if *psi_edge == edge
    )
    .then_some(edge)
}

fn shared_boolean_control_return_edge(control: &TargetBooleanControl) -> Option<EdgeId> {
    match control {
        TargetBooleanControl::ReturnImmediate {
            psi_return_edge, ..
        } => Some(*psi_return_edge),
        TargetBooleanControl::Conditional {
            when_true,
            when_false,
            ..
        }
        | TargetBooleanControl::ConditionalExpression {
            when_true,
            when_false,
            ..
        } => {
            let when_true = shared_boolean_control_return_edge(&when_true.control)?;
            let when_false = shared_boolean_control_return_edge(&when_false.control)?;
            (when_true == when_false).then_some(when_true)
        }
        TargetBooleanControl::Crash { .. }
        | TargetBooleanControl::ReturnParameter { .. }
        | TargetBooleanControl::ReturnNotParameter { .. }
        | TargetBooleanControl::ReturnExpression { .. } => None,
    }
}

fn lower_structural_return_function(
    function: &AbstractFunction,
    result: &psi_terminal::StructuralResultDeclaration,
    target: NativeTarget,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<TargetFunction, LoweringError> {
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
        AbstractOperation::ReturnStructural {
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
    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: TerminalPsiProvenance {
            operations: trivial_affine_locals
                .iter()
                .map(|(operation, _, _)| *operation)
                .collect(),
            edges: vec![*psi_edge],
        },
        operation: TargetOperation::ReturnStructuralParameter {
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

fn exact_fully_consumed_affine_pair_root(
    function: &AbstractFunction,
    parameters: &[TargetStructuralParameter],
    operations: &[TargetUnitOperation],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
) -> Option<PlaceId> {
    let ([source_parameter], [parameter], [first, second]) = (
        function.structural_parameters.as_slice(),
        parameters,
        operations,
    ) else {
        return None;
    };
    if source_parameter.position != 0
        || source_parameter.is_self
        || !function.entry_claims.is_empty()
        || source_parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine
        || source_parameter.access != psi_terminal::StructuralAccess::Owned
        || !source_parameter.qualifications.is_empty()
        || source_parameter.place != parameter.place
        || source_parameter.structural_type != parameter.structural_type
    {
        return None;
    }
    let root = structural_types.get(&parameter.structural_type).copied()?;
    let StructuralTypeShape::FixedArray { element, length: 2 } = root.shape else {
        return None;
    };
    if !matches!(
        structural_types
            .get(&element)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Record { .. })
    ) {
        return None;
    }
    let moved_index = |operation: &TargetUnitOperation| {
        let TargetUnitOperation::Call {
            callee,
            arguments,
            claim_transfers,
            ..
        } = operation
        else {
            return None;
        };
        let callee = functions.get(callee).copied()?;
        let [callee_parameter] = callee.structural_parameters.as_slice() else {
            return None;
        };
        let [argument] = arguments.as_slice() else {
            return None;
        };
        let [StructuralPathSegment::FixedIndex(index @ (0 | 1))] = argument.path.as_slice() else {
            return None;
        };
        let stride = argument.element_stride?;
        let expected_stride = u32::from(argument.shape.byte_size)
            .checked_next_multiple_of(u32::from(argument.shape.alignment))?;
        (callee.result == AbstractFunctionResult::Unit
            && callee.parameters.is_empty()
            && callee.entry_claims.is_empty()
            && callee_parameter.position == 0
            && !callee_parameter.is_self
            && callee_parameter.structural_type == element
            && callee_parameter.multiplicity == psi_terminal::StructuralMultiplicity::Affine
            && callee_parameter.access == psi_terminal::StructuralAccess::Owned
            && callee_parameter.qualifications.is_empty()
            && claim_transfers.is_empty()
            && argument.place == parameter.place
            && argument.access == psi_terminal::StructuralAccess::Owned
            && argument.root_structural_type == parameter.structural_type
            && argument.structural_type == element
            && argument.fixed_array_length == Some(2)
            && stride == expected_stride
            && argument.source == parameter.placement
            && argument.source.shape == parameter.shape
            && argument.source.shape.alignment == argument.shape.alignment
            && u32::from(argument.source.shape.byte_size) == stride.checked_mul(2)?
            && argument.source_byte_offset == stride.checked_mul(u32::try_from(*index).ok()?)?)
        .then_some((*index, argument.shape, stride))
    };
    let first = moved_index(first)?;
    let second = moved_index(second)?;
    (first.0 != second.0 && first.1 == second.1 && first.2 == second.2).then_some(parameter.place)
}

fn lower_unit_function(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
) -> Result<TargetFunction, LoweringError> {
    if !function.parameters.is_empty() {
        return Err(LoweringError::UnitFunctionHasScalarParameters(
            function.machine,
        ));
    }
    if function.block_entries.len() != 1
        || function.block_entries[0].block != function.entry
        || !function.block_entries[0].parameters.is_empty()
    {
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
            |((parameter, shape), placement)| TargetStructuralParameter {
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
            AbstractOperation::EstablishPayloadlessCase { .. } => {
                return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
            }
            AbstractOperation::EstablishByteSequenceLiteral {
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
                operations.push(TargetUnitOperation::EstablishByteSequenceLiteral {
                    psi_operation: *psi_operation,
                    place: place.clone(),
                    structural_type: structural_type.clone(),
                    bytes: bytes.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation,
                place,
                structural_type,
            } => {
                operations.push(TargetUnitOperation::EstablishTrivialAffineLocal {
                    psi_operation: *psi_operation,
                    place: place.clone(),
                    structural_type: structural_type.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                claim_transfers,
            } => {
                let callee_function = functions
                    .get(callee)
                    .copied()
                    .ok_or(LoweringError::UnknownCallTarget(*callee))?;
                if callee_function.result != AbstractFunctionResult::Unit
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
                        Ok(TargetStructuralArgument {
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
                operations.push(TargetUnitOperation::Call {
                    psi_operation: *psi_operation,
                    callee: *callee,
                    arguments,
                    claim_transfers: claim_transfers.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::PortWrite {
                psi_operation,
                service,
                port,
                value,
            } => {
                operations.push(TargetUnitOperation::PortWrite {
                    psi_operation: *psi_operation,
                    service: *service,
                    port: *port,
                    value: *value,
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::BoundaryCall {
                psi_operation,
                result,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            } => {
                if let Some(installed) =
                    installed_calls.get(&(function.machine, *psi_operation, *boundary))
                {
                    let callee = functions
                        .get(&installed.provider.candidate)
                        .copied()
                        .ok_or(LoweringError::UnknownCallTarget(
                            installed.provider.candidate,
                        ))?;
                    let declaration = boundary_machines
                        .get(boundary)
                        .copied()
                        .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
                    if result.is_some()
                        || !arguments.is_empty()
                        || callee.result != AbstractFunctionResult::Unit
                        || !callee.parameters.is_empty()
                        || structural_arguments.len() != callee.structural_parameters.len()
                        || declaration.structural_parameters.len()
                            != callee.structural_parameters.len()
                        || installed.provider.signature.parameters.len()
                            != callee.structural_parameters.len()
                    {
                        return Err(LoweringError::InstalledProviderCallShapeMismatch {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        });
                    }
                    let callee_shapes = callee
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
                    let mut target_arguments = Vec::with_capacity(structural_arguments.len());
                    for (index, (((argument, boundary_parameter), callee_parameter), signature)) in
                        structural_arguments
                            .iter()
                            .zip(&declaration.structural_parameters)
                            .zip(&callee.structural_parameters)
                            .zip(&installed.provider.signature.parameters)
                            .enumerate()
                    {
                        let source = parameters_by_place.get(&argument.place).copied().ok_or(
                            LoweringError::UnknownStructuralArgumentPlace {
                                machine: function.machine,
                                place: argument.place,
                            },
                        )?;
                        let caller_parameter = function
                            .structural_parameters
                            .iter()
                            .find(|parameter| parameter.place == argument.place)
                            .ok_or(LoweringError::UnknownStructuralArgumentPlace {
                                machine: function.machine,
                                place: argument.place,
                            })?;
                        if !argument.path.is_empty()
                            || argument.access != psi_terminal::StructuralAccess::Owned
                            || source.access != psi_terminal::StructuralAccess::Owned
                            || boundary_parameter.access != psi_terminal::StructuralAccess::Owned
                            || callee_parameter.access != psi_terminal::StructuralAccess::Owned
                            || boundary_parameter.position != index as u32
                            || callee_parameter.position != index as u32
                            || signature.position != index as u32
                            || boundary_parameter.is_self
                            || callee_parameter.is_self
                            || signature.is_self
                            || source.structural_type != boundary_parameter.structural_type
                            || source.structural_type != callee_parameter.structural_type
                            || source.structural_type != signature.structural_type
                            || source.multiplicity != boundary_parameter.multiplicity
                            || source.multiplicity != callee_parameter.multiplicity
                            || source.multiplicity != signature.multiplicity
                            || source.access != signature.access
                            || source.structural_type
                                != callee.structural_parameters[index].structural_type
                            || source.placement.shape != callee_shapes[index]
                            || source.placement.shape != callee_plan.parameters[index].shape
                            || caller_parameter.qualifications.iter().any(|qualification| {
                                !boundary_parameter.qualifications.contains(qualification)
                                    || !callee_parameter.qualifications.contains(qualification)
                                    || !signature.qualifications.contains(qualification)
                            })
                            || caller_parameter.qualifications != boundary_parameter.qualifications
                            || caller_parameter.qualifications != callee_parameter.qualifications
                            || caller_parameter.qualifications != signature.qualifications
                        {
                            return Err(LoweringError::InstalledProviderCallShapeMismatch {
                                machine: function.machine,
                                operation: *psi_operation,
                                boundary: *boundary,
                            });
                        }
                        target_arguments.push(TargetStructuralArgument {
                            place: argument.place,
                            access: argument.access,
                            path: Vec::new(),
                            root_structural_type: source.structural_type,
                            structural_type: source.structural_type,
                            shape: source.shape,
                            source_byte_offset: 0,
                            fixed_array_length: None,
                            element_stride: None,
                            source: source.placement.clone(),
                            destination: callee_plan.parameters[index].clone(),
                        });
                    }
                    let claim_transfers = completion_receipts
                        .iter()
                        .map(|receipt| psi_terminal::ClaimTransfer {
                            claim: receipt.claim,
                            argument_index: receipt.argument_index,
                        })
                        .collect::<Vec<_>>();
                    if completion_receipts.len() != callee.entry_claims.len()
                        || callee.entry_claims.iter().any(|entry| {
                            let Some(index) = callee
                                .structural_parameters
                                .iter()
                                .position(|parameter| parameter.place == entry.input)
                            else {
                                return true;
                            };
                            let Some(receipt) = completion_receipts
                                .iter()
                                .find(|receipt| receipt.argument_index as usize == index)
                            else {
                                return true;
                            };
                            let Some(source) = completion_claim_sources
                                .iter()
                                .find(|source| source.claim == receipt.claim)
                            else {
                                return true;
                            };
                            source.entry.as_ref().is_none_or(|source| {
                                source.input != structural_arguments[index].place
                                    || source.path != entry.path
                            })
                        })
                    {
                        return Err(LoweringError::InstalledProviderClaimTransferMismatch {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        });
                    }
                    operations.push(TargetUnitOperation::InstalledProviderCall {
                        psi_operation: *psi_operation,
                        boundary: *boundary,
                        provider: installed.provider.clone(),
                        source_arguments: structural_arguments.clone(),
                        arguments: target_arguments,
                        claim_transfers,
                        completion_claim_sources: completion_claim_sources.clone(),
                        completion_receipts: completion_receipts.clone(),
                    });
                    provenance.operations.push(*psi_operation);
                    continue;
                }
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
                    BoundaryRealization::MetadataOnlyPort(realization) => {
                        if !arguments.is_empty()
                            || !matches!(
                                operations.last(),
                                Some(TargetUnitOperation::PortWrite {
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
                    BoundaryRealization::ClaimCompletionOnly(_) => {
                        if !claim_completion_only_boundary_is_exact(
                            function,
                            declaration,
                            arguments,
                            structural_arguments,
                            completion_claim_sources,
                            completion_receipts,
                            &parameters_by_place,
                        ) {
                            return Err(LoweringError::InvalidClaimCompletionOnlyShape {
                                machine: function.machine,
                                operation: *psi_operation,
                                boundary: *boundary,
                            });
                        }
                    }
                    BoundaryRealization::LinuxWriteLine(_) => {
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
                        byte_sequence_arguments.push(BoundaryByteSequenceArgument {
                            argument: argument.clone(),
                            literal_operation: *literal_operation,
                            structural_type: structural_type.clone(),
                            bytes: bytes.clone(),
                        });
                    }
                    BoundaryRealization::LinuxExitGroupI32(_) => {
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
                        scalar_arguments.push(BoundaryScalarArgument {
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
                    BoundaryRealization::DirectPortReadU8(_) => {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    }
                }
                operations.push(TargetUnitOperation::BoundarySettlement {
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
            AbstractOperation::ReturnUnit {
                psi_edge,
                cleanup_actions,
            } => {
                if nonreturning_boundary && !cleanup_actions.is_empty() {
                    return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
                }
                let local_places = operations
                    .iter()
                    .filter_map(|operation| match operation {
                        TargetUnitOperation::EstablishTrivialAffineLocal { place, .. } => {
                            Some(place.id)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let fully_consumed_affine_pair = exact_fully_consumed_affine_pair_root(
                    function,
                    &parameters,
                    &operations,
                    structural_types,
                    functions,
                );
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
                                    && Some(parameter.place) != fully_consumed_affine_pair
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
                    && (root_discards != expected_roots
                        || operations.iter().any(|operation| {
                            matches!(operation,
                            TargetUnitOperation::Call { arguments, .. }
                                if arguments.iter().any(|argument| {
                                    !argument.path.is_empty()
                                        && root_discards.contains(&argument.place)
                                }))
                        }))
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
                            TargetUnitOperation::Call { arguments, .. } => Some(arguments),
                            _ => None,
                        })
                        .flatten()
                        .filter(|argument| argument.place == residual_root)
                        .collect::<Vec<_>>();
                    let mut moved_subtrees = Vec::with_capacity(moved_arguments.len());
                    if moved_arguments.is_empty()
                        || moved_arguments.iter().any(|argument| {
                            argument.root_structural_type != parameter.structural_type
                                || !is_partial_cleanup_path(&argument.path)
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
                    let fixed_array_call_count = structural_types
                        .get(&parameter.structural_type)
                        .and_then(|declaration| match declaration.shape {
                            StructuralTypeShape::FixedArray { length: 2, .. } => Some(1),
                            StructuralTypeShape::FixedArray { length: 3, .. } => Some(2),
                            _ => None,
                        });
                    if parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine
                        || fixed_array_call_count.is_some_and(|expected_calls| {
                            function.structural_parameters.len() != 1
                                || !function.entry_claims.is_empty()
                                || !function.published_service_ceiling.is_empty()
                                || parameter.position != 0
                                || parameter.is_self
                                || parameter.access != psi_terminal::StructuralAccess::Owned
                                || !parameter.qualifications.is_empty()
                                || !local_places.is_empty()
                                || operations.len() != expected_calls
                                || operations.iter().any(|operation| {
                                    !matches!(operation, TargetUnitOperation::Call { .. })
                                })
                        })
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
                            || cleanup_function.result != AbstractFunctionResult::Unit
                            || !cleanup_function.parameters.is_empty()
                            || !cleanup_function.structural_parameters.is_empty()
                            || !cleanup_function.entry_claims.is_empty()
                            || !cleanup_function.published_service_ceiling.is_empty()
                            || cleanup_function.block_entries.as_slice()
                                != [omega_abstract_operations::AbstractBlockEntry {
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
                operations.push(TargetUnitOperation::Return {
                    psi_edge: *psi_edge,
                    cleanup_actions: cleanup_actions.clone(),
                });
                provenance.edges.push(*psi_edge);
                returned = true;
            }
            AbstractOperation::IntegerConstant {
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
                operations.push(TargetUnitOperation::IntegerConstant {
                    psi_operation: *psi_operation,
                    result: *result,
                    scalar_type: *scalar_type,
                    value: *value,
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::Crash { .. }
            | AbstractOperation::Call { .. }
            | AbstractOperation::CallStructuralScalar { .. }
            | AbstractOperation::CallStructural { .. }
            | AbstractOperation::IntegerConstant { .. }
            | AbstractOperation::BooleanConstant { .. }
            | AbstractOperation::BooleanStructuralField { .. }
            | AbstractOperation::BooleanNot { .. }
            | AbstractOperation::BooleanEqual { .. }
            | AbstractOperation::IntegerEqual { .. }
            | AbstractOperation::IntegerLessThan { .. }
            | AbstractOperation::IntegerLessOrEqual { .. }
            | AbstractOperation::IntegerBitwiseNot { .. }
            | AbstractOperation::IntegerWiden { .. }
            | AbstractOperation::IntegerExactCast { .. }
            | AbstractOperation::IntegerBitwiseAnd { .. }
            | AbstractOperation::IntegerBitwiseOr { .. }
            | AbstractOperation::IntegerBitwiseXor { .. }
            | AbstractOperation::WrappingIntegerShiftLeft { .. }
            | AbstractOperation::WrappingIntegerShiftRight { .. }
            | AbstractOperation::ExactIntegerShiftLeft { .. }
            | AbstractOperation::ExactIntegerShiftRight { .. }
            | AbstractOperation::WrappingIntegerAdd { .. }
            | AbstractOperation::ExactIntegerAdd { .. }
            | AbstractOperation::SaturatingIntegerAdd { .. }
            | AbstractOperation::WrappingIntegerSubtract { .. }
            | AbstractOperation::ExactIntegerSubtract { .. }
            | AbstractOperation::SaturatingIntegerSubtract { .. }
            | AbstractOperation::WrappingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerMultiply { .. }
            | AbstractOperation::SaturatingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerDivide { .. }
            | AbstractOperation::ExactIntegerRemainder { .. }
            | AbstractOperation::WrappingIntegerDivide { .. }
            | AbstractOperation::WrappingIntegerRemainder { .. }
            | AbstractOperation::SaturatingIntegerDivide { .. }
            | AbstractOperation::SaturatingIntegerRemainder { .. }
            | AbstractOperation::Jump { .. }
            | AbstractOperation::Conditional { .. }
            | AbstractOperation::Return { .. }
            | AbstractOperation::ReturnStructural { .. } => {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
        }
    }
    if !returned {
        return Err(LoweringError::FunctionHasNoReturn(function.machine));
    }
    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance,
        operation: TargetOperation::UnitBody(TargetUnitBody {
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

fn claim_completion_only_boundary_is_exact(
    function: &AbstractFunction,
    declaration: &psi_terminal::BoundaryMachineDeclaration,
    scalar_arguments: &[ValueId],
    structural_arguments: &[psi_terminal::StructuralArgument],
    completion_claim_sources: &[CompletionClaimSource],
    completion_receipts: &[psi_terminal::CompletionReceipt],
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
) -> bool {
    if !scalar_arguments.is_empty()
        || !declaration.scalar_parameters.is_empty()
        || declaration.result.is_some()
        || !declaration.program_local_root_introductions.is_empty()
        || !declaration.content_guarantees.is_empty()
        || !declaration.published_service_ceiling.is_empty()
        || structural_arguments.is_empty()
        || structural_arguments.len() != declaration.structural_parameters.len()
        || declaration.requires.iter().any(|requirement| {
            requirement.argument_index as usize >= declaration.structural_parameters.len()
        })
        || completion_receipts.is_empty()
        || completion_claim_sources
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || completion_receipts
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return false;
    }

    for (index, (argument, boundary_parameter)) in structural_arguments
        .iter()
        .zip(&declaration.structural_parameters)
        .enumerate()
    {
        let Some(source) = parameters_by_place.get(&argument.place).copied() else {
            return false;
        };
        let Some(caller_parameter) = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
        else {
            return false;
        };
        let mut expected_qualifications = boundary_parameter.qualifications.clone();
        expected_qualifications.extend(
            declaration
                .requires
                .iter()
                .filter(|requirement| requirement.argument_index as usize == index)
                .map(|requirement| requirement.domain),
        );
        expected_qualifications.sort_unstable();
        expected_qualifications.dedup();
        if !argument.path.is_empty()
            || argument.access != psi_terminal::StructuralAccess::Owned
            || source.access != psi_terminal::StructuralAccess::Owned
            || boundary_parameter.access != psi_terminal::StructuralAccess::Owned
            || source.multiplicity != psi_terminal::StructuralMultiplicity::Linear
            || boundary_parameter.multiplicity != psi_terminal::StructuralMultiplicity::Linear
            || boundary_parameter.position != index as u32
            || source.structural_type != boundary_parameter.structural_type
            || caller_parameter.qualifications != expected_qualifications
        {
            return false;
        }
    }

    let canonical_sources = function
        .entry_claims
        .iter()
        .cloned()
        .map(|entry| CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    if completion_claim_sources != canonical_sources {
        return false;
    }

    let expected = structural_arguments
        .iter()
        .enumerate()
        .flat_map(|(argument_index, argument)| {
            completion_claim_sources.iter().filter_map(move |source| {
                (source.input() == argument.place).then_some((argument_index as u32, source.claim))
            })
        })
        .collect::<BTreeSet<_>>();
    let actual = completion_receipts
        .iter()
        .map(|receipt| (receipt.argument_index, receipt.claim))
        .collect::<BTreeSet<_>>();
    expected == actual && actual.len() == completion_receipts.len()
}

fn validate_scalar_cleanup_frontier(
    caller: MachineId,
    cleanup_actions: &[psi_terminal::TerminalAffineCleanupAction],
    structural_parameters: &[TargetStructuralParameter],
    functions: &BTreeMap<MachineId, &AbstractFunction>,
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

fn finite_boolean_cleanup_return_edges(control: &TargetBooleanControl) -> Option<Vec<EdgeId>> {
    fn collect(
        control: &TargetBooleanControl,
        decision_count: &mut usize,
        return_edges: &mut Vec<EdgeId>,
    ) -> Option<()> {
        match control {
            TargetBooleanControl::ReturnImmediate {
                psi_return_edge, ..
            }
            | TargetBooleanControl::ReturnParameter {
                psi_return_edge, ..
            }
            | TargetBooleanControl::ReturnNotParameter {
                psi_return_edge, ..
            }
            | TargetBooleanControl::ReturnExpression {
                psi_return_edge, ..
            } => return_edges.push(*psi_return_edge),
            TargetBooleanControl::Conditional {
                when_true,
                when_false,
                ..
            }
            | TargetBooleanControl::ConditionalExpression {
                when_true,
                when_false,
                ..
            } => {
                *decision_count = decision_count.checked_add(1)?;
                collect(&when_true.control, decision_count, return_edges)?;
                collect(&when_false.control, decision_count, return_edges)?;
            }
            TargetBooleanControl::Crash { .. } => return None,
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
    function: &AbstractFunction,
    return_edges: &[EdgeId],
    structural_parameters: &[TargetStructuralParameter],
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<Vec<psi_terminal::TerminalAffineCleanupAction>, LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(function.machine);
    let mut returns = BTreeMap::new();
    for operation in &function.operations {
        let AbstractOperation::Return {
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
    cleanup_function: &AbstractFunction,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
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
            AbstractOperation::ReturnUnit { cleanup_actions, .. }
                if cleanup_actions.is_empty())
    {
        return Err(invalid());
    }
    let helper_sites = helper_calls
        .iter()
        .map(|operation| match operation {
            AbstractOperation::CallUnit {
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
            || helper.result != AbstractFunctionResult::Unit
            || !helper.parameters.is_empty()
            || !helper.structural_parameters.is_empty()
            || !helper.entry_claims.is_empty()
            || !helper.published_service_ceiling.is_empty()
            || helper.block_entries.as_slice()
                != [omega_abstract_operations::AbstractBlockEntry {
                    block: helper.entry,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }]
            || !matches!(helper_declaration.shape,
                psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty())
            || !matches!(helper.operations.as_slice(),
                [AbstractOperation::ReturnUnit { cleanup_actions, .. }]
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
            StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. } => {
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
        // Current native targets are 64-bit. The semantic carrier
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
    if moved
        .iter()
        .all(|(path, _)| matches!(path.as_slice(), [StructuralPathSegment::FixedIndex(_)]))
    {
        let declaration = declarations.get(&root_type).copied()?;
        let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
            return None;
        };
        if !matches!((length, moved.len()), (2, 1) | (3, 2))
            || moved.iter().any(|(_, moved_type)| *moved_type != element)
            || !matches!(
                declarations
                    .get(&element)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Record { .. })
            )
        {
            return None;
        }
        let moved_indexes = moved
            .iter()
            .filter_map(|(path, _)| match path.as_slice() {
                [StructuralPathSegment::FixedIndex(index)] if *index < length => Some(*index),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if moved_indexes.len() != moved.len() {
            return None;
        }
        let residuals = (0..length)
            .filter(|index| !moved_indexes.contains(index))
            .map(|index| (vec![StructuralPathSegment::FixedIndex(index)], element))
            .collect::<Vec<_>>();
        return (residuals.len() == 1).then_some(residuals);
    }
    let borrowed = moved
        .iter()
        .map(|(path, structural_type)| (path.as_slice(), *structural_type))
        .collect::<Vec<_>>();
    let mut residuals = Vec::new();
    append_maximal_residual_subtrees(root_type, &[], &borrowed, declarations, &mut residuals)?;
    (!residuals.is_empty()).then_some(residuals)
}

fn is_partial_cleanup_path(path: &[StructuralPathSegment]) -> bool {
    (!path.is_empty()
        && path.iter().all(
            |segment| matches!(segment, StructuralPathSegment::Field(identity) if !identity.is_empty()),
        )) || matches!(path, [StructuralPathSegment::FixedIndex(0 | 1 | 2)])
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
                        | StructuralFieldType::ByteSequence(
                            psi_terminal::ByteSequenceCarrier::BoundedOwned { .. }
                        )
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
    functions: &BTreeMap<MachineId, &AbstractFunction>,
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
            Ok(TargetCallArgument {
                scalar_type: parameter.scalar_type,
                location: scalar_parameter_location(parameter, placement)?,
                expression,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(match scalar_type {
        ScalarType::Boolean => KnownScalar::BooleanRuntime(TargetBooleanExpression::Call {
            psi_operation,
            source_value: result,
            callee,
            arguments,
        }),
        ScalarType::Integer(scalar_type) => KnownScalar::Integer {
            scalar_type,
            value: KnownInteger::Runtime(TargetIntegerExpression::Call {
                psi_operation,
                source_value: result,
                callee,
                arguments,
            }),
        },
    })
}

fn scalar_function_result(function: &AbstractFunction) -> Result<AbstractResult, LoweringError> {
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
    parameter: &AbstractParameter,
    placement: &ValuePlacement,
) -> Result<ScalarParameterLocation, LoweringError> {
    let expected_bytes = scalar_shape(parameter.value, parameter.scalar_type, true)?.byte_size;
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == expected_bytes => Ok(ScalarParameterLocation::Register(*register)),
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == expected_bytes => Ok(ScalarParameterLocation::IncomingStack {
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
    BooleanRuntime(TargetBooleanExpression),
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
            Self::BooleanRuntime(TargetBooleanExpression::Parameter {
                parameter_index,
                location,
                ..
            }) => Self::BooleanRuntime(TargetBooleanExpression::Parameter {
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
    ) -> Result<TargetScalarExpression, LoweringError> {
        Ok(match self {
            Self::Boolean(value) => {
                TargetScalarExpression::Boolean(TargetBooleanExpression::Immediate {
                    source_value,
                    value,
                })
            }
            Self::BooleanRuntime(expression) => TargetScalarExpression::Boolean(expression),
            Self::Integer { scalar_type, value } => TargetScalarExpression::Integer {
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
        KnownScalar::BooleanRuntime(TargetBooleanExpression::Not { operand, .. }) => {
            Ok(KnownScalar::BooleanRuntime(*operand))
        }
        KnownScalar::BooleanRuntime(expression) => {
            Ok(KnownScalar::BooleanRuntime(TargetBooleanExpression::Not {
                psi_operation,
                operand: Box::new(expression),
            }))
        }
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
            KnownScalar::BooleanRuntime(TargetBooleanExpression::Equal {
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
            TargetBooleanExpression::IntegerEqual {
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
                TargetBooleanExpression::IntegerLessOrEqual {
                    psi_operation,
                    scalar_type: left_type,
                    left,
                    right,
                }
            } else {
                TargetBooleanExpression::IntegerLessThan {
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
    expression: TargetBooleanExpression,
    value: ValueId,
) -> Result<(usize, ScalarParameterLocation, bool), LoweringError> {
    match expression {
        TargetBooleanExpression::Parameter {
            parameter_index,
            location,
            ..
        } => Ok((parameter_index, location, false)),
        TargetBooleanExpression::Not { operand, .. } => match *operand {
            TargetBooleanExpression::Parameter {
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
    Runtime(TargetIntegerExpression),
}

impl KnownInteger {
    fn into_expression(self, source_value: ValueId) -> TargetIntegerExpression {
        match self {
            Self::Immediate(value) => TargetIntegerExpression::Immediate {
                source_value,
                value,
            },
            Self::Runtime(expression) => expression,
        }
    }

    fn rebind_direct_parameter(self, source_value: ValueId) -> Self {
        match self {
            Self::Runtime(TargetIntegerExpression::Parameter {
                parameter_index,
                location,
                ..
            }) => Self::Runtime(TargetIntegerExpression::Parameter {
                source_value,
                parameter_index,
                location,
            }),
            value => value,
        }
    }
}

fn conditional_provenance(
    function: &AbstractFunction,
    operations: Vec<psi_core::OperationId>,
    edges: Vec<psi_core::EdgeId>,
) -> TerminalPsiProvenance {
    let mut operations = operations.into_iter().collect::<BTreeSet<_>>();
    let mut edges = edges.into_iter().collect::<BTreeSet<_>>();
    let mut provenance = TerminalPsiProvenance::default();
    for operation in &function.operations {
        let psi_operation = match operation {
            AbstractOperation::EstablishPayloadlessCase { psi_operation, .. }
            | AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. }
            | AbstractOperation::EstablishTrivialAffineLocal { psi_operation, .. }
            | AbstractOperation::CallUnit { psi_operation, .. }
            | AbstractOperation::CallStructuralScalar { psi_operation, .. }
            | AbstractOperation::CallStructural { psi_operation, .. }
            | AbstractOperation::BoundaryCall { psi_operation, .. }
            | AbstractOperation::PortWrite { psi_operation, .. }
            | AbstractOperation::Call { psi_operation, .. }
            | AbstractOperation::IntegerConstant { psi_operation, .. }
            | AbstractOperation::BooleanConstant { psi_operation, .. }
            | AbstractOperation::BooleanStructuralField { psi_operation, .. }
            | AbstractOperation::BooleanNot { psi_operation, .. }
            | AbstractOperation::BooleanEqual { psi_operation, .. }
            | AbstractOperation::IntegerEqual { psi_operation, .. }
            | AbstractOperation::IntegerLessThan { psi_operation, .. }
            | AbstractOperation::IntegerLessOrEqual { psi_operation, .. }
            | AbstractOperation::IntegerBitwiseNot { psi_operation, .. }
            | AbstractOperation::IntegerWiden { psi_operation, .. }
            | AbstractOperation::IntegerExactCast { psi_operation, .. }
            | AbstractOperation::IntegerBitwiseAnd { psi_operation, .. }
            | AbstractOperation::IntegerBitwiseOr { psi_operation, .. }
            | AbstractOperation::IntegerBitwiseXor { psi_operation, .. }
            | AbstractOperation::WrappingIntegerShiftLeft { psi_operation, .. }
            | AbstractOperation::WrappingIntegerShiftRight { psi_operation, .. }
            | AbstractOperation::ExactIntegerShiftLeft { psi_operation, .. }
            | AbstractOperation::ExactIntegerShiftRight { psi_operation, .. }
            | AbstractOperation::WrappingIntegerAdd { psi_operation, .. }
            | AbstractOperation::ExactIntegerAdd { psi_operation, .. }
            | AbstractOperation::SaturatingIntegerAdd { psi_operation, .. }
            | AbstractOperation::WrappingIntegerSubtract { psi_operation, .. }
            | AbstractOperation::ExactIntegerSubtract { psi_operation, .. }
            | AbstractOperation::SaturatingIntegerSubtract { psi_operation, .. }
            | AbstractOperation::WrappingIntegerMultiply { psi_operation, .. }
            | AbstractOperation::ExactIntegerMultiply { psi_operation, .. }
            | AbstractOperation::SaturatingIntegerMultiply { psi_operation, .. } => {
                Some(*psi_operation)
            }
            AbstractOperation::ExactIntegerDivide { psi_operation, .. } => Some(*psi_operation),
            AbstractOperation::ExactIntegerRemainder { psi_operation, .. } => Some(*psi_operation),
            AbstractOperation::WrappingIntegerDivide { psi_operation, .. } => Some(*psi_operation),
            AbstractOperation::WrappingIntegerRemainder { psi_operation, .. } => {
                Some(*psi_operation)
            }
            AbstractOperation::SaturatingIntegerDivide { psi_operation, .. } => {
                Some(*psi_operation)
            }
            AbstractOperation::SaturatingIntegerRemainder { psi_operation, .. } => {
                Some(*psi_operation)
            }
            AbstractOperation::Jump { .. }
            | AbstractOperation::Conditional { .. }
            | AbstractOperation::Return { .. }
            | AbstractOperation::ReturnUnit { .. }
            | AbstractOperation::ReturnStructural { .. }
            | AbstractOperation::Crash { .. } => None,
        };
        if let Some(psi_operation) = psi_operation
            && operations.remove(&psi_operation)
        {
            provenance.operations.push(psi_operation);
        }
        match operation {
            AbstractOperation::Jump { psi_edge, .. }
            | AbstractOperation::Return { psi_edge, .. }
            | AbstractOperation::ReturnUnit { psi_edge, .. }
            | AbstractOperation::ReturnStructural { psi_edge, .. }
            | AbstractOperation::Crash { psi_edge, .. } => {
                if edges.remove(psi_edge) {
                    provenance.edges.push(*psi_edge);
                }
            }
            AbstractOperation::Conditional {
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
    ProviderInstallationIdentityMismatch,
    DuplicateInstalledProviderCall {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    UnknownInstalledProviderCall {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    InstalledProviderCallEvidenceMismatch {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    InstalledProviderCallShapeMismatch {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    InstalledProviderClaimTransferMismatch {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    BoundarySettlementOverlapsInstalledProvider(BoundaryMachineId),
    PartialInstalledProviderBoundary {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    DuplicateBoundarySettlement(BoundaryMachineId),
    UnknownBoundarySettlement(BoundaryMachineId),
    MissingBoundarySettlement(BoundaryMachineId),
    UnusedBoundarySettlement(BoundaryMachineId),
    BoundaryRealizationMismatch(BoundaryMachineId),
    InvalidClaimCompletionOnlyShape {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
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
