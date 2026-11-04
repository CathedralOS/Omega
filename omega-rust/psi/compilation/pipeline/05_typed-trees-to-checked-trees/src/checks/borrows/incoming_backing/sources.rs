//! Keep the data reached by a pointer separate from the field storing it.
//! Resolve every possible source at its recorded read or call-entry snapshot.
//! A recast may identify only an enclosing storage region; keep that whole
//! region unless checked field/index coordinates identify a smaller projection.
use arena::Handle;
use checked_trees::{
    CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact, IncomingReferenceOrigin,
};
use facts::{PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use validation::CanonicalPlace;
use validation::completed_storage::StoredOperand;

use super::{Backing, IncomingBacking, IncomingLoad, Invocation, Storage};

pub(super) fn state_flow(
    facts: &CheckFacts,
    symbol: SymbolHandle,
) -> Result<&FlowStateFact, &'static str> {
    let mut states = facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, state)| (state.state_symbol == symbol).then_some(state));
    let state = states
        .next()
        .ok_or("the compatibility relation has no checked state")?;
    if states.next().is_some() {
        return Err("the compatibility relation has ambiguous checked state ownership");
    }
    Ok(state)
}

pub(super) fn require_static_path(path: &[PlaceSegment]) -> Result<(), &'static str> {
    if path.iter().all(|segment| {
        matches!(
            segment,
            PlaceSegment::Field { .. }
                | PlaceSegment::Case { .. }
                | PlaceSegment::FixedIndex { .. }
        )
    }) {
        Ok(())
    } else {
        Err("incoming compatibility requires exact captured structural coordinates")
    }
}

/// The activation identifies the completed read which supplied this pointer.
/// A later copy or a later load of the same slot cannot replace that event.
pub(super) fn incoming_backing(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    identity: Handle<checked_trees::IncomingReferenceIdentity>,
) -> Result<IncomingLoad, &'static str> {
    let load = validation::reference_result_custody::completed_descriptor_load(
        program, facts, state, identity,
    )?;
    let mut pending = vec![load.sources];
    let mut output = Vec::new();
    while let Some(source) = pending.pop() {
        match source {
            StoredOperand::Incoming(slot) => {
                require_input_slot(program, state, &slot)?;
                output.push(IncomingBacking {
                    slot,
                    suffix: Vec::new(),
                });
            }
            StoredOperand::Alternatives(alternatives) if !alternatives.is_empty() => {
                pending.extend(alternatives)
            }
            _ => {
                return Err(
                    "the descriptor load has an unavailable or non-entry source alternative",
                );
            }
        }
    }
    if output.is_empty() {
        return Err("the descriptor load has no incoming source alternatives");
    }
    Ok(IncomingLoad {
        capture: load.capture,
        sources: output,
    })
}

fn require_input_slot(
    program: &dyn validation::CheckingInput,
    state: &FlowStateFact,
    slot: &CanonicalPlace,
) -> Result<(), &'static str> {
    require_static_path(&slot.segments)?;
    let declaration = program
        .state_by_symbol(state.state_symbol)
        .ok_or("the incoming slot has no declaration state")?;
    if !slot.root.as_symbol().is_some_and(|symbol| {
        program
            .state_parameter_by_symbol(declaration, symbol)
            .is_some()
    }) {
        return Err("the unchanged descriptor slot has no exact invocation input");
    }
    Ok(())
}

pub(super) fn input_storage(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    place: &CapturedPlace,
) -> Result<CanonicalPlace, &'static str> {
    let mut slot = CanonicalPlace {
        root: place.root,
        segments: place.segments.clone(),
    };
    if place.incoming_reference.is_valid() {
        if !facts
            .borrow
            .incoming_references
            .is_valid(place.incoming_reference)
        {
            return Err("the exclusive storage has no checked incoming namespace");
        }
        let incoming = facts
            .borrow
            .incoming_references
            .get(place.incoming_reference);
        if incoming.machine_symbol != state.machine_symbol
            || incoming.state_symbol != state.state_symbol
        {
            return Err("the exclusive storage belongs to another invocation");
        }
        match incoming.origin {
            IncomingReferenceOrigin::Parameter { symbol } => {
                slot.root = PlaceRoot::Symbol(symbol);
                slot.segments = crate::borrow::owner_place_segments(
                    facts
                        .borrow
                        .owner_segments
                        .span(incoming.owner_path)
                        .ok_or("the exclusive storage has no complete input owner path")?,
                );
                slot.segments.extend_from_slice(&place.incoming_owner_path);
                slot.segments.extend_from_slice(&place.segments);
            }
            IncomingReferenceOrigin::ParameterReferent { symbol } => {
                slot.root = PlaceRoot::Symbol(symbol)
            }
            _ => return Err("the exclusive storage has no exact input-relative relation"),
        }
    }
    require_input_slot(program, state, &slot)?;
    let declaration = program
        .state_by_symbol(state.state_symbol)
        .ok_or("the exclusive storage has no declaration state")?;
    let symbol = slot
        .root
        .as_symbol()
        .ok_or("the exclusive storage has no parameter identity")?;
    let parameter = program
        .state_parameter_by_symbol(declaration, symbol)
        .ok_or("the exclusive storage has no exact input parameter")?;
    if !validation::reference_shell(program, parameter.type_reference)
        .is_some_and(|shell| shell.access.is_exclusive())
    {
        return Err("the compatibility demand is not supplied by an exclusive input");
    }
    Ok(slot)
}

pub(super) fn actual_for(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    invocation: &Invocation<'_>,
    parameter: PlaceRoot,
) -> Result<Handle<FlowOperandFact>, &'static str> {
    let target = program
        .state_by_symbol(validation::call_target_entry_symbol(
            program,
            invocation.call.target_symbol,
        ))
        .ok_or("the ordinary invocation has no exact target state")?;
    let parameter = parameter
        .as_symbol()
        .ok_or("the relation is not rooted in an input parameter")?;
    let index = program
        .state_parameters(target)
        .iter()
        .position(|formal| formal.symbol == parameter)
        .ok_or("the relation's parameter is absent from its invocation target")?;
    let actuals = invocation.call.argument_operands();
    let index = u32::try_from(index)
        .map_err(|_| "the input ordinal does not fit the completed call roster")?;
    if index >= actuals.count() {
        return Err("the ordinary invocation has no completed actual for the relation");
    }
    let actual = actuals.handle_at(index);
    if !facts.flow.control.operands.is_valid(actual)
        || facts.flow.control.operands.get(actual).state_symbol != invocation.caller.state_symbol
    {
        return Err("the ordinary invocation's actual has no exact completed owner");
    }
    Ok(actual)
}

pub(super) fn collect_backings(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    source: StoredOperand,
    output: &mut Vec<Backing>,
) -> Result<(), &'static str> {
    match source {
        StoredOperand::Unavailable => return Err("an actual's descriptor source is unavailable"),
        StoredOperand::Alternatives(alternatives) => {
            if alternatives.is_empty() {
                return Err("an actual's descriptor source alternatives are empty");
            }
            for alternative in alternatives {
                collect_backings(program, facts, state, alternative, output)?;
            }
        }
        StoredOperand::Incoming(slot) => {
            require_input_slot(program, state, &slot)?;
            output.push(Backing::Incoming(IncomingBacking {
                slot,
                suffix: Vec::new(),
            }));
        }
        StoredOperand::Value {
            capture,
            projection,
            ..
        } => {
            if !facts.flow.control.operands.is_valid(capture) {
                return Err("an actual's descriptor source capture is missing");
            }
            let operand = facts.flow.control.operands.get(capture);
            let descriptor_loan = operand.descriptor_permission.loaded_loan();
            if projection.is_empty() && facts.borrow.loans.is_valid(descriptor_loan) {
                let loan = facts.borrow.loans.get(descriptor_loan);
                if facts
                    .borrow
                    .incoming_references
                    .is_valid(loan.incoming_reference)
                    && matches!(
                        facts
                            .borrow
                            .incoming_references
                            .get(loan.incoming_reference)
                            .origin,
                        IncomingReferenceOrigin::DescriptorLoad { .. }
                    )
                {
                    for mut source in
                        incoming_backing(program, facts, state, loan.incoming_reference)?.sources
                    {
                        source
                            .suffix
                            .extend_from_slice(facts.borrow.loan_segments(loan));
                        output.push(Backing::Incoming(source));
                    }
                    return Ok(());
                }
            }
            let places = super::super::captured_descriptors::places(
                program,
                facts,
                state,
                capture,
                &projection,
            )
            .ok_or("an actual's completed descriptor has no complete backing association")?;
            if places.is_empty() {
                // The backing owner returns a known empty set only for
                // established immutable image/empty descriptor producers.
                output.push(Backing::Immutable);
            }
            let projectable = validation::completed_storage::projectable_reference_places(
                program,
                &facts.flow.control,
                capture,
                &projection,
            );
            for place in places {
                if facts
                    .borrow
                    .incoming_references
                    .is_valid(place.incoming_reference)
                    && matches!(
                        facts
                            .borrow
                            .incoming_references
                            .get(place.incoming_reference)
                            .origin,
                        IncomingReferenceOrigin::DescriptorLoad { .. }
                    )
                {
                    for mut source in
                        incoming_backing(program, facts, state, place.incoming_reference)?.sources
                    {
                        source.suffix.extend_from_slice(&place.segments);
                        output.push(Backing::Incoming(source));
                    }
                } else {
                    let exact = projectable.as_ref().is_some_and(|places| {
                        places.iter().any(|candidate| {
                            candidate.root == place.root && candidate.segments == place.segments
                        })
                    });
                    output.push(Backing::Place {
                        place,
                        projectable: exact,
                    });
                }
            }
        }
    }
    Ok(())
}

/// Locate the field being overwritten without following its stored pointer.
/// Replacing the pointer changes that field's bytes, not the data it pointed to.
pub(super) fn actual_storage(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    actual: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    current: Handle<checked_trees::FlowStorageSnapshot>,
) -> Result<Vec<Storage>, &'static str> {
    require_static_path(path)?;
    let operand = facts.flow.control.operands.get(actual);
    let mut crossing = 0;
    for length in 1..path.len() {
        let reference = validation::project_type_reference_from_segments(
            program,
            operand.type_reference,
            &path[..length],
        )
        .ok_or("the exclusive actual has no exact typed storage projection")?;
        if validation::reference_shell(program, reference).is_some() {
            crossing = length;
        }
    }
    // The outer pointer was captured when this argument was evaluated.
    // Resolve fields behind it at call entry, after all arguments have finished.
    let mut backings = Vec::new();
    if crossing == 0 {
        collect_backings(
            program,
            facts,
            state,
            StoredOperand::Value {
                capture: actual,
                reference: operand.type_reference,
                projection: Vec::new(),
                slot: None,
            },
            &mut backings,
        )?;
    } else {
        let selected = validation::completed_storage::reference_sources_at_path(
            program,
            &facts.flow.control,
            actual,
            &path[..crossing],
            current,
        );
        collect_backings(program, facts, state, selected, &mut backings)?;
    }
    let mut output = Vec::new();
    for backing in backings {
        match backing {
            Backing::Place {
                mut place,
                projectable,
            } => {
                // A recast enclosure remains whole. Only the storage owner's
                // independently retained typed coordinates permit this suffix.
                if projectable {
                    place.segments =
                        facts::project_captured_view_segments(&place.segments, &path[crossing..])
                            .ok_or("the exclusive actual cannot project its captured storage")?;
                }
                output.push(Storage::Place(place));
            }
            Backing::Incoming(mut source) => {
                source.slot.segments.extend_from_slice(&source.suffix);
                source.slot.segments.extend_from_slice(&path[crossing..]);
                output.push(Storage::Incoming(source.slot));
            }
            Backing::Immutable => {
                return Err("immutable image backing cannot supply exclusive storage");
            }
        }
    }
    Ok(output)
}

pub(super) fn input_backing(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    place: &CapturedPlace,
) -> Result<IncomingBacking, &'static str> {
    let mut slot = CanonicalPlace {
        root: place.root,
        segments: Vec::new(),
    };
    if place.incoming_reference.is_valid() {
        if !facts
            .borrow
            .incoming_references
            .is_valid(place.incoming_reference)
        {
            return Err("the backing has no checked incoming namespace");
        }
        let incoming = facts
            .borrow
            .incoming_references
            .get(place.incoming_reference);
        if incoming.machine_symbol != state.machine_symbol
            || incoming.state_symbol != state.state_symbol
        {
            return Err("the backing belongs to another invocation");
        }
        match incoming.origin {
            IncomingReferenceOrigin::Parameter { symbol } => {
                slot.root = PlaceRoot::Symbol(symbol);
                slot.segments = crate::borrow::owner_place_segments(
                    facts
                        .borrow
                        .owner_segments
                        .span(incoming.owner_path)
                        .ok_or("the backing has no complete input owner path")?,
                );
                slot.segments.extend_from_slice(&place.incoming_owner_path);
            }
            IncomingReferenceOrigin::ParameterReferent { symbol } => {
                slot.root = PlaceRoot::Symbol(symbol)
            }
            _ => return Err("the backing has no exact input-relative relation"),
        }
    }
    require_input_slot(program, state, &slot)?;
    Ok(IncomingBacking {
        slot,
        suffix: place.segments.clone(),
    })
}
