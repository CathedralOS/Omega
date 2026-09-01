//! Optimizer module role: reconstruction leaf. Current-IR countdown evidence inference.

use super::*;

mod invariant_constants;

pub(super) fn derive(
    unit: &PsiOptimizationUnit,
    snapshot: &OptimizerCycleComponentSnapshot,
) -> Result<Vec<OptimizerUnsignedCountdownRankingCertificate>, OptimizationUnitValidationError> {
    let mut certificates = Vec::new();
    for component in &snapshot.components {
        let function = unit
            .functions
            .iter()
            .find(|function| function.machine == component.id.machine)
            .ok_or_else(|| mismatch(component))?;
        certificates.push(derive_one(function, component)?);
    }
    certificates.sort_by(|left, right| left.component.cmp(&right.component));
    Ok(certificates)
}

fn derive_one(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
) -> Result<OptimizerUnsignedCountdownRankingCertificate, OptimizationUnitValidationError> {
    let mut candidates = component
        .members
        .iter()
        .filter_map(|header| derive_from_header(function, component, *header).ok());
    let certificate = candidates.next().ok_or_else(|| mismatch(component))?;
    if candidates.next().is_some() {
        return Err(mismatch(component));
    }
    Ok(certificate)
}

fn derive_from_header(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    header_id: BlockId,
) -> Result<OptimizerUnsignedCountdownRankingCertificate, ()> {
    let header = block(function, header_id).ok_or(())?;
    let terminator = header.nodes.last().ok_or(())?;
    let O::Conditional {
        condition,
        when_true,
        ..
    } = &terminator.operation
    else {
        return Err(());
    };
    let comparison = scalar_node(header, *condition).ok_or(())?;
    let O::IntegerLessThan {
        psi_operation: comparison_operation,
        result,
        left: zero,
        right: rank_parameter,
    } = comparison.operation
    else {
        return Err(());
    };
    if result != *condition {
        return Err(());
    }
    let rank_type = header
        .parameters
        .iter()
        .find_map(|parameter| (parameter.value == rank_parameter).then_some(parameter.scalar_type))
        .and_then(|scalar_type| match scalar_type {
            ScalarType::Integer(integer) => Some(integer),
            _ => None,
        })
        .filter(|integer| {
            integer.carrier() == IntegerCarrier::Fixed && integer.sign() == IntegerSign::Unsigned
        })
        .ok_or(())?;
    let zero = invariant_constants::resolve(
        function,
        component,
        header_id,
        zero,
        rank_type,
        IntegerValue::Unsigned(0),
    )?;
    let backedge = component
        .id
        .internal_edges
        .iter()
        .copied()
        .find(|edge| edge.target == header_id && edge.source == when_true.target)
        .ok_or(())?;
    if when_true.psi_edge == backedge.edge {
        return Err(());
    }
    let decrement = block(function, backedge.source).ok_or(())?;
    let jump = decrement.nodes.last().ok_or(())?;
    let O::Jump {
        psi_edge,
        target,
        bindings,
        ..
    } = &jump.operation
    else {
        return Err(());
    };
    if *psi_edge != backedge.edge || *target != header_id {
        return Err(());
    }
    let (argument_index, binding) = bindings
        .iter()
        .enumerate()
        .find(|(_, binding)| binding.parameter == rank_parameter)
        .ok_or(())?;
    if binding.scalar_type != ScalarType::Integer(rank_type) {
        return Err(());
    }
    let subtract = scalar_node(decrement, binding.argument).ok_or(())?;
    let O::ExactIntegerSubtract {
        psi_operation: subtract_operation,
        obligation: subtract_obligation,
        result: argument,
        scalar_type: subtract_type,
        left: source_parameter,
        right: one,
    } = subtract.operation
    else {
        return Err(());
    };
    if argument != binding.argument
        || subtract_type != rank_type
        || source_parameter != rank_parameter
    {
        return Err(());
    }
    let one = invariant_constants::resolve(
        function,
        component,
        backedge.source,
        one,
        rank_type,
        IntegerValue::Unsigned(1),
    )?;
    invariant_constants::validate_canonical_preheader_suffix(
        function,
        component,
        header_id,
        [header_id, backedge.source],
        [zero.location, one.location],
    )?;
    Ok(OptimizerUnsignedCountdownRankingCertificate {
        component: component.id.clone(),
        header: header_id,
        rank_parameter,
        rank_type,
        lower_bound: rank_type.minimum_value(),
        upper_bound: rank_type.maximum_value(),
        guard: OptimizerUnsignedPositiveGuard {
            block: header_id,
            edge: when_true.psi_edge,
            condition: *condition,
            parameter: rank_parameter,
            zero: zero.result,
            zero_operation: zero.operation,
            comparison_operation,
        },
        descent: OptimizerUnsignedMinusOneDescent {
            backedge,
            argument_index: u32::try_from(argument_index).map_err(|_| ())?,
            argument,
            source_parameter,
            target_parameter: binding.parameter,
            one: one.result,
            one_operation: one.operation,
            subtract_operation,
            subtract_obligation,
        },
    })
}

fn block(
    function: &PsiOptimizationFunction,
    id: BlockId,
) -> Option<&omega_optimization_unit::OptimizationBlock> {
    function.blocks.iter().find(|block| block.id == id)
}

fn scalar_node(
    block: &omega_optimization_unit::OptimizationBlock,
    value: ValueId,
) -> Option<&omega_optimization_unit::OptimizationNode> {
    block.nodes.iter().find(|node| {
        node.definitions
            .iter()
            .any(|definition| definition.value == value)
    })
}

fn mismatch(component: &OptimizerCycleComponent) -> OptimizationUnitValidationError {
    OptimizationUnitValidationError::RankedCycleRankingEvidenceMismatch {
        machine: component.id.machine,
    }
}
