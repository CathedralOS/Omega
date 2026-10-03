//! Source boundary authority and the exact checked calls that exercise it.
//!
//! This check uses declarations and checked flow only. Selected providers,
//! target mechanisms and receiver permissions are separate Omega obligations.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// Validate checked call coordinates and reject ordinary whole-trait
/// implementations that attempt to mint the called boundary capability.
/// This does not select a provider or grant native execution authority.
pub fn validate_boundary_calls(
    checked: &checked_trees::CheckedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let program = &checked.typed;
    let calls = checked_boundary_call_coordinates(checked)?;
    let approved = approved_boundary_traits(program);
    let diagnostics: Vec<_> = calls
        .into_iter()
        .filter(|call| !approved.contains(&call.boundary_trait_symbol))
        .map(|call| {
            Diagnostic::error(format!(
                "unapproved boundary call: {} in {} exercises a boundary capability with no approved provider for that exact capability",
                symbol_name(program, call.boundary_trait_symbol),
                symbol_name(program, call.state_symbol),
            ))
        })
        .collect();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Approval is exact to the authored boundary declaration, never its service
/// reach or a provider selected for another capability. All calls consume the
/// same derived roster; it carries no provider-selection authority.
fn approved_boundary_traits(program: &typed_trees::TypedTrees) -> Vec<SymbolHandle> {
    let mut approved = Vec::new();
    for definition in program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        let exact_symbol_owners = program
            .traits()
            .iter()
            .filter(|candidate| candidate.symbol == definition.symbol)
            .count();
        let declaration_module = program.symbols.symbol_module(definition.symbol);
        let exact_name_owners = program
            .traits()
            .iter()
            .filter(|candidate| {
                candidate.is_boundary
                    && candidate.name.as_str() == definition.name.as_str()
                    && program.symbols.symbol_module(candidate.symbol) == declaration_module
                    && program
                        .symbols
                        .same_symbol_source_package(candidate.symbol, definition.symbol)
            })
            .count();
        let has_exact_identity = definition.symbol.is_valid()
            && !definition.name.as_str().is_empty()
            && exact_symbol_owners == 1
            && exact_name_owners == 1;
        // A requirement-level checked adapter or external leaf is a supply
        // edge, not an in-package implementation of the whole capability.
        let implemented_in_package = program.conformances().iter().any(|conformance| {
            matches!(
                conformance.subject,
                typed_trees::trait_definition::ConformanceSubject::Carrier(_)
            ) && conformance.trait_symbol == definition.symbol
        });
        if has_exact_identity && !implemented_in_package {
            approved.push(definition.symbol);
        }
    }
    approved
}

/// Private scratch coordinates preserve call order and exact requirement
/// ownership during validation. They are not another published evidence form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundaryCallCoordinate {
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
    boundary_trait_symbol: SymbolHandle,
    boundary_signature_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
}

fn checked_boundary_call_coordinates(
    checked: &checked_trees::CheckedTrees,
) -> Result<Vec<BoundaryCallCoordinate>, Vec<Diagnostic>> {
    let program = &checked.typed;
    let flow = &checked.facts.flow.control;
    let mut diagnostics = Vec::new();
    let mut state_coordinates = Vec::new();
    let mut coordinates = Vec::new();

    let traits = program.traits();
    if traits.len() != program.roots.traits.count() as usize {
        diagnostics.push(Diagnostic::error(
            "provider approval checked flow has an invalid typed trait span",
        ));
    }
    for definition in traits {
        if program.trait_machine_signatures(definition).len()
            != definition.machines.count() as usize
        {
            diagnostics.push(Diagnostic::error(format!(
                "provider approval checked flow has an invalid typed signature span for trait {:?}",
                definition.symbol,
            )));
        }
    }
    let machines = program.machines();
    if machines.len() != program.roots.machines.count() as usize {
        diagnostics.push(Diagnostic::error(
            "provider approval checked flow has an invalid typed machine span",
        ));
    }
    for machine in machines {
        if program.machine_states(machine).len() != machine.states.count() as usize {
            diagnostics.push(Diagnostic::error(format!(
                "provider approval checked flow has an invalid typed state span for machine {:?}",
                machine.symbol,
            )));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    for (_, flow_state) in flow.states.iter() {
        let state_coordinate = (flow_state.machine_symbol, flow_state.state_symbol);
        if state_coordinates.contains(&state_coordinate) {
            diagnostics.push(Diagnostic::error(format!(
                "provider approval checked flow contains duplicate state coordinate ({:?}, {:?})",
                flow_state.machine_symbol, flow_state.state_symbol,
            )));
            continue;
        }
        state_coordinates.push(state_coordinate);

        let machines = program
            .machines()
            .iter()
            .filter(|machine| machine.symbol == flow_state.machine_symbol)
            .collect::<Vec<_>>();
        let machine = match machines.as_slice() {
            [machine] => *machine,
            [] => {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} has no exact typed machine owner {:?}",
                    flow_state.state_symbol, flow_state.machine_symbol,
                )));
                continue;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} has duplicate exact typed machine owners {:?}",
                    flow_state.state_symbol, flow_state.machine_symbol,
                )));
                continue;
            }
        };

        let state_owners = program
            .machines()
            .iter()
            .flat_map(|candidate_machine| {
                program
                    .machine_states(candidate_machine)
                    .iter()
                    .filter(move |state| state.symbol == flow_state.state_symbol)
                    .map(move |state| (candidate_machine, state))
            })
            .collect::<Vec<_>>();
        let typed_state = match state_owners.as_slice() {
            [(owner, state)] if owner.symbol == machine.symbol => *state,
            [(owner, _)] => {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} belongs to typed machine {:?}, not {:?}",
                    flow_state.state_symbol, owner.symbol, flow_state.machine_symbol,
                )));
                continue;
            }
            [] => {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} is missing from its exact typed machine {:?}",
                    flow_state.state_symbol, flow_state.machine_symbol,
                )));
                continue;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} has duplicate typed state owners",
                    flow_state.state_symbol,
                )));
                continue;
            }
        };

        let Some(calls) = flow.calls.span(flow_state.calls) else {
            diagnostics.push(Diagnostic::error(format!(
                "provider approval checked-flow state {:?} has an invalid call span",
                flow_state.state_symbol,
            )));
            continue;
        };
        let statements = program
            .statement_table
            .statements(typed_state.statement_nodes);
        if statements.len() != typed_state.statement_nodes.count() as usize {
            diagnostics.push(Diagnostic::error(format!(
                "provider approval checked-flow state {:?} has an invalid typed statement span",
                flow_state.state_symbol,
            )));
            continue;
        }
        let statement_count = statements.len();
        let mut call_coordinates = Vec::new();
        for call in calls {
            let call_coordinate = (call.statement_index, call.call_ordinal);
            if call_coordinates.contains(&call_coordinate) {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} contains duplicate call coordinate (statement {}, call {})",
                    flow_state.state_symbol, call.statement_index, call.call_ordinal,
                )));
                continue;
            }
            call_coordinates.push(call_coordinate);
            if call.statement_index >= statement_count {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow call in state {:?} has out-of-range statement index {} for {} typed statements",
                    flow_state.state_symbol, call.statement_index, statement_count,
                )));
                continue;
            }
            let Some(boundary_edges) = checked
                .facts
                .flow
                .boundaries
                .edges
                .span(call.boundary_edges)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow call in state {:?} has an invalid boundary-edge span",
                    flow_state.state_symbol,
                )));
                continue;
            };
            if !call.target_symbol.is_valid() {
                if boundary_edges.is_empty() {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow call in state {:?} has no valid target symbol",
                    flow_state.state_symbol,
                )));
                continue;
            }
            if boundary_edges.is_empty() {
                match exact_direct_boundary_requirement(program, call.target_symbol) {
                    Ok(Some((boundary_trait_symbol, boundary_signature_symbol))) => {
                        coordinates.push(BoundaryCallCoordinate {
                            machine_symbol: flow_state.machine_symbol,
                            state_symbol: flow_state.state_symbol,
                            target_state_symbol: call.target_symbol,
                            boundary_trait_symbol,
                            boundary_signature_symbol,
                            statement_index: call.statement_index,
                            call_ordinal: call.call_ordinal,
                        });
                    }
                    Ok(None) => {}
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
                continue;
            }

            let mut exact_edges = Vec::new();
            for edge in boundary_edges {
                if edge.statement_index != call.statement_index
                    || edge.call_ordinal != call.call_ordinal
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "provider approval checked boundary edge in state {:?} has coordinate (statement {}, call {}), not its owning call coordinate (statement {}, call {})",
                        flow_state.state_symbol,
                        edge.statement_index,
                        edge.call_ordinal,
                        call.statement_index,
                        call.call_ordinal,
                    )));
                    continue;
                }
                if edge.target_symbol != call.target_symbol {
                    diagnostics.push(Diagnostic::error(format!(
                        "provider approval checked boundary edge in state {:?} targets {:?}, not its owning call target {:?}",
                        flow_state.state_symbol, edge.target_symbol, call.target_symbol,
                    )));
                    continue;
                }
                let edge_identity = (edge.boundary_trait_symbol, edge.boundary_signature_symbol);
                if exact_edges.contains(&edge_identity) {
                    diagnostics.push(Diagnostic::error(format!(
                        "provider approval checked call in state {:?} contains duplicate exact boundary edge ({:?}, {:?})",
                        flow_state.state_symbol,
                        edge.boundary_trait_symbol,
                        edge.boundary_signature_symbol,
                    )));
                    continue;
                }
                exact_edges.push(edge_identity);
                if let Err(diagnostic) = validate_exact_boundary_requirement(
                    program,
                    edge.boundary_trait_symbol,
                    edge.boundary_signature_symbol,
                    "provider approval checked boundary edge",
                ) {
                    diagnostics.push(diagnostic);
                    continue;
                }
                coordinates.push(BoundaryCallCoordinate {
                    machine_symbol: flow_state.machine_symbol,
                    state_symbol: flow_state.state_symbol,
                    target_state_symbol: call.target_symbol,
                    boundary_trait_symbol: edge.boundary_trait_symbol,
                    boundary_signature_symbol: edge.boundary_signature_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                });
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(coordinates)
    } else {
        Err(diagnostics)
    }
}

fn exact_direct_boundary_requirement(
    program: &typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> Result<Option<(SymbolHandle, SymbolHandle)>, Diagnostic> {
    let matches = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == target_symbol)
                .map(move |signature| (definition.symbol, signature.symbol))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [(boundary_trait_symbol, boundary_signature_symbol)] => {
            validate_exact_boundary_requirement(
                program,
                *boundary_trait_symbol,
                *boundary_signature_symbol,
                "provider approval direct abstract boundary call",
            )?;
            Ok(Some((*boundary_trait_symbol, *boundary_signature_symbol)))
        }
        _ => Err(Diagnostic::error(format!(
            "provider approval call target {:?} resolves to {} exact direct boundary requirements",
            target_symbol,
            matches.len(),
        ))),
    }
}

fn validate_exact_boundary_requirement(
    program: &typed_trees::TypedTrees,
    boundary_trait_symbol: SymbolHandle,
    boundary_signature_symbol: SymbolHandle,
    context: &str,
) -> Result<(), Diagnostic> {
    if !boundary_trait_symbol.is_valid() || !boundary_signature_symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "{context} has an invalid exact boundary trait or signature symbol",
        )));
    }
    let owners = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == boundary_trait_symbol)
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err(Diagnostic::error(format!(
            "{context} trait {:?} resolves to {} exact typed owners",
            boundary_trait_symbol,
            owners.len(),
        )));
    };
    if !owner.is_boundary {
        return Err(Diagnostic::error(format!(
            "{context} trait {:?} is not a boundary trait",
            boundary_trait_symbol,
        )));
    }
    let owned_signatures = program
        .trait_machine_signatures(owner)
        .iter()
        .filter(|signature| signature.symbol == boundary_signature_symbol)
        .count();
    let global_signatures = program
        .traits()
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .filter(|signature| signature.symbol == boundary_signature_symbol)
        .count();
    let state_matches = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .filter(|state| state.symbol == boundary_signature_symbol)
        .count();
    if owned_signatures != 1 || global_signatures != 1 || state_matches != 0 {
        return Err(Diagnostic::error(format!(
            "{context} signature {:?} is missing, duplicated, cross-owned, or collides with a typed state",
            boundary_signature_symbol,
        )));
    }
    Ok(())
}

fn symbol_name(program: &typed_trees::TypedTrees, symbol: SymbolHandle) -> String {
    if !symbol.is_valid() {
        return "unknown".to_owned();
    }

    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
    {
        return machine.name.as_str().to_owned();
    }

    if let Some(state) = program.state_by_symbol(symbol)
    {
        return state.name.as_str().to_owned();
    }

    program.symbols.name(symbol).to_owned()
}

#[cfg(test)]
mod tests;
