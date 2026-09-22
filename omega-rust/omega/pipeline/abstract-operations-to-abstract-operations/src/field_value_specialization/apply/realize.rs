//! Optimizer module role: application leaf. Exact observation resolution and derived-coordinate refresh.
//!
//! A `Constant` row replaces its observation node with a
//! `BooleanConstant`/`IntegerConstant` carrying the proven stored value: the
//! folded node keeps the read's Psi operation custody —
//! `expected_provenance` derives `PsiProvenance::Operation(psi_operation)`
//! for both operations — so the provenance roster, fuel settlements, effect
//! link, value definition, successors, and ownership events are unchanged.
//! A `Forward` row instead rebinds every covered scalar-operand use of the
//! read's result to the proven initializer, then retires the observation
//! node: the node that inherits the vacated index absorbs the read's
//! provenance and fuel settlement, and the function's derived metadata —
//! definitions, uses, successor custody, ownership, effects, facts, and
//! declared places — is restamped from operation shape.

use super::super::{
    FieldValuePlan, FieldValueSpecializationError, O, PsiOptimizationFunction, PsiOptimizationUnit,
    ResolvedFieldValue, ScalarType, admission, recompute_psi_optimization_unit_identity,
};
use super::substitution::rewrite_scalar_uses;
use optimization_unit::{
    FieldValueResolution, FoldedFieldValue, OptimizationBlock, OptimizationNode,
};
use semantic_vocabulary::BlockId;
use std::collections::{BTreeMap, BTreeSet};

/// The folded node shape a `Constant` resolution admits at its site: a
/// `BooleanConstant`/`IntegerConstant` with the read's own custody identity
/// and result value. Application and the independent custody walk share this
/// construction so both name exactly the same admitted difference. `Forward`
/// rows retire their node instead of folding it, so this construction never
/// applies to them.
pub(crate) fn folded_node(
    row: &ResolvedFieldValue,
    function: &PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    analysis: &admission::FunctionAnalysis,
) -> Result<OptimizationNode, FieldValueSpecializationError> {
    let (block, node_index, node) = site_node(function, row)?;
    let (psi_operation, result, scalar_type, source, path, field, kind) = match &node.operation {
        O::BooleanStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => (
            psi_operation,
            *result,
            ScalarType::Boolean,
            source,
            path,
            field,
            admission::ObservedFieldKind::Boolean,
        ),
        O::IntegerStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => (
            psi_operation,
            result.value,
            result.scalar_type,
            source,
            path,
            field,
            admission::ObservedFieldKind::Integer,
        ),
        _ => return Err(FieldValueSpecializationError::CandidateMismatch),
    };
    if row.site.machine != function.machine
        || *psi_operation != row.psi_operation
        || result != row.result
        || *source != row.source
        || path.as_slice() != row.path.as_slice()
        || *field != row.field
    {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    }
    // The claimed basis must re-derive under the place's own evidence: the
    // establishment witness and resolution must equal the row's.
    let Some(evidence) = admission::field_evidence(function, row.source) else {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    };
    let proven = admission::proven_field_value(
        unit,
        &evidence,
        function,
        analysis,
        block,
        node_index,
        node,
        result,
        scalar_type,
        path,
        *field,
        kind,
    )
    .ok_or(FieldValueSpecializationError::CandidateMismatch)?;
    if proven.producer != row.producer || proven.resolution != row.resolution {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    }
    let operation = match &row.resolution {
        FieldValueResolution::Constant(FoldedFieldValue::Boolean(value)) => O::BooleanConstant {
            psi_operation: *psi_operation,
            result,
            value: *value,
        },
        FieldValueResolution::Constant(FoldedFieldValue::Integer(value)) => O::IntegerConstant {
            psi_operation: *psi_operation,
            result,
            scalar_type,
            value: *value,
        },
        FieldValueResolution::Forward(_) => {
            return Err(FieldValueSpecializationError::CandidateMismatch);
        }
    };
    Ok(OptimizationNode {
        operation,
        provenance: node.provenance.clone(),
        fuel: node.fuel.clone(),
        effect: node.effect,
        definitions: node.definitions.clone(),
        uses: node.uses.clone(),
        successors: node.successors.clone(),
        ownership: node.ownership.clone(),
    })
}

/// The node a row's site names, with its block and index. `MissingSite` when
/// the block or coordinate does not exist; `CoordinateOverflow` when the
/// claimed index does not fit.
fn site_node<'a>(
    function: &'a PsiOptimizationFunction,
    row: &ResolvedFieldValue,
) -> Result<(&'a OptimizationBlock, usize, &'a OptimizationNode), FieldValueSpecializationError> {
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == row.site.block)
        .ok_or(FieldValueSpecializationError::MissingSite {
            machine: function.machine,
            block: row.site.block,
            node: row.site.node,
        })?;
    let node_index = usize::try_from(row.site.node)
        .map_err(|_| FieldValueSpecializationError::CoordinateOverflow)?;
    let node = block
        .nodes
        .get(node_index)
        .ok_or(FieldValueSpecializationError::MissingSite {
            machine: function.machine,
            block: row.site.block,
            node: row.site.node,
        })?;
    Ok((block, node_index, node))
}

/// Re-verifies a `Forward` row against the input function: the site must name
/// the claimed observation and re-admission must produce exactly the claimed
/// row — initializer, scalar type, and use-site roster included.
fn admitted_forward_row(
    row: &ResolvedFieldValue,
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    analysis: &admission::FunctionAnalysis,
) -> Result<(), FieldValueSpecializationError> {
    let Some(evidence) = admission::field_evidence(function, row.source) else {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    };
    let (block, node_index, node) = site_node(function, row)?;
    let admitted =
        admission::admit_field_node(unit, &evidence, function, analysis, block, node_index, node)
            .ok_or(FieldValueSpecializationError::CandidateMismatch)?;
    if admitted != *row {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    }
    Ok(())
}

/// The complete admitted transformation of `function` under `plan`, shared
/// by application and the independent custody walk: constant rows fold in
/// place, forward rows rebind every covered use of the read's result to the
/// proven initializer then retire the observation node — fusing its
/// provenance and fuel settlement into the node that inherits the vacated
/// index — and every derived field restamps from operation shape. `unit`
/// supplies the read-only evidence the re-admission resolves against while
/// `function` is the function being rewritten.
pub(crate) fn transform_function(
    unit: &PsiOptimizationUnit,
    function: &mut PsiOptimizationFunction,
    plan: &FieldValuePlan,
) -> Result<(), FieldValueSpecializationError> {
    let input_function = unit
        .functions
        .iter()
        .find(|candidate| candidate.machine == plan.machine)
        .ok_or(FieldValueSpecializationError::UnknownPlace)?;
    let analysis = admission::function_analysis(input_function);
    // Derive every admitted difference from the input before mutating, so a
    // malformed row cannot leave a partially rewritten machine.
    let folded = plan
        .reads
        .iter()
        .filter(|row| matches!(row.resolution(), FieldValueResolution::Constant(_)))
        .map(|row| folded_node(row, input_function, unit, &analysis).map(|node| (row.site(), node)))
        .collect::<Result<Vec<_>, FieldValueSpecializationError>>()?;
    for row in plan
        .reads
        .iter()
        .filter(|row| matches!(row.resolution(), FieldValueResolution::Forward(_)))
    {
        admitted_forward_row(row, unit, input_function, &analysis)?;
    }
    for (location, node) in folded {
        let index = usize::try_from(location.node)
            .map_err(|_| FieldValueSpecializationError::CoordinateOverflow)?;
        let Some(slot) = function
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .and_then(|block| block.nodes.get_mut(index))
        else {
            return Err(FieldValueSpecializationError::MissingSite {
                machine: plan.machine,
                block: location.block,
                node: location.node,
            });
        };
        *slot = node;
    }
    // Rebind every covered use of each forwarded result to its proven
    // initializer before retiring the observation nodes.
    for row in &plan.reads {
        let FieldValueResolution::Forward(forwarded) = row.resolution() else {
            continue;
        };
        for node in function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.nodes)
        {
            rewrite_scalar_uses(&mut node.operation, row.result(), forwarded.initializer);
        }
    }
    // Retire each forwarded observation in descending node order inside its
    // block so earlier removals never shift a pending coordinate, and fuse
    // the retired custody into the node inheriting the vacated index.
    let mut removals: BTreeMap<BlockId, BTreeSet<u32>> = BTreeMap::new();
    for row in &plan.reads {
        if matches!(row.resolution(), FieldValueResolution::Forward(_)) {
            removals
                .entry(row.site().block)
                .or_default()
                .insert(row.site().node);
        }
    }
    for (block_id, sites) in removals {
        let block = function
            .blocks
            .iter_mut()
            .find(|block| block.id == block_id)
            .ok_or(FieldValueSpecializationError::MissingSite {
                machine: plan.machine,
                block: block_id,
                node: 0,
            })?;
        for site in sites.iter().rev() {
            let index = usize::try_from(*site)
                .map_err(|_| FieldValueSpecializationError::CoordinateOverflow)?;
            let removed = block.nodes.remove(index);
            let receiver =
                block
                    .nodes
                    .get_mut(index)
                    .ok_or(FieldValueSpecializationError::MissingSite {
                        machine: plan.machine,
                        block: block_id,
                        node: *site,
                    })?;
            receiver.provenance.extend_from_slice(&removed.provenance);
            receiver.fuel.extend_from_slice(&removed.fuel);
        }
    }
    optimization_unit::restamp_psi_function_derived_metadata(function)
        .map_err(|_| FieldValueSpecializationError::CoordinateOverflow)?;
    Ok(())
}

pub(crate) fn realize(
    unit: &PsiOptimizationUnit,
    plan: &FieldValuePlan,
) -> Result<PsiOptimizationUnit, FieldValueSpecializationError> {
    if !unit
        .functions
        .iter()
        .any(|function| function.machine == plan.machine)
    {
        return Err(FieldValueSpecializationError::UnknownPlace);
    }
    let mut output = unit.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.machine)
        .ok_or(FieldValueSpecializationError::UnknownPlace)?;
    transform_function(unit, function, plan)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    Ok(output)
}
