//! Stateful operation dispatch for an attached Unit body.

use super::super::shared::*;
use super::super::structural_layout::structural_shape;
use super::boundary_call::lower_boundary_call;
use super::dynamic::{
    lower_dynamic_scalar_call, lower_dynamic_unit_call, lower_stored_descriptor,
    lower_stored_dynamic_scalar_call,
};
use super::return_unit::lower_unit_return;
use super::scalar_call::{KnownUnitInteger, lower_scalar_call};
use super::scalar_definitions::{
    lower_boolean_constant, lower_ieee_float_constant, lower_ieee_float_fma, lower_integer_constant,
};
use super::structural_call::{StructuralCallLocalSource, lower_structural_unit_call};
use super::structural_result::lower_structural_result_call;
use super::structural_scalar::{
    lower_dynamic_argument_scalar_call, lower_dynamic_argument_unit_call,
};
use super::structural_scalar::{lower_field_store, lower_structural_scalar_call};
use super::write_only_primitive_store::lower_write_only_primitive_store;

pub(super) struct LoweredUnitBody {
    pub(super) operations: Vec<TargetUnitOperation>,
    pub(super) provenance: TerminalPsiProvenance,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_unit_body(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
    fixed_integer_scalar_abis: &BTreeMap<MachineId, FixedIntegerScalarFunctionAbi>,
    ieee_float_fma: &BTreeMap<OperationId, TargetX86ScalarFmaSettlement>,
    native_callbacks: &BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>,
    scalar_parameters: &[UnitScalarAbiValue],
    parameters: &[TargetStructuralParameter],
) -> Result<LoweredUnitBody, LoweringError> {
    let parameters_by_place = parameters
        .iter()
        .map(|parameter| (parameter.place, parameter))
        .collect::<BTreeMap<_, _>>();
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut operations = Vec::with_capacity(function.operations.len());
    let mut provenance = TerminalPsiProvenance::default();
    let mut returned = false;
    let mut established_byte_sequences =
        BTreeMap::<PlaceId, (OperationId, StructuralTypeDeclaration, Vec<u8>)>::new();
    let mut established_affine_local_sources =
        BTreeMap::<PlaceId, StructuralCallLocalSource>::new();
    let mut integer_constants =
        BTreeMap::<ValueId, (OperationId, IntegerType, IntegerValue)>::new();
    let mut boolean_constants = BTreeMap::<ValueId, (OperationId, bool)>::new();
    let mut ieee_float_constants =
        BTreeMap::<ValueId, (OperationId, semantic_vocabulary::IeeeFloatValue)>::new();
    let mut scalar_values = scalar_parameters
        .iter()
        .enumerate()
        .filter_map(|(parameter_index, parameter)| match parameter.scalar_type {
            ScalarType::Boolean => None,
            ScalarType::Integer(scalar_type) => Some(
                u32::try_from(parameter_index)
                    .map(|parameter_index| {
                        (
                            parameter.value,
                            KnownUnitInteger::Parameter {
                                parameter_index,
                                scalar_type,
                            },
                        )
                    })
                    .map_err(|_| LoweringError::UnitFunctionHasScalarParameters(function.machine)),
            ),
            ScalarType::IeeeFloat(_) => Some(Err(LoweringError::UnitFunctionHasScalarParameters(
                function.machine,
            ))),
        })
        .collect::<Result<BTreeMap<ValueId, KnownUnitInteger>, LoweringError>>()?;
    let mut nonreturning_boundary = false;

    for operation in &function.operations {
        if returned {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        match operation {
            AbstractOperation::DynamicDescriptorParameter { .. } => {}
            AbstractOperation::StoreDynamicDescriptor { .. } => lower_stored_descriptor(
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
            AbstractOperation::WriteOnlyPrimitiveStore { .. } => lower_write_only_primitive_store(
                operation,
                function,
                structural_types,
                &parameters_by_place,
                &scalar_values,
                &boolean_constants,
                &ieee_float_constants,
                &mut operations,
                &mut provenance,
            )?,
            AbstractOperation::StructuralScalarFieldStore { .. } => lower_field_store(
                operation,
                function,
                structural_types,
                &parameters_by_place,
                &scalar_values,
                &boolean_constants,
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
                            semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                                structural_type: place_type,
                                ..
                            },
                            StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView
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
                    place: *place,
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
                let shape = structural_shape(
                    structural_type.id,
                    structural_types,
                    &mut shape_cache,
                    &mut active,
                )?;
                if !matches!(
                    (&place.kind, &structural_type.shape),
                    (
                        semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                            structural_type: place_type,
                            ..
                        },
                        StructuralTypeShape::Record { fields }
                    ) if *place_type == structural_type.id
                        && fields.is_empty()
                        && shape == ValueShape::integer(0, 1)
                ) || established_affine_local_sources
                    .insert(
                        place.id,
                        StructuralCallLocalSource {
                            structural_type: structural_type.id,
                            shape,
                            placement: ValuePlacement {
                                shape,
                                locations: Vec::new(),
                            },
                        },
                    )
                    .is_some()
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                operations.push(TargetUnitOperation::EstablishTrivialAffineLocal {
                    psi_operation: *psi_operation,
                    place: *place,
                    structural_type: structural_type.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::EstablishAffineScalarRecord {
                psi_operation,
                result,
                field,
                value,
            } => {
                let declaration = structural_types
                    .get(&result.structural_type)
                    .copied()
                    .ok_or(LoweringError::UnknownStructuralType(result.structural_type))?;
                let shape = structural_shape(
                    result.structural_type,
                    structural_types,
                    &mut shape_cache,
                    &mut active,
                )?;
                let exact_i64_record = matches!(
                    &declaration.shape,
                    StructuralTypeShape::Record { fields }
                        if matches!(fields.as_slice(), [candidate]
                            if candidate.id == *field
                                && candidate.relevance == terminal_psi::BindingRelevance::Relevant
                                && matches!(candidate.field_type,
                                    StructuralFieldType::Scalar(ScalarType::Integer(integer))
                                        if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                                            && integer.sign() == semantic_vocabulary::IntegerSign::Signed
                                            && integer.bits() == 64))
                );
                let i64_type = IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 64)
                    .expect("signed i64 is valid");
                if !exact_i64_record
                    || shape != ValueShape::integer(8, 8)
                    || result.multiplicity != StructuralMultiplicity::Affine
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                    || !i64_type.admits(*value)
                    || established_affine_local_sources
                        .insert(
                            result.place,
                            StructuralCallLocalSource {
                                structural_type: result.structural_type,
                                shape,
                                placement: ValuePlacement {
                                    shape,
                                    locations: Vec::new(),
                                },
                            },
                        )
                        .is_some()
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                operations.push(TargetUnitOperation::EstablishAffineScalarRecord {
                    psi_operation: *psi_operation,
                    result: result.clone(),
                    field: *field,
                    value: *value,
                    shape,
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
                &established_affine_local_sources,
                &scalar_values,
                &boolean_constants,
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
                &scalar_values,
                &mut shape_cache,
                &mut active,
                &mut operations,
                &mut provenance,
            )?,
            AbstractOperation::CallStructural { .. } => lower_structural_result_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                &parameters_by_place,
                &scalar_values,
                &mut shape_cache,
                &mut active,
                &mut operations,
                &mut provenance,
            )?,
            AbstractOperation::CallStructuralScalarWithDynamicArguments { .. } => {
                let _ = lower_dynamic_argument_scalar_call(
                    operation,
                    function,
                    target,
                    functions,
                    structural_types,
                    &parameters_by_place,
                    &mut shape_cache,
                    &mut active,
                    &mut scalar_values,
                    &mut operations,
                    &mut provenance,
                )?;
            }
            AbstractOperation::CallDynamicScalar { .. } => {
                let _ = lower_dynamic_scalar_call(
                    operation,
                    function,
                    target,
                    functions,
                    structural_types,
                    &parameters_by_place,
                    &mut shape_cache,
                    &mut active,
                    &mut scalar_values,
                    &mut operations,
                    &mut provenance,
                )?;
            }
            AbstractOperation::CallStoredDynamicScalar { .. } => {
                let _ = lower_stored_dynamic_scalar_call(
                    operation,
                    function,
                    target,
                    functions,
                    structural_types,
                    &parameters_by_place,
                    &mut shape_cache,
                    &mut active,
                    &mut scalar_values,
                    &mut operations,
                    &mut provenance,
                )?;
            }
            AbstractOperation::CallDynamicParameterScalar { psi_operation, .. } => {
                return Err(LoweringError::InvalidDynamicDispatch {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            AbstractOperation::CallDynamicUnit { .. } => lower_dynamic_unit_call(
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
            AbstractOperation::CallUnitWithDynamicArguments { .. } => {
                lower_dynamic_argument_unit_call(
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
                )?
            }
            AbstractOperation::CallDynamicParameterUnit { psi_operation, .. } => {
                return Err(LoweringError::UnsupportedDynamicUnitDispatch {
                    machine: function.machine,
                    operation: *psi_operation,
                });
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
                parameters,
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
            AbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value,
            } => lower_boolean_constant(
                function.machine,
                *psi_operation,
                *result,
                *value,
                nonreturning_boundary,
                &mut boolean_constants,
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
            | AbstractOperation::IntegerConstant { .. }
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
            | AbstractOperation::StructuralCase { .. }
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
    Ok(LoweredUnitBody {
        operations,
        provenance,
    })
}
