//! Find the original storage behind a saved reference, including references
//! inside records. Resolve its completed storage and permission dependencies
//! without creating access, restoration authority, or call destinations.

use crate::checks::borrows::access_sources::resolve_captured_all;
use crate::checks::borrows::aliases::ResolvedAlias;
use crate::checks::borrows::loan_occurrences::LoanOccurrences;
use crate::flow::{captured_call_operands, expressions};
use crate::published::places::place_at;
use arena::{Handle, HandleSpan};
use checked_trees::{CapturedPlace, CheckFacts, FlowCallFact, FlowOperandFact, FlowStateFact};

/// Lifetime retention follows the captured contents of every stored reference
/// crossing, just as a referent access does. Resolving only a reference local
/// leaves `holder.reference_field` anchored to the holder instead of its
/// referent. The original invocation access remains unchanged; each crossed
/// stored loan joins the parent roster so suspension also retains its resource
/// obligations. This does not change the alias checker's access exemptions.
pub(in crate::checks::borrows) fn retained_places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &FlowStateFact,
    statement: usize,
    constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    source: CapturedPlace,
    active_calls: &mut Vec<Handle<FlowCallFact>>,
) -> Option<Vec<ResolvedAlias>> {
    let typed_state = program.state_by_symbol(state.state_symbol)?;
    let mut pending = resolve_captured_all(
        program,
        facts.view(),
        loan_occurrences,
        state,
        statement,
        constraints,
        source,
    )?
    .into_iter()
    .map(|source| (source.place, source.lineage))
    .collect::<Vec<_>>();
    let mut roots = Vec::new();
    while let Some((place, ancestors)) = pending.pop() {
        if place.incoming_reference.is_valid() {
            roots.push(ResolvedAlias {
                place,
                lineage: ancestors,
            });
            continue;
        }
        if let Some(expression) = place.root.as_expression() {
            let mut calls = state.calls.handles().filter(|handle| {
                let call = facts.flow.control.calls.get(*handle);
                call.authored_expression == expression && call.statement_index == statement
            });
            if let Some(call) = calls.next() {
                if calls.next().is_some() || active_calls.contains(&call) {
                    return None;
                }
                active_calls.push(call);
                let returned = returned_places(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    call,
                    &place.segments,
                    constraints,
                    active_calls,
                );
                active_calls.pop();
                for mut source in returned? {
                    for ancestor in &ancestors {
                        if !source.lineage.contains(ancestor) {
                            source.lineage.push(*ancestor);
                        }
                    }
                    roots.push(source);
                }
                continue;
            }
        }
        let mut crossed = false;
        for handle in facts.flow.borrow_loan_constraints(constraints) {
            let loan = facts.borrow.loans.get(handle);
            let destinations = crate::borrow::store_destination_places(
                program,
                typed_state,
                place.clone(),
                true,
                std::iter::once((
                    loan.storage_owner,
                    facts.borrow.loan_owner_path(loan),
                    loan.incoming_reference,
                    facts
                        .borrow
                        .access_segments
                        .span_or_empty(loan.incoming_owner_path),
                    loan.root,
                    facts.borrow.loan_segments(loan),
                )),
            );
            if destinations.as_slice() == [place.clone()] {
                continue;
            }
            // Reusing one supplying occurrence would be a cycle even if its
            // projection grows on each traversal. Bound by loan identities.
            if ancestors.contains(&handle) {
                return None;
            }
            crossed = true;
            let mut suppliers = ancestors.clone();
            suppliers.push(handle);
            pending.extend(
                destinations
                    .into_iter()
                    .map(|place| (place, suppliers.clone())),
            );
        }
        if !crossed {
            // Current loan rows above own every replacement of a stored
            // descriptor. An otherwise unchanged incoming carrier already has
            // opaque reference permissions from its parameter contract; join
            // those exact identities instead of treating its fields as local
            // descriptors with missing provenance. This retains lifetime only:
            // the invocation's captured access and geometry remain unchanged.
            for (incoming_reference, incoming) in facts.borrow.incoming_references.iter() {
                if incoming.machine_symbol != state.machine_symbol
                    || incoming.state_symbol != state.state_symbol
                {
                    continue;
                }
                let checked_trees::IncomingReferenceOrigin::Parameter { symbol } = incoming.origin
                else {
                    continue;
                };
                let root = facts::PlaceRoot::Symbol(symbol);
                let destinations = crate::borrow::store_destination_places(
                    program,
                    typed_state,
                    place.clone(),
                    true,
                    std::iter::once((
                        root,
                        facts
                            .borrow
                            .owner_segments
                            .span_or_empty(incoming.owner_path),
                        incoming_reference,
                        &[][..],
                        root,
                        &[][..],
                    )),
                );
                if destinations.as_slice() != [place.clone()] {
                    crossed = true;
                    pending.extend(
                        destinations
                            .into_iter()
                            .map(|place| (place, ancestors.clone())),
                    );
                }
            }
        }
        if !crossed {
            if !terminal_storage_is_resolved(program, typed_state, &place)? {
                return None;
            }
            roots.push(ResolvedAlias {
                place,
                lineage: ancestors,
            });
        }
    }
    Some(roots)
}

/// The terminal footprint must not stop at an unresolved descriptor. Regions
/// select storage, not one scalar value: the scalar projector deliberately
/// cannot type them. Inspect the collection's complete element frontier instead,
/// retaining the original region unchanged. A region containing references still
/// needs their supplying loans; neither an incoming root nor a range can erase
/// that obligation. Bounds and interpretation were checked at loan formation.
fn terminal_storage_is_resolved(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    place: &CapturedPlace,
) -> Option<bool> {
    let (segments, region) = match place.segments.split_last() {
        Some((
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange,
            prefix,
        )) => (prefix, true),
        _ => (place.segments.as_slice(), false),
    };
    let selected_type = validation::canonical_place_type_reference(
        program,
        state.symbol,
        state.statement_nodes.count() as usize,
        &validation::CanonicalPlace {
            root: place.root,
            segments: segments.to_vec(),
        },
    )?;
    // A scalar element cannot authenticate the descriptor selecting it. Only
    // the bare incoming parameter may supply its own unresolved reference;
    // stored reference fields still need the crossings resolved above.
    let incoming_reference = segments.is_empty()
        && place
            .root
            .as_symbol()
            .is_some_and(|symbol| program.state_parameter_by_symbol(state, symbol).is_some());
    if validation::reference_shell(program, selected_type).is_some() && !incoming_reference {
        return Some(false);
    }
    if region {
        let element = validation::collection_element_type_reference(program, selected_type)?;
        let carriers = crate::borrow::view_link::borrow_carrying_owner_paths(program, element)?;
        return Some(carriers.is_empty());
    }
    Some(true)
}

/// A returned descriptor belongs to the completed invocation, not a new local
/// owning its pointee. Instantiate its normal-return origins against the exact
/// captured actuals. Declaration lifetime demands remain additional retention
/// obligations; they do not replace the normal-return geometry.
fn returned_places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &FlowStateFact,
    call_handle: Handle<FlowCallFact>,
    result_suffix: &[facts::PlaceSegment],
    completion: HandleSpan<checked_trees::FlowConstraintRef>,
    active_calls: &mut Vec<Handle<FlowCallFact>>,
) -> Option<Vec<ResolvedAlias>> {
    use checked_trees::{
        CheckedReferenceOriginProjection as Projection, CheckedUnitStructuralPathSegment as Segment,
    };
    let call = facts.flow.control.calls.get(call_handle);
    let (operands, receiver_offset) =
        captured_call_operands(program, facts.flow.view(), state, call)?;
    let parameters = validation::call_target_parameters(program, call.target_symbol)?;
    let target = validation::call_target_state(program, call.target_symbol)?;
    let sources = validation::reference_result_custody::call_return_sources(
        program,
        facts,
        call.target_symbol,
    )?;
    let mut output = Vec::new();
    let mut matched = false;
    for source in sources {
        let result_path = validation::reference_result_custody::loan_place_segments(
            program,
            target.return_type,
            &source.path,
        )?;
        let Some(suffix) = result_suffix.strip_prefix(result_path.as_slice()) else {
            continue;
        };
        if !suffix.is_empty() {
            let returned_type = validation::project_type_reference_from_segments(
                program,
                target.return_type,
                &result_path,
            )?;
            let returned = validation::reference_shell(program, returned_type)?;
            // A region origin retains storage but cannot interpret a result's
            // field coordinates. Only an exact typed projected correspondence
            // can transport those coordinates into the captured input.
            if source.source.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
                || source.source.type_identity != program.normalized_type_identity(returned.referee)
            {
                return None;
            }
        }
        matched = true;
        let checked_trees::CheckedReferenceOriginRoot::Parameter { index } = source.source.root
        else {
            // An invocation-owned result is not backed by any formal input.
            // Its opaque incoming identity must survive as such; this query
            // may resolve caller-owned homes only from parameter origins.
            return None;
        };
        let (position, parameter) = parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| {
                !parameter.is_const
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .nth(index as usize)?;
        let operand = operands.get(receiver_offset + position)?;
        let backings = projected_operand_places(
            program,
            facts,
            loan_occurrences,
            state,
            operand,
            parameter.type_reference,
            &source.source.path,
            call.entry_constraints,
            active_calls,
        )?;
        for mut backing in backings {
            backing.place.segments.extend_from_slice(suffix);
            // Selection through the returned descriptor happens after this
            // nested call, not at its invocation. Cross any newly selected
            // stored descriptor using that completed observation's loans.
            for mut selected in retained_places(
                program,
                facts,
                loan_occurrences,
                state,
                call.statement_index,
                completion,
                backing.place,
                active_calls,
            )? {
                for parent in &backing.lineage {
                    if !selected.lineage.contains(parent) {
                        selected.lineage.push(*parent);
                    }
                }
                output.push(selected);
            }
        }
    }
    if !matched {
        return None;
    }
    let typed_trees::expression::ExpressionNode::Call(expression) =
        expressions(program).expression(call.authored_expression)
    else {
        return None;
    };
    for (source, _) in
        crate::borrow::call_reference_sources(program, state.state_symbol, expression)?
    {
        let result_path =
            crate::borrow::carrier_path(program, target.return_type, &source.owner_path)?;
        let result_path = validation::reference_result_custody::loan_place_segments(
            program,
            target.return_type,
            &result_path,
        )?;
        if !result_suffix.starts_with(&result_path) {
            continue;
        }
        let operand = operands.get(receiver_offset + source.parameter_index)?;
        let parameter = parameters.get(source.parameter_index)?;
        let mut path = Vec::new();
        if source.source == validation::borrow_contracts::ViewReturnInputSource::Stored {
            let carrier = validation::reference_result_custody::input_carrier_type(
                program,
                parameter.type_reference,
            )?;
            if validation::reference_shell(program, parameter.type_reference).is_some() {
                path.push(Projection::Exact(Segment::Referent));
            }
            path.extend(
                crate::borrow::carrier_path(program, carrier, &source.source_path)?
                    .into_iter()
                    .map(Projection::Exact),
            );
            path.push(Projection::Exact(Segment::Referent));
        }
        output.extend(projected_operand_places(
            program,
            facts,
            loan_occurrences,
            state,
            operand,
            parameter.type_reference,
            &path,
            call.entry_constraints,
            active_calls,
        )?);
    }
    Some(output)
}

/// Each descriptor crossing chooses its own epoch. The actual's descriptor
/// is frozen at operand completion; storage reached through that address is
/// observed at this nested invocation, before its body runs. Callee writes
/// have already selected replacement inputs in the normal-return relation.
fn projected_operand_places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &FlowStateFact,
    operand: &FlowOperandFact,
    reference: typed_trees::types::TypeReferenceHandle,
    path: &[checked_trees::CheckedReferenceOriginProjection],
    invocation: HandleSpan<checked_trees::FlowConstraintRef>,
    active_calls: &mut Vec<Handle<FlowCallFact>>,
) -> Option<Vec<ResolvedAlias>> {
    use checked_trees::{
        CheckedReferenceOriginProjection as Projection, CheckedUnitStructuralPathSegment as Segment,
    };
    if let checked_trees::FlowOperandCompletion::Selected(alternatives) = operand.completion {
        let alternatives = facts.flow.control.operand_alternatives.span(alternatives)?;
        if alternatives.is_empty() {
            return None;
        }
        let mut complete = Vec::new();
        for alternative in alternatives {
            if !alternative.source_arm.is_valid()
                || !facts.flow.control.operands.is_valid(alternative.value)
            {
                return None;
            }
            let places = projected_operand_places(
                program,
                facts,
                loan_occurrences,
                state,
                facts.flow.control.operands.get(alternative.value),
                reference,
                path,
                invocation,
                active_calls,
            )?;
            if places.is_empty() {
                return None;
            }
            complete.extend(places);
        }
        return Some(complete);
    }
    // A by-value constructed input already owns captures for its children.
    // Select the child before asking for a storage place: the constructor need
    // not have a named home, and later siblings cannot rewrite its capture.
    if !operand.constructed_children().is_empty() {
        let count = path
            .iter()
            .take_while(|projection| !matches!(projection, Projection::Exact(Segment::Referent)))
            .count();
        let selected = validation::reference_result_custody::origin_place_segments(
            program,
            reference,
            path[..count].iter().cloned(),
        )?;
        let mut children = facts
            .flow
            .control
            .operand_children
            .span(operand.constructed_children())?
            .iter()
            .filter_map(|child| {
                let prefix = facts.flow.control.operand_child_paths.span(child.path)?;
                selected
                    .starts_with(prefix)
                    .then_some((child, prefix.len()))
            });
        let (child, consumed) = children.next()?;
        if children.next().is_some() || consumed == 0 {
            return None;
        }
        return projected_operand_places(
            program,
            facts,
            loan_occurrences,
            state,
            facts.flow.control.operands.get(child.value),
            child.type_reference,
            &path[consumed..],
            invocation,
            active_calls,
        );
    }
    let source = place_at(
        program,
        state.state_symbol,
        operand.statement_index,
        operand.expression,
    )?;
    let borrowed = validation::reference_shell(program, reference);
    let mut selected = borrowed.as_ref().map_or(reference, |shell| shell.referee);
    let mut remaining = path;
    if borrowed.is_some()
        && matches!(
            remaining.first(),
            Some(Projection::Exact(Segment::Referent))
        )
    {
        remaining = &remaining[1..];
    }
    let mut backings = if borrowed.is_some() {
        retained_places(
            program,
            facts,
            loan_occurrences,
            state,
            operand.statement_index,
            operand.constraints,
            CapturedPlace {
                root: source.root,
                segments: source.segments,
                ..Default::default()
            },
            active_calls,
        )?
    } else {
        vec![ResolvedAlias {
            place: CapturedPlace {
                root: source.root,
                segments: source.segments,
                ..Default::default()
            },
            lineage: Vec::new(),
        }]
    };
    let mut constraints = if borrowed.is_some() {
        invocation
    } else {
        operand.constraints
    };
    loop {
        let count = remaining
            .iter()
            .take_while(|projection| !matches!(projection, Projection::Exact(Segment::Referent)))
            .count();
        let segments = validation::reference_result_custody::origin_place_segments(
            program,
            selected,
            remaining[..count].iter().cloned(),
        )?;
        selected = validation::project_type_reference_from_segments(program, selected, &segments)?;
        for backing in &mut backings {
            backing.place.segments.extend_from_slice(&segments);
        }
        remaining = &remaining[count..];
        if remaining.is_empty() {
            return Some(backings);
        }
        selected = validation::reference_shell(program, selected)?.referee;
        let mut crossed = Vec::new();
        for backing in backings {
            for mut source in retained_places(
                program,
                facts,
                loan_occurrences,
                state,
                operand.statement_index,
                constraints,
                backing.place,
                active_calls,
            )? {
                for parent in &backing.lineage {
                    if !source.lineage.contains(parent) {
                        source.lineage.push(*parent);
                    }
                }
                crossed.push(source);
            }
        }
        backings = crossed;
        constraints = invocation;
        remaining = &remaining[1..];
    }
}
