//! Join a completed operand to its existing call access or stored loan.
//!
//! Call entry is too late to recover an earlier operand's descriptor contents.
//! Resolve checked accesses in its completion constraints, preserving the original
//! access occurrence and every retained ancestor. Constructed children use
//! their own evaluations and resolved destination paths. Missing provenance
//! produces no resource: carry must still establish complete leaf coverage.
//! Lifetime retention keeps the complete enclosing owner, not a second copy
//! of referent geometry; the evaluated actual already owns its captured address.
//! These rows retain demands; they authorize no access, restoration or parking.

use arena::{Arena, Handle, HandleSpan};
use checked_trees::{
    BorrowLoanFact, BorrowLoanOwnerSegment, CapturedPlace, CheckFacts,
    CheckedCallBorrowLoanResource, CheckedCallBorrowLoanSource, CheckedDirectBorrowParentLifetime,
    FlowCallFact, FlowOperandFact, FlowStateFact,
};

struct ResourceDraft {
    resource: CheckedCallBorrowLoanResource,
    path: Vec<BorrowLoanOwnerSegment>,
}

pub(super) fn install(program: &dyn validation::CheckingInput, facts: &mut CheckFacts) {
    let mut resources = Vec::new();
    let mut parents = Arena::new();
    for (_, state) in facts.flow.control.states.iter() {
        for call in state.calls.handles() {
            let row = facts.flow.control.calls.get(call);
            for operand in row.argument_operands().handles() {
                collect(
                    program,
                    facts,
                    state,
                    call,
                    operand,
                    operand,
                    &[],
                    &mut resources,
                    &mut parents,
                );
            }
        }
    }
    facts.borrow.call_loan_resources.reset_retain_capacity();
    facts.borrow.call_loan_paths.reset_retain_capacity();
    facts.borrow.call_loan_parents = parents;
    for mut draft in resources {
        let owner = facts
            .flow
            .control
            .operands
            .get(draft.resource.owner_operand);
        let Some(path) = crate::borrow::carrier_path(program, owner.type_reference, &draft.path)
        else {
            continue;
        };
        draft.resource.capture_path = facts.borrow.call_loan_paths.insert_many(path);
        draft.resource.owner_path = facts.borrow.owner_segments.insert_many(draft.path);
        facts.borrow.call_loan_resources.insert(draft.resource);
    }
}

fn collect(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    owner_operand: Handle<FlowOperandFact>,
    operand: Handle<FlowOperandFact>,
    path: &[BorrowLoanOwnerSegment],
    resources: &mut Vec<ResourceDraft>,
    parents: &mut Arena<Handle<BorrowLoanFact>>,
) {
    if !facts.flow.control.operands.is_valid(operand) {
        return;
    }
    let capture = facts.flow.control.operands.get(operand);
    let Some(paths) =
        crate::borrow::view_link::borrow_carrying_owner_paths(program, capture.type_reference)
    else {
        return;
    };
    if paths.is_empty() {
        return;
    }
    if !capture.alternatives.is_empty() {
        let mut complete = Vec::new();
        for alternative in capture.alternatives.handles() {
            let mut branch = Vec::new();
            collect(
                program,
                facts,
                state,
                call,
                owner_operand,
                alternative,
                path,
                &mut branch,
                parents,
            );
            if !paths.iter().all(|leaf| {
                branch
                    .iter()
                    .any(|draft| draft.path.strip_prefix(path) == Some(leaf.as_slice()))
            }) {
                return;
            }
            complete.extend(branch);
        }
        resources.extend(complete);
        return;
    }
    if !capture.children.is_empty() {
        for child in facts
            .flow
            .control
            .operand_children
            .span_or_empty(capture.children)
        {
            let mut child_path = path.to_vec();
            child_path.extend(crate::borrow::owner_path_from_place_segments(
                program,
                facts
                    .flow
                    .control
                    .operand_child_paths
                    .span_or_empty(child.path),
            ));
            collect(
                program,
                facts,
                state,
                call,
                owner_operand,
                child.value,
                &child_path,
                resources,
                parents,
            );
        }
        return;
    }
    let invocation = facts.flow.control.calls.get(call);
    let accesses = invocation.accesses.handles().filter(|access| {
        let access = facts.borrow.argument_accesses.get(*access);
        access.expression == capture.expression && access.end_before_access.is_none()
    });
    if validation::reference_shell(program, capture.type_reference).is_some() {
        // Leaf coverage is all-or-nothing. Publishing only the alternatives
        // that resolved would make an incomplete backing roster look complete.
        let mut captured_resources = Vec::new();
        for access in accesses {
            let access_row = facts.borrow.argument_accesses.get(access);
            let source = CapturedPlace {
                incoming_reference: Handle::invalid(),
                root: access_row.root,
                segments: facts.borrow.access_segments(access_row).to_vec(),
            };
            let Some(backings) = retained_places(
                program,
                facts,
                state,
                capture.statement_index,
                capture.constraints,
                source,
                &mut Vec::new(),
            ) else {
                return;
            };
            for backing in backings {
                captured_resources.push(ResourceDraft {
                    resource: CheckedCallBorrowLoanResource {
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        call,
                        owner_operand,
                        operand,
                        owner_path: HandleSpan::empty(),
                        capture_path: HandleSpan::empty(),
                        source: CheckedCallBorrowLoanSource::CallAccess {
                            access,
                            parents: parents.insert_many(backing.lineage),
                        },
                        access: access_row.kind.clone(),
                        parent_lifetime: CheckedDirectBorrowParentLifetime {
                            machine_symbol: state.machine_symbol,
                            state_symbol: state.state_symbol,
                            root: backing.place.root,
                        },
                    },
                    path: path.to_vec(),
                });
            }
        }
        resources.extend(captured_resources);
        // A reference actual must have a retained invocation access. A copied
        // observation or a nested call's expired access cannot stand in for it.
        return;
    }
    let Some(source) = validation::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        capture.statement_index,
        capture.expression,
    ) else {
        return;
    };
    let source_prefix = crate::borrow::owner_path_from_place_segments(program, &source.segments);
    for leaf in paths {
        let mut source_path = source_prefix.clone();
        source_path.extend(leaf.iter().copied());
        for loan in facts
            .flow
            .borrow_loan_constraints(capture.constraints)
            .filter(|loan| {
                let loan = facts.borrow.loans.get(*loan);
                loan.storage_owner == source.root
                    && facts.borrow.loan_owner_path(loan) == source_path
            })
        {
            let row = facts.borrow.loans.get(loan);
            let mut owner_path = path.to_vec();
            owner_path.extend(leaf.iter().copied());
            resources.push(ResourceDraft {
                resource: CheckedCallBorrowLoanResource {
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    call,
                    owner_operand,
                    operand,
                    owner_path: HandleSpan::empty(),
                    capture_path: HandleSpan::empty(),
                    source: CheckedCallBorrowLoanSource::StoredLoan { loan },
                    access: row.kind.clone(),
                    parent_lifetime: CheckedDirectBorrowParentLifetime {
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        root: row.root,
                    },
                },
                path: owner_path,
            });
        }
    }
}

/// Lifetime retention follows the captured contents of every stored reference
/// crossing, just as a referent access does. Resolving only a reference local
/// leaves `holder.reference_field` anchored to the holder instead of its
/// referent. The original invocation access remains unchanged; each crossed
/// stored loan joins the parent roster so suspension also retains its resource
/// obligations. This does not change the alias checker's access exemptions.
fn retained_places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: usize,
    constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    source: CapturedPlace,
    active_calls: &mut Vec<Handle<FlowCallFact>>,
) -> Option<Vec<crate::checks::borrows::aliases::ResolvedAlias>> {
    let typed_state = program.state_by_symbol(state.state_symbol)?;
    let mut pending = crate::checks::borrows::access_sources::resolve_captured_all(
        program,
        facts,
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
            roots.push(crate::checks::borrows::aliases::ResolvedAlias {
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
            let selected_type = validation::canonical_place_type_reference(
                program,
                state.state_symbol,
                typed_state.statement_nodes.count() as usize,
                &validation::CanonicalPlace {
                    root: place.root,
                    segments: place.segments.clone(),
                },
            )?;
            let incoming_parameter = place.root.as_symbol().is_some_and(|symbol| {
                program
                    .state_parameter_by_symbol(typed_state, symbol)
                    .is_some()
            });
            if validation::reference_shell(program, selected_type).is_some() && !incoming_parameter
            {
                // The geometry helper's unchanged fallback is not evidence
                // that an unresolved local descriptor owns its referent.
                return None;
            }
            roots.push(crate::checks::borrows::aliases::ResolvedAlias {
                place,
                lineage: ancestors,
            });
        }
    }
    Some(roots)
}

/// A returned descriptor belongs to the completed invocation, not a new local
/// owning its pointee. Instantiate its normal-return origins against the exact
/// captured actuals. Declaration lifetime demands remain additional retention
/// obligations; they do not replace the normal-return geometry.
fn returned_places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call_handle: Handle<FlowCallFact>,
    result_suffix: &[facts::PlaceSegment],
    completion: HandleSpan<checked_trees::FlowConstraintRef>,
    active_calls: &mut Vec<Handle<FlowCallFact>>,
) -> Option<Vec<crate::checks::borrows::aliases::ResolvedAlias>> {
    use checked_trees::{
        CheckedReferenceOriginProjection as Projection, CheckedUnitStructuralPathSegment as Segment,
    };
    let call = facts.flow.control.calls.get(call_handle);
    let (operands, receiver_offset) =
        crate::flow::captured_call_operands(program, &facts.flow, state, call)?;
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
    let typed_trees::expression::ExpressionNode::Call(expression) = program
        .expression_table
        .expression(call.authored_expression)
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
    state: &FlowStateFact,
    operand: &FlowOperandFact,
    reference: typed_trees::types::TypeReferenceHandle,
    path: &[checked_trees::CheckedReferenceOriginProjection],
    invocation: HandleSpan<checked_trees::FlowConstraintRef>,
    active_calls: &mut Vec<Handle<FlowCallFact>>,
) -> Option<Vec<crate::checks::borrows::aliases::ResolvedAlias>> {
    use checked_trees::{
        CheckedReferenceOriginProjection as Projection, CheckedUnitStructuralPathSegment as Segment,
    };
    if !operand.alternatives.is_empty() {
        let mut complete = Vec::new();
        for alternative in facts.flow.control.operands.span(operand.alternatives)? {
            let places = projected_operand_places(
                program,
                facts,
                state,
                alternative,
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
    if !operand.children.is_empty() {
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
            .span(operand.children)?
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
            state,
            facts.flow.control.operands.get(child.value),
            child.type_reference,
            &path[consumed..],
            invocation,
            active_calls,
        );
    }
    let source = validation::canonical_place_from_expression_in_state(
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
            state,
            operand.statement_index,
            operand.constraints,
            CapturedPlace {
                incoming_reference: Handle::invalid(),
                root: source.root,
                segments: source.segments,
            },
            active_calls,
        )?
    } else {
        vec![crate::checks::borrows::aliases::ResolvedAlias {
            place: CapturedPlace {
                incoming_reference: Handle::invalid(),
                root: source.root,
                segments: source.segments,
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
