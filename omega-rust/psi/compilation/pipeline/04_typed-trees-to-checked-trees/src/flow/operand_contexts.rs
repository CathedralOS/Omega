//! One invocation operand's evidence: detached copies retain completion facts;
//! exact views read their referents at invocation. Operators and ordinary calls
//! share this law, including the conservative context-identity intersection.
use arena::HandleSpan;
use checked_trees::{FlowConstraintRef, FlowFacts, FlowOperandFact};
use facts::{FactContextHandle, FactPlan};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Authenticate the completed operand roster against this exact invocation.
/// An expression receiver occupies an evaluation position but not an explicit
/// argument slot. Consumers share this binding before choosing a value epoch.
pub(crate) fn captured_call_operands<'facts>(
    program: &dyn ::validation::CheckingInput,
    flow: &'facts FlowFacts,
    state: &checked_trees::FlowStateFact,
    call: &checked_trees::FlowCallFact,
) -> Option<(&'facts [FlowOperandFact], usize)> {
    let site = validation::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let receiver = match &site {
        validation::CallSite::Expression { call, .. } if call.receiver.is_valid() => {
            Some(call.receiver)
        }
        _ => None,
    };
    let receiver_offset = usize::from(receiver.is_some());
    let expected = receiver.into_iter().chain(
        validation::call_site_argument_expressions(program, &site)
            .iter()
            .copied(),
    );
    let operands = flow.control.operands.span(call.operands)?;
    operands
        .iter()
        .map(|operand| operand.expression)
        .eq(expected)
        .then_some((operands, receiver_offset))
}

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

/// A source descriptor prefix is the binding evaluated before later children.
/// Payload writes below it cannot redirect that address; replacement can.
pub(crate) fn operand_entry_binding_origin_is_stable(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    operand: &FlowOperandFact,
    place: &validation::CanonicalPlace,
) -> bool {
    binding_origin_is_stable(
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
    operand_sources_agree(
        program,
        flow,
        first,
        second,
        place,
        super::value_origins::ValueObservation::Contents,
    )
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
    operand_sources_agree(
        program,
        flow,
        first,
        second,
        place,
        super::value_origins::ValueObservation::Binding,
    )
}

fn operand_sources_agree(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    first: &FlowOperandFact,
    second: &FlowOperandFact,
    place: &validation::CanonicalPlace,
    observation: super::value_origins::ValueObservation,
) -> bool {
    let write_changes: SourceWriteRelation = match observation {
        super::value_origins::ValueObservation::Contents => contents_overlap,
        super::value_origins::ValueObservation::Binding => binding_overwrite,
    };
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
        let Some(state) = program.state_by_symbol(first.state_symbol) else {
            return false;
        };
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let (Some(first_position), Some(second_position)) = (
            order.position(first.statement_index),
            order.position(second.statement_index),
        ) else {
            return false;
        };
        let (earlier, later) = if first_position < second_position {
            (first, second)
        } else {
            (second, first)
        };
        return observation_reaches_statement(
            program,
            flow,
            earlier,
            later.state_symbol,
            later.statement_index,
            place,
            observation,
        ) && entry_origin_is_stable_when(
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

/// A completed operand and a later statement read the same exact contents.
/// The admitted control order owns chronology; skipped assembly blocks do not
/// contribute writes, and the operand retains its own earlier sibling prefix.
pub(crate) fn operand_observation_before_statement_agrees(
    program: &dyn validation::CheckingInput,
    flow: &FlowFacts,
    operand: &FlowOperandFact,
    state: symbols::SymbolHandle,
    statement: usize,
    place: &validation::CanonicalPlace,
) -> bool {
    observation_reaches_statement(
        program,
        flow,
        operand,
        state,
        statement,
        place,
        super::value_origins::ValueObservation::Contents,
    )
}
/// Descriptor identity is independent of writes through its retained loan.
pub(crate) fn operand_binding_before_statement_agrees(
    program: &dyn validation::CheckingInput,
    flow: &FlowFacts,
    operand: &FlowOperandFact,
    state: symbols::SymbolHandle,
    statement: usize,
    place: &validation::CanonicalPlace,
) -> bool {
    observation_reaches_statement(
        program,
        flow,
        operand,
        state,
        statement,
        place,
        super::value_origins::ValueObservation::Binding,
    )
}
/// A normal return supplies a historical storage observation. Expression
/// returns use the retained completed operand; a statement call ends its
/// statement, so only later admitted statements can change that value.
pub(crate) fn call_return_before_statement_agrees(
    program: &dyn validation::CheckingInput,
    flow: &FlowFacts,
    caller: &checked_trees::FlowStateFact,
    call: &checked_trees::FlowCallFact,
    statement: usize,
    place: &validation::CanonicalPlace,
) -> bool {
    let Some(site) = validation::find_call_site(
        program,
        caller.machine_symbol,
        caller.state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return false;
    };
    match site {
        validation::CallSite::Expression { expression, .. } => {
            let operands = flow
                .control
                .operands
                .iter()
                .map(|(_, operand)| operand)
                .filter(|operand| {
                    operand.state_symbol == caller.state_symbol
                        && operand.statement_index == call.statement_index
                        && operand.expression == expression
                })
                .collect::<Vec<_>>();
            !operands.is_empty()
                && operands.iter().all(|operand| {
                    operand_observation_before_statement_agrees(
                        program,
                        flow,
                        operand,
                        caller.state_symbol,
                        statement,
                        place,
                    )
                })
        }
        validation::CallSite::Statement(_) => {
            let Some(state) = program.state_by_symbol(caller.state_symbol) else {
                return false;
            };
            let order = program
                .statement_table
                .admitted_execution_order(state.statement_nodes);
            let (Some(start), Some(end)) = (
                order.position(call.statement_index),
                order.position(statement),
            ) else {
                return false;
            };
            start < end
                && order.indices()[start + 1..end].iter().all(|index| {
                    flow.state_statement(caller, *index).is_some_and(|row| {
                        entry_origin_is_stable(
                            program,
                            &flow.control,
                            row.completed_content_writes,
                            row.completed_content_unknown_write,
                            place,
                        )
                    })
                })
        }
        validation::CallSite::TransitionNamed { .. } => false,
    }
}

fn observation_reaches_statement(
    program: &dyn validation::CheckingInput,
    flow: &FlowFacts,
    operand: &FlowOperandFact,
    state_symbol: symbols::SymbolHandle,
    statement: usize,
    place: &validation::CanonicalPlace,
    observation: super::value_origins::ValueObservation,
) -> bool {
    if operand.state_symbol != state_symbol || operand.entry_origin_unknown_write {
        return false;
    }
    let Some(state) = program.state_by_symbol(state_symbol) else {
        return false;
    };
    let Some(control) = flow
        .control
        .states
        .iter()
        .map(|(_, row)| row)
        .find(|row| row.state_symbol == state_symbol)
    else {
        return false;
    };
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let (Some(start), Some(end)) = (
        order.position(operand.statement_index),
        order.position(statement).or_else(|| {
            (statement
                == program
                    .statement_table
                    .statements(state.statement_nodes)
                    .len())
            .then_some(order.indices().len())
        }),
    ) else {
        return false;
    };
    if start > end {
        return false;
    }
    let relation: SourceWriteRelation = match observation {
        super::value_origins::ValueObservation::Contents => contents_overlap,
        super::value_origins::ValueObservation::Binding => binding_overwrite,
    };
    if start == end {
        return entry_origin_is_stable_when(
            program,
            &flow.control,
            operand.entry_origin_writes,
            operand.entry_origin_unknown_write,
            place,
            relation,
        );
    }
    let Some(producer) = flow.state_statement(control, operand.statement_index) else {
        return false;
    };
    let (completed, unknown) = observation.statement_writes(producer);
    if unknown
        || completed.count() < operand.entry_origin_writes.count()
        || !prefixes_agree(
            program,
            &flow.control,
            operand.entry_origin_writes,
            completed,
            place,
            relation,
        )
    {
        return false;
    }
    order.indices()[start + 1..end].iter().all(|index| {
        flow.state_statement(control, *index).is_some_and(|row| {
            let (writes, unknown) = observation.statement_writes(row);
            entry_origin_is_stable_when(program, &flow.control, writes, unknown, place, relation)
        })
    })
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
