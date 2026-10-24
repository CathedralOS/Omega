//! Exact checked claim transfers and their machine-local Terminal identities.
//!
//! Project an owner's stored type only when checked custody actually transfers
//! a claim frontier. Borrowed values carry loans instead: an immutable literal
//! can establish backing without an owned declaration whose claims need lowering.

use super::super::{
    CarryPolicy, CheckedUnitEntryClaimPlan, CheckedUnitStructuralParameterPlan, ClaimId,
    EntryClaim, PermissionClaimIdentity, StructuralParameterDeclaration, claim_id,
};
use super::{
    CheckedTrees, LoweringError, Multiplicity, PlaceId, StructuralPlaceKind, TerminalMachineResult,
    allocate_dense, lookup_claim_id, unsupported,
};
use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
#[derive(Clone)]
pub(super) struct LoweredUnitClaims {
    pub(super) entry_claims: Vec<EntryClaim>,
    pub(super) source_claims: Vec<(PermissionClaimIdentity, ClaimId)>,
    pub(super) unreceived_results: Vec<crate::emission::claim_bindings::UnreceivedResultClaim>,
}

impl LoweredUnitClaims {
    pub(super) fn published(&self) -> crate::emission::claim_bindings::PublishedClaims<'_> {
        crate::emission::claim_bindings::PublishedClaims {
            bindings: &self.source_claims,
            unreceived_results: &self.unreceived_results,
        }
    }
}

/// A normal return transfers the exit's current frontier, not the claims an
/// input or local held when it was born. Field replacement can retire those
/// earlier identities without changing the whole owner's storage place.
/// Ordinary bodies and graph states resolve the same checked exit transfers;
/// lookup continues custody and never manufactures a replacement claim.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_return_claims(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    result: &checked_trees::CheckedUnitStructuralReturnPlan,
    operations: &crate::emission::operation_emission::buffer::OperationBuffer,
    bindings: &[(PermissionClaimIdentity, ClaimId)],
    structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
    let (statement_index, expression, evaluation) = match result.completion {
        checked_trees::CheckedStructuralReturnCompletion::Value {
            statement_index,
            expression,
            evaluation,
        } => (statement_index, expression, evaluation),
        checked_trees::CheckedStructuralReturnCompletion::Call { coordinate } => {
            // A terminal call forwards its freshly completed concrete result.
            // Normalized callee outcomes are origin templates, not identities in
            // this machine's namespace. The producer has already joined these
            // claims through the exact call occurrence (including unreceived ones).
            let mut matching = operations.source_calls.iter().filter(|occurrence| {
                occurrence.source_state == state
                    && occurrence.statement_index == coordinate.statement_index as usize
                    && occurrence.call_ordinal == coordinate.call_ordinal as usize
            });
            let occurrence = matching.next().ok_or(LoweringError::Unsupported(
                "terminal return call occurrence is absent",
            ))?;
            if matching.next().is_some() {
                return unsupported("terminal return call occurrence is repeated");
            }
            let mut matching = operations
                .iter()
                .filter(|operation| occurrence.terminal_site.operation() == Some(operation.id));
            let produced = matching
                .next()
                .and_then(|operation| operation.result.as_structural())
                .ok_or(LoweringError::Unsupported(
                    "terminal return call lost its concrete result",
                ))?;
            if matching.next().is_some() || produced.structural_type != structural_type {
                return unsupported(
                    "terminal return call result is ambiguous or has another carrier",
                );
            }
            return Ok(produced.claims.clone());
        }
    };
    if validation::canonical_place_from_expression_in_state(
        &checked.typed,
        state,
        statement_index as usize,
        expression,
    )
    .is_none_or(|place| matches!(place.root, facts::PlaceRoot::Expression(_)))
    {
        // An immediate computed value has no pre-existing source place to
        // transfer at completion. Its materialization already transported
        // the exact operand claims into this result home. Rejoin that checked
        // binding, rather than interpreting an absent second Transfer event
        // as claim-free. Existing places still use current exit transfers
        // below: their birth roster can be stale after field replacement.
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } = result.source
        else {
            return unsupported("computed return has no materialized result binding");
        };
        let mut matching = operations
            .structural_values
            .iter()
            .filter(|(ordinal, _)| *ordinal == binding_ordinal);
        let (_, binding) = matching.next().ok_or(LoweringError::Unsupported(
            "computed return lost its materialized custody",
        ))?;
        if matching.next().is_some()
            || !binding.path().is_empty()
            || binding.structural_type() != structural_type
            || binding.multiplicity()
                != crate::terminal_identities::structural_multiplicity(result.multiplicity)
        {
            return unsupported("computed return binding is ambiguous or has another carrier");
        }
        let mut claims = binding.claims().to_vec();
        claims.sort_by_key(|binding| binding.claim);
        return Ok(claims);
    }
    lower_owned_transfer_claims(
        identities,
        checked,
        machine,
        state,
        statement_index,
        expression,
        evaluation,
        bindings,
        structural_type,
        structural_types,
    )
}

/// Continue exactly the identities transferred from this authored operand at
/// its consuming evaluation. Storage declarations describe birth custody;
/// field reseating can change that roster without changing the physical home.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_owned_transfer_claims(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    evaluation: checked_trees::FlowPermissionEvaluation,
    bindings: &[(PermissionClaimIdentity, ClaimId)],
    structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
    lower_owned_operand_claims(
        identities,
        checked,
        machine,
        state,
        statement_index,
        expression,
        Some(evaluation),
        Some(language_semantics::PermissionEventSource::Statement {
            statement_index: statement_index as usize,
        }),
        bindings,
        structural_type,
        structural_types,
    )
}

/// Resolve one consumed occurrence, not every move from its source place in
/// the same statement. A constructor's exact operand can inherit either a
/// statement consumer or a call consumer; its checked events must name one
/// unique coordinate. A selected operand's receipt authenticates these same
/// actual move events; it is not a second claim frontier to append or a
/// substitute for evaluating an arm. Exact expression coordinates retain the
/// arm's custody without importing moves from another alternative.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_owned_operand_claims(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    expected_evaluation: Option<checked_trees::FlowPermissionEvaluation>,
    expected_source: Option<language_semantics::PermissionEventSource>,
    bindings: &[(PermissionClaimIdentity, ClaimId)],
    structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
    if !expression.is_valid() {
        return unsupported("owned operand has no exact consuming expression");
    }
    let statement = usize::try_from(statement_index)
        .map_err(|_| LoweringError::Unsupported("structural return coordinate exceeds usize"))?;
    let expression =
        validation::ownership_source_expression(&checked.typed, state, statement, expression);
    let place = validation::ownership_place_from_expression_in_state(
        &checked.typed,
        state,
        statement,
        expression,
    );
    let (root, prefix) = if let Some(place) = &place {
        (place.root, place.segments.as_slice())
    } else {
        (facts::PlaceRoot::Expression(expression), [].as_slice())
    };
    let evaluated_transfers = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == machine
                && event.state_symbol == state
                && event.root == root
                && event.expression == expression
                && event.kind == language_semantics::PermissionEventKind::Transfer
                && event.access == language_semantics::PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live)
                .then_some(event)
        })
        .collect::<Vec<_>>();
    let selection = checked
        .facts
        .flow
        .ownership
        .owned_selection_at(state, statement_index);
    let has_selected_transfer = if let Some((_, receipt)) = selection {
        checked
            .facts
            .flow
            .ownership
            .selection_transfers
            .span(receipt.transfers)
            .ok_or(LoweringError::Unsupported(
                "owned operand selection transfers are stale",
            ))?
            .iter()
            .any(|transfer| transfer.expression == expression)
    } else {
        false
    };
    // Borrowed carriers transport their loans, not an owned claim frontier.
    if evaluated_transfers.is_empty() && !has_selected_transfer {
        return Ok(Vec::new());
    }
    let root_type = if let Some(place) = &place {
        validation::canonical_place_type_reference(
            &checked.typed,
            state,
            statement,
            &validation::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        )
        .ok_or(LoweringError::Unsupported(
            "returned owner has no exact carrier",
        ))?
    } else {
        checked.facts.resolved_types.result(expression)
    };
    let (prefix, _) = validation::structural_owned_path(&checked.typed, root_type, prefix)
        .map_err(LoweringError::Unsupported)?;
    let mut claims = Vec::new();
    let mut consumer = expected_source;
    let mut evaluation = expected_evaluation;
    for event in &evaluated_transfers {
        if event.evaluation == checked_trees::FlowPermissionEvaluation::Unknown
            || evaluation.is_some_and(|lane| lane != event.evaluation)
        {
            return unsupported("owned operand has ambiguous consuming evaluation");
        }
        evaluation = Some(event.evaluation);
        match event.source {
            language_semantics::PermissionEventSource::Statement { statement_index }
            | language_semantics::PermissionEventSource::Call {
                statement_index, ..
            } if statement_index == statement => {}
            _ => return unsupported("owned operand transfer has another consumer statement"),
        }
        if consumer.is_some_and(|source| source != event.source) {
            return unsupported("owned operand has multiple consuming coordinates");
        }
        consumer = Some(event.source);
        let path = validation::structural_claim_path(
            &checked.typed,
            root_type,
            checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments),
        )
        .map_err(LoweringError::Unsupported)?;
        let Some(relative) = path.strip_prefix(prefix.as_slice()) else {
            continue;
        };
        claims.push(terminal_psi::StructuralResultClaimBinding {
            claim: lookup_claim_id(bindings, event.claim_identity)?,
            path: lower_static_argument_path(
                identities,
                relative,
                structural_type,
                structural_types,
            )?,
        });
    }
    if let Some((_, receipt)) = selection {
        let transfers = checked
            .facts
            .flow
            .ownership
            .selection_transfers
            .span(receipt.transfers)
            .ok_or(LoweringError::Unsupported(
                "owned operand selection transfers are stale",
            ))?;
        let mut matching = transfers
            .iter()
            .filter(|transfer| transfer.expression == expression);
        if let Some(transfer) = matching.next() {
            if matching.next().is_some() {
                return unsupported("owned operand has ambiguous selected custody");
            }
            if !transfer.source.is_valid() {
                return unsupported("selected owned operand has no named source custody");
            }
            let source = checked
                .facts
                .flow
                .ownership
                .selection_sources
                .get(transfer.source);
            let source_path = validation::structural_owned_path(
                &checked.typed,
                root_type,
                checked
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(transfer.path),
            )
            .map_err(LoweringError::Unsupported)?
            .0;
            if receipt.machine != machine
                || root != facts::PlaceRoot::Symbol(source.symbol)
                || prefix != source_path
            {
                return unsupported("selected custody names another operand source");
            }
            let selected = checked
                .facts
                .flow
                .ownership
                .selection_transfer_claims
                .span(transfer.claims)
                .ok_or(LoweringError::Unsupported(
                    "owned operand selection claims are stale",
                ))?;
            let mut selected_claims = Vec::new();
            for claim in selected {
                if !evaluated_transfers.iter().any(|event| {
                    event.claim_identity == claim.claim_identity
                        && event.provenance == claim.provenance
                        && checked
                            .facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(event.segments)
                            == checked
                                .facts
                                .flow
                                .ownership
                                .segments
                                .span_or_empty(claim.path)
                }) {
                    return unsupported("selected claim has no matching evaluated transfer");
                }
                let path = validation::structural_claim_path(
                    &checked.typed,
                    root_type,
                    checked
                        .facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(claim.path),
                )
                .map_err(LoweringError::Unsupported)?;
                let relative =
                    path.strip_prefix(prefix.as_slice())
                        .ok_or(LoweringError::Unsupported(
                            "selected claim is outside its operand",
                        ))?;
                selected_claims.push(terminal_psi::StructuralResultClaimBinding {
                    claim: lookup_claim_id(bindings, claim.claim_identity)?,
                    path: lower_static_argument_path(
                        identities,
                        relative,
                        structural_type,
                        structural_types,
                    )?,
                });
            }
            claims.sort();
            selected_claims.sort();
            if claims != selected_claims {
                return unsupported("selected custody differs from its evaluated claim frontier");
            }
        }
    }
    claims.sort();
    if claims.iter().enumerate().any(|(index, claim)| {
        claims[..index]
            .iter()
            .any(|prior| prior.claim == claim.claim || prior.path == claim.path)
    }) {
        return unsupported("owned operand repeats a claim identity or path");
    }
    Ok(claims)
}

pub(super) fn lower_unit_entry_claims(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    claims: &[CheckedUnitEntryClaimPlan],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<LoweredUnitClaims, LoweringError> {
    let mut entry_claims = Vec::with_capacity(claims.len());
    let mut source_claims = Vec::with_capacity(claims.len());
    let mut next_claim = 1_u64;
    for claim in claims {
        if claim.carry != CarryPolicy::STRICT {
            return unsupported("Unit entry claim has a non-default carry policy");
        }
        let parameter = parameters
            .get(usize::try_from(claim.parameter_index).map_err(|_| {
                LoweringError::Unsupported("Unit entry claim parameter index exceeds usize")
            })?)
            .ok_or(LoweringError::Unsupported(
                "Unit entry claim has an invalid parameter index",
            ))?;
        let PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: language_semantics::PermissionEventSource::StateEntry,
            ..
        } = claim.claim_identity
        else {
            return unsupported("Unit entry claim is not an exact checked state-entry claim");
        };
        if machine_symbol != machine || state_symbol != state {
            return unsupported("Unit entry claim belongs to another checked state");
        }
        if source_claims
            .iter()
            .any(|(identity, _)| *identity == claim.claim_identity)
        {
            return unsupported("Unit entry claim identity is duplicated");
        }
        let id = claim_id(allocate_dense(&mut next_claim)?);
        entry_claims.push(EntryClaim {
            claim: id,
            input: parameter.place,
            path: lower_static_argument_path(
                identities,
                &claim.path,
                parameter.structural_type,
                structural_types,
            )?,
        });
        source_claims.push((claim.claim_identity, id));
    }
    Ok(LoweredUnitClaims {
        entry_claims,
        source_claims,
        unreceived_results: Vec::new(),
    })
}

/// Rebase checked identity conservation onto the ordinary invocation and result
/// places. The semantic claim identity, rather than a coincident dense ordinal,
/// joins the content proof to the entry custody namespace.
pub(super) fn lower_result_identity(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    checked_parameters: &[CheckedUnitStructuralParameterPlan],
    parameters: &[StructuralParameterDeclaration],
    result: &TerminalMachineResult,
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<Vec<terminal_psi::ContentIdentityReshuffle>, LoweringError> {
    let facts = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        .cloned()
        .collect::<Vec<_>>();
    let mut lowered =
        crate::proofs::content_conservation::lower_content_identity_reshuffles(&facts)?;
    let remap =
        |place: PlaceId| -> Result<PlaceId, LoweringError> {
            let declaration = lowered
                .structural_places
                .iter()
                .find(|declaration| declaration.id == place)
                .ok_or(LoweringError::Unsupported(
                    "content result has an absent structural root",
                ))?;
            match declaration.kind {
                StructuralPlaceKind::Parameter { position } => checked_parameters
                    .iter()
                    .zip(parameters)
                    .find(|(source, _)| source.position == position)
                    .map(|(_, parameter)| parameter.place)
                    .ok_or(LoweringError::Unsupported(
                        "content result lost its exact input parameter",
                    )),
                StructuralPlaceKind::Result => {
                    result.structural().map(|result| result.place).ok_or(
                        LoweringError::Unsupported("content identity has no structural result"),
                    )
                }
                _ => unsupported("content identity requires whole invocation roots"),
            }
        };
    for reshuffle in &mut lowered.reshuffles {
        let source = lowered
            .source_claims
            .iter()
            .find(|(_, claim)| *claim == reshuffle.claim)
            .ok_or(LoweringError::Unsupported(
                "content result lost its semantic claim",
            ))?
            .0;
        reshuffle.claim = lookup_claim_id(source_claims, source)?;
        reshuffle.input.root = remap(reshuffle.input.root)?;
        reshuffle.output.root = remap(reshuffle.output.root)?;
    }
    Ok(lowered.reshuffles)
}
