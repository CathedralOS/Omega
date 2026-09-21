//! Optimizer module role: application leaf. Exact observation folding and derived-coordinate refresh.
//!
//! For each admitted field row, the observation node is replaced by a
//! `BooleanConstant`/`IntegerConstant` carrying the proven stored value. The
//! folded node keeps the read's Psi operation custody —
//! `expected_provenance` derives `PsiProvenance::Operation(psi_operation)`
//! for both operations, so the provenance roster, fuel settlements, effect
//! link, value definition, successors, and ownership events are unchanged;
//! only the operation and its recomputed unit identity differ.

use super::super::{
    FieldValuePlan, FieldValueSpecializationError, O, PsiOptimizationFunction, PsiOptimizationUnit,
    ResolvedFieldValue, admission, recompute_psi_optimization_unit_identity,
};
use optimization_unit::{FoldedFieldValue, OptimizationFact, OptimizationNode, PsiProvenance};
use std::collections::BTreeMap;

/// The folded node shape a specialization admits at its site: a
/// `BooleanConstant`/`IntegerConstant` with the read's own custody identity
/// and result value. Application and the independent custody walk share this
/// construction so both name exactly the same admitted difference.
pub(crate) fn folded_node(
    row: &ResolvedFieldValue,
    function: &PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<OptimizationNode, FieldValueSpecializationError> {
    let node = node_at(function, row)?;
    let (psi_operation, result, source, path, field, kind) = match &node.operation {
        O::BooleanStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => (
            psi_operation,
            *result,
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
    // establishment witness and proven value must equal the row's.
    let Some(evidence) = admission::field_evidence(function, row.source) else {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    };
    let proven = admission::proven_field_value(unit, &evidence, function, path, *field, kind)
        .ok_or(FieldValueSpecializationError::CandidateMismatch)?;
    if proven.producer != row.producer || proven.value != row.value {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    }
    let operation = match row.value {
        FoldedFieldValue::Boolean(value) => O::BooleanConstant {
            psi_operation: *psi_operation,
            result,
            value,
        },
        FoldedFieldValue::Integer(value) => O::IntegerConstant {
            psi_operation: *psi_operation,
            result,
            scalar_type: match &node.operation {
                O::IntegerStructuralField { result, .. } => result.scalar_type,
                _ => return Err(FieldValueSpecializationError::CandidateMismatch),
            },
            value,
        },
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

fn node_at<'a>(
    function: &'a PsiOptimizationFunction,
    row: &ResolvedFieldValue,
) -> Result<&'a OptimizationNode, FieldValueSpecializationError> {
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == row.site.block)
        .ok_or(FieldValueSpecializationError::MissingSite {
            machine: function.machine,
            block: row.site.block,
            node: row.site.node,
        })?;
    block
        .nodes
        .get(
            usize::try_from(row.site.node)
                .map_err(|_| FieldValueSpecializationError::CoordinateOverflow)?,
        )
        .ok_or(FieldValueSpecializationError::MissingSite {
            machine: function.machine,
            block: row.site.block,
            node: row.site.node,
        })
}

pub(crate) fn realize(
    unit: &PsiOptimizationUnit,
    plan: &FieldValuePlan,
) -> Result<PsiOptimizationUnit, FieldValueSpecializationError> {
    let input_function = unit
        .functions
        .iter()
        .find(|function| function.machine == plan.machine)
        .ok_or(FieldValueSpecializationError::UnknownPlace)?;
    // Derive every folded node from the input before mutating, so a malformed
    // row cannot leave a partially rewritten machine.
    let folded = plan
        .reads
        .iter()
        .map(|row| folded_node(row, input_function, unit).map(|node| (row.site, node)))
        .collect::<Result<Vec<_>, FieldValueSpecializationError>>()?;
    let mut output = unit.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.machine)
        .ok_or(FieldValueSpecializationError::UnknownPlace)?;
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
    refresh_facts(function, plan)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    Ok(output)
}

/// Rebuilds a function's retained optimization-fact index after folding.
/// Every fact row carries its emitting node's `support` custody, so the walk
/// consumes each untouched node's retained rows in order and pushes one
/// constant fact at each folded site. Application and the independent
/// custody walk share this reconstruction.
pub(crate) fn refresh_facts(
    function: &mut PsiOptimizationFunction,
    plan: &FieldValuePlan,
) -> Result<(), FieldValueSpecializationError> {
    let folded = plan
        .reads
        .iter()
        .map(|row| ((row.site.block, row.site.node), row))
        .collect::<BTreeMap<_, _>>();
    let mut retained = std::mem::take(&mut function.facts).into_iter().peekable();
    let mut next_facts = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            if let Some(PsiProvenance::Operation(operation)) = node.provenance.first() {
                while let Some(fact) = retained.peek() {
                    let support = match fact {
                        OptimizationFact::OperationObligationReference { support, .. }
                        | OptimizationFact::BooleanConstant { support, .. }
                        | OptimizationFact::IntegerConstant { support, .. } => *support,
                    };
                    if support != *operation {
                        break;
                    }
                    next_facts.push(retained.next().expect("peeked fact exists"));
                }
            }
            if let Some(row) = folded.get(&(
                block.id,
                u32::try_from(node_index)
                    .map_err(|_| FieldValueSpecializationError::CoordinateOverflow)?,
            )) {
                next_facts.push(match row.value {
                    FoldedFieldValue::Boolean(constant) => OptimizationFact::BooleanConstant {
                        value: row.result,
                        constant,
                        support: row.psi_operation,
                    },
                    FoldedFieldValue::Integer(constant) => OptimizationFact::IntegerConstant {
                        value: row.result,
                        constant,
                        support: row.psi_operation,
                    },
                });
            }
        }
    }
    next_facts.extend(retained);
    function.facts = next_facts;
    Ok(())
}
