//! Exact checked state-entry claims and their machine-local Terminal identities.

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
    lower_owned_transfer_claims(
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
    let statement = usize::try_from(statement_index)
        .map_err(|_| LoweringError::Unsupported("structural return coordinate exceeds usize"))?;
    let place = validation::canonical_place_from_expression_in_state(
        &checked.typed,
        state,
        statement,
        expression,
    );
    let (root, prefix, root_type) = if let Some(place) = place {
        let root_type = validation::canonical_place_type_reference(
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
        ))?;
        (place.root, place.segments, root_type)
    } else {
        (
            facts::PlaceRoot::Expression(expression),
            Vec::new(),
            checked.facts.resolved_types.result(expression),
        )
    };
    let (prefix, _) = validation::structural_owned_path(&checked.typed, root_type, &prefix)
        .map_err(LoweringError::Unsupported)?;
    let mut claims = Vec::new();
    for (_, event) in checked.facts.flow.ownership.permissions.iter() {
        if event.machine_symbol != machine
            || event.state_symbol != state
            || event.root != root
            || event.kind != language_semantics::PermissionEventKind::Transfer
            || event.access != language_semantics::PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Linear
            || !event.obligation_live
            || event.evaluation != evaluation
            || event.source
                != (language_semantics::PermissionEventSource::Statement {
                    statement_index: statement,
                })
        {
            continue;
        }
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
            path: lower_static_argument_path(relative, structural_type, structural_types)?,
        });
    }
    claims.sort();
    Ok(claims)
}

pub(super) fn validate_entry_claims(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    claims: &[CheckedUnitEntryClaimPlan],
) -> Result<(), LoweringError> {
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine
                && event.state_symbol == state.symbol
                && event.source == language_semantics::PermissionEventSource::StateEntry
                && event.kind == language_semantics::PermissionEventKind::Establish
                && event.access == language_semantics::PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .collect::<Vec<_>>();
    if events.len() != claims.len() {
        return unsupported("structural graph entry claim roster disagrees with checked ownership");
    }
    for claim in claims {
        let parameter =
            parameters
                .get(claim.parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural graph entry claim parameter missing",
                ))?;
        if checked
            .state_parameters(state)
            .get(parameter.position as usize)
            .is_none()
        {
            return unsupported("structural graph entry claim source missing");
        }
    }
    Ok(())
}

pub(super) fn lower_unit_entry_claims(
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
