use abstract_operations::AbstractStructuralCallResult;
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
    unit: &optimization_unit::PsiOptimizationUnit,
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
    let paths = operation.structural_paths();
    // Returned-origin interpretations describe backing only. An executable
    // operand must use its actual established view carrier, never a recipe
    // that would introduce observation authority without formation.
    if paths.iter().flat_map(|path| path.iter()).any(|segment| {
        matches!(
            segment,
            terminal_psi::StructuralPathSegment::RepresentationView { .. }
        )
    }) {
        return false;
    }
    let selectors_match = paths.into_iter().flatten()
        .filter_map(terminal_psi::StructuralPathSegment::runtime_index).all(|(index, _)| {
        matches!(scalar(index), Some(ScalarType::Integer(integer)) if integer.bits() <= 64)
    });
    if !selectors_match {
        return false;
    }
    match operation {
        O::EstablishPlacedReference { .. } => true,
        O::EstablishReference { storage, .. } => storage.as_ref().is_none_or(|view| {
            matches!(scalar(view.byte_offset), Some(ScalarType::Integer(integer))
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                    && integer.bits() == 64 && fixed(integer))
        }),
        O::SelectImplementation { result, table, .. } => {
            matches!(result.scalar_type, ScalarType::Integer(integer)
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                    && integer.bits() == 64 && fixed(integer))
                && unit.dynamic_dispatch.table(*table).is_some()
        }
        O::CallStructural {
            callee: abstract_operations::AbstractCallTarget::Dynamic { selector, interface, slot },
            arguments,
            result,
            ..
        } => {
            let Some(signature) = unit.dynamic_dispatch.interface(*interface)
                .and_then(|interface| usize::try_from(*slot).ok().and_then(|slot| interface.slots.get(slot)))
            else { return false; };
            matches!(scalar(*selector), Some(ScalarType::Integer(integer))
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                    && integer.bits() == 64 && fixed(integer))
                && arguments.len() == signature.scalar_parameters.len()
                && arguments.iter().zip(&signature.scalar_parameters)
                    .all(|(argument, expected)| scalar(*argument) == Some(*expected))
                && match result {
                    AbstractStructuralCallResult::Unit => signature.result.is_unit(),
                    AbstractStructuralCallResult::Scalar(result) => signature.result.scalar() == Some(result.scalar_type),
                    AbstractStructuralCallResult::Structural { .. } => matches!(signature.result, terminal_psi::BoundaryMachineResult::Structural(_)),
                }
        }
        O::EstablishByteSequenceConcatenation { .. }
        | abstract_operations::byte_sequence_store_pair_pattern!( .. )
        | abstract_operations::byte_sequence_write_pair_pattern!(..)
        | abstract_operations::byte_read_predicate_equal_pattern!(..)
        | O::ByteSequenceLength { .. }
        | abstract_operations::byte_len_or_field_read_pattern!(..)
        | O::ByteSequenceSubslice { .. } => byte_views::types_match(operation, definitions),
        O::EstablishElementView { .. }
        | O::ElementViewLength { .. }
        | abstract_operations::element_view_read_write_pair_pattern!(..)
        | O::ElementViewSubslice { .. } => element_views::types_match(function, operation, definitions),
        abstract_operations::primitive_local_triple_pattern!(value, ..)
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
        | O::EstablishScalarArray { .. } | abstract_operations::structural_array_or_case_pattern!(..)
        | abstract_operations::byte_literal_establish_pair_pattern!(..)
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::ReleaseReference { .. }
        | O::EstablishRecord { .. }
        // Restoration-window ops carry only structural places; their scalar
        // surface is empty, so there is nothing for this contract to check.
        | abstract_operations::move_or_store_structural_field_pattern!()
        | O::StructuralByteViewFieldStore { .. }
        | O::StructuralLeafCopy { .. }
        | O::PortWrite { .. }
        | O::StructuralCase { .. }
        | O::ReturnUnit { .. }
        | abstract_operations::return_structural_or_crash_pattern!() => true,
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
            policy.accepts_operand_types(*format, scalar)
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
        abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
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
        | abstract_operations::bitwise_or_xor_pair_pattern!(scalar_type,
            left,
            right, ..) => binary(*left, *right, *scalar_type),
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
                    terminal_psi::trapping_arith_set_pattern!(left,right, ..) => {
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
            result: AbstractStructuralCallResult::Unit,
            ..
        } => callee.machine().and_then(|machine| functions.get(&machine)).is_some_and(|callee| {
            matches!(
                callee.result,
                abstract_operations::AbstractFunctionResult::Unit
            ) && arguments.len() == callee.parameters.len()
                && arguments
                    .iter()
                    .zip(&callee.parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(parameter.scalar_type))
        }),
        O::CallStructural {
            result: AbstractStructuralCallResult::Scalar(result),
            callee,
            arguments,
            ..
        } => callee.machine().and_then(|machine| functions.get(&machine)).is_some_and(|callee| {
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
            callee,
            arguments,
            result: AbstractStructuralCallResult::Structural { .. },
            ..
        } => callee.machine().and_then(|machine| functions.get(&machine)).is_some_and(|callee| {
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
        O::ExternalCompletion { boundary, kind, arguments, .. } => boundary_machines.get(boundary).is_some_and(|boundary| {
            boundary.completion == semantic_vocabulary::BoundaryCompletion::External(*kind)
                && boundary.result.is_unit()
                && arguments.len() == boundary.scalar_parameters.len()
                && arguments.iter().zip(&boundary.scalar_parameters).all(|(argument, parameter)| scalar(*argument) == Some(*parameter))
        }),
        O::BoundaryCall {
            result,
            boundary,
            arguments,
            ..
        } => boundary_machines.get(boundary).is_some_and(|boundary| {
            boundary.completion == semantic_vocabulary::BoundaryCompletion::NormalReturn && (match (&boundary.result, result) {
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
                        && actual.projected_qualifications == expected.projected_qualifications
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
