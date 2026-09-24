//! Optimizer module role: proposal leaf. Proven field-value specialization plans.
//!
//! A `BooleanStructuralField`/`IntegerStructuralField` observation is
//! foldable when the stored value it reads is proven by the unit itself. At
//! an empty path an `EstablishRecord` producer proves the field's
//! initializer; at a lone `Case` path an `EstablishScalarCase` producer
//! whose `result_case` matches proves the payload field's initializer; and
//! a `Field` segment descends the same two proofs into a nested position
//! when the record stores the field as an owned, complete structural child
//! whose declared type is exactly the field's declared carrier. An
//! initializer that is a same-function constant folds the read to a literal;
//! a nonconstant initializer instead substitutes itself at every use of the
//! read's result and retires the observation node when the substitution
//! lane covers every use and the initializer dominates them all. At any
//! resolvable path a field declared `BoundedInteger` with a singleton bound
//! proves the value independently of producer. `Field` segments descend
//! through `Record`/`Mixed` common fields and case-payload fields by
//! identity, `FixedIndex` descends a `FixedArray` element, and `Case` enters
//! a `Sum`/`Mixed` case payload namespace. Reads whose resolved field the
//! unit cannot prove stay unfolded; the rule freezes machines holding an
//! authenticated cyclic component before asking for a plan.

use super::{
    FieldValueSpecializationRewrite, PlaceId, PsiOptimizationFunction, PsiOptimizationUnit,
    admission,
};

/// Independently derived specialization plan for one place, or `None` when
/// the place is not rostered in `function` or carries no field-value
/// evidence. An admissible plan carries every proven field observation of
/// the place, in node order: each row's basis is the place's establishment
/// witness or the declared singleton bound at the resolved position. A plan
/// with no reads means the place is proven but currently unobserved.
pub(crate) fn plan(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<FieldValueSpecializationRewrite> {
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
    reads.sort_by_key(|row| (row.site.block, row.site.node));
    Some(FieldValueSpecializationRewrite {
        machine: function.machine,
        place,
        producer: evidence.root_producer.operation(),
        reads,
    })
}
