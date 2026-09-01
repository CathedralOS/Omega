use super::boundary_settlements::claim_completion_only_boundary_is_exact;
use super::cleanup::validate_bounded_nominal_cleanup_body;
use super::shared::*;
use super::structural::exact_fully_consumed_affine_pair_root;
use super::structural_layout::{
    expected_maximal_residual_subtrees, is_partial_cleanup_path, structural_shape,
};

mod boundary_call;
mod preflight;
mod return_unit;
mod scalar_call;
mod scalar_definitions;
mod structural_call;
mod structural_scalar;

use boundary_call::lower_boundary_call;
use preflight::validate_unit_function_shape;
use return_unit::lower_unit_return;
use scalar_call::{KnownUnitInteger, insert_known_unit_integer, lower_scalar_call};
use scalar_definitions::{
    lower_ieee_float_constant, lower_ieee_float_fma, lower_integer_constant,
    validate_unit_scalar_definitions,
};
use structural_call::lower_structural_unit_call;
use structural_scalar::{lower_field_store, lower_structural_scalar_call};

pub(super) fn lower_unit_function(
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
    fixed_integer_scalar_abis: &BTreeMap<MachineId, FixedIntegerScalarFunctionAbi>,
    ieee_float_fma: &BTreeMap<OperationId, TargetX86ScalarFmaSettlement>,
    native_callbacks: &BTreeMap<OperationId, omega_target_operations::TargetNativeCallbackArgument>,
) -> Result<TargetFunction, LoweringError> {
    validate_unit_function_shape(function)?;
    validate_unit_scalar_definitions(function)?;
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
                projected_qualifications: parameter.projected_qualifications.clone(),
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
    let mut ieee_float_constants =
        BTreeMap::<ValueId, (OperationId, psi_core::IeeeFloatValue)>::new();
    let mut scalar_values = BTreeMap::<ValueId, KnownUnitInteger>::new();
    let mut nonreturning_boundary = false;
    for operation in &function.operations {
        if returned {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        match operation {
            AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. } => {
                return Err(LoweringError::UnsupportedWriteOnlyPrimitiveStore {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            AbstractOperation::StructuralScalarFieldStore { .. } => lower_field_store(
                operation,
                function,
                structural_types,
                &parameters_by_place,
                &scalar_values,
                &mut shape_cache,
                &mut active,
                &mut operations,
                &mut provenance,
            )?,
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
            AbstractOperation::CallUnit { .. } => lower_structural_unit_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                &parameters_by_place,
                &mut shape_cache,
                &mut active,
                &mut operations,
                &mut provenance,
            )?,
            AbstractOperation::Call { .. } => {
                if nonreturning_boundary {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                lower_scalar_call(
                    operation,
                    function,
                    target,
                    functions,
                    fixed_integer_scalar_abis,
                    &mut scalar_values,
                    &mut operations,
                    &mut provenance,
                )?;
            }
            AbstractOperation::CallStructuralScalar { .. } => lower_structural_scalar_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                &parameters_by_place,
                &mut shape_cache,
                &mut active,
                &mut operations,
                &mut provenance,
            )?,
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
            AbstractOperation::BoundaryCall { .. } => lower_boundary_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                boundary_machines,
                settlements,
                installed_calls,
                native_callbacks,
                &parameters_by_place,
                &mut shape_cache,
                &mut active,
                &established_byte_sequences,
                &integer_constants,
                &mut scalar_values,
                &mut operations,
                &mut provenance,
                &mut nonreturning_boundary,
            )?,
            AbstractOperation::ReturnUnit { .. } => lower_unit_return(
                operation,
                function,
                &parameters,
                &mut operations,
                structural_types,
                functions,
                nonreturning_boundary,
                &mut provenance,
                &mut returned,
            )?,
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type: ScalarType::Integer(scalar_type),
                value,
            } => lower_integer_constant(
                function.machine,
                *psi_operation,
                *result,
                *scalar_type,
                *value,
                nonreturning_boundary,
                &mut integer_constants,
                &mut scalar_values,
                &mut operations,
                &mut provenance,
            )?,
            AbstractOperation::IeeeFloatConstant {
                psi_operation,
                result,
                value,
            } => lower_ieee_float_constant(
                *psi_operation,
                *result,
                *value,
                nonreturning_boundary,
                &mut ieee_float_constants,
                &mut operations,
                &mut provenance,
            )?,
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
                psi_operation,
                result,
                format,
                left,
                right,
                addend,
            } => lower_ieee_float_fma(
                function.machine,
                *psi_operation,
                *result,
                *format,
                *left,
                *right,
                *addend,
                nonreturning_boundary,
                &ieee_float_constants,
                ieee_float_fma,
                &mut operations,
                &mut provenance,
            )?,
            AbstractOperation::Crash { .. }
            | AbstractOperation::CallStructural { .. }
            | AbstractOperation::IntegerConstant { .. }
            | AbstractOperation::BooleanConstant { .. }
            | AbstractOperation::BooleanStructuralField { .. }
            | AbstractOperation::IntegerStructuralField { .. }
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
        fixed_integer_scalar_abi: None,
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
