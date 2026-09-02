//! Bounded attached-Unit equality diamond ending in two nonreturning exits.

use super::super::shared::*;
use super::boundary_call::lower_boundary_call;
use super::dynamic_scalar::lower_dynamic_scalar_call;
use super::scalar_call::KnownUnitInteger;
use super::scalar_definitions::lower_integer_constant;
use super::structural_scalar::lower_dynamic_argument_scalar_call;

pub(super) fn has_bounded_shape(function: &AbstractFunction) -> bool {
    function.block_entries.len() == 3
        && function.operations.len() == 10
        && function
            .block_entries
            .iter()
            .all(|entry| entry.parameters.is_empty())
        && function.block_entries[0].block == function.entry
        && function.block_entries[0].operation_offset == 0
        && function.block_entries[1].operation_offset == 4
        && function.block_entries[2].operation_offset == 7
        && matches!(
            function.operations[0],
            AbstractOperation::CallDynamicScalar { .. }
                | AbstractOperation::CallStructuralScalarWithDynamicArguments { .. }
        )
        && matches!(
            function.operations[1],
            AbstractOperation::IntegerConstant { .. }
        )
        && matches!(
            function.operations[2],
            AbstractOperation::IntegerEqual { .. }
        )
        && matches!(
            function.operations[3],
            AbstractOperation::Conditional { .. }
        )
        && matches!(
            function.operations[4],
            AbstractOperation::IntegerConstant { .. }
        )
        && matches!(
            function.operations[5],
            AbstractOperation::BoundaryCall { .. }
        )
        && matches!(function.operations[6], AbstractOperation::ReturnUnit { .. })
        && matches!(
            function.operations[7],
            AbstractOperation::IntegerConstant { .. }
        )
        && matches!(
            function.operations[8],
            AbstractOperation::BoundaryCall { .. }
        )
        && matches!(function.operations[9], AbstractOperation::ReturnUnit { .. })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
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
    native_callbacks: &BTreeMap<OperationId, omega_target_operations::TargetNativeCallbackArgument>,
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

    match &function.operations[0] {
        AbstractOperation::CallDynamicScalar { .. } => lower_dynamic_scalar_call(
            &function.operations[0],
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
                &function.operations[0],
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
    }
    lower_constant(
        function,
        &function.operations[1],
        false,
        &mut integer_constants,
        &mut scalar_values,
        &mut operations,
        &mut provenance,
    )?;

    let AbstractOperation::IntegerEqual {
        psi_operation,
        result,
        left,
        right,
    } = function.operations[2]
    else {
        unreachable!("bounded shape fixes the equality operation")
    };
    let AbstractOperation::Conditional {
        condition,
        ref when_true,
        ref when_false,
    } = function.operations[3]
    else {
        unreachable!("bounded shape fixes the conditional operation")
    };
    let true_block = &function.block_entries[1];
    let false_block = &function.block_entries[2];
    if condition != result
        || when_true.target != true_block.block
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
    let left_known = scalar_values
        .get(&left)
        .copied()
        .ok_or(LoweringError::UnknownValue(left))?;
    let right_known = scalar_values
        .get(&right)
        .copied()
        .ok_or(LoweringError::UnknownValue(right))?;
    let scalar_type = left_known.scalar_type();
    if right_known.scalar_type() != scalar_type || scalar_type.bits() != 32 {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let AbstractOperation::ReturnUnit {
        psi_edge: true_return_edge,
        cleanup_actions: true_cleanup,
    } = &function.operations[6]
    else {
        unreachable!("bounded shape fixes the true return")
    };
    let AbstractOperation::ReturnUnit {
        psi_edge: false_return_edge,
        cleanup_actions: false_cleanup,
    } = &function.operations[9]
    else {
        unreachable!("bounded shape fixes the false return")
    };
    if !true_cleanup.is_empty() || !false_cleanup.is_empty() {
        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
    }

    let conditional_ordinal = operations.len();
    let true_ordinal = conditional_ordinal + 2;
    operations.push(TargetUnitOperation::ConditionalIntegerEqual {
        psi_operation,
        result,
        scalar_type,
        left: left_known.into_target_source(left),
        right: right_known.into_target_source(right),
        when_true: omega_target_operations::TargetUnitConditionalSuccessor {
            psi_edge: when_true.psi_edge,
            operation_ordinal: u32::try_from(true_ordinal)
                .map_err(|_| LoweringError::UnsupportedOperationInUnitFunction(function.machine))?,
            nominal_return_edge: *true_return_edge,
        },
        // Patched after the true arm has been lowered.
        when_false: omega_target_operations::TargetUnitConditionalSuccessor {
            psi_edge: when_false.psi_edge,
            operation_ordinal: 0,
            nominal_return_edge: *false_return_edge,
        },
    });
    operations.push(TargetUnitOperation::ConditionalDispatch {
        fallthrough_edge: when_true.psi_edge,
    });
    provenance.operations.push(psi_operation);
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
        4,
        5,
    )?;
    operations.push(TargetUnitOperation::NonreturningTail {
        psi_edge: *true_return_edge,
    });
    let false_ordinal = operations.len();
    let TargetUnitOperation::ConditionalIntegerEqual { when_false, .. } =
        &mut operations[conditional_ordinal]
    else {
        unreachable!("the bounded condition was just inserted")
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
        7,
        8,
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
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
    native_callbacks: &BTreeMap<OperationId, omega_target_operations::TargetNativeCallbackArgument>,
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
