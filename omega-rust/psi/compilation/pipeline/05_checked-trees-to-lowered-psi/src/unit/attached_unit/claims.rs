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
    CheckedTrees, LoweringError, PlaceId, StructuralPlaceKind, TerminalMachineResult,
    allocate_dense, lookup_claim_id, unsupported,
};
use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
#[derive(Clone)]
pub(super) struct LoweredUnitClaims {
    pub(super) entry_claims: Vec<EntryClaim>,
    pub(super) source_claims: Vec<(PermissionClaimIdentity, ClaimId)>,
    pub(super) unreceived_results: Vec<crate::emission::claim_bindings::UnreceivedResultClaim>,
    pub(super) completed_values: Vec<crate::emission::claim_bindings::CompletedValueClaim>,
}

impl LoweredUnitClaims {
    pub(super) fn published(&self) -> crate::emission::claim_bindings::PublishedClaims<'_> {
        crate::emission::claim_bindings::PublishedClaims {
            bindings: &self.source_claims,
            unreceived_results: &self.unreceived_results,
            completed_values: &self.completed_values,
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
    source: &terminal_psi::StructuralArgument,
    operations: &crate::emission::operation_emission::buffer::OperationBuffer,
    current_claims: &crate::emission::current_claims::CurrentClaimNames,
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
    let materialized =
        if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } = result.source
        {
            let mut matching = operations
                .structural_values
                .iter()
                .filter(|(ordinal, _)| *ordinal == binding_ordinal);
            let binding =
                matching
                    .next()
                    .map(|(_, binding)| binding)
                    .ok_or(LoweringError::Unsupported(
                        "return lost its materialized source custody",
                    ))?;
            if matching.next().is_some() {
                return unsupported("return has ambiguous materialized custody");
            }
            if binding.structural_type() != structural_type
                || binding.multiplicity()
                    != crate::terminal_identities::structural_multiplicity(result.multiplicity)
            {
                return unsupported("return materialized custody has another carrier");
            }
            Some(binding)
        } else {
            None
        };
    let completed_claims = current_claims.source(source)?;
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
        let binding = materialized.ok_or(LoweringError::Unsupported(
            "computed return has no materialized result binding",
        ))?;
        if !binding.path().is_empty() {
            return unsupported("computed return binding is projected");
        }
        let mut claims = completed_claims.ok_or(LoweringError::Unsupported(
            "computed return lost its current source custody",
        ))?;
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
        completed_claims.as_deref(),
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
    completed_claims: Option<&[terminal_psi::StructuralResultClaimBinding]>,
    structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
    crate::emission::claim_bindings::lower_owned_operand_claims(
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
        completed_claims,
        structural_type,
        structural_types,
    )
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
        completed_values: Vec::new(),
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
