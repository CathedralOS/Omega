//! One invocation operand's evidence: detached copies retain completion facts;
//! exact views read their referents at invocation. Operators and ordinary calls
//! share this law, including the conservative context-identity intersection.
use arena::HandleSpan;
use checked_trees::{FlowConstraintRef, FlowFacts, FlowOperandFact};
use facts::{FactContextHandle, FactPlan};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn operand_contexts(
    program: &dyn ::validation::CheckingInput,
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
    // Resolving a view's backing does not replace its own observation namespace.
    // A local descriptor can retain a predicate established at formation; keep
    // that captured evidence only while its exact context is still live. Facts
    // established later on the original referent remain independently usable.
    captured.retain(|candidate| {
        flow.semantic_constraint_contexts(invocation)
            .any(|context| context == *candidate)
    });
    if !operand.referents.is_empty() {
        for context in
            invocation_contexts_at_referents(program, flow, semantic, operand, invocation)
        {
            if !captured.contains(&context) {
                captured.push(context);
            }
        }
    }
    captured
}

fn invocation_contexts_at_referents(
    program: &dyn ::validation::CheckingInput,
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
                    let Some(place) = fact.place.as_place() else {
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
                        &[],
                        validation::ContentsRequirement::StableObservation,
                    );
            }
            TypeReferenceNode::Generic { .. } | TypeReferenceNode::FixedArray { .. } => {
                return validation::has_contents(
                    program,
                    reference,
                    &[],
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
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    operand: &FlowOperandFact,
    place: &validation::CanonicalPlace,
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
    program: &dyn ::validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    writes: HandleSpan<checked_trees::FlowOperandReferent>,
    unknown: bool,
    place: &validation::CanonicalPlace,
) -> bool {
    entry_origin_is_stable_when(program, control, writes, unknown, place, contents_overlap)
}

fn contents_overlap(
    program: &dyn ::validation::CheckingInput,
    place: &validation::CanonicalPlace,
    segments: &[facts::PlaceSegment],
) -> bool {
    crate::flow::canonical_place_segments_may_overlap(program, &place.segments, segments)
}

pub(super) fn binding_overwrite(
    program: &dyn ::validation::CheckingInput,
    place: &validation::CanonicalPlace,
    segments: &[facts::PlaceSegment],
) -> bool {
    // A write replacing this binding or an enclosing owner changes its stored
    // descriptor/pointer. A descendant referent write changes contents only.
    segments.len() <= place.segments.len() && contents_overlap(program, place, segments)
}

/// A stored view's descriptor survives writes to its referent's descendants.
/// Unknown writes and replacement of the descriptor or its enclosing owner
/// stop the retained address; this grants no access to the backing contents.
pub(super) fn binding_origin_is_stable(
    program: &dyn ::validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    writes: HandleSpan<checked_trees::FlowOperandReferent>,
    unknown: bool,
    place: &validation::CanonicalPlace,
) -> bool {
    entry_origin_is_stable_when(program, control, writes, unknown, place, binding_overwrite)
}

type SourceWriteRelation = fn(
    &dyn ::validation::CheckingInput,
    &validation::CanonicalPlace,
    &[facts::PlaceSegment],
) -> bool;

fn entry_origin_is_stable_when(
    program: &dyn ::validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    writes: HandleSpan<checked_trees::FlowOperandReferent>,
    unknown: bool,
    place: &validation::CanonicalPlace,
    write_changes: SourceWriteRelation,
) -> bool {
    !unknown
        && control
            .operand_origin_writes
            .span(writes)
            .is_some_and(|writes| {
                writes.iter().all(|write| {
                    place.root != write.root
                        || control
                            .operand_referent_segments
                            .span(write.segments)
                            .is_some_and(|segments| !write_changes(program, place, segments))
                })
            })
}

/// Unknown observations may share a place identity only while its contents
/// agree at both capture points. Prefix spans can be copied by arena growth;
/// compare their exact rows, then inspect intervening writes. Final values and
/// equal rendered paths cannot erase an intermediate write.
pub(crate) fn operand_observations_agree(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    first: &FlowOperandFact,
    second: &FlowOperandFact,
    place: &validation::CanonicalPlace,
) -> bool {
    operand_sources_agree(program, flow, first, second, place, contents_overlap)
}

/// Stored reference/descriptor identity is captured independently of its live
/// referent contents. The caller supplies an authentic typed binding place;
/// this judgment grants no referent access, value equality or loan permission.
pub(crate) fn operand_bindings_agree(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    first: &FlowOperandFact,
    second: &FlowOperandFact,
    place: &validation::CanonicalPlace,
) -> bool {
    operand_sources_agree(program, flow, first, second, place, binding_overwrite)
}

fn operand_sources_agree(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    first: &FlowOperandFact,
    second: &FlowOperandFact,
    place: &validation::CanonicalPlace,
    write_changes: SourceWriteRelation,
) -> bool {
    if std::ptr::eq(first, second) {
        return true;
    }
    if first.entry_origin_unknown_write || second.entry_origin_unknown_write {
        return false;
    }
    if first.state_symbol != second.state_symbol {
        return false;
    }
    if first.statement_index != second.statement_index {
        let (earlier, later) = if first.statement_index < second.statement_index {
            (first, second)
        } else {
            (second, first)
        };
        let Some((_, state)) = flow
            .control
            .states
            .iter()
            .find(|(_, state)| state.state_symbol == earlier.state_symbol)
        else {
            return false;
        };
        let Some(statements) = flow.control.statements.span(state.statements) else {
            return false;
        };
        let Some(statement) = statements
            .iter()
            .find(|statement| statement.statement_index == earlier.statement_index)
        else {
            return false;
        };
        return !statement.completed_unknown_write
            && prefixes_agree(
                program,
                &flow.control,
                earlier.entry_origin_writes,
                statement.completed_writes,
                place,
                write_changes,
            )
            && statements
                .iter()
                .filter(|statement| {
                    statement.statement_index > earlier.statement_index
                        && statement.statement_index < later.statement_index
                })
                .all(|statement| {
                    entry_origin_is_stable_when(
                        program,
                        &flow.control,
                        statement.completed_writes,
                        statement.completed_unknown_write,
                        place,
                        write_changes,
                    )
                })
            && entry_origin_is_stable_when(
                program,
                &flow.control,
                later.entry_origin_writes,
                later.entry_origin_unknown_write,
                place,
                write_changes,
            );
    }
    prefixes_agree(
        program,
        &flow.control,
        first.entry_origin_writes,
        second.entry_origin_writes,
        place,
        write_changes,
    )
}

fn prefixes_agree(
    program: &dyn ::validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    first: HandleSpan<checked_trees::FlowOperandReferent>,
    second: HandleSpan<checked_trees::FlowOperandReferent>,
    place: &validation::CanonicalPlace,
    write_changes: SourceWriteRelation,
) -> bool {
    let Some(first) = control.operand_origin_writes.span(first) else {
        return false;
    };
    let Some(second) = control.operand_origin_writes.span(second) else {
        return false;
    };
    let (prefix, extended) = if first.len() <= second.len() {
        (first, second)
    } else {
        (second, first)
    };
    if !prefix.iter().zip(extended).all(|(left, right)| {
        left.root == right.root
            && control
                .operand_referent_segments
                .span(left.segments)
                .zip(control.operand_referent_segments.span(right.segments))
                .is_some_and(|(left, right)| left == right)
    }) {
        return false;
    }
    extended[prefix.len()..].iter().all(|write| {
        place.root != write.root
            || control
                .operand_referent_segments
                .span(write.segments)
                .is_some_and(|segments| !write_changes(program, place, segments))
    })
}
