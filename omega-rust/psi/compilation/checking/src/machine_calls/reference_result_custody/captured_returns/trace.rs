//! Opt-in snapshots of a refused completed-reference origin join.

use super::{CheckFacts, CheckedStructuralValueKind, SymbolHandle, TypeReferenceHandle};

pub(super) fn return_sources(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    target: SymbolHandle,
) {
    let Some(state) = crate::call_target_state(program, target) else {
        eprintln!("RETURN_SOURCES missing target state {target:?}");
        return;
    };
    let describe_type = |reference: TypeReferenceHandle| {
        if program
            .type_reference_table
            .contains_type_reference(reference)
        {
            eprintln!(
                "RETURN_TYPE {reference:?} {:?} normalized={:?}",
                program.type_reference_table.type_reference(reference),
                program.normalized_type_identity(reference)
            );
        } else {
            eprintln!("RETURN_TYPE {reference:?} unavailable");
        }
    };
    let parameters = program.state_parameters(state);
    eprintln!(
        "RETURN_SOURCES target={target:?} state={:?} return={:?} parameter_count={} parameters={:?}",
        state.symbol,
        state.return_type,
        parameters.len(),
        &parameters[..parameters.len().min(32)]
    );
    describe_type(state.return_type);
    for parameter in parameters.iter().take(32) {
        describe_type(parameter.type_reference);
        if program
            .type_reference_table
            .contains_type_reference(parameter.type_reference)
            && let Some(shell) = crate::reference_shell(program, parameter.type_reference)
        {
            describe_type(shell.referee);
        }
    }
    for (_, borrowed) in facts
        .borrow
        .states
        .iter()
        .filter(|(_, row)| row.state_symbol == state.symbol)
    {
        eprintln!("RETURN_BORROW_STATE {borrowed:?}");
        if let Some(loans) = facts.borrow.returned_loans.span(borrowed.returned_loans) {
            for loan in loans.iter().take(32) {
                eprintln!(
                    "RETURN_LOAN {loan:?} owner={:?} segments={:?}",
                    facts.borrow.owner_segments.span(loan.owner_path),
                    facts.borrow.access_segments.span(loan.segments)
                );
                if facts
                    .borrow
                    .incoming_references
                    .is_valid(loan.incoming_reference)
                {
                    let incoming = facts
                        .borrow
                        .incoming_references
                        .get(loan.incoming_reference);
                    eprintln!(
                        "RETURN_INCOMING {incoming:?} owner={:?}",
                        facts.borrow.owner_segments.span(incoming.owner_path)
                    );
                    describe_type(incoming.carrier_type);
                }
            }
        }
    }
    if let Some(frame) = crate::CallFrameResolver::new(program) {
        let origins = frame.reference_result_leaf_origins(state.symbol);
        eprintln!(
            "RETURN_FRAME_ORIGINS count={:?}",
            origins.as_ref().map(Vec::len)
        );
        if let Some(origins) = origins {
            for origin in origins.iter().take(32) {
                eprintln!(
                    "RETURN_FRAME_ORIGIN root={:?} result_path={:?} segments={:?} representation={:?} recast_count={}",
                    origin.source.root,
                    &origin.result_path[..origin.result_path.len().min(64)],
                    &origin.source.segments[..origin.source.segments.len().min(64)],
                    origin
                        .source
                        .representation
                        .as_ref()
                        .map(|(reference, path)| (
                            *reference,
                            path.len(),
                            &path[..path.len().min(64)]
                        )),
                    origin.source.recasts.len()
                );
                for recast in origin.source.recasts.iter().take(16) {
                    let predecessor = recast
                        .representation
                        .as_ref()
                        .map(|(reference, path)| (*reference, path.as_slice()))
                        .or_else(|| {
                            crate::state_root_type(program, state, recast.root)
                                .map(|reference| (reference, recast.segments.as_slice()))
                        });
                    let selected = predecessor
                        .filter(|(reference, _)| {
                            program
                                .type_reference_table
                                .contains_type_reference(*reference)
                        })
                        .and_then(|(reference, path)| {
                            crate::project_type_reference_from_segments(program, reference, path)
                        })
                        .filter(|reference| {
                            program
                                .type_reference_table
                                .contains_type_reference(*reference)
                        })
                        .map(|reference| {
                            crate::reference_shell(program, reference)
                                .map_or(reference, |shell| shell.referee)
                        });
                    eprintln!(
                        "RETURN_RECAST formation={:?} root={:?} target={:?} segments={:?} representation={:?} selected={selected:?} ordinary_storage={:?}",
                        recast.formation,
                        recast.root,
                        recast.target_type,
                        &recast.segments[..recast.segments.len().min(64)],
                        recast.representation.as_ref().map(|(reference, path)| (
                            *reference,
                            path.len(),
                            &path[..path.len().min(64)]
                        )),
                        selected
                            .filter(|_| program
                                .type_reference_table
                                .contains_type_reference(recast.target_type))
                            .and_then(|reference| crate::reference_view_storage(
                                program,
                                reference,
                                recast.target_type
                            ))
                    );
                    if let Some(selected) = selected {
                        describe_type(selected);
                    }
                    describe_type(recast.target_type);
                }
            }
        }
    }
    for (handle, value) in facts
        .values
        .structural_values
        .nodes
        .iter()
        .filter(|(_, value)| value.state == state.symbol)
        .take(32)
    {
        eprintln!("RETURN_VALUE {handle:?} {value:?}");
        describe_type(value.type_reference);
        if let CheckedStructuralValueKind::Reference { backing_type, .. } = value.kind {
            describe_type(backing_type);
        }
    }
    for (_, exit) in facts
        .flow
        .control
        .exits
        .iter()
        .filter(|(_, exit)| exit.state_symbol == state.symbol)
        .take(16)
    {
        eprintln!("RETURN_EXIT {exit:?}");
        if facts.flow.control.operands.is_valid(exit.returned_value) {
            eprintln!(
                "RETURN_OPERAND {:?}",
                facts.flow.control.operands.get(exit.returned_value)
            );
        }
    }
}
