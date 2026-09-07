//! Bounded attached-Unit equality diamond ending in two nonreturning exits.

use super::super::shared::*;
use super::boundary_call::lower_boundary_call;
use super::dynamic::{
    lower_dynamic_scalar_call, lower_stored_descriptor, lower_stored_dynamic_scalar_call,
};
use super::scalar_call::KnownUnitInteger;
use super::scalar_definitions::lower_integer_constant;
use super::structural_scalar::lower_dynamic_argument_scalar_call;

pub(super) fn has_bounded_shape(function: &AbstractFunction) -> bool {
    let Some(prefix_len) = dynamic_prefix_len(function) else {
        return false;
    };
    let shift = prefix_len - 1;
    let common = function.block_entries.len() == 3
        && function
            .block_entries
            .iter()
            .all(|entry| entry.parameters.is_empty())
        && function.block_entries[0].block == function.entry
        && function.block_entries[0].operation_offset == 0
        && function.operations.len() >= prefix_len;
    if !common {
        return false;
    }
    let integer = function.operations.len() == 10 + shift
        && function.block_entries[1].operation_offset == 4 + shift
        && function.block_entries[2].operation_offset == 7 + shift
        && matches!(
            function.operations[prefix_len],
            AbstractOperation::IntegerConstant { .. }
        )
        && matches!(
            function.operations[prefix_len + 1],
            AbstractOperation::IntegerEqual { .. }
        )
        && matches!(
            function.operations[prefix_len + 2],
            AbstractOperation::Conditional { .. }
        )
        && matches!(
            function.operations[prefix_len + 3],
            AbstractOperation::IntegerConstant { .. }
        )
        && matches!(
            function.operations[prefix_len + 4],
            AbstractOperation::BoundaryCall { .. }
        )
        && matches!(
            function.operations[prefix_len + 5],
            AbstractOperation::ReturnUnit { .. }
        )
        && matches!(
            function.operations[prefix_len + 6],
            AbstractOperation::IntegerConstant { .. }
        )
        && matches!(
            function.operations[prefix_len + 7],
            AbstractOperation::BoundaryCall { .. }
        )
        && matches!(
            function.operations[prefix_len + 8],
            AbstractOperation::ReturnUnit { .. }
        );
    let boolean = function.operations.len() == 8 + shift
        && function.block_entries[1].operation_offset == 2 + shift
        && function.block_entries[2].operation_offset == 5 + shift
        && matches!(
            function.operations[prefix_len],
            AbstractOperation::Conditional { .. }
        )
        && matches!(
            function.operations[prefix_len + 1],
            AbstractOperation::IntegerConstant { .. }
        )
        && matches!(
            function.operations[prefix_len + 2],
            AbstractOperation::BoundaryCall { .. }
        )
        && matches!(
            function.operations[prefix_len + 3],
            AbstractOperation::ReturnUnit { .. }
        )
        && matches!(
            function.operations[prefix_len + 4],
            AbstractOperation::IntegerConstant { .. }
        )
        && matches!(
            function.operations[prefix_len + 5],
            AbstractOperation::BoundaryCall { .. }
        )
        && matches!(
            function.operations[prefix_len + 6],
            AbstractOperation::ReturnUnit { .. }
        );
    common && (integer || boolean)
}

fn dynamic_prefix_len(function: &AbstractFunction) -> Option<usize> {
    match function.operations.as_slice() {
        [
            AbstractOperation::CallDynamicScalar { .. }
            | AbstractOperation::CallStructuralScalarWithDynamicArguments { .. },
            ..,
        ] => Some(1),
        [
            AbstractOperation::StoreDynamicDescriptor { stored, .. },
            AbstractOperation::CallStoredDynamicScalar {
                dynamic_dispatch, ..
            },
            ..,
        ] if *stored == dynamic_dispatch.stored => Some(2),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderCallEvidence,
    >,
    native_callbacks: &BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>,
    parameters: &[TargetStructuralParameter],
) -> Result<super::body::LoweredUnitBody, LoweringError> {
    if !has_bounded_shape(function) || !function.parameters.is_empty() {
        return Err(LoweringError::UnitFunctionNotStraightLine(function.machine));
    }

    let parameters_by_place = parameters
        .iter()
        .map(|parameter| (parameter.place, parameter))
        .collect::<BTreeMap<_, _>>();
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut operations = Vec::with_capacity(8);
    let mut provenance = TerminalPsiProvenance::default();
    let mut integer_constants = BTreeMap::new();
    let mut scalar_values = BTreeMap::new();

    let prefix_len = dynamic_prefix_len(function)
        .ok_or(LoweringError::UnitFunctionNotStraightLine(function.machine))?;
    let result_index = prefix_len - 1;
    let result_home = match &function.operations[result_index] {
        AbstractOperation::CallDynamicScalar { .. } => lower_dynamic_scalar_call(
            &function.operations[result_index],
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
        )?,
        AbstractOperation::CallStructuralScalarWithDynamicArguments { .. } => {
            lower_dynamic_argument_scalar_call(
                &function.operations[result_index],
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
            )?
        }
        AbstractOperation::CallStoredDynamicScalar { .. } => {
            lower_stored_descriptor(
                &function.operations[0],
                function,
                target,
                functions,
                structural_types,
                &parameters_by_place,
                &mut shape_cache,
                &mut active,
                &mut operations,
                &mut provenance,
            )?;
            lower_stored_dynamic_scalar_call(
                &function.operations[result_index],
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
            )?
        }
        _ => unreachable!("bounded shape fixes the dynamic scalar operation"),
    };
    let boolean_control = result_home.scalar_type == ScalarType::Boolean;
    if boolean_control != (function.operations.len() == 8 + (prefix_len - 1)) {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let (
        conditional_index,
        true_constant,
        true_boundary,
        true_return,
        false_constant,
        false_boundary,
        false_return,
    ) = if boolean_control {
        (
            prefix_len,
            prefix_len + 1,
            prefix_len + 2,
            prefix_len + 3,
            prefix_len + 4,
            prefix_len + 5,
            prefix_len + 6,
        )
    } else {
        (
            prefix_len + 2,
            prefix_len + 3,
            prefix_len + 4,
            prefix_len + 5,
            prefix_len + 6,
            prefix_len + 7,
            prefix_len + 8,
        )
    };
    if !boolean_control {
        lower_constant(
            function,
            &function.operations[prefix_len],
            false,
            &mut integer_constants,
            &mut scalar_values,
            &mut operations,
            &mut provenance,
        )?;
    }

    let AbstractOperation::Conditional {
        condition,
        ref when_true,
        ref when_false,
    } = function.operations[conditional_index]
    else {
        unreachable!("bounded shape fixes the conditional operation")
    };
    let true_block = &function.block_entries[1];
    let false_block = &function.block_entries[2];
    if when_true.target != true_block.block
        || when_false.target != false_block.block
        || !when_true.bindings.is_empty()
        || !when_false.bindings.is_empty()
        || !when_true.trivial_affine_discards.is_empty()
        || !when_false.trivial_affine_discards.is_empty()
    {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let AbstractOperation::ReturnUnit {
        psi_edge: true_return_edge,
        cleanup_actions: true_cleanup,
    } = &function.operations[true_return]
    else {
        unreachable!("bounded shape fixes the true return")
    };
    let AbstractOperation::ReturnUnit {
        psi_edge: false_return_edge,
        cleanup_actions: false_cleanup,
    } = &function.operations[false_return]
    else {
        unreachable!("bounded shape fixes the false return")
    };
    if !true_cleanup.is_empty() || !false_cleanup.is_empty() {
        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
    }

    let conditional_ordinal = operations.len();
    let true_ordinal = conditional_ordinal + if boolean_control { 1 } else { 2 };
    let target_when_true = target_operations::TargetUnitConditionalSuccessor {
        psi_edge: when_true.psi_edge,
        operation_ordinal: u32::try_from(true_ordinal)
            .map_err(|_| LoweringError::UnsupportedOperationInUnitFunction(function.machine))?,
        nominal_return_edge: *true_return_edge,
    };
    let target_when_false = target_operations::TargetUnitConditionalSuccessor {
        psi_edge: when_false.psi_edge,
        operation_ordinal: 0,
        nominal_return_edge: *false_return_edge,
    };
    if boolean_control {
        if condition != result_home.source_value {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ));
        }
        operations.push(TargetUnitOperation::ConditionalBoolean {
            condition: result_home,
            when_true: target_when_true,
            when_false: target_when_false,
        });
    } else {
        let AbstractOperation::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        } = function.operations[prefix_len + 1]
        else {
            unreachable!("bounded integer shape fixes the equality operation")
        };
        let left_known = scalar_values
            .get(&left)
            .copied()
            .ok_or(LoweringError::UnknownValue(left))?;
        let right_known = scalar_values
            .get(&right)
            .copied()
            .ok_or(LoweringError::UnknownValue(right))?;
        let scalar_type = left_known.scalar_type();
        if condition != result
            || right_known.scalar_type() != scalar_type
            || scalar_type.bits() != 32
        {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ));
        }
        operations.push(TargetUnitOperation::ConditionalIntegerEqual {
            psi_operation,
            result,
            scalar_type,
            left: left_known.into_target_source(left),
            right: right_known.into_target_source(right),
            when_true: target_when_true,
            when_false: target_when_false,
        });
        provenance.operations.push(psi_operation);
    }
    if !boolean_control {
        operations.push(TargetUnitOperation::ConditionalDispatch {
            fallthrough_edge: when_true.psi_edge,
        });
    }
    provenance
        .edges
        .extend([when_true.psi_edge, when_false.psi_edge]);

    lower_exit_arm(
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
        &mut integer_constants,
        &mut scalar_values,
        &mut operations,
        &mut provenance,
        true_constant,
        true_boundary,
    )?;
    operations.push(TargetUnitOperation::NonreturningTail {
        psi_edge: *true_return_edge,
    });
    let false_ordinal = operations.len();
    let when_false = match &mut operations[conditional_ordinal] {
        TargetUnitOperation::ConditionalIntegerEqual { when_false, .. }
        | TargetUnitOperation::ConditionalBoolean { when_false, .. } => when_false,
        _ => unreachable!("the bounded condition was just inserted"),
    };
    when_false.operation_ordinal = u32::try_from(false_ordinal)
        .map_err(|_| LoweringError::UnsupportedOperationInUnitFunction(function.machine))?;
    lower_exit_arm(
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
        &mut integer_constants,
        &mut scalar_values,
        &mut operations,
        &mut provenance,
        false_constant,
        false_boundary,
    )?;

    operations.push(TargetUnitOperation::Return {
        psi_edge: *false_return_edge,
        cleanup_actions: Vec::new(),
    });
    provenance
        .edges
        .extend([*true_return_edge, *false_return_edge]);

    Ok(super::body::LoweredUnitBody {
        operations,
        provenance,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_exit_arm(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderCallEvidence,
    >,
    native_callbacks: &BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    integer_constants: &mut BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
    constant_index: usize,
    boundary_index: usize,
) -> Result<(), LoweringError> {
    lower_constant(
        function,
        &function.operations[constant_index],
        false,
        integer_constants,
        scalar_values,
        operations,
        provenance,
    )?;
    let mut nonreturning = false;
    lower_boundary_call(
        &function.operations[boundary_index],
        function,
        target,
        functions,
        structural_types,
        boundary_machines,
        settlements,
        installed_calls,
        native_callbacks,
        parameters_by_place,
        shape_cache,
        active,
        &BTreeMap::new(),
        integer_constants,
        scalar_values,
        operations,
        provenance,
        &mut nonreturning,
    )?;
    if !nonreturning {
        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_constant(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    nonreturning: bool,
    integer_constants: &mut BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::IntegerConstant {
        psi_operation,
        result,
        scalar_type: ScalarType::Integer(scalar_type),
        value,
    } = operation
    else {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    };
    lower_integer_constant(
        function.machine,
        *psi_operation,
        *result,
        *scalar_type,
        *value,
        nonreturning,
        integer_constants,
        scalar_values,
        operations,
        provenance,
    )
}
