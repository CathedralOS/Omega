//! Bounded Boolean split whose two predecessors supply distinct dynamic
//! descriptor arguments to one shared scalar or result-less helper.

use super::super::shared::*;
use super::body::LoweredUnitBody;
use super::scalar_call::KnownUnitInteger;
use super::structural_scalar::{
    lower_dynamic_argument_scalar_call, lower_dynamic_argument_unit_call,
};

pub(super) fn has_bounded_shape(function: &AbstractFunction) -> bool {
    matches!(
        function.parameters.as_slice(),
        [AbstractParameter {
            scalar_type: ScalarType::Boolean,
            ..
        }]
    ) && function.attachment.is_some()
        && matches!(function.result, AbstractFunctionResult::Unit)
        && function.entry_claims.is_empty()
        && function.published_service_ceiling.is_empty()
        && function.block_entries.len() == 3
        && function.block_entries[0].block == function.entry
        && function.block_entries[0].parameters.is_empty()
        && function.block_entries[0].operation_offset == 0
        && function.block_entries[1].parameters.is_empty()
        && function.block_entries[1].operation_offset == 1
        && function.block_entries[2].parameters.is_empty()
        && function.block_entries[2].operation_offset == 3
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::Conditional { .. },
                AbstractOperation::CallStructuralScalarWithDynamicArguments { .. },
                AbstractOperation::ReturnUnit { .. },
                AbstractOperation::CallStructuralScalarWithDynamicArguments { .. },
                AbstractOperation::ReturnUnit { .. },
            ] | [
                AbstractOperation::Conditional { .. },
                AbstractOperation::CallUnitWithDynamicArguments { .. },
                AbstractOperation::ReturnUnit { .. },
                AbstractOperation::CallUnitWithDynamicArguments { .. },
                AbstractOperation::ReturnUnit { .. },
            ]
        )
}

pub(super) fn lower(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    scalar_parameters: &[UnitScalarAbiValue],
    parameters: &[TargetStructuralParameter],
) -> Result<LoweredUnitBody, LoweringError> {
    if !has_bounded_shape(function) {
        return Err(LoweringError::UnitFunctionNotStraightLine(function.machine));
    }
    let [condition_parameter] = scalar_parameters else {
        return Err(LoweringError::UnitFunctionHasScalarParameters(
            function.machine,
        ));
    };
    let AbstractOperation::Conditional {
        condition,
        when_true,
        when_false,
    } = &function.operations[0]
    else {
        unreachable!("bounded join fixes its entry operation")
    };
    let true_block = &function.block_entries[1];
    let false_block = &function.block_entries[2];
    if *condition != condition_parameter.value
        || condition_parameter.scalar_type != ScalarType::Boolean
        || condition_parameter.placement.shape != ValueShape::integer(1, 1)
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
    let (true_return_edge, true_cleanup) = return_parts(&function.operations[2]);
    let (false_return_edge, false_cleanup) = return_parts(&function.operations[4]);
    let (Some(true_return_edge), Some(false_return_edge)) = (true_return_edge, false_return_edge)
    else {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    };
    if !true_cleanup.is_empty() || !false_cleanup.is_empty() {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }

    let parameters_by_place = parameters
        .iter()
        .map(|parameter| (parameter.place, parameter))
        .collect::<BTreeMap<_, _>>();
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut scalar_values = BTreeMap::<ValueId, KnownUnitInteger>::new();
    let mut operations = Vec::with_capacity(function.operations.len());
    let mut provenance = TerminalPsiProvenance::default();

    operations.push(TargetUnitOperation::ConditionalBooleanParameter {
        condition: condition_parameter.clone(),
        when_true: target_operations::TargetUnitConditionalSuccessor {
            psi_edge: when_true.psi_edge,
            operation_ordinal: 1,
            nominal_return_edge: true_return_edge,
        },
        when_false: target_operations::TargetUnitConditionalSuccessor {
            psi_edge: when_false.psi_edge,
            operation_ordinal: 3,
            nominal_return_edge: false_return_edge,
        },
    });
    provenance
        .edges
        .extend([when_true.psi_edge, when_false.psi_edge]);

    lower_branch_call(
        &function.operations[1],
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
    operations.push(TargetUnitOperation::Return {
        psi_edge: true_return_edge,
        cleanup_actions: Vec::new(),
    });
    provenance.edges.push(true_return_edge);

    lower_branch_call(
        &function.operations[3],
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
    operations.push(TargetUnitOperation::Return {
        psi_edge: false_return_edge,
        cleanup_actions: Vec::new(),
    });
    provenance.edges.push(false_return_edge);

    Ok(LoweredUnitBody {
        operations,
        provenance,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_branch_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    match operation {
        AbstractOperation::CallStructuralScalarWithDynamicArguments { .. } => {
            lower_dynamic_argument_scalar_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                parameters_by_place,
                shape_cache,
                active,
                scalar_values,
                operations,
                provenance,
            )?;
        }
        AbstractOperation::CallUnitWithDynamicArguments { .. } => {
            lower_dynamic_argument_unit_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                parameters_by_place,
                shape_cache,
                active,
                operations,
                provenance,
            )?;
        }
        _ => {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ));
        }
    }
    Ok(())
}

fn return_parts(operation: &AbstractOperation) -> (Option<EdgeId>, &[TerminalAffineCleanupAction]) {
    match operation {
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions,
        } => (Some(*psi_edge), cleanup_actions),
        _ => (None, &[]),
    }
}
