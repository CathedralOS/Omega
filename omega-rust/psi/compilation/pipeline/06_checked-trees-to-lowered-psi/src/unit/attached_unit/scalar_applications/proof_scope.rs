//! Close application coordinates while Psi still owns the symbol table.
//!
//! Publication assigns fresh artifact-local ids. Distinct overloads, binders
//! and specializations cannot alias just because their nominal names coincide.
use crate::machine_lowering::declarations;
use crate::shared::lowering_error::LoweringError;
use checked_trees::CheckedTrees;
use semantic_vocabulary::proof_applications::ProofCallGraph;
use symbols::SymbolHandle;
use terminal_psi::{
    ProofApplicationScope, ProofDeclarationId, ProofScopeDeclaration, TerminalIdentityTableBuilder,
};

pub(super) struct PublishedScope {
    pub scope: ProofApplicationScope,
    pub coordinates: Vec<(SymbolHandle, ProofDeclarationId)>,
}

pub(super) fn publish(
    checked: &CheckedTrees,
    identities: &mut TerminalIdentityTableBuilder,
    sources: &[(source::SourceId, [u8; 32])],
    machines: &[(SymbolHandle, semantic_vocabulary::MachineId)],
    boundaries: &[super::super::operation_frame::BoundaryParameters],
) -> Result<PublishedScope, LoweringError> {
    let source = &checked.facts.proof_applications;
    let mut coordinates = Vec::new();
    // Every declaration of the program asks for its identity; finding it by
    // scanning the identities assigned so far made publication quadratic.
    let mut assigned = std::collections::BTreeMap::new();
    let mut descriptions = arena::Arena::default();
    let mut identity = |symbol: SymbolHandle| -> Result<ProofDeclarationId, LoweringError> {
        if !symbol.is_valid() {
            return Ok(ProofDeclarationId::default());
        }
        let key = (symbol.arena_index(), symbol.generation());
        if let Some(identity) = assigned.get(&key) {
            return Ok(*identity);
        }
        let ordinal = u32::try_from(coordinates.len())
            .ok()
            .and_then(|position| position.checked_add(1))
            .ok_or(LoweringError::Unsupported(
                "proof declaration namespace is exhausted",
            ))?;
        let identity = ProofDeclarationId(ordinal);
        let description = describe(checked, identities, sources, symbol);
        descriptions.insert(description.unwrap_or_else(ProofScopeDeclaration::Unavailable));
        coordinates.push((symbol, identity));
        assigned.insert(key, identity);
        Ok(identity)
    };
    let plan = source.try_map(&mut identity)?;
    let mut calls = ProofCallGraph::capture(
        &checked.facts.operational,
        source.machines.iter().map(|(_, row)| row.machine),
        &mut identity,
    )?;
    retain_selected_algebra_calls(checked, &mut calls, &mut identity)?;
    calls.index_machines();
    let mut callables = arena::Arena::new();
    for &(source, body) in machines {
        callables.insert(terminal_psi::ProofCallableAssociation::Callable {
            declaration: identity(source)?,
            callable: terminal_psi::TargetMachineCallable::Machine(body),
        });
    }
    for boundary in boundaries {
        callables.insert(terminal_psi::ProofCallableAssociation::Callable {
            declaration: identity(boundary.source)?,
            callable: terminal_psi::TargetMachineCallable::Boundary(boundary.id),
        });
    }
    // A selected generic template may have no emitted body or full signature.
    // Keep its independent header on the same declaration roster nonetheless.
    let mut headers = checked
        .facts
        .declaration_descriptions
        .headers
        .iter()
        .map(|(_, header)| Ok((identity(header.machine)?, header)))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let (recursive_components, recursive_members, recursive_calls) =
        retain_recursive_components(checked, &mut identity)?;
    let mut pending_evaluation_conditions = arena::Arena::new();
    for (_, owner) in checked
        .facts
        .declaration_descriptions
        .evaluation_dependencies
        .pending_conditions
        .iter()
    {
        pending_evaluation_conditions.insert(identity(*owner)?);
    }
    drop(identity);
    headers.sort_by_key(|(declaration, _)| *declaration);
    let mut callable_headers = arena::Arena::new();
    for (declaration, header) in headers {
        let callable = if header.callable.is_valid() {
            Ok(identities.intern(&checked.callable_identity_text(header.callable)))
        } else {
            Err("checked machine has no callable signature identity".to_owned())
        };
        let synchronous_invocations = header
            .synchronous_invocations
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|span| {
                let targets = checked
                    .facts
                    .declaration_descriptions
                    .invocation_targets
                    .span(*span)
                    .ok_or_else(|| {
                        "checked declaration lost its retained invocation targets".to_owned()
                    })?;
                Ok(targets
                    .iter()
                    .map(|target| terminal_psi::DeclaredSynchronousBoundaryTarget {
                        declaration: identities.intern(&target.declaration),
                        package: target.package,
                        application: target
                            .application
                            .as_ref()
                            .map(|application| identities.intern(application)),
                    })
                    .collect())
            });
        callable_headers.insert(terminal_psi::ProofCallableHeader {
            declaration,
            callable,
            package: header.package,
            source_target: header.source_target.clone(),
            product_eligible: header.product_eligible,
            supplies_checked_body: header.supplies_checked_body,
            type_parameter_count: header.type_parameter_count,
            compiler_evaluation: header.compiler_evaluation.clone(),
            synchronous_invocations,
        });
    }
    Ok(PublishedScope {
        scope: ProofApplicationScope {
            declarations: descriptions,
            callable_headers,
            callables,
            plan,
            calls,
            recursive_components,
            recursive_members,
            recursive_calls,
            pending_evaluation_conditions,
        },
        coordinates,
    })
}

/// Operator selection already owns the exact realization association. Preserve
/// those execution edges after ordinary operational calls, just as compiler
/// evaluation's Source adapter does; existing call-site ordinals stay stable.
fn retain_selected_algebra_calls(
    checked: &CheckedTrees,
    graph: &mut ProofCallGraph<ProofDeclarationId>,
    identity: &mut impl FnMut(SymbolHandle) -> Result<ProofDeclarationId, LoweringError>,
) -> Result<(), LoweringError> {
    let operators = &checked.facts.operators;
    let mut selected = operators
        .resolved_expression_selections()
        .filter_map(|(expression, origin, _, _)| {
            let owner = origin.machine_symbol()?;
            let candidate = operators.selected_trait_candidate_in_machine(expression, owner)?;
            (candidate.realization_machine_symbol.is_valid()
                && candidate.realization_state_symbol.is_valid())
            .then_some((owner, candidate))
        })
        .collect::<Vec<_>>();
    selected.sort_by_key(|(owner, _)| (owner.arena_index(), owner.generation()));
    let mut position = 0;
    while position < selected.len() {
        let owner = selected[position].0;
        let end =
            position + selected[position..].partition_point(|(candidate, _)| *candidate == owner);
        let declaration = identity(owner)?;
        if let Some((row, span)) = graph
            .machines()
            .iter()
            .find_map(|(handle, row)| (row.machine == declaration).then_some((handle, row.calls)))
        {
            let mut calls = graph.calls.span_or_empty(span).to_vec();
            for (_, candidate) in &selected[position..end] {
                calls.push(
                    semantic_vocabulary::proof_applications::ProofApplicationCall {
                        binder: ProofDeclarationId::default(),
                        callee: identity(candidate.realization_machine_symbol)?,
                        state: identity(candidate.realization_state_symbol)?,
                        operator: identity(candidate.trait_requirement_symbol)?,
                        arguments: arena::HandleSpan::empty(),
                    },
                );
            }
            let calls = graph.calls.insert_many(calls);
            graph.update_machine(row, |machine| machine.calls = calls);
        }
        position = end;
    }
    Ok(())
}

fn retain_recursive_components(
    checked: &CheckedTrees,
    identity: &mut impl FnMut(SymbolHandle) -> Result<ProofDeclarationId, LoweringError>,
) -> Result<
    (
        arena::Arena<terminal_psi::ProofCheckedRecursiveComponent>,
        arena::Arena<ProofDeclarationId>,
        arena::Arena<terminal_psi::ProofCheckedRecursiveCall>,
    ),
    LoweringError,
> {
    let source = &checked
        .facts
        .declaration_descriptions
        .evaluation_dependencies;
    let mut components = arena::Arena::new();
    let mut members = arena::Arena::new();
    let mut calls = arena::Arena::new();
    for (_, component) in source.recursive_components.iter() {
        let mapped_members = source
            .recursive_members
            .span_or_empty(component.members)
            .iter()
            .map(|body| identity(body.machine))
            .collect::<Result<Vec<_>, _>>()?;
        let mapped_calls = source
            .recursive_calls
            .span_or_empty(component.calls)
            .iter()
            .map(|call| {
                Ok(terminal_psi::ProofCheckedRecursiveCall {
                    caller: identity(call.caller)?,
                    call_ordinal: call.call_ordinal,
                    callee: identity(call.callee)?,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        components.insert(terminal_psi::ProofCheckedRecursiveComponent {
            members: members.insert_many(mapped_members),
            calls: calls.insert_many(mapped_calls),
        });
    }
    Ok((components, members, calls))
}

fn describe(
    checked: &CheckedTrees,
    identities: &mut TerminalIdentityTableBuilder,
    sources: &[(source::SourceId, [u8; 32])],
    symbol: SymbolHandle,
) -> Result<ProofScopeDeclaration, String> {
    if let Some(declaration) =
        declarations::boundary_identity(identities, checked, sources, symbol)?
    {
        return Ok(ProofScopeDeclaration::Callable(declaration));
    }
    if let Some(operator) = checked.typed.operator_definition_by_symbol(symbol)
        && operator.is_boundary
    {
        return declarations::operator_requirement(
            identities,
            checked,
            sources,
            symbol,
            checked
                .typed
                .boundary_operator_requirement_callable_identity(operator),
        )
        .map(ProofScopeDeclaration::Operator)
        .map_err(str::to_owned);
    }
    declarations::semantic_identity(identities, checked, symbol).map(ProofScopeDeclaration::Nominal)
}
