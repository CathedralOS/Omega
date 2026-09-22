//! Optimizer module role: proposal leaf. Proven field-value specialization candidates.
//!
//! A `BooleanStructuralField`/`IntegerStructuralField` observation is
//! foldable when the stored value it reads is proven by the unit itself. At
//! an empty path an `EstablishRecord` producer proves the field's
//! initializer; at a lone `Case` path an `EstablishScalarCase` producer
//! whose `result_case` matches proves the payload field's initializer. An
//! initializer that is a same-function constant folds the read to a literal;
//! a nonconstant initializer instead substitutes itself at every use of the
//! read's result and retires the observation node when the substitution
//! lane covers every use and the initializer dominates them all. At any
//! resolvable path a field declared `BoundedInteger` with a singleton bound
//! proves the value independently of producer. `Field` segments descend
//! through `Record`/`Mixed` common fields and case-payload fields by
//! identity, `FixedIndex` descends a `FixedArray` element, and `Case` enters
//! a `Sum`/`Mixed` case payload namespace. Machines holding an authenticated
//! cyclic component are frozen byte-exact for this family and never yield
//! rows. Reads whose resolved field the unit cannot prove stay unfolded.

use super::{
    FieldValuePlan, FieldValueSpecializationCandidate, FieldValueSpecializationError, PlaceId,
    PsiOptimizationFunction, PsiOptimizationUnit, VerifiedPsiOptimizationSession, admission, apply,
    candidate_identity,
};
use std::collections::BTreeSet;

pub(super) fn all(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<Vec<FieldValueSpecializationCandidate>, FieldValueSpecializationError> {
    let unit = session.unit();
    // Machines holding an authenticated cyclic component are frozen
    // byte-exact for this family; their nodes cannot be rewritten.
    let frozen = session
        .cycle_components()
        .components()
        .iter()
        .map(|component| component.id.machine)
        .collect::<BTreeSet<_>>();
    let mut candidates = Vec::new();
    for function in &unit.functions {
        if frozen.contains(&function.machine) {
            continue;
        }
        for declaration in &function.structural_places {
            let Some(plan) = plan(unit, function, declaration.id) else {
                continue;
            };
            if plan.reads.is_empty() {
                continue;
            }
            candidates.push(from_plan(unit, plan)?);
        }
    }
    let required = u64::try_from(candidates.len())
        .map_err(|_| FieldValueSpecializationError::CoordinateOverflow)?;
    if required > candidate_limit {
        return Err(FieldValueSpecializationError::CandidateBudgetExhausted {
            required,
            limit: candidate_limit,
        });
    }
    Ok(candidates)
}

/// Independently derived specialization plan for one place, or `None` when
/// no field read observing it is proven by the unit. An admissible plan
/// carries every proven field observation of the place, in node order: each
/// row's basis is the place's establishment witness or the declared
/// singleton bound at the resolved position.
pub(crate) fn plan(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<FieldValuePlan> {
    let evidence = admission::field_evidence(function, place)?;
    let analysis = admission::function_analysis(function);
    let mut reads = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let Some(row) = admission::admit_field_node(
                unit, &evidence, function, &analysis, block, node_index, node,
            ) else {
                continue;
            };
            reads.push(row);
        }
    }
    reads.sort_by_key(|row| (row.site().block, row.site().node));
    Some(FieldValuePlan {
        machine: function.machine,
        place,
        producer: evidence.root_producer.operation(),
        reads,
    })
}

pub(super) fn from_plan(
    unit: &PsiOptimizationUnit,
    plan: FieldValuePlan,
) -> Result<FieldValueSpecializationCandidate, FieldValueSpecializationError> {
    let output = apply::realize(unit, &plan)?;
    let identity = candidate_identity(
        unit.identity,
        output.identity,
        plan.machine,
        plan.place,
        &plan.reads,
    );
    Ok(FieldValueSpecializationCandidate {
        identity,
        input: unit.identity,
        output: output.identity,
        machine: plan.machine,
        place: plan.place,
        producer: plan.producer,
        reads: plan.reads,
    })
}
