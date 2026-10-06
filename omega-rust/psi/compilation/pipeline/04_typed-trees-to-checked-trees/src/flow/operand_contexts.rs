//! One invocation operand's evidence: detached copies retain completion facts;
//! exact views read their referents at invocation. Operators and ordinary calls
//! share this law, including the conservative context-identity intersection.
use arena::HandleSpan;
use checked_trees::{FlowConstraintRef, FlowFacts, FlowOperandFact};
use facts::{FactContextHandle, FactPlan};
use typed_trees::{
    TypedTrees,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

pub(crate) fn operand_contexts(
    program: &TypedTrees,
    flow: &FlowFacts,
    semantic: &FactPlan,
    operand: &FlowOperandFact,
    type_reference: TypeReferenceHandle,
    invocation: HandleSpan<FlowConstraintRef>,
) -> Vec<FactContextHandle> {
    let mut captured = flow
        .semantic_constraint_contexts(operand.constraints)
        .collect::<Vec<_>>();
    if operand_binds_stable_copy(program, type_reference) {
        return captured;
    }
    if operand.referents.is_empty() {
        captured.retain(|candidate| {
            flow.semantic_constraint_contexts(invocation)
                .any(|context| context == *candidate)
        });
        captured
    } else {
        invocation_contexts_at_referents(program, flow, semantic, operand, invocation)
    }
}

fn invocation_contexts_at_referents(
    program: &TypedTrees,
    flow: &FlowFacts,
    semantic: &FactPlan,
    operand: &FlowOperandFact,
    invocation: HandleSpan<FlowConstraintRef>,
) -> Vec<FactContextHandle> {
    let referents = flow
        .control
        .operand_referents
        .span_or_empty(operand.referents);
    flow.semantic_constraint_contexts(invocation)
        .filter(|context| {
            semantic
                .refs
                .span_or_empty(semantic.contexts.get(*context).facts)
                .iter()
                .any(|fact_ref| {
                    let fact = semantic.facts.get(fact_ref.fact);
                    let facts::FactPlace::Place(place) = fact.place else {
                        return false;
                    };
                    let Some(place) = crate::flow::canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(place),
                    ) else {
                        return false;
                    };
                    referents.iter().any(|referent| {
                        place.root == referent.root
                            && crate::flow::canonical_place_segments_may_overlap(
                                program,
                                &place.segments,
                                flow.control
                                    .operand_referent_segments
                                    .span_or_empty(referent.segments),
                            )
                    })
                })
        })
        .collect()
}

/// Positive view-carrier classification. An opaque owned value is not a live
/// referent merely because its contents cannot be proved stable. Constraints
/// preserve the carrier's reference or slice identity.
pub(crate) fn operand_binds_live_view(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> bool {
    if !reference.is_valid() {
        return false;
    }
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => return true,
            _ => return false,
        }
    }
}

/// Whether the operand bound to this formal is a detached copy of stable
/// contents, so its operand-time facts describe the invocation value outright.
/// A copied scalar is the narrow case; a by-value record, sum, or fixed array
/// with stable observable contents is equally detached — a later operand's
/// write to the source storage cannot reach the copy, and no loan or interior
/// authority inside it can retarget what the operator received. Reference and
/// slice formals are views onto shared storage: a view operand's recorded
/// referents transport the facts live on that storage at invocation; anything
/// the contents classifier cannot prove stable, or whose referent could not
/// be resolved exactly, keeps the capture ∩ invocation intersection.
pub(crate) fn operand_binds_stable_copy(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { .. } => {
                return program.primitive_type_reference(reference).is_some()
                    || validation::has_contents(
                        program,
                        reference,
                        validation::ContentsRequirement::StableObservation,
                    );
            }
            TypeReferenceNode::Generic { .. } | TypeReferenceNode::FixedArray { .. } => {
                return validation::has_contents(
                    program,
                    reference,
                    validation::ContentsRequirement::StableObservation,
                );
            }
            _ => return false,
        }
    }
}

/// A projected read can retain its before-statement entry origin only if no
/// already-evaluated operand write overlaps it. This prefix excludes later
/// operands and shares the flow producer's canonical overlap law.
pub(crate) fn operand_entry_origin_is_stable(
    program: &TypedTrees,
    flow: &FlowFacts,
    operand: &FlowOperandFact,
    place: &crate::flow::CanonicalPlace,
) -> bool {
    entry_origin_is_stable(
        program,
        &flow.control,
        operand.entry_origin_writes,
        operand.entry_origin_unknown_write,
        place,
    )
}

pub(crate) fn entry_origin_is_stable(
    program: &TypedTrees,
    control: &checked_trees::FlowControlFacts,
    writes: HandleSpan<checked_trees::FlowOperandReferent>,
    unknown: bool,
    place: &crate::flow::CanonicalPlace,
) -> bool {
    !unknown
        && control
            .operand_origin_writes
            .span(writes)
            .is_some_and(|writes| {
                writes.iter().all(|write| {
                    place.root != write.root
                        || !crate::flow::canonical_place_segments_may_overlap(
                            program,
                            &place.segments,
                            control
                                .operand_referent_segments
                                .span_or_empty(write.segments),
                        )
                })
            })
}

/// Within one invocation, unknown observations may share a place identity only while its contents
/// agree at both capture points. Prefix spans can be copied by arena growth;
/// compare their exact rows, then inspect intervening writes. Final values and
/// equal rendered paths cannot erase an intermediate write.
pub(crate) fn operand_observations_agree(
    program: &TypedTrees,
    flow: &FlowFacts,
    first: &FlowOperandFact,
    second: &FlowOperandFact,
    place: &crate::flow::CanonicalPlace,
) -> bool {
    if std::ptr::eq(first, second) {
        return true;
    }
    if first.entry_origin_unknown_write || second.entry_origin_unknown_write {
        return false;
    }
    let Some(first) = flow
        .control
        .operand_origin_writes
        .span(first.entry_origin_writes)
    else {
        return false;
    };
    let Some(second) = flow
        .control
        .operand_origin_writes
        .span(second.entry_origin_writes)
    else {
        return false;
    };
    let (prefix, extended) = if first.len() <= second.len() {
        (first, second)
    } else {
        (second, first)
    };
    if !prefix.iter().zip(extended).all(|(left, right)| {
        left.root == right.root
            && flow
                .control
                .operand_referent_segments
                .span(left.segments)
                .zip(flow.control.operand_referent_segments.span(right.segments))
                .is_some_and(|(left, right)| left == right)
    }) {
        return false;
    }
    extended[prefix.len()..].iter().all(|write| {
        place.root != write.root
            || flow
                .control
                .operand_referent_segments
                .span(write.segments)
                .is_some_and(|segments| {
                    !crate::flow::canonical_place_segments_may_overlap(
                        program,
                        &place.segments,
                        segments,
                    )
                })
    })
}
