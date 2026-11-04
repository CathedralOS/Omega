//! Rejoin a retained input edge to the original applied call declaration.
//! An older stored capture supplies provenance, not a stronger loan contract.
use checked_trees::{FlowControlFacts, FlowOperandRetainedSupplier, ViewReturnInputSource};
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn source_type(
    program: &dyn validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    control: &FlowControlFacts,
    supplier: &FlowOperandRetainedSupplier,
) -> Option<TypeReferenceHandle> {
    let contract = supplier.contract?;
    if !control.operands.is_valid(contract.input) || !control.operands.is_valid(supplier.operand) {
        return None;
    }
    let input = control.operands.get(contract.input);
    let machine = program.machine_holding_state(contract.state_symbol)?;
    let borrowed = borrow.call_at(
        machine.symbol,
        contract.state_symbol,
        contract.statement_index,
        contract.call_ordinal,
    )?;
    let borrowed = borrow.calls.get(borrowed);
    if borrowed.authored_expression != contract.invocation {
        return None;
    }
    let site = validation::find_call_site(
        program,
        machine.symbol,
        contract.state_symbol,
        contract.statement_index,
        contract.call_ordinal,
    )?;
    let call = contract
        .invocation
        .is_valid()
        .then(|| program.expression_table.call_node(contract.invocation))
        .flatten();
    let receiver = usize::from(call.is_some_and(|call| call.receiver.is_valid()));
    let arguments = if contract.output_parameter.is_none() {
        program.expression_table.expression_handles(call?.arguments)
    } else {
        validation::call_site_argument_expressions(program, &site)
    };
    if arguments.get(contract.parameter_index).copied() != Some(input.expression)
        || input.state_symbol != contract.state_symbol
        || input.statement_index != contract.statement_index
    {
        return None;
    }
    // The actual performed tuple, not an equal type or source spelling,
    // authenticates the input retained by this edge.
    let state = control.unique_state(machine.symbol, contract.state_symbol)?;
    let mut actual_calls = state.calls.handles().filter_map(|handle| {
        let recorded = control.calls.get(handle);
        (recorded.statement_index == contract.statement_index
            && recorded.call_ordinal == contract.call_ordinal
            && recorded.authored_expression == contract.invocation
            && recorded.target_symbol == borrowed.target_symbol)
            .then_some(recorded)
    });
    let actual = actual_calls.next()?;
    if actual_calls.next().is_some() {
        return None;
    }
    if actual.operands.count() as usize != arguments.len() + receiver
        || actual
            .operands
            .handles()
            .nth(contract.parameter_index + receiver)
            != Some(contract.input)
    {
        return None;
    }
    let source_path = control.operand_retained_paths.span(contract.source_path)?;
    let owner_path = control.operand_retained_paths.span(contract.owner_path)?;
    let projection = control.operand_retained_paths.span(supplier.projection)?;
    if supplier.operand != contract.input || !same_path(program, projection, source_path) {
        return None;
    }
    match contract.source {
        ViewReturnInputSource::Stored => {
            validate_stored_sources(program, control, &contract, actual.invocation_storage)?;
        }
        ViewReturnInputSource::Referent if !contract.stored_sources.is_empty() => return None,
        ViewReturnInputSource::Referent => {}
    }
    let identity_type = program.normalized_type_identity(contract.source_type);
    if !identity_type.is_valid() {
        return None;
    }
    let (source, source_type, path) = if let Some(position) = contract.output_parameter {
        let parameters = site.parameters(program, machine.symbol)?;
        let parameter = parameters.get(position)?;
        let identity = facts::InvocationOutputIdentity {
            state_symbol: contract.state_symbol,
            statement_index: contract.statement_index,
            call_ordinal: contract.call_ordinal,
            parameter_symbol: parameter.symbol,
        };
        let completion = completed_output_retention(
            borrow,
            machine.symbol,
            contract.state_symbol,
            contract.statement_index,
            borrowed,
            identity,
            parameter.type_reference,
            &crate::borrow::owner_path_from_place_segments(program, owner_path),
        );
        if let Err(reason) = completion {
            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                eprintln!(
                    "STRUCT output-lifetime-contract-refused output={identity:?} reason={reason}"
                );
            }
            return None;
        }
        let relation =
            validation::borrow_contracts::resolve_provider_outputs(program, parameters).ok()?;
        if relation.readiness != validation::borrow_contracts::BorrowRelationReadiness::Complete {
            return None;
        }
        let output = relation.outputs.iter().find(|output| {
            output.destination
                == validation::borrow_contracts::ProviderOutputDestination::Parameter(position)
                && same_path(
                    program,
                    &crate::borrow::owner_place_segments(&output.owner_path),
                    owner_path,
                )
        })?;
        let edge = output.dependencies.iter().find(|edge| {
            edge.parameter_index == contract.parameter_index
                && edge.source == contract.source
                && program.normalized_type_identity(edge.source_type) == identity_type
                && same_path(
                    program,
                    &crate::borrow::owner_place_segments(&edge.source_path),
                    source_path,
                )
        })?;
        (edge.source, edge.source_type, source_path)
    } else {
        let edges =
            crate::borrow::call_reference_lifetime_sources(program, contract.state_symbol, call?)?;
        let edge = edges.iter().find(|edge| {
            edge.parameter_index == contract.parameter_index
                && edge.source == contract.source
                && program.normalized_type_identity(edge.source_type) == identity_type
                && same_path(
                    program,
                    &crate::borrow::owner_place_segments(&edge.source_path),
                    source_path,
                )
                && same_path(
                    program,
                    &crate::borrow::owner_place_segments(&edge.owner_path),
                    owner_path,
                )
        })?;
        (edge.source, edge.source_type, source_path)
    };
    let carrier = match source {
        ViewReturnInputSource::Referent => source_type,
        ViewReturnInputSource::Stored => {
            validation::reference_result_custody::input_carrier_type(program, source_type)?
        }
    };
    validation::project_type_reference_from_segments(program, carrier, path)
}

/// Rejoin every frozen member to the same immutable invocation-entry cut.
/// This checks which value was selected; its borrowing permission is checked
/// by the source consumer, independently of the declared lifetime edge.
fn validate_stored_sources(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    contract: &checked_trees::FlowOperandLifetimeContract,
    snapshot: arena::Handle<checked_trees::FlowStorageSnapshot>,
) -> Option<()> {
    use checked_trees::FlowOperandStoredSource;
    let input = control.operands.get(contract.input);
    let carrier =
        validation::reference_result_custody::input_carrier_type(program, input.type_reference)?;
    let path = validation::corresponding_place_projection(
        program,
        carrier,
        control.operand_retained_paths.span(contract.source_path)?,
    )?;
    let selected = validation::project_type_reference_from_segments(program, carrier, &path)?;
    let leaves = if validation::reference_shell(program, selected).is_some() {
        vec![Vec::new()]
    } else {
        validation::borrow_contracts::carried_reference_paths(program, selected)?
            .into_iter()
            .map(|(path, _)| crate::borrow::owner_place_segments(&path))
            .collect()
    };
    let mut recorded = control
        .operand_stored_sources
        .span(contract.stored_sources)?
        .iter();
    if leaves.is_empty() {
        return None;
    }
    for leaf in leaves {
        let mut full_path = path.clone();
        full_path.extend_from_slice(&leaf);
        let sources = validation::completed_storage::stored_reference_sources_at_path(
            program,
            control,
            contract.input,
            &full_path,
            snapshot,
        );
        let mut expected = Vec::new();
        flatten_stored_sources(sources, &mut expected)?;
        if expected.is_empty() {
            return None;
        }
        for expected in expected {
            let recorded = recorded.next()?;
            let (FlowOperandStoredSource::Value { path, .. }
            | FlowOperandStoredSource::Incoming { path, .. }) = recorded;
            if !same_path(program, control.operand_retained_paths.span(*path)?, &leaf) {
                return None;
            }
            match (*recorded, expected) {
                (
                    FlowOperandStoredSource::Value {
                        operand,
                        reference,
                        projection,
                        slot,
                        ..
                    },
                    validation::completed_storage::StoredOperand::Value {
                        capture,
                        reference: expected_reference,
                        projection: expected_projection,
                        slot: expected_slot,
                    },
                ) => {
                    if operand != capture
                        || reference != expected_reference
                        || !same_path(
                            program,
                            control.operand_retained_paths.span(projection)?,
                            &expected_projection,
                        )
                    {
                        return None;
                    }
                    match (slot, expected_slot) {
                        (Some(slot), Some(expected))
                            if stored_slot_matches(program, control, slot, &expected) => {}
                        (None, None) => {}
                        _ => return None,
                    }
                }
                (
                    FlowOperandStoredSource::Incoming { slot, .. },
                    validation::completed_storage::StoredOperand::Incoming(expected),
                ) if stored_slot_matches(program, control, slot, &expected) => {}
                _ => return None,
            }
        }
    }
    recorded.next().is_none().then_some(())
}

fn flatten_stored_sources(
    source: validation::completed_storage::StoredOperand,
    output: &mut Vec<validation::completed_storage::StoredOperand>,
) -> Option<()> {
    use validation::completed_storage::StoredOperand;
    match source {
        StoredOperand::Unavailable => return None,
        StoredOperand::Alternatives(sources) => {
            if sources.is_empty() {
                return None;
            }
            for source in sources {
                flatten_stored_sources(source, output)?;
            }
        }
        source => output.push(source),
    }
    Some(())
}

fn stored_slot_matches(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    slot: checked_trees::FlowOperandReferent,
    expected: &validation::CanonicalPlace,
) -> bool {
    slot.root == expected.root
        && control
            .operand_referent_segments
            .span(slot.segments)
            .is_some_and(|path| same_path(program, path, &expected.segments))
}

pub(crate) enum RetainedLifetimeInput<'a> {
    Value {
        operand: arena::Handle<checked_trees::FlowOperandFact>,
        path: &'a [facts::PlaceSegment],
        bound: TypeReferenceHandle,
    },
    Incoming {
        bound: TypeReferenceHandle,
    },
}

/// A saved field can hold either of several references. Keep those possible
/// occupants together: returning access requires the same duty on every arm,
/// while lifetime retention conservatively holds every possible input.
/// Separate groups are simultaneous declared leaves, not alternative occupants.
pub(crate) fn retained_inputs<'a>(
    program: &dyn validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    control: &'a FlowControlFacts,
    supplier: &FlowOperandRetainedSupplier,
) -> Option<Vec<Vec<RetainedLifetimeInput<'a>>>> {
    let bound = source_type(program, borrow, control, supplier)?;
    let contract = supplier.contract?;
    if contract.source == ViewReturnInputSource::Referent {
        return Some(vec![vec![RetainedLifetimeInput::Value {
            operand: supplier.operand,
            path: control.operand_retained_paths.span(supplier.projection)?,
            bound,
        }]]);
    }
    let mut inputs: Vec<(Vec<facts::PlaceSegment>, Vec<RetainedLifetimeInput<'a>>)> = Vec::new();
    for member in control
        .operand_stored_sources
        .span(contract.stored_sources)?
    {
        use checked_trees::FlowOperandStoredSource;
        let (FlowOperandStoredSource::Value { path, .. }
        | FlowOperandStoredSource::Incoming { path, .. }) = member;
        let path = validation::corresponding_place_projection(
            program,
            bound,
            control.operand_retained_paths.span(*path)?,
        )?;
        let bound = validation::project_type_reference_from_segments(program, bound, &path)?;
        let input = match *member {
            FlowOperandStoredSource::Value {
                operand,
                projection,
                ..
            } => RetainedLifetimeInput::Value {
                operand,
                path: control.operand_retained_paths.span(projection)?,
                bound,
            },
            FlowOperandStoredSource::Incoming { .. } => RetainedLifetimeInput::Incoming { bound },
        };
        if let Some((_, group)) = inputs.iter_mut().find(|(candidate, _)| *candidate == path) {
            group.push(input);
        } else {
            inputs.push((path, vec![input]));
        }
    }
    (!inputs.is_empty()).then(|| inputs.into_iter().map(|(_, group)| group).collect())
}

fn same_path(
    program: &dyn validation::CheckingInput,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            validation::storage_path_relations::canonical_place_segments_equal(
                program, *left, *right,
            )
        })
}

/// Check the whole declared input list before using any one captured edge.
/// Each returned access is the declaration's ceiling, which can be weaker
/// than the reference type stored inside a shared container.
pub(crate) fn output_input_accesses(
    program: &dyn validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    control: &FlowControlFacts,
    identity: facts::InvocationOutputIdentity,
    owner_path: &[checked_trees::BorrowLoanOwnerSegment],
    sources: arena::HandleSpan<FlowOperandRetainedSupplier>,
) -> Result<Vec<language_semantics::ReferenceAccess>, &'static str> {
    let machine = program
        .machine_holding_state(identity.state_symbol)
        .ok_or("output lifetime inputs have no owning machine")?;
    let site = validation::find_call_site(
        program,
        machine.symbol,
        identity.state_symbol,
        identity.statement_index,
        identity.call_ordinal,
    )
    .ok_or("output lifetime inputs have no original call")?;
    let parameters = site
        .parameters(program, machine.symbol)
        .ok_or("output lifetime inputs have no applied parameter list")?;
    let position = parameters
        .iter()
        .position(|parameter| parameter.symbol == identity.parameter_symbol)
        .ok_or("output lifetime inputs name another output parameter")?;
    let relation = validation::borrow_contracts::resolve_provider_outputs(program, parameters)
        .map_err(|_| "output lifetime input relation is unavailable")?;
    if relation.readiness != validation::borrow_contracts::BorrowRelationReadiness::Complete {
        return Err("output lifetime input relation is not complete");
    }
    let mut outputs = relation.outputs.iter().filter(|output| {
        output.destination
            == validation::borrow_contracts::ProviderOutputDestination::Parameter(position)
            && same_path(
                program,
                &crate::borrow::owner_place_segments(&output.owner_path),
                &crate::borrow::owner_place_segments(owner_path),
            )
    });
    let output = outputs
        .next()
        .ok_or("output lifetime inputs have no declared output leaf")?;
    if outputs.next().is_some() {
        return Err("output lifetime inputs have ambiguous declared leaves");
    }
    let recorded = control
        .operand_retained_suppliers
        .span(sources)
        .ok_or("output lifetime inputs have an unavailable capture list")?;
    if recorded.len() != output.dependencies.len() {
        return Err("output lifetime inputs do not cover the whole declared frontier");
    }
    let mut used = vec![false; output.dependencies.len()];
    let mut accesses = Vec::with_capacity(recorded.len());
    for supplier in recorded {
        let contract = supplier
            .contract
            .ok_or("output lifetime input has no applied edge")?;
        if contract.output_parameter != Some(position)
            || contract.state_symbol != identity.state_symbol
            || contract.statement_index != identity.statement_index
            || contract.call_ordinal != identity.call_ordinal
            || !same_path(
                program,
                control
                    .operand_retained_paths
                    .span(contract.owner_path)
                    .ok_or("output lifetime input has no output path")?,
                &crate::borrow::owner_place_segments(owner_path),
            )
            || source_type(program, borrow, control, supplier).is_none()
        {
            return Err("output lifetime input does not match its exact captured declaration");
        }
        let source_path = control
            .operand_retained_paths
            .span(contract.source_path)
            .ok_or("output lifetime input has no source path")?;
        let matches = output
            .dependencies
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                edge.parameter_index == contract.parameter_index
                    && edge.source == contract.source
                    && program.normalized_type_identity(edge.source_type)
                        == program.normalized_type_identity(contract.source_type)
                    && same_path(
                        program,
                        &crate::borrow::owner_place_segments(&edge.source_path),
                        source_path,
                    )
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [index] = matches.as_slice() else {
            return Err("output lifetime input has no unique declared edge");
        };
        if std::mem::replace(&mut used[*index], true) {
            return Err("output lifetime input repeats one edge instead of another");
        }
        accesses.push(output.dependencies[*index].access);
    }
    Ok(accesses)
}

/// Authenticate a completed output namespace and every retained input debt.
/// This is source correspondence; use sites must separately establish the
/// exact performed FlowCall exit or a retained descendant of that occurrence.
pub(crate) fn completed_output_retention(
    borrow: &checked_trees::BorrowFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: usize,
    call: &checked_trees::BorrowCallFact,
    identity: facts::InvocationOutputIdentity,
    carrier_type: typed_trees::types::TypeReferenceHandle,
    output_path: &[checked_trees::BorrowLoanOwnerSegment],
) -> Result<Vec<facts::PlaceRoot>, &'static str> {
    let performed = borrow
        .call_at(machine, state, statement_index, call.call_ordinal)
        .filter(|handle| borrow.calls.get(*handle) == call)
        .ok_or("output has no exact performed invocation")?;
    if identity.state_symbol != state
        || identity.statement_index != statement_index
        || identity.call_ordinal != call.call_ordinal
    {
        return Err("output identity names another invocation");
    }
    let mut selected = Vec::new();
    for handle in call.completed_reference_loans.handles() {
        if !borrow.loans.is_valid(handle) {
            return Err("output completion names an unavailable loan");
        }
        let loan = borrow.loans.get(handle);
        if loan.established_by_call != performed {
            return Err("output loan belongs to another invocation");
        }
        if !loan.incoming_reference.is_valid() {
            continue;
        }
        if !borrow.incoming_references.is_valid(loan.incoming_reference) {
            return Err("output loan has an unavailable identity");
        }
        let incoming = borrow.incoming_references.get(loan.incoming_reference);
        if incoming.origin == (checked_trees::IncomingReferenceOrigin::Output { identity }) {
            if borrow.supplied_output_call(handle) != Some(performed) {
                return Err("output permission has no normal-call source correspondence");
            }
            if incoming.machine_symbol != machine || incoming.state_symbol != state {
                return Err("output permission belongs to another state");
            }
            if incoming.carrier_type != carrier_type {
                return Err("output permission does not match the captured carrier type");
            }
            if borrow.owner_segments.span_or_empty(incoming.owner_path) != output_path {
                return Err("output permission does not match the captured descriptor leaf");
            }
            if loan.root != facts::PlaceRoot::InvocationOutput(identity)
                || !loan.segments.is_empty()
            {
                return Err("output permission does not match the exact opaque backing");
            }
            if loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency {
                return Err("output lifetime debt cannot supply descriptor permission");
            }
            selected.push(loan);
        }
    }
    if selected.is_empty() {
        return Err("completed invocation has no supplied descriptor for this leaf");
    }
    let mut roots = Vec::new();
    for output in selected {
        for handle in call.completed_reference_loans.handles() {
            let loan = borrow.loans.get(handle);
            if loan.storage_owner != output.storage_owner
                || borrow.loan_owner_path(loan) != borrow.loan_owner_path(output)
                || loan.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
            {
                continue;
            }
            let known = match loan.root {
                facts::PlaceRoot::Symbol(symbol) => symbol.is_valid(),
                facts::PlaceRoot::Expression(expression) => expression.is_valid(),
                facts::PlaceRoot::InvocationOutput(identity) => {
                    identity.is_valid() && loan.incoming_reference.is_valid()
                }
                facts::PlaceRoot::InvocationReferent { .. } => loan.incoming_reference.is_valid(),
                _ => false,
            };
            if !known {
                return Err("an output lifetime debt has no known backing identity");
            }
            crate::shared::collections::push_unique(&mut roots, loan.root);
        }
    }
    Ok(roots)
}
