//! Optimizer module role: application leaf. Exact observation folding and derived-coordinate refresh.
//!
//! For each admitted membership row, the observation node is replaced by a
//! `BooleanConstant` carrying the proven verdict. The folded node keeps the
//! membership's Psi operation custody — `expected_provenance` derives
//! `PsiProvenance::Operation(psi_operation)` for both operations, so the
//! provenance roster, fuel settlements, effect link, value definition,
//! successors, and ownership events are unchanged; only the operation and
//! its recomputed unit identity differ.

use super::super::{
    CaseMembershipPlan, CaseMembershipSpecializationError, O, PsiOptimizationFunction,
    PsiOptimizationUnit, ResolvedCaseMembership, ScalarType, admission,
    recompute_psi_optimization_unit_identity,
};
use optimization_unit::{OptimizationFact, OptimizationNode, PsiProvenance};
use semantic_vocabulary::StructuralPlaceKind;
use std::collections::BTreeMap;

/// The folded node shape a specialization admits at its site: a
/// `BooleanConstant` with the membership's own custody identity and result
/// value. Application and the independent custody walk share this
/// construction so both name exactly the same admitted difference.
pub(crate) fn folded_node(
    row: &ResolvedCaseMembership,
    function: &PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<OptimizationNode, CaseMembershipSpecializationError> {
    let node = node_at(function, row)?;
    let O::StructuralCaseMembership {
        psi_operation,
        result,
        source,
        path,
        case,
    } = &node.operation
    else {
        return Err(CaseMembershipSpecializationError::CandidateMismatch);
    };
    if row.site.machine != function.machine
        || *psi_operation != row.psi_operation
        || result.value != row.result
        || result.scalar_type != ScalarType::Boolean
        || *source != row.source
        || *case != row.observed_case
        || row.outcome != (row.proven_case == row.observed_case)
    {
        return Err(CaseMembershipSpecializationError::CandidateMismatch);
    }
    match row.producer {
        // The claimed producer must be the place's declared operation-result
        // producer and an `EstablishScalarCase` fixing the claimed case.
        // Establishment proves only the place's root case, so a row carrying
        // a path can never hold this basis.
        Some(producer) => {
            if !path.is_empty() {
                return Err(CaseMembershipSpecializationError::CandidateMismatch);
            }
            let declared_producer = function
                .structural_places
                .iter()
                .find(|declaration| declaration.id == row.source)
                .and_then(|declaration| match declaration.kind {
                    StructuralPlaceKind::OperationResult { producer, .. } => Some(producer),
                    _ => None,
                });
            if declared_producer != Some(producer)
                || !function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes)
                    .any(|candidate| {
                        matches!(
                            &candidate.operation,
                            O::EstablishScalarCase {
                                psi_operation,
                                result,
                                result_case,
                                ..
                            } if *psi_operation == producer
                                && result.place == row.source
                                && *result_case == row.proven_case
                        )
                    })
            {
                return Err(CaseMembershipSpecializationError::CandidateMismatch);
            }
        }
        // With no producer claim, the membership's path position must itself
        // resolve to a closed roster of exactly the claimed case: the place's
        // root type at an empty path, or the nested type its path descends to.
        None => {
            let declaration = function
                .structural_places
                .iter()
                .find(|declaration| declaration.id == row.source);
            if declaration.and_then(|declaration| {
                admission::sole_case_at_path(unit, function, declaration, path)
            }) != Some(row.proven_case)
            {
                return Err(CaseMembershipSpecializationError::CandidateMismatch);
            }
        }
    }
    let operation = O::BooleanConstant {
        psi_operation: *psi_operation,
        result: result.value,
        value: row.outcome,
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
    row: &ResolvedCaseMembership,
) -> Result<&'a OptimizationNode, CaseMembershipSpecializationError> {
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == row.site.block)
        .ok_or(CaseMembershipSpecializationError::MissingSite {
            machine: function.machine,
            block: row.site.block,
            node: row.site.node,
        })?;
    block
        .nodes
        .get(
            usize::try_from(row.site.node)
                .map_err(|_| CaseMembershipSpecializationError::CoordinateOverflow)?,
        )
        .ok_or(CaseMembershipSpecializationError::MissingSite {
            machine: function.machine,
            block: row.site.block,
            node: row.site.node,
        })
}

pub(crate) fn realize(
    unit: &PsiOptimizationUnit,
    plan: &CaseMembershipPlan,
) -> Result<PsiOptimizationUnit, CaseMembershipSpecializationError> {
    let input_function = unit
        .functions
        .iter()
        .find(|function| function.machine == plan.machine)
        .ok_or(CaseMembershipSpecializationError::UnknownPlace)?;
    // Derive every folded node from the input before mutating, so a malformed
    // row cannot leave a partially rewritten machine.
    let folded = plan
        .memberships
        .iter()
        .map(|row| folded_node(row, input_function, unit).map(|node| (row.site, node)))
        .collect::<Result<Vec<_>, CaseMembershipSpecializationError>>()?;
    let mut output = unit.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.machine)
        .ok_or(CaseMembershipSpecializationError::UnknownPlace)?;
    for (location, node) in folded {
        let index = usize::try_from(location.node)
            .map_err(|_| CaseMembershipSpecializationError::CoordinateOverflow)?;
        let Some(slot) = function
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .and_then(|block| block.nodes.get_mut(index))
        else {
            return Err(CaseMembershipSpecializationError::MissingSite {
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
/// `BooleanConstant` fact at each folded site. Application and the
/// independent custody walk share this reconstruction.
pub(crate) fn refresh_facts(
    function: &mut PsiOptimizationFunction,
    plan: &CaseMembershipPlan,
) -> Result<(), CaseMembershipSpecializationError> {
    let folded = plan
        .memberships
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
                    .map_err(|_| CaseMembershipSpecializationError::CoordinateOverflow)?,
            )) {
                next_facts.push(OptimizationFact::BooleanConstant {
                    value: row.result,
                    constant: row.outcome,
                    support: row.psi_operation,
                });
            }
        }
    }
    next_facts.extend(retained);
    function.facts = next_facts;
    Ok(())
}
