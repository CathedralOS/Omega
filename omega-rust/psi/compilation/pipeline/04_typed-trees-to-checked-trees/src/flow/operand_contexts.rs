//! One invocation operand's evidence: detached copies retain completion facts;
//! exact views read their referents at invocation. Operators and ordinary calls
//! share this law, including the conservative context-identity intersection.
mod selections;
mod write_intervals;
pub(crate) use selections::{OperandContextSelection, OperandContextSelections};
pub(super) use write_intervals::{ContentObservationPoint, contents_observation_survives};

use arena::HandleSpan;
use checked_trees::{
    FlowConstraintRef, FlowContextFacts, FlowControlFacts, FlowFacts, FlowOperandFact,
};
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
    contexts: &FlowContextFacts,
    control: &FlowControlFacts,
    semantic: &FactPlan,
    operand: &FlowOperandFact,
    type_reference: TypeReferenceHandle,
    invocation: HandleSpan<FlowConstraintRef>,
) -> Vec<FactContextHandle> {
    let mut captured = contexts
        .semantic_constraint_contexts(operand.constraints)
        .collect::<Vec<_>>();
    if operand_binds_stable_copy(program, type_reference) {
        return captured;
    }
    let descriptor_contexts_agree = |context| {
        super::observations::descriptor_context_agrees(program, control, operand, context)
    };
    captured.retain(|context| descriptor_contexts_agree(*context));
    if operand.constraints == invocation {
        return captured;
    }
    // Resolving a view's backing does not replace its own observation namespace.
    // A local descriptor can retain a predicate established at formation; keep
    // that captured evidence only while its exact context is still live. Facts
    // established later on the original referent remain independently usable.
    let invocation = contexts
        .semantic_constraint_contexts(invocation)
        .collect::<Vec<_>>();
    let identity = |context: FactContextHandle| (context.arena_index(), context.generation());
    let mut live = invocation.iter().copied().map(identity).collect::<Vec<_>>();
    live.sort_unstable();
    live.dedup();
    captured.retain(|candidate| live.binary_search(&identity(*candidate)).is_ok());
    if !operand.referents.is_empty()
        || matches!(
            operand.completion,
            checked_trees::FlowOperandCompletion::Copied { .. }
        )
    {
        let mut retained = captured.iter().copied().map(identity).collect::<Vec<_>>();
        retained.sort_unstable();
        retained.dedup();
        // Membership search uses sorted identities, but evidence publication
        // keeps the old order, including the first occurrence of a newly live
        // context. No duplicate or later occurrence replaces its earlier one.
        let mut additional =
            invocation_contexts_at_referents(program, control, semantic, operand, &invocation)
                .into_iter()
                .enumerate()
                .collect::<Vec<_>>();
        additional.sort_unstable_by_key(|(ordinal, context)| (identity(*context), *ordinal));
        additional.dedup_by_key(|(_, context)| *context);
        additional.retain(|(_, context)| retained.binary_search(&identity(*context)).is_err());
        additional.sort_unstable_by_key(|(ordinal, _)| *ordinal);
        captured.extend(additional.into_iter().map(|(_, context)| context));
    }
    captured.retain(|context| descriptor_contexts_agree(*context));
    captured
}

fn invocation_contexts_at_referents(
    program: &dyn ::validation::CheckingInput,
    control: &FlowControlFacts,
    semantic: &FactPlan,
    operand: &FlowOperandFact,
    invocation: &[FactContextHandle],
) -> Vec<FactContextHandle> {
    let mut referents = control
        .operand_referents
        .span_or_empty(operand.referents)
        .iter()
        .collect::<Vec<_>>();
    // An owned copied product can retain reference descriptors without itself
    // being a view. Their exact entry-time origins supply live-memory contexts;
    // this adds no loan, access, lifetime or referent-geometry authority.
    if let checked_trees::FlowOperandCompletion::Copied { copy, path } = operand.completion {
        let copy = control.copied_values.get(copy);
        let prefix = control.operand_child_paths.span_or_empty(path);
        for leaf in control
            .captured_reference_leaves
            .span_or_empty(copy.references)
        {
            if control
                .operand_child_paths
                .span_or_empty(leaf.path)
                .starts_with(prefix)
            {
                referents.extend(control.operand_referents.span_or_empty(leaf.referents));
            }
        }
    }
    invocation
        .iter()
        .copied()
        .filter(|context| {
            semantic
                .refs
                .span_or_empty(semantic.contexts.get(*context).facts)
                .iter()
                .any(|fact_ref| {
                    let fact = semantic.facts.get(fact_ref.fact);
                    let Some(place_handle) = fact.place.as_place() else {
                        return false;
                    };
                    let semantic_place = semantic.places.get(place_handle);
                    if !matches!(semantic_place.root, facts::PlaceRoot::Expression(_)) {
                        if matches!(semantic_place.root, facts::PlaceRoot::Unknown)
                            || matches!(
                                semantic_place.root,
                                facts::PlaceRoot::Symbol(s) if !s.is_valid()
                            )
                        {
                            return false;
                        }
                        let segments = semantic
                            .place_segments
                            .span_or_empty(semantic_place.segments);
                        return referents.iter().any(|referent| {
                            semantic_place.root == referent.root
                                && crate::flow::canonical_place_segments_may_overlap(
                                    program,
                                    segments,
                                    control
                                        .operand_referent_segments
                                        .span_or_empty(referent.segments),
                                )
                        });
                    }
                    let Some(place) = crate::flow::canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic_place,
                    ) else {
                        return false;
                    };
                    referents.iter().any(|referent| {
                        place.root == referent.root
                            && crate::flow::canonical_place_segments_may_overlap(
                                program,
                                &place.segments,
                                control
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
        operand.entry_binding_writes,
        operand.entry_binding_unknown_write,
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

/// A case observation names only its discriminator. A resolved descendant
/// write cannot replace it; unresolved, equal, and ancestor writes still can.
pub(super) fn case_overwrite(
    program: &dyn validation::CheckingInput,
    place: &validation::CanonicalPlace,
    segments: &[facts::PlaceSegment],
) -> bool {
    !(segments.len() > place.segments.len()
        && segments
            .iter()
            .chain(&place.segments)
            .all(|segment| !super::place_segment_has_unresolved_identity(*segment)))
        && contents_overlap(program, place, segments)
}

pub(super) fn case_origin_is_stable(
    program: &dyn validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    writes: HandleSpan<checked_trees::FlowOperandReferent>,
    unknown: bool,
    place: &validation::CanonicalPlace,
) -> bool {
    entry_origin_is_stable_when(program, control, writes, unknown, place, case_overwrite)
}

pub(super) use validation::storage_path_relations::{binding_overwrite, contents_overlap};

/// A stored view's descriptor survives writes to its referent's descendants.
/// Unknown writes and replacement of the descriptor or its enclosing owner
/// stop the retained address; this grants no access to the backing contents.
pub(crate) fn binding_origin_is_stable(
    program: &dyn ::validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    writes: HandleSpan<checked_trees::FlowOperandReferent>,
    unknown: bool,
    place: &validation::CanonicalPlace,
) -> bool {
    entry_origin_is_stable_when(program, control, writes, unknown, place, binding_overwrite)
}

pub(super) type SourceWriteRelation = fn(
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
    let statements = if first.statement_index == second.statement_index {
        HandleSpan::empty()
    } else {
        flow.control
            .state_by_symbol(first.state_symbol)
            .map_or(HandleSpan::empty(), |row| row.statements)
    };
    operand_sources_agree_in_state(
        program,
        &flow.control,
        statements,
        first,
        second,
        place,
        observation,
    )
}

/// Compare two completed occurrences while the state is still being built.
/// The statement span is the same admitted prefix later published in FlowStateFact.
pub(super) fn operand_bindings_agree_in_state(
    program: &dyn ::validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    statements: HandleSpan<checked_trees::FlowStatementFact>,
    first: &FlowOperandFact,
    second: &FlowOperandFact,
    place: &validation::CanonicalPlace,
) -> bool {
    operand_sources_agree_in_state(
        program,
        control,
        statements,
        first,
        second,
        place,
        super::value_origins::ValueObservation::Binding,
    )
}

pub(super) fn operand_sources_agree_in_state(
    program: &dyn ::validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    statements: HandleSpan<checked_trees::FlowStatementFact>,
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
    let (first_writes, first_unknown) = observation.operand_writes(first);
    let (second_writes, second_unknown) = observation.operand_writes(second);
    if first_unknown || second_unknown {
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
        return observation_reaches_statement_in_state(
            program,
            control,
            statements,
            earlier,
            later.state_symbol,
            later.statement_index,
            place,
            observation,
        ) && entry_origin_is_stable_when(
            program,
            control,
            observation.operand_writes(later).0,
            observation.operand_writes(later).1,
            place,
            write_changes,
        );
    }
    prefixes_agree(
        program,
        control,
        first_writes,
        second_writes,
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
    let Some(row) = flow.control.state_by_symbol(state_symbol) else {
        return false;
    };
    observation_reaches_statement_in_state(
        program,
        &flow.control,
        row.statements,
        operand,
        state_symbol,
        statement,
        place,
        observation,
    )
}

fn observation_reaches_statement_in_state(
    program: &dyn validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    statements: HandleSpan<checked_trees::FlowStatementFact>,
    operand: &FlowOperandFact,
    state_symbol: symbols::SymbolHandle,
    statement: usize,
    place: &validation::CanonicalPlace,
    observation: super::value_origins::ValueObservation,
) -> bool {
    let (operand_writes, operand_unknown) = observation.operand_writes(operand);
    if operand.state_symbol != state_symbol || operand_unknown {
        return false;
    }
    let Some(state) = program.state_by_symbol(state_symbol) else {
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
            control,
            operand_writes,
            operand_unknown,
            place,
            relation,
        );
    }
    let Some(producer) = control
        .statements
        .span_or_empty(statements)
        .iter()
        .find(|row| row.statement_index == operand.statement_index)
    else {
        return false;
    };
    let (completed, unknown) = observation.statement_writes(producer);
    if unknown
        || completed.count() < operand_writes.count()
        || !prefixes_agree(program, control, operand_writes, completed, place, relation)
    {
        return false;
    }
    order.indices()[start + 1..end].iter().all(|index| {
        control
            .statements
            .span_or_empty(statements)
            .iter()
            .find(|row| row.statement_index == *index)
            .is_some_and(|row| {
                let (writes, unknown) = observation.statement_writes(row);
                entry_origin_is_stable_when(program, control, writes, unknown, place, relation)
            })
    })
}

/// Join a completed read to the current guard epoch. Both prefixes belong to
/// the same admitted state and statement; callers authenticate that occurrence
/// before asking whether the selected contents survived intervening writes.
pub(super) fn observation_prefixes_agree(
    program: &dyn validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    first: HandleSpan<checked_trees::FlowOperandReferent>,
    first_unknown: bool,
    second: HandleSpan<checked_trees::FlowOperandReferent>,
    second_unknown: bool,
    place: &validation::CanonicalPlace,
) -> bool {
    !first_unknown
        && !second_unknown
        && prefixes_agree(program, control, first, second, place, contents_overlap)
}

/// The complete directed contents-write interval between two completed
/// occurrences. Consumers supply their own typed storage/permission judgment;
/// this owner supplies chronology and refuses missing or unknown prefixes.
/// No source expression, final value or surviving context replaces an event.
pub(crate) fn operand_contents_writes_between<'facts>(
    program: &dyn validation::CheckingInput,
    flow: &'facts FlowFacts,
    first: &FlowOperandFact,
    second: &FlowOperandFact,
) -> Option<Vec<(usize, &'facts checked_trees::FlowOperandReferent)>> {
    let statements = flow.control.state_by_symbol(first.state_symbol)?.statements;
    write_intervals::contents_writes_between(
        program,
        &flow.control,
        statements,
        ContentObservationPoint::from_operand(first),
        ContentObservationPoint::from_operand(second),
    )
}

/// Prefixes can be copied during arena growth. Authenticate the complete
/// footprint rows rather than requiring the same allocation or silently
/// discarding writes preceding the later completion.
fn write_prefix_extension(
    control: &checked_trees::FlowControlFacts,
    prefix: HandleSpan<checked_trees::FlowOperandReferent>,
    extended: HandleSpan<checked_trees::FlowOperandReferent>,
) -> Option<&[checked_trees::FlowOperandReferent]> {
    let prefix = control.operand_origin_writes.span(prefix)?;
    let extended = control.operand_origin_writes.span(extended)?;
    if prefix.len() > extended.len()
        || !prefix.iter().zip(extended).all(|(left, right)| {
            left.root == right.root
                && control
                    .operand_referent_segments
                    .span(left.segments)
                    .zip(control.operand_referent_segments.span(right.segments))
                    .is_some_and(|(left, right)| left == right)
        })
    {
        return None;
    }
    Some(&extended[prefix.len()..])
}

fn prefixes_agree(
    program: &dyn ::validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    first: HandleSpan<checked_trees::FlowOperandReferent>,
    second: HandleSpan<checked_trees::FlowOperandReferent>,
    place: &validation::CanonicalPlace,
    write_changes: SourceWriteRelation,
) -> bool {
    let (prefix, extended) = if first.count() <= second.count() {
        (first, second)
    } else {
        (second, first)
    };
    let Some(writes) = write_prefix_extension(control, prefix, extended) else {
        return false;
    };
    writes.iter().all(|write| {
        place.root != write.root
            || control
                .operand_referent_segments
                .span(write.segments)
                .is_some_and(|segments| !write_changes(program, place, segments))
    })
}
