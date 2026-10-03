use abstract_operations::AbstractStructuralCallResult;
use abstract_operations::{AbstractDynamicCallResult, AbstractDynamicDispatch};
use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy, IntegerShiftPolicy,
};
use optimization_unit::{PsiOptimizationFunction, ValueDefinition};
use semantic_vocabulary::{
    BoundaryMachineId, IntegerCarrier, IntegerType, MachineId, ScalarType, StructuralPlaceKind,
    ValueId,
};
use std::collections::BTreeMap;

mod byte_views;
mod element_views;

pub(crate) fn operation_scalar_types_match(
    function: &PsiOptimizationFunction,
    operation: &O,
    definitions: &BTreeMap<ValueId, ValueDefinition>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
) -> bool {
    let scalar = |value: ValueId| definitions.get(&value).map(|row| row.scalar_type);
    let integer = |value: ValueId, expected: IntegerType| {
        scalar(value) == Some(ScalarType::Integer(expected))
    };
    let ieee_float =
        |value: ValueId, expected| scalar(value) == Some(ScalarType::IeeeFloat(expected));
    let fixed = |integer: IntegerType| integer.carrier() == IntegerCarrier::Fixed;
    let binary = |left: ValueId, right: ValueId, expected: IntegerType| {
        integer(left, expected) && integer(right, expected)
    };
    // Every runtime-selected path element names a defined integer selector
    // the 64-bit address model can extend; its bound stays the operation's
    // verified obligation.
    let selectors_match = operation.runtime_indices().into_iter().all(|(index, _)| {
        matches!(scalar(index), Some(ScalarType::Integer(integer)) if integer.bits() <= 64)
    });
    if !selectors_match {
        return false;
    }
    match operation {
        O::EstablishByteSequenceConcatenation { .. }
        | O::ByteSequenceStore { .. } | O::StructuralByteSequenceFieldStore { .. }
        | O::StructuralByteSequenceFieldByteStore { .. }
        | O::ByteSequenceWrite { .. }
        | O::ByteSequenceRead { .. }
        | O::ByteSequencePredicate { .. }
        | O::ByteSequenceEqual { .. }
        | O::ByteSequenceLength { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::StructuralByteSequenceFieldRead { .. }
        | O::ByteSequenceSubslice { .. } => byte_views::types_match(operation, definitions),
        O::EstablishElementView { .. }
        | O::ElementViewLength { .. }
        | O::ElementViewRead { .. }
        | O::ElementViewWrite { .. }
        | O::ElementViewSubslice { .. } => element_views::types_match(function, operation, definitions),
        O::DynamicDescriptorParameter { parameter } => {
            parameter.owner == function.machine
                && !parameter.trait_identity.is_empty()
                && !parameter.requirements.is_empty()
                && parameter
                    .requirements
                    .iter()
                    .enumerate()
                    .all(|(slot, requirement)| {
                        u32::try_from(slot) == Ok(requirement.slot)
                            && !requirement.declaring_trait_identity.is_empty()
                            && !requirement.public_requirement_identity.is_empty()
                    })
        }
        O::StoreDynamicDescriptor {
            psi_operation,
            stored,
        } => stored.has_complete_custody(function.machine, *psi_operation),
        O::EstablishPrimitiveLocal { value, .. }
        | O::PrimitiveLocalStore { value, .. }
        | O::WriteOnlyPrimitiveStore { value, .. }
        | O::StructuralScalarFieldStore { value, .. } => {
            scalar(value.value) == Some(value.scalar_type)
        }
        O::StructuralCaseMembership { result, .. } => result.scalar_type == ScalarType::Boolean,
        O::AtomicEvent { event, .. } => {
            use abstract_operations::AbstractAtomicEvent as E;
            // Independent recheck, not producer trust: replay the retained
            // ordering legality and result custody, then require every
            // scalar operand and the instruction-observed prior to agree on
            // the resident type the event carries.
            event.ordering_is_legal()
                && event.custody_is_consistent()
                && match event {
                    E::Load { result, .. } => scalar(result.value) == Some(result.scalar_type),
                    E::Store { value, .. } => scalar(*value).is_some(),
                    E::ReadModifyWrite { operand, prior, .. } => {
                        scalar(*operand) == Some(prior.scalar_type)
                            && scalar(prior.value) == Some(prior.scalar_type)
                    }
                    E::Swap { value, prior, .. } => {
                        scalar(*value) == Some(prior.scalar_type)
                            && scalar(prior.value) == Some(prior.scalar_type)
                    }
                    E::CompareExchange {
                        expected,
                        replacement,
                        observed,
                        ..
                    } => {
                        scalar(*expected) == Some(observed.scalar_type)
                            && scalar(*replacement) == Some(observed.scalar_type)
                            && scalar(observed.value) == Some(observed.scalar_type)
                    }
                    E::CompareExchangeOnce {
                        expected,
                        replacement,
                        ..
                    } => scalar(*expected).is_some() && scalar(*expected) == scalar(*replacement),
                    E::Fence { .. } => true,
                }
        }
        O::PrimitiveScalarRead { .. }
        | O::EstablishScalarArray { .. } | O::EstablishStructuralArray { .. }
        | O::EstablishCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::EstablishReference { .. }
        | O::ReleaseReference { .. }
        | O::EstablishRecord { .. }
        // Restoration-window ops carry only structural places; their scalar
        // surface is empty, so there is nothing for this contract to check.
        | O::MoveStructuralField { .. }
        | O::StoreStructuralField { .. }
        | O::StructuralByteViewFieldStore { .. }
        | O::StructuralLeafCopy { .. }
        | O::PortWrite { .. }
        | O::StructuralCase { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => true,
        // The read's result is the field's Boolean or integer type; the
        // structural-root pass matches it against the declared field.
        O::StructuralScalarFieldRead { result, .. } => {
            matches!(
                result.scalar_type,
                ScalarType::Boolean | ScalarType::Integer(_)
            )
        }
        O::IntegerConstant {
            scalar_type, value, ..
        } => match scalar_type {
            ScalarType::Integer(integer) => integer.admits(*value),
            ScalarType::Boolean => false,
            ScalarType::IeeeFloat(_) => false,
        },
        O::IeeeFloatConstant { .. } => true,
        O::FloatResultPolicy { format, policy, .. } => {
            policy.operands().into_iter().all(|value| ieee_float(value, *format))
        }
        O::IeeeFloatCompare {
            format,
            left,
            right,
            ..
        } => ieee_float(*left, *format) && ieee_float(*right, *format),
        O::NearestIeeeFloatFusedMultiplyAdd {
            format,
            left,
            right,
            addend,
            ..
        } => {
            ieee_float(*left, *format)
                && ieee_float(*right, *format)
                && ieee_float(*addend, *format)
        }
        O::BooleanConstant { .. } => true,
        O::BooleanNot { operand, .. } => scalar(*operand) == Some(ScalarType::Boolean),
        O::BooleanEqual { left, right, .. } => {
            scalar(*left) == Some(ScalarType::Boolean)
                && scalar(*right) == Some(ScalarType::Boolean)
        }
        O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. } => {
            match (scalar(*left), scalar(*right)) {
                (Some(ScalarType::Integer(left)), Some(ScalarType::Integer(right))) =>
                    left == right || (!left.is_address() && !right.is_address()),
                _ => false,
            }
        }
        O::IntegerBitwiseNot {
            scalar_type,
            operand,
            ..
        } => integer(*operand, *scalar_type),
        O::IntegerWiden {
            source_type,
            target_type,
            operand,
            ..
        } => integer(*operand, *source_type) && source_type.can_widen_to(*target_type),
        O::ScalarRepresentationView {
            source_type,
            target_type,
            operand,
            ..
        } => {
            scalar(*operand) == Some(*source_type)
                && terminal_semantics::ScalarRepresentationViewPair::classify(
                    *source_type,
                    *target_type,
                )
                .is_some()
        }
        O::IntegerExactCast {
            source_type,
            target_type,
            operand,
            ..
        } => {
            integer(*operand, *source_type)
                && source_type.can_exact_cast_to(*target_type)
                && !source_type.can_widen_to(*target_type)
                && source_type != target_type
        }
        O::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
            ..
        }
        | O::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
            ..
        } => binary(*left, *right, *scalar_type),
        // Exact subtraction alone admits address/count roles. Other
        // proof-bearing pairs still require same-type fixed operands.
        O::IntegerArithmetic {
            scalar_type,
            arithmetic,
            left,
            right,
            ..
        } => {
            if arithmetic.is(IntegerOverflowPolicy::Exact, IntegerArithmeticOperator::Subtract) {
                matches!((scalar(*left), scalar(*right)),
                    (Some(ScalarType::Integer(left)), Some(ScalarType::Integer(right)))
                        if scalar_type.admits_exact_subtract_operands(left, right))
            } else {
                (arithmetic.obligation().is_none() || fixed(*scalar_type))
                    && binary(*left, *right, *scalar_type)
            }
        }
        O::IntegerShift {
            value_type,
            count_type,
            policy,
            value,
            count,
            ..
        } => {
            let carriers_admitted = match policy {
                IntegerShiftPolicy::Wrapping => true,
                IntegerShiftPolicy::Exact { .. } => fixed(*value_type) && fixed(*count_type),
            };
            carriers_admitted && integer(*value, *value_type) && integer(*count, *count_type)
        }
        // A Trapping primitive rejoins both carrier axes: binary operands the
        // result carrier, a shift's value the result carrier and its count
        // the independent operand type, a conversion's operand the source.
        O::TrappingInteger {
            scalar_type,
            operand_type,
            operation,
            ..
        } => {
            use terminal_psi::TrappingIntegerOperation as T;
            fixed(*scalar_type)
                && fixed(*operand_type)
                && match *operation {
                    T::Add { left, right }
                    | T::Subtract { left, right }
                    | T::Multiply { left, right }
                    | T::Divide { left, right }
                    | T::Remainder { left, right } => {
                        operand_type == scalar_type && binary(left, right, *scalar_type)
                    }
                    T::ShiftLeft { value, count } | T::ShiftRight { value, count } => {
                        integer(value, *scalar_type) && integer(count, *operand_type)
                    }
                    T::Convert { operand } => integer(operand, *operand_type),
                }
        }
        O::Jump { .. } => true,
        O::Conditional { condition, .. } => scalar(*condition) == Some(ScalarType::Boolean),
        O::Return {
            result,
            value,
            scalar_type,
            ..
        } => {
            scalar(*value) == Some(*scalar_type)
                && matches!(
                    function.result,
                    abstract_operations::AbstractFunctionResult::Scalar(signature)
                        if signature.value == *result && signature.scalar_type == *scalar_type
                )
        }
        O::Call {
            result: _,
            scalar_type,
            callee,
            arguments,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            // Declared places include locals, not only call inputs. Each
            // function's catalog, producers and ownership are independently
            // replayed; scalar calls forbid boundary roots, not local storage.
            callee.structural_parameters.is_empty()
                && callee.entry_claim_declarations.is_empty()
                && !callee.structural_places.iter().any(|place| {
                    matches!(
                        place.kind,
                        StructuralPlaceKind::Parameter { .. }
                            | StructuralPlaceKind::Result
                            | StructuralPlaceKind::ProviderAttachment { .. }
                    )
                })
                && matches!(
                    callee.result,
                    abstract_operations::AbstractFunctionResult::Scalar(signature)
                        if signature.scalar_type == *scalar_type
                )
                && arguments.len() == callee.parameters.len()
                && arguments
                    .iter()
                    .zip(&callee.parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(parameter.scalar_type))
        }),
        O::CallStructural {
            callee,
            arguments,
            dynamic_arguments,
            result: AbstractStructuralCallResult::Unit,
            ..
        } if dynamic_arguments.is_empty() => functions.get(callee).is_some_and(|callee| {
            matches!(
                callee.result,
                abstract_operations::AbstractFunctionResult::Unit
            ) && arguments.len() == callee.parameters.len()
                && arguments
                    .iter()
                    .zip(&callee.parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(parameter.scalar_type))
        }),
        // A call that forwards descriptors carries no scalar arguments, and
        // its callee declares none.
        O::CallStructural {
            psi_operation,
            callee,
            arguments,
            dynamic_arguments,
            result: AbstractStructuralCallResult::Unit,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            arguments.is_empty()
                && callee.parameters.is_empty()
                && matches!(
                    callee.result,
                    abstract_operations::AbstractFunctionResult::Unit
                )
                && dynamic_arguments_match(function, *psi_operation, callee, dynamic_arguments)
        }),
        O::CallStructural {
            result: AbstractStructuralCallResult::Scalar(result),
            callee,
            arguments,
            dynamic_arguments,
            ..
        } if dynamic_arguments.is_empty() => functions.get(callee).is_some_and(|callee| {
            arguments.len() == callee.parameters.len()
                && arguments
                    .iter()
                    .zip(&callee.parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(parameter.scalar_type))
                && matches!(
                    callee.result,
                    abstract_operations::AbstractFunctionResult::Scalar(signature)
                        if signature.scalar_type == result.scalar_type
                )
        }),
        O::CallStructural {
            psi_operation,
            result: AbstractStructuralCallResult::Scalar(result),
            callee,
            arguments,
            dynamic_arguments,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            arguments.is_empty()
                && callee.parameters.is_empty()
                && matches!(
                callee.result,
                abstract_operations::AbstractFunctionResult::Scalar(signature)
                        if signature.scalar_type == result.scalar_type
                )
                && dynamic_arguments_match(function, *psi_operation, callee, dynamic_arguments)
        }),
        O::CallDynamic {
            psi_operation,
            dispatch,
            result,
            ..
        } => dynamic_call_types_match(function, *psi_operation, dispatch, *result, functions),
        O::CallStructural {
            callee,
            arguments,
            result: AbstractStructuralCallResult::Structural { .. },
            ..
        } => functions.get(callee).is_some_and(|callee| {
            callee.parameters.len() == arguments.len()
                && callee
                    .parameters
                    .iter()
                    .zip(arguments)
                    .all(|(parameter, argument)| scalar(*argument) == Some(parameter.scalar_type))
                && matches!(
                    callee.result,
                    abstract_operations::AbstractFunctionResult::Structural(_)
                )
        }),
        O::BoundaryCall {
            result,
            boundary,
            arguments,
            ..
        } => boundary_machines.get(boundary).is_some_and(|boundary| {
            (match (&boundary.result, result) {
                (
                    terminal_psi::BoundaryMachineResult::Unit,
                    abstract_operations::AbstractBoundaryResult::Unit,
                ) => true,
                (
                    terminal_psi::BoundaryMachineResult::Scalar(expected),
                    abstract_operations::AbstractBoundaryResult::Scalar(actual),
                ) => actual.scalar_type == *expected,
                (
                    terminal_psi::BoundaryMachineResult::Structural(expected),
                    abstract_operations::AbstractBoundaryResult::Structural(actual),
                ) => {
                    actual.structural_type == expected.structural_type
                        && actual.multiplicity == expected.multiplicity
                        && actual.qualifications == expected.qualifications
                        && actual.projected_qualifications.is_empty()
                        && actual.claims.is_empty()
                }
                _ => false,
            }) && arguments.len() == boundary.scalar_parameters.len()
                && arguments
                    .iter()
                    .zip(&boundary.scalar_parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(*parameter))
        }),
    }
}

fn dynamic_arguments_match(
    function: &PsiOptimizationFunction,
    psi_operation: semantic_vocabulary::OperationId,
    callee: &PsiOptimizationFunction,
    dynamic_arguments: &[abstract_operations::AbstractDynamicDescriptorArgument],
) -> bool {
    !dynamic_arguments.is_empty()
        && dynamic_arguments.iter().all(|argument| {
            argument.has_complete_custody(function.machine, psi_operation, callee.machine)
                && callee
                    .blocks
                    .iter()
                    .find(|block| block.id == callee.entry)
                    .is_some_and(|entry| {
                        entry.nodes.iter().any(|node| {
                            matches!(
                                &node.operation,
                                O::DynamicDescriptorParameter { parameter }
                                    if parameter == &argument.target
                            )
                        })
                    })
        })
        && dynamic_arguments
            .windows(2)
            .all(|pair| pair[0].target.ordinal < pair[1].target.ordinal)
}

/// One descriptor call's custody, selected callable row and realization
/// signature agree with its result role. Each custody route keeps its own
/// replay: a rebound or stored descriptor names its realization, while a
/// received parameter exposes only the closed interface slot.
fn dynamic_call_types_match(
    function: &PsiOptimizationFunction,
    psi_operation: semantic_vocabulary::OperationId,
    dispatch: &AbstractDynamicDispatch,
    result: AbstractDynamicCallResult,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
) -> bool {
    let Some(expected) = dynamic_result_class(result) else {
        return false;
    };
    match dispatch {
        AbstractDynamicDispatch::Rebound(dynamic_dispatch) => functions
            .get(&dynamic_dispatch.dispatch.realization)
            .is_some_and(|callee| {
                dynamic_dispatch.has_complete_application_custody(function.machine, psi_operation)
                    && rebound_result_matches(dynamic_dispatch, expected)
                    && realization_signature_matches(callee, result)
            }),
        // Stage 00 never produces stored custody with a Unit result.
        AbstractDynamicDispatch::Stored(_) if result.is_unit() => false,
        AbstractDynamicDispatch::Stored(dynamic_dispatch) => functions
            .get(&dynamic_dispatch.dispatch.realization)
            .is_some_and(|callee| {
                dynamic_dispatch.has_complete_custody(function.machine, psi_operation)
                    && stored_result_matches(dynamic_dispatch, expected)
                    && realization_signature_matches(callee, result)
            }),
        AbstractDynamicDispatch::Parameter(dynamic_dispatch) => {
            let parameter = &dynamic_dispatch.parameter;
            let dispatch = &dynamic_dispatch.dispatch;
            let requirements = parameter
                .requirements
                .iter()
                .filter(|requirement| requirement.slot == dispatch.requirement_slot)
                .collect::<Vec<_>>();
            matches!(requirements.as_slice(), [requirement]
                if parameter.owner == function.machine
                    && dispatch.owner == function.machine
                    && dispatch.operation == psi_operation
                    && dispatch.parameter_ordinal == parameter.ordinal
                    && requirement.result == expected)
        }
    }
}

/// The closed callable-result class a dynamic call's result role names, or
/// `None` for a scalar type no callable row can return.
fn dynamic_result_class(
    result: AbstractDynamicCallResult,
) -> Option<terminal_psi::ClosedConformanceCallableResult> {
    match result {
        AbstractDynamicCallResult::Unit => {
            Some(terminal_psi::ClosedConformanceCallableResult::Unit)
        }
        AbstractDynamicCallResult::Scalar(result) => scalar_result_class(result.scalar_type),
    }
}

/// A realization receives only the descriptor's instance and returns exactly
/// the call's result role.
fn realization_signature_matches(
    callee: &PsiOptimizationFunction,
    result: AbstractDynamicCallResult,
) -> bool {
    callee.parameters.is_empty()
        && callee.structural_parameters.len() == 1
        && match (&callee.result, result) {
            (
                abstract_operations::AbstractFunctionResult::Unit,
                AbstractDynamicCallResult::Unit,
            ) => true,
            (
                abstract_operations::AbstractFunctionResult::Scalar(signature),
                AbstractDynamicCallResult::Scalar(result),
            ) => signature.scalar_type == result.scalar_type,
            _ => false,
        }
}

fn rebound_result_matches(
    dynamic_dispatch: &abstract_operations::AbstractReboundDynamicDispatch,
    expected: terminal_psi::ClosedConformanceCallableResult,
) -> bool {
    dynamic_dispatch
        .application
        .realization_callables
        .iter()
        .find(|callable| {
            callable.source_callable_identity
                == dynamic_dispatch.dispatch.realization_callable_identity
                && callable.machine == dynamic_dispatch.dispatch.realization
        })
        .is_some_and(|callable| callable.result == expected)
}

fn stored_result_matches(
    dynamic_dispatch: &abstract_operations::AbstractStoredDynamicDispatch,
    expected: terminal_psi::ClosedConformanceCallableResult,
) -> bool {
    dynamic_dispatch
        .stored
        .application
        .realization_callables
        .iter()
        .find(|callable| {
            callable.source_callable_identity
                == dynamic_dispatch.dispatch.realization_callable_identity
                && callable.machine == dynamic_dispatch.dispatch.realization
        })
        .is_some_and(|callable| callable.result == expected)
}

fn scalar_result_class(
    scalar_type: ScalarType,
) -> Option<terminal_psi::ClosedConformanceCallableResult> {
    if scalar_type == ScalarType::Boolean {
        Some(terminal_psi::ClosedConformanceCallableResult::Bool)
    } else if scalar_type
        == ScalarType::Integer(
            IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32).expect("i32 is valid"),
        )
    {
        Some(terminal_psi::ClosedConformanceCallableResult::I32)
    } else {
        None
    }
}
