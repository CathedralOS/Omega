//! Rejoin a saved descriptor's content observation through its held permission.
//!
//! The source predicate is queried separately at its original completion. This
//! judgment never restores an old context to the current storage namespace.
//! A recorded invocation operand, exact descriptor geometry and a shared hold
//! must agree. Every intervening write must fit an authenticated permission
//! envelope compatible with that hold. Unknown writes and interior descriptor
//! crossings refuse; observation footprints never become physical addresses.

use arena::Handle;
use checked_trees::{BorrowAccessKind, CapturedPlace, CheckFacts, FlowCallFact, FlowStateFact};
use facts::PlaceRoot;
use validation::CanonicalPlace;

/// Return only the actual earlier value capture whose requested predicate may
/// be checked. Classification supplies no permission: the consuming Read and
/// complete retained source authority are independently authenticated below.
pub(crate) fn shared_reference_observation<'facts>(
    program: &dyn validation::CheckingInput,
    facts: &'facts CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    captured: &crate::flow::CapturedValue<'facts>,
) -> Result<crate::flow::CapturedValue<'facts>, &'static str> {
    let control = &facts.flow.control;
    let observation = captured
        .observation
        .recorded_in(control)
        .ok_or("the observation has no recorded operand association")?;
    let capture = captured
        .operand
        .recorded_in(control)
        .ok_or("the value producer has no recorded operand association")?;
    let (actuals, _) = crate::flow::captured_call_operands(program, &facts.flow, state, call)
        .ok_or("the invocation has no complete actual roster")?;
    if !call.operands.handles().any(|actual| actual == observation)
        || captured.observation.state_symbol != state.state_symbol
        || captured.observation.statement_index != call.statement_index
        || actuals.iter().any(|actual| {
            actual.state_symbol != state.state_symbol
                || actual.statement_index != call.statement_index
        })
        || actuals
            .iter()
            .filter(|actual| actual.expression == captured.observation.expression)
            .count()
            != 1
    {
        return Err("the observation is not this invocation's unique actual");
    }
    let borrow_call = facts
        .borrow
        .call_at(
            state.machine_symbol,
            state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        )
        .filter(|handle| facts.borrow.calls.is_valid(*handle))
        .map(|handle| facts.borrow.calls.get(handle))
        .ok_or("the invocation has no checked access roster")?;
    if borrow_call.target_symbol != call.target_symbol
        || borrow_call.authored_expression != call.authored_expression
        || borrow_call.accesses != call.accesses
    {
        return Err("the checked access roster belongs to another invocation");
    }
    let mut accesses = facts
        .borrow
        .argument_accesses
        .span(call.accesses)
        .ok_or("the invocation access span is unavailable")?
        .iter()
        .filter(|access| access.expression == captured.observation.expression);
    let access = accesses.next().ok_or("the actual has no held access")?;
    if accesses.next().is_some()
        || access.kind != BorrowAccessKind::Read
        || access.end_before_access.is_some()
        || access.released_at.is_valid()
    {
        return Err("the actual has no unique continuing shared hold");
    }
    let selected_type = validation::project_type_reference_from_segments(
        program,
        captured.operand.type_reference,
        &captured.projection.remaining,
    )
    .ok_or("the saved projection has no exact type")?;
    let reference = validation::reference_shell(program, selected_type)
        .filter(|reference| reference.access == language_semantics::ReferenceAccess::Shared)
        .ok_or("the saved projection is not a shared reference")?;
    if !validation::has_contents(
        program,
        reference.referee,
        &[],
        validation::ContentsRequirement::StableObservation,
    ) {
        return Err("the saved referee has mutable or unclassified observations");
    }
    let mut source = capture;
    let mut path = captured.projection.remaining.clone();
    let mut visited = Vec::new();
    while let checked_trees::FlowOperandCompletion::Copied { copy, path: prefix } =
        control.operands.get(source).completion
    {
        if !control.copied_values.is_valid(copy) || visited.contains(&source) {
            return Err("the saved copy graph is missing or cyclic");
        }
        visited.push(source);
        let prefix = control
            .operand_child_paths
            .span(prefix)
            .ok_or("the saved copy has no declared projection")?;
        let mut projected = prefix.to_vec();
        projected.extend(path);
        path = projected;
        source = control.copied_values.get(copy).source;
        if !control.operands.is_valid(source) {
            return Err("the saved copy has no source occurrence");
        }
    }
    if visited.is_empty() {
        return Err("the reference has no saved copy occurrence");
    }
    let earlier = control.operands.get(source);
    let source_reference =
        validation::project_type_reference_from_segments(program, earlier.type_reference, &path)
            .and_then(|reference| validation::reference_shell(program, reference))
            .filter(|source| {
                source.access == language_semantics::ReferenceAccess::Shared
                    && program.normalized_type_identity(source.referee)
                        == program.normalized_type_identity(reference.referee)
            })
            .ok_or("the saved source has no matching shared referee type")?;
    if !validation::has_contents(
        program,
        source_reference.referee,
        &[],
        validation::ContentsRequirement::StableObservation,
    ) {
        return Err("the source referee has no stable observation shape");
    }
    let epoch = actuals
        .last()
        .ok_or("the invocation has no completed actual cut")?;
    let backings = super::completed_backings::resolve_at_path(
        program,
        facts,
        state,
        capture,
        &captured.projection.remaining,
    )?;
    if backings.places.is_empty() {
        return Err("the saved reference has no complete backing roster");
    }
    let actual = super::place_access::argument_backings(
        program,
        facts,
        state,
        borrow_call,
        call.entry_constraints,
        access,
    )?;
    let same = |left: &super::aliases::ResolvedAlias, right: &super::aliases::ResolvedAlias| {
        left.place == right.place
            && left.lineage.iter().all(|loan| right.lineage.contains(loan))
            && right.lineage.iter().all(|loan| left.lineage.contains(loan))
    };
    if !actual.represents(observation)
        || actual.places.is_empty()
        || !actual
            .places
            .iter()
            .all(|actual| backings.places.iter().any(|saved| same(actual, saved)))
        || !backings
            .places
            .iter()
            .all(|saved| actual.places.iter().any(|actual| same(actual, saved)))
    {
        return Err(
            "the continuing access has a different descriptor occurrence or supplier frontier",
        );
    }
    let writes = crate::flow::operand_contents_writes_between(program, &facts.flow, earlier, epoch)
        .ok_or("the observation interval has missing, reordered or unknown writes")?;
    let mut lookup = None;
    for (statement, write) in writes {
        let grants =
            write_permission_envelope(program, facts, state, earlier, epoch, statement, write)?;
        if grants.is_empty()
            || !grants.iter().all(|grant| {
                backings.places.iter().all(|saved| {
                    super::overlap::captured_place_compatibility(
                        program,
                        &saved.place,
                        &BorrowAccessKind::Read,
                        &grant.place,
                        &BorrowAccessKind::Mutable,
                        &[],
                        &mut lookup,
                    )
                    .non_interfering
                })
            })
        {
            return Err("an intervening write may change the saved referee");
        }
    }
    let source = crate::flow::CapturedOperand::recorded(control, source)
        .ok_or("the earlier observation association is unavailable")?;
    let projection = crate::flow::LiteralExpressionProjection {
        expression: source.expression,
        remaining: path,
        destination: Vec::new(),
    };
    Ok(crate::flow::CapturedValue {
        observation: source,
        observation_type: source.type_reference,
        observation_projection: projection.clone(),
        operand: source,
        type_reference: source.type_reference,
        projection,
        choices: captured.choices.clone(),
    })
}

/// A contents footprint may overapproximate a whole invocation referent. Use
/// the complete grant, not guessed bytes: a mutable Buffer grant can replace
/// its pointer fields without acquiring the nested shared Cell permission.
/// Crossing a stored descriptor would require that descriptor's write-time
/// occurrence, which this footprint does not carry; it therefore refuses.
fn write_permission_envelope(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    earlier: &checked_trees::FlowOperandFact,
    epoch: &checked_trees::FlowOperandFact,
    statement: usize,
    write: &checked_trees::FlowOperandReferent,
) -> Result<Vec<super::aliases::ResolvedAlias>, &'static str> {
    let symbol = match write.root {
        PlaceRoot::Symbol(symbol) => symbol,
        PlaceRoot::InvocationReferent {
            parameter,
            type_reference,
        } => {
            let machine = program
                .machine_by_symbol(state.machine_symbol)
                .ok_or("the write has no owning machine")?;
            let entry = program
                .machine_states(machine)
                .first()
                .filter(|entry| entry.symbol == state.state_symbol)
                .ok_or("the invocation envelope has no current entry binding")?;
            let parameter_row = program
                .state_parameter_by_symbol(entry, parameter)
                .filter(|row| {
                    row.type_reference == type_reference
                        && !row.is_const
                        && !row.relevance.is_erased()
                })
                .ok_or("the invocation envelope has no authenticated formal")?;
            if validation::reference_shell(program, parameter_row.type_reference).is_none() {
                return Err("the invocation envelope has no reference grant");
            }
            parameter
        }
        _ => return Err("the write has no authenticated storage permission"),
    };
    let root = CanonicalPlace {
        root: PlaceRoot::Symbol(symbol),
        segments: Vec::new(),
    };
    let root_type = validation::canonical_place_type_reference(
        program,
        state.state_symbol,
        earlier.statement_index,
        &root,
    )
    .ok_or("the writer did not exist at the saved observation")?;
    let segments = facts
        .flow
        .control
        .operand_referent_segments
        .span(write.segments)
        .ok_or("the write footprint has no complete path")?;
    let reference = validation::reference_shell(program, root_type);
    let owned_type = if let Some(reference) = reference.as_ref() {
        if !matches!(
            reference.access,
            language_semantics::ReferenceAccess::Mutable
                | language_semantics::ReferenceAccess::WriteOnly
        ) || !crate::flow::operand_bindings_agree(program, &facts.flow, earlier, epoch, &root)
        {
            return Err("the writer has no unchanged mutable descriptor grant");
        }
        reference.referee
    } else {
        root_type
    };
    if validation::owned_projection_type_reference(program, owned_type, segments).is_none()
        || validation::canonical_place_type_reference(program, state.state_symbol, statement, &root)
            != Some(root_type)
    {
        return Err("the write crosses an unavailable or mutable descriptor boundary");
    }
    let place = CapturedPlace {
        incoming_reference: Handle::invalid(),
        root: root.root,
        segments: Vec::new(),
    };
    if reference.is_none() {
        return Ok(vec![super::aliases::ResolvedAlias {
            place,
            lineage: Vec::new(),
        }]);
    }
    super::access_sources::resolve_descriptor_all(
        program,
        facts,
        state,
        earlier.statement_index,
        earlier.constraints,
        place,
    )
    .ok_or("the writer has no retained permission correspondence")
}
