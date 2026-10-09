use super::super::super::{
    BlockId, ScalarType, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
    StructuralTypeDeclaration, SuccessorEdge, ValueId, block_id,
};
use super::super::{
    Block, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, LoweringError,
    Multiplicity, OperationKind, OperationResult, PlaceId, StructuralMultiplicity,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId, StructuralTypeShape,
    Terminator, ValueDeclaration, allocate_dense, argument_evaluation, edge_id, lookup_type_id,
    lower_structural_path, obligation_id,
    operation_frame::{
        allocate_structural_result, block_parameter_declaration, operation_result_declaration,
        structural_parameter_declaration,
    },
    place_id, unsupported, value_id,
};
use super::CheckedTrees;
use crate::emission::operation_emission::buffer::{OperationBuffer, scalar_result_declaration};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::emit_scalar_leaf;
use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
use crate::expression_preparation::bindings::{LoanAccess, ScalarBindings};
use crate::expression_preparation::byte_literals;
use arena::HandleSpan;
use checked_trees::{
    CheckedStructuralValue, CheckedStructuralValueHandle, CheckedStructuralValueKind,
};

mod completed_storage;
mod completions;
mod copied_place;
mod dispatch;
mod element_views;
mod parameters;
mod reference;
mod residual_complement;

pub(super) type StructuralOperandEmitter<'a> = dyn FnMut(
        &CheckedUnitEffectOperationPlan,
        Option<&[ValueDeclaration]>,
        usize,
        &mut argument_evaluation::Evaluation,
        &mut Vec<ValueDeclaration>,
        &mut u64,
        &mut u64,
        &mut u64,
        &mut CallEmissionContext<'_>,
        &mut OperationBuffer,
        &mut u64,
        &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<Option<StructuralPlaceDeclaration>, LoweringError>
    + 'a;

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[crate::terminal_identities::StructuralDomainBinding],
    next_place: &mut u64,
    temporary_places: &mut Vec<StructuralPlaceDeclaration>,
    operand_emitter: &mut StructuralOperandEmitter<'_>,
    calls: &mut CallEmissionContext<'_>,
    evaluation: &mut argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    result_claims: Vec<terminal_psi::StructuralResultClaimBinding>,
) -> Result<Option<StructuralPlaceDeclaration>, LoweringError> {
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result,
        value,
        calls: operand_calls,
        ..
    } = operation
    else {
        return unsupported("structural value producer missing");
    };
    let structural_type = lookup_type_id(type_ids, &result.type_identity)?;
    let multiplicity = crate::terminal_identities::structural_multiplicity(result.multiplicity);
    let authored_state = crate::declarations::state_declaration(checked, state)?.1;
    let sources = checked
        .facts
        .flow
        .ownership
        .owned_selection_at(state, result.statement_index)
        .map(|(_, receipt)| {
            checked
                .facts
                .flow
                .ownership
                .selection_sources
                .span(receipt.sources)
                .ok_or(LoweringError::Unsupported(
                    "owned selection source span is stale",
                ))?
                .iter()
                .rev()
                .map(|source| {
                    if let Some(argument) =
                        crate::expression_preparation::bindings::structural_local_by_root(
                            &evaluation.structural_locals,
                            facts::PlaceRoot::Symbol(source.symbol),
                        )
                        .map(|local| local.argument.clone())
                    {
                        return Ok(argument);
                    }
                    // Parameter sources keep their physical ingress place: the
                    // receipt row names the authored position and the
                    // signature declaration supplies its whole-owned custody.
                    let parameter = checked
                        .state_parameters(authored_state)
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| parameter.symbol == source.symbol)
                        .ok_or(LoweringError::Unsupported(
                            "owned selection source has no established physical local",
                        ))?;
                    let declaration = evaluation
                        .structural_parameters
                        .iter()
                        .find(|(position, _)| *position == parameter.0 as u32)
                        .map(|(_, declaration)| declaration)
                        .ok_or(LoweringError::Unsupported(
                            "owned selection parameter source has no signature place",
                        ))?;
                    if !declaration.is_unqualified_owned_affine() {
                        return Err(LoweringError::Unsupported(
                            "owned selection parameter source lacks whole plain-affine custody",
                        ));
                    }
                    Ok(StructuralArgument {
                        place: declaration.place,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if !sources.is_empty() && multiplicity != StructuralMultiplicity::Affine {
        return unsupported("owned selection requires uniform affine root cleanup");
    }
    // A referent alias observes storage through the selection's address join.
    // A materialized descriptor instead transfers its own affine lifetime
    // resource, even when the source binding has shared referent access.
    // Statement construction records that carrier multiplicity before
    // lowering; local access alone would lend the descriptor itself while
    // every incoming arm transfers it owned. Keep those two custody domains
    // distinct without choosing a route from the initializer's source shape.
    let result_access = if result.local.access
        == checked_trees::CheckedStructuralAccess::SharedBorrow
        && result.multiplicity == Multiplicity::Unrestricted
    {
        StructuralAccess::SharedBorrow
    } else {
        StructuralAccess::Owned
    };
    if result_access == StructuralAccess::SharedBorrow && !sources.is_empty() {
        return unsupported("borrowed selection cannot carry owned residual custody");
    }
    let owners = if sources.is_empty() {
        Vec::new()
    } else {
        let owners = prepare_owners(
            checked,
            machine,
            state,
            result.statement_index,
            structural_types,
            evaluation,
            operations,
        )?;
        if owners
            .iter()
            .filter(|owner| {
                sources
                    .iter()
                    .any(|source| source.place == owner.value.place)
            })
            .count()
            != sources.len()
        {
            return unsupported("owned selection source is absent from the live emitted frontier");
        }
        owners
    };
    let mut emission = Emission {
        checked,
        machine,
        state,
        statement: result.statement_index,
        structural_type,
        multiplicity,
        structural_types,
        type_ids,
        domain_ids,
        type_reference: structural_values(checked)
            .root_for_expression(
                state,
                result.statement_index,
                structural_values(checked).nodes.get(*value).expression,
            )
            .map(|root| root.type_reference)
            .unwrap_or_default(),
        next_place,
        temporary_places,
        calls,
        operand_calls,
        operand_emitter,
        evaluation,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        sources,
        owners,
        result_access,
        published_places: Vec::new(),
        arrival_claims: Vec::new(),
        completion_claims: result_claims.clone(),
    };
    // An owned projection of an already established place transfers one child
    // and preserves the remaining siblings
    // (`wiki/spec/terminal-psi/structural_access.md`,
    // `wiki/spec/language/ownership.md`): the authored binding establishes no
    // storage of its own, so it lowers to no operation and no edge. Bind it as
    // a projected alias over the root's place and let each later use project
    // the composed path at its real consuming operation, which is the lane
    // Terminal Psi admits (`validation/frontier/operations.rs` records the
    // hole in `partial_custody_paths` while the root stays live). The Jump
    // lane categorically refuses a projected owned `Linear` leaf, so
    // synthesizing a selection join here would open a linearity hole instead.
    //
    // Two restrictions are deliberate. A statement that carries an
    // owned-selection receipt keeps the selection route, because that receipt
    // schedules its sources' residual cleanup on the edge and an alias has no
    // edge to carry it. And only a `Place` source aliases: a projection of a
    // call result names a temporary whose own residual complement nothing else
    // consumes, so that shape keeps the route it already had.
    if emission.sources.is_empty()
        && let CheckedStructuralValueKind::Projection {
            source,
            path,
            type_identity,
        } = structural_values(checked).nodes.get(*value).kind.clone()
        && matches!(
            structural_values(checked).nodes.get(source).kind,
            CheckedStructuralValueKind::Place(_)
        )
    {
        let projected = emission.projected_root(source, path, type_identity)?;
        let alias_path = projected.lowered_path(emission.type_ids, emission.structural_types)?;
        // The leaf's claim is published where its root was established: the
        // alias names that exact place and path, so the consuming operation
        // resolves the binding already on the root's result. An alias has no
        // result of its own, so a claim this statement minted fresh would
        // have nowhere to land; require the root to carry each one.
        // The root's claim roster is on its producing operation's result,
        // or -- when its value completed at a control join -- on the
        // completed binding the join published for it.
        let mut published_claims =
            completed_place_claims(emission.operations, projected.root).collect::<Vec<_>>();
        // Invocation parameters have no producing operation. Join their
        // checked entry frontier to the same physical source home and claim
        // namespace, including a later state's retained entry-place alias.
        if let Some((position, _)) =
            emission
                .evaluation
                .structural_parameters
                .iter()
                .find(|(_, parameter)| {
                    parameter.place == projected.root && parameter.access == StructuralAccess::Owned
                })
        {
            let parameter = checked
                .state_parameters(authored_state)
                .get(*position as usize)
                .ok_or(LoweringError::Unsupported(
                    "projected parameter has no source declaration",
                ))?;
            for (_, event) in checked.facts.flow.ownership.permissions.iter() {
                if event.state_symbol != state
                    || event.root != facts::PlaceRoot::Symbol(parameter.symbol)
                    || event.source != language_semantics::PermissionEventSource::StateEntry
                    || event.kind != language_semantics::PermissionEventKind::Establish
                    || event.access != language_semantics::PermissionAccess::Owned
                    || event.multiplicity != Multiplicity::Linear
                    || !event.obligation_live
                {
                    continue;
                }
                let path = validation::structural_claim_path(
                    &checked.typed,
                    parameter.type_reference,
                    checked
                        .facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments),
                )
                .map_err(LoweringError::Unsupported)?;
                published_claims.push(terminal_psi::StructuralResultClaimBinding {
                    claim: crate::terminal_identities::lookup_claim_id(
                        emission.calls.claims.bindings(),
                        event.claim_identity,
                    )?,
                    path: lower_static_argument_path(
                        &path,
                        lookup_type_id(emission.type_ids, &projected.root_type_identity)?,
                        emission.structural_types,
                    )?,
                });
            }
        }
        // A field store reseats its subtree: the stored value's claims, rebased
        // under the stored field, replace whatever the root held there. Stores
        // apply in emission order after the root's establishment.
        for operation in &emission.operations.operations {
            if let OperationKind::StoreStructuralField {
                destination,
                path,
                field: Some(field),
                value,
            } = &operation.kind
                && *destination == projected.root
                && value.path.is_empty()
            {
                let mut stored_path = path.clone();
                stored_path.push(terminal_psi::StructuralPathSegment::Field(
                    crate::structural_types::field_identity(emission.structural_types, *field)?,
                ));
                let stored_claims =
                    completed_place_claims(emission.operations, value.place).collect::<Vec<_>>();
                published_claims.retain(|claim| !claim.path.starts_with(&stored_path));
                published_claims.extend(stored_claims.into_iter().map(|claim| {
                    let mut path = stored_path.clone();
                    path.extend(claim.path.iter().cloned());
                    terminal_psi::StructuralResultClaimBinding {
                        claim: claim.claim,
                        path,
                    }
                }));
            }
            // Loan completion establishes custody at the existing home, not
            // an operation result. Interleave it with stores; collecting all
            // completions last would overwrite a later store's real claims.
            for completion in emission
                .operations
                .loan_completions
                .iter()
                .filter(|completion| {
                    completion.operation == operation.id && completion.place == projected.root
                })
            {
                published_claims.retain(|claim| !claim.path.starts_with(&completion.path));
                published_claims.extend(completion.claims.iter().cloned());
            }
        }
        for binding in &result_claims {
            let mut published_path = alias_path.clone();
            published_path.extend(binding.path.iter().cloned());
            if !published_claims.iter().any(|published| {
                published.claim == binding.claim && published.path == published_path
            }) {
                return unsupported("projected alias claim is not published by its root");
            }
        }
        // A projection establishes no new owner, but still carries every
        // existing claim below its selected path. Return-only occurrences
        // have no lexical Establish event: using that event's empty roster
        // here would erase custody rather than continue the root's identities.
        let selected_claims = published_claims
            .into_iter()
            .filter_map(|claim| {
                claim
                    .path
                    .strip_prefix(alias_path.as_slice())
                    .map(|suffix| terminal_psi::StructuralResultClaimBinding {
                        claim: claim.claim,
                        path: suffix.to_vec(),
                    })
            })
            .collect();
        let alias =
            crate::emission::operation_emission::buffer::StructuralBinding::ProjectedAlias {
                place: projected.root,
                path: alias_path,
                structural_type,
                multiplicity,
                claims: selected_claims,
            };
        emission.evaluation.establish_structural_binding(
            result,
            alias,
            structural_types,
            emission.operations,
        )?;
        return Ok(None);
    }
    let place = if emission.sources.is_empty()
        && let Some((source, region, loan)) =
            match &structural_values(checked).nodes.get(*value).kind {
                CheckedStructuralValueKind::BorrowedSliceView { source, region }
                    if source.access == checked_trees::CheckedStructuralAccess::SharedBorrow =>
                {
                    Some((source, region, LoanAccess::Shared))
                }
                CheckedStructuralValueKind::MutableSliceView { source, region }
                    if source.access == checked_trees::CheckedStructuralAccess::MutableBorrow =>
                {
                    Some((source, region, LoanAccess::Mutable))
                }
                _ => None,
            } {
        emission.direct_element_view(
            source,
            region,
            structural_values(checked).nodes.get(*value).expression,
            loan,
        )?
    } else {
        let place = emission.value(*value, None)?;
        // A moved place is materialized into the result binding; a copied
        // parameter is already a fresh operation result.
        if emission.sources.is_empty()
            && let CheckedStructuralValueKind::Place(argument) =
                &structural_values(checked).nodes.get(*value).kind
            && emission.copied_parameter(argument)?.is_none()
            && !matches!(
                argument.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
            )
        {
            emission.materialize_place(place, &result_claims)?
        } else {
            place
        }
    };
    // Private arm producers are not authored result ordinals. Publish exactly
    // one completed place for this binding, whether a direct producer or join.
    // A call-rooted producer's place was catalogued by the call emitter as its
    // own call result, so it claims no temporary row and yields no declaration
    // for the caller to republish.
    let declaration = emission
        .temporary_places
        .iter()
        .position(|item| item.id == place)
        .map(|position| emission.temporary_places.remove(position));
    let published_declaration = emission.published_places.iter().find(|row| row.id == place);
    let completed_declaration =
        declaration
            .as_ref()
            .or(published_declaration)
            .ok_or(LoweringError::Unsupported(
                "structural value place was not declared",
            ))?;
    let arrival_claims = emission
        .arrival_claims
        .iter()
        .find(|(arrival, _)| *arrival == place)
        .map(|(_, claims)| claims.as_slice())
        .unwrap_or_default();
    if let StructuralPlaceKind::OperationResult { .. } = completed_declaration.kind
        && !result_claims.is_empty()
    {
        Emission::bind_completion_claims(emission.operations, place, &result_claims)?;
    }
    let completed = completed_storage::completed_result(
        place,
        completed_declaration,
        emission.operations,
        emission.evaluation.structural_root_parameters(),
        arrival_claims,
    )?;
    emission.evaluation.establish_structural_binding(
        result,
        completed.into(),
        structural_types,
        emission.operations,
    )?;
    Ok(declaration)
}

/// The structural-value plans recorded for the checked unit./// Allocate a fresh dense id for each carried value declaration.
fn fresh_value_declarations(
    next_value: &mut u64,
    values: &[ValueDeclaration],
) -> Result<Vec<ValueDeclaration>, LoweringError> {
    values
        .iter()
        .map(|value| {
            Ok(ValueDeclaration {
                id: value_id(allocate_dense(next_value)?),
                ..*value
            })
        })
        .collect()
}

fn structural_values(checked: &CheckedTrees) -> &checked_trees::CheckedStructuralValuePlans {
    &checked.facts.values.structural_values
}

fn prepare_owners(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    structural_types: &[StructuralTypeDeclaration],
    evaluation: &argument_evaluation::Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<argument_evaluation::StructuralValueOwner>, LoweringError> {
    let source_parameters =
        checked.state_parameters(crate::declarations::state_declaration(checked, state)?.1);
    let available = |symbol: symbols::SymbolHandle,
                     formation: u32|
     -> Result<bool, LoweringError> {
        if !symbol.is_valid() {
            return Ok(true);
        }
        // Parameter slots are ordering coordinates, not statement formations:
        // state-entry owners precede every statement, including statement zero.
        let entry = source_parameters
            .iter()
            .any(|parameter| parameter.symbol == symbol);
        for (_, receipt) in checked.facts.flow.ownership.owned_selections.iter() {
            if receipt.machine == machine
                && receipt.state == state
                && (entry || receipt.statement_ordinal > formation)
                && receipt.statement_ordinal < statement
                && checked
                    .facts
                    .flow
                    .ownership
                    .selection_sources
                    .span_or_empty(receipt.sources)
                    .iter()
                    .any(|source| source.symbol == symbol)
            {
                return Ok(false);
            }
        }
        for (_, event) in checked.facts.flow.ownership.permissions.iter() {
            if event.machine_symbol != machine
                || event.state_symbol != state
                || event.root != facts::PlaceRoot::Symbol(symbol)
                || event.access != language_semantics::PermissionAccess::Owned
            {
                continue;
            }
            let ordinal = match event.source {
                language_semantics::PermissionEventSource::Statement { statement_index }
                | language_semantics::PermissionEventSource::Call {
                    statement_index, ..
                } => statement_index,
                _ => continue,
            };
            if (!entry && ordinal <= formation as usize)
                || ordinal >= statement as usize
                || event.kind == language_semantics::PermissionEventKind::Establish
            {
                continue;
            }
            if !event.segments.is_empty() {
                return unsupported(
                    "selection pass-through of partially moved roots requires retained residual frontier correspondence",
                );
            }
            return Ok(false);
        }
        Ok(true)
    };
    let mut owners = Vec::new();
    for owner in &evaluation.structural_value_owners {
        if available(owner.symbol, owner.statement)? {
            owners.push(owner.clone());
        }
    }
    let locals = checked.facts.state_locals.locals(state);
    for crate::expression_preparation::bindings::StructuralLocalBinding {
        root, argument, ..
    } in &evaluation.structural_locals
    {
        let Some(symbol) = root.as_symbol() else {
            continue;
        };
        let local =
            checked
                .facts
                .state_locals
                .named(state, symbol)
                .ok_or(LoweringError::Unsupported(
                    "selection survivor has no authored local",
                ))?;
        if owners.iter().any(|owner| owner.symbol == symbol)
            || !available(symbol, local.statement_index)?
        {
            continue;
        }
        if checked.type_multiplicity(local.declared_type) != Multiplicity::Affine {
            continue;
        }
        if local.statement_index >= statement
            || !argument.path.is_empty()
            || argument.access != StructuralAccess::Owned
            || !crate::content_classes::owned_or_shared_view_contents(checked, local.declared_type)
        {
            return unsupported("selection survivor requires whole plain-affine custody");
        }
        let value = operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == argument.place)
            .ok_or(LoweringError::Unsupported(
                "selection survivor requires an exact emitted operation-result frontier",
            ))?;
        if value.multiplicity != StructuralMultiplicity::Affine
            || !value.claims.is_empty()
            || !value.qualifications.is_empty()
            || !value.projected_qualifications.is_empty()
            || !structural_types
                .iter()
                .any(|declaration| declaration.id == value.structural_type)
        {
            return unsupported("selection survivor requires claim-free plain-affine transport");
        }
        owners.push(argument_evaluation::StructuralValueOwner {
            symbol: symbol,
            statement: local.statement_index,
            value: value.clone(),
        });
    }
    // Selected parameter sources join the frontier as owners: their ingress
    // places are real signature places and their residual slots die on the
    // receipt's actual death edge like any local source.
    if let Some((_, receipt)) = checked
        .facts
        .flow
        .ownership
        .owned_selection_at(state, statement)
    {
        for (position, parameter) in source_parameters.iter().enumerate() {
            if !checked
                .facts
                .flow
                .ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .iter()
                .any(|source| {
                    source.symbol == parameter.symbol && source.statement_ordinal == position as u32
                })
            {
                continue;
            }
            if parameter.is_const {
                return unsupported("selection parameter source is not owned runtime ingress");
            }
            let declaration = evaluation
                .structural_parameters
                .iter()
                .find(|(source_position, _)| *source_position as usize == position)
                .map(|(_, declaration)| declaration)
                .ok_or(LoweringError::Unsupported(
                    "selection parameter source has no signature place",
                ))?;
            if !declaration.is_unqualified_owned_affine()
                || checked.type_multiplicity(parameter.type_reference) != Multiplicity::Affine
                || !crate::content_classes::owned_or_shared_view_contents(
                    checked,
                    parameter.type_reference,
                )
            {
                return unsupported(
                    "selection parameter source requires whole plain-affine custody",
                );
            }
            if owners
                .iter()
                .any(|owner| owner.value.place == declaration.place)
            {
                return unsupported("selection parameter source duplicated its frontier entry");
            }
            owners.push(argument_evaluation::StructuralValueOwner {
                symbol: parameter.symbol,
                statement: u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported("selection parameter position exceeds u32")
                })?,
                value: terminal_psi::StructuralOperationResult {
                    qualification_establishments: Vec::new(),
                    place: declaration.place,
                    structural_type: declaration.structural_type,
                    multiplicity: declaration.multiplicity,
                    qualifications: declaration.qualifications.clone(),
                    projected_qualifications: declaration.projected_qualifications.clone(),
                    claims: Vec::new(),
                },
            });
        }
    }
    // Deterministic establishment order: parameters were established at state
    // entry, so their slots precede locals kept in ascending statement order.
    // The join binds these slots positionally against the receipt's source
    // complement, and the return splice's reverse-establishment roster then
    // matches the verifier's reverse parameter disposal exactly.
    owners.sort_by_key(|owner| {
        let is_parameter = source_parameters
            .iter()
            .any(|parameter| parameter.symbol == owner.symbol);
        (!is_parameter, owner.statement)
    });
    for local in locals
        .iter()
        .take_while(|local| local.statement_index < statement)
    {
        if checked.type_multiplicity(local.declared_type) == Multiplicity::Affine
            && available(local.symbol, local.statement_index)?
            && !owners.iter().any(|owner| owner.symbol == local.symbol)
        {
            return unsupported(
                "selection transport requires a typed frontier entry for every live owned local",
            );
        }
    }
    Ok(owners)
}

pub(super) struct Emission<'a, 'b, 'calls> {
    pub(super) checked: &'a CheckedTrees,
    pub(super) machine: symbols::SymbolHandle,
    pub(super) state: symbols::SymbolHandle,
    pub(super) statement: u32,
    pub(super) structural_type: StructuralTypeId,
    pub(super) multiplicity: StructuralMultiplicity,
    pub(super) structural_types: &'a [StructuralTypeDeclaration],
    pub(super) type_ids: &'a [(String, StructuralTypeId)],
    pub(super) domain_ids: &'a [crate::terminal_identities::StructuralDomainBinding],
    type_reference: checked_trees::types::TypeReferenceHandle,
    pub(super) next_place: &'b mut u64,
    pub(super) temporary_places: &'b mut Vec<StructuralPlaceDeclaration>,
    pub(super) calls: &'b mut CallEmissionContext<'calls>,
    pub(super) operand_calls: &'a [checked_trees::CheckedStructuralValueCall],
    pub(super) operand_emitter: &'b mut StructuralOperandEmitter<'a>,
    pub(super) evaluation: &'b mut argument_evaluation::Evaluation,
    pub(super) values: &'b mut Vec<ValueDeclaration>,
    pub(super) next_value: &'b mut u64,
    pub(super) next_block: &'b mut u64,
    pub(super) next_edge: &'b mut u64,
    pub(super) operations: &'b mut OperationBuffer,
    sources: Vec<StructuralArgument>,
    owners: Vec<argument_evaluation::StructuralValueOwner>,
    /// The join result slot's custody: `Owned` for an owned selection,
    /// `SharedBorrow` for a `&T` result whose branches rejoin shared borrows.
    result_access: StructuralAccess,
    /// Exact declarations already published by nested operation emission.
    /// These are receipts, not new catalog entries owned by this value.
    published_places: Vec<StructuralPlaceDeclaration>,
    /// Claim identities arriving through whole-owned edges. These are copied
    /// from the source custody, never attached to a fictitious producer.
    pub(super) arrival_claims: Vec<(PlaceId, Vec<terminal_psi::StructuralResultClaimBinding>)>,
    /// Authenticated claims of the value being completed, relative to its
    /// current constructor context. A child carries the selected suffix before
    /// its parent can consume it; attaching claims only to the outermost result
    /// would create them after the actual establishment.
    completion_claims: Vec<terminal_psi::StructuralResultClaimBinding>,
}

pub(super) struct ValueContinuation {
    block: BlockId,
    parameters: Vec<ValueDeclaration>,
    structural_parameters: Vec<StructuralParameterDeclaration>,
    place: PlaceId,
    remaining_owners: Vec<argument_evaluation::StructuralValueOwner>,
    pass_through: Vec<(PlaceId, PlaceId)>,
    residuals: Vec<PlaceId>,
}

/// One projected affine child moved on a selected edge. The root place keeps
/// its once-evaluated owner, `path` is the exact checked field/fixed-index
/// projection, and the two identities let the edge argument carry the moved
/// leaf type while `residual_complement` derives the root's untouched subtrees.
pub(super) struct ProjectedMove {
    pub(super) root: PlaceId,
    /// The already lowered walk from `root`'s storage to the checked source
    /// the projection reads. It is empty unless that source is itself a
    /// projected owned alias, which holds no storage of its own.
    pub(super) root_path: Vec<terminal_psi::StructuralPathSegment>,
    pub(super) path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub(super) leaf_type_identity: String,
    pub(super) root_type_identity: String,
    pub(super) root_source: checked_trees::CheckedUnitStructuralArgumentSourcePlan,
}

impl ProjectedMove {
    /// The complete walk from `root`'s storage to the projected leaf.
    fn lowered_path(
        &self,
        type_ids: &[(String, StructuralTypeId)],
        types: &[StructuralTypeDeclaration],
    ) -> Result<Vec<terminal_psi::StructuralPathSegment>, LoweringError> {
        let mut path = self.root_path.clone();
        path.extend(lower_static_argument_path(
            &self.path,
            lookup_type_id(type_ids, &self.root_type_identity)?,
            types,
        )?);
        Ok(path)
    }
}

impl Emission<'_, '_, '_> {
    /// The argument source arriving at `place`, when the receipt carries one.
    fn source_at(&self, place: PlaceId) -> Option<&StructuralArgument> {
        self.sources.iter().find(|source| source.place == place)
    }

    fn bind_completion_claims(
        operations: &mut OperationBuffer,
        place: PlaceId,
        claims: &[terminal_psi::StructuralResultClaimBinding],
    ) -> Result<(), LoweringError> {
        let produced = operations
            .operations
            .iter_mut()
            .find_map(|operation| match &mut operation.result {
                OperationResult::Structural(result) if result.place == place => Some(result),
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "owned completion has no operation result",
            ))?;
        if !produced.claims.is_empty() && produced.claims != claims {
            return unsupported("owned completion changes its established claim identities");
        }
        produced.claims = claims.to_vec();
        Ok(())
    }

    /// Execute this call's retained preparation only when the value evaluator
    /// reaches its node. The ordinary argument schedule interleaves scalar
    /// observations and structural producers in authored formal order.
    fn emit_operand_call(
        &mut self,
        call: &checked_trees::CheckedStructuralValueCall,
    ) -> Result<StructuralPlaceDeclaration, LoweringError> {
        use super::super::argument_schedule::{self, Step};
        let planned = call
            .operands()
            .iter()
            .chain(std::iter::once(call.operation()))
            .cloned()
            .collect::<Vec<_>>();
        let schedule =
            argument_schedule::build_operations(self.checked, self.machine, self.state, &planned)?;
        let mut staged_values = vec![Vec::new(); planned.len()];
        let mut source_count = None;
        let mut completed = None;
        for step in schedule {
            let (index, staged) = match step {
                Step::CaptureBefore(statement) => {
                    self.evaluation.prepare_storage_before(
                        self.checked,
                        self.state,
                        statement,
                        self.structural_types,
                        self.values,
                        self.next_place,
                        self.next_value,
                        self.operations,
                        self.temporary_places,
                    )?;
                    continue;
                }
                Step::Begin => {
                    if source_count.replace(self.values.len()).is_some() {
                        return unsupported("structural operand argument group is already active");
                    }
                    continue;
                }
                Step::End => {
                    let count = source_count.take().ok_or(LoweringError::Unsupported(
                        "structural operand argument group is absent",
                    ))?;
                    self.values.truncate(count);
                    continue;
                }
                Step::Argument { operation, ordinal } => {
                    let count = source_count.ok_or(LoweringError::Unsupported(
                        "structural operand scalar has no source prefix",
                    ))?;
                    if staged_values[operation].len() != ordinal {
                        return unsupported("structural operand scalar ordinals are not dense");
                    }
                    let value = self.evaluation.argument_at(
                        self.checked,
                        self.machine,
                        self.state,
                        &planned[operation],
                        ordinal,
                        count,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.operations,
                        self.calls,
                    )?;
                    staged_values[operation].push(self.values.len());
                    self.values.push(value);
                    continue;
                }
                Step::Subslice { .. } => {
                    return unsupported(
                        "structural value call requires retained slice argument transport",
                    );
                }
                Step::Produce(index) | Step::Call(index) if source_count.is_some() => (index, true),
                Step::Ordinary(index) if source_count.is_none() => (index, false),
                _ => return unsupported("structural operand argument schedule is unbalanced"),
            };
            let operation = &planned[index];
            let count = source_count.unwrap_or(self.values.len());
            let evaluated = if staged && operation.call().is_some() {
                Some(
                    staged_values[index]
                        .iter()
                        .map(|slot| {
                            self.values
                                .get(*slot)
                                .copied()
                                .ok_or(LoweringError::Unsupported(
                                    "structural operand scalar lost its retained slot",
                                ))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            } else if operation.call().is_some() {
                self.evaluation.arguments(
                    self.checked,
                    self.machine,
                    self.state,
                    operation,
                    self.values,
                    self.next_value,
                    self.next_block,
                    self.next_edge,
                    self.operations,
                    self.calls,
                )?
            } else {
                None
            };
            let declaration = (self.operand_emitter)(
                operation,
                evaluated.as_deref(),
                count,
                self.evaluation,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.calls,
                self.operations,
                self.next_place,
                self.temporary_places,
            )?;
            if index + 1 == planned.len() {
                completed = declaration;
            }
        }
        let completed = completed.ok_or(LoweringError::Unsupported(
            "structural operand call did not publish its result",
        ))?;
        self.published_places.push(completed);
        Ok(completed)
    }

    /// Rebind completion custody beneath a child's complete typed prefix.
    /// Payload prefixes retain Case as well as Field; storage overlay does not
    /// erase the discriminator guard on an ownership obligation.
    pub(super) fn child_constructor_operand(
        &mut self,
        child_type: StructuralTypeId,
        value: CheckedStructuralValueHandle,
        reference: checked_trees::types::TypeReferenceHandle,
        prefix: &[terminal_psi::StructuralPathSegment],
    ) -> Result<StructuralArgument, LoweringError> {
        let parent_type = self.structural_type;
        let parent_multiplicity = self.multiplicity;
        let child_claims = self
            .completion_claims
            .iter()
            .filter_map(|claim| {
                claim.path.strip_prefix(prefix).map(|path| {
                    terminal_psi::StructuralResultClaimBinding {
                        claim: claim.claim,
                        path: path.to_vec(),
                    }
                })
            })
            .collect();
        let parent_claims = std::mem::replace(&mut self.completion_claims, child_claims);
        self.structural_type = child_type;
        self.multiplicity = crate::terminal_identities::structural_multiplicity(
            crate::reference_custody::result_multiplicity(self.checked, reference),
        );
        let argument = self.constructor_operand(value, reference);
        self.structural_type = parent_type;
        self.multiplicity = parent_multiplicity;
        self.completion_claims = parent_claims;
        argument
    }

    /// Complete one constructor operand without erasing an owned projection.
    /// The consuming constructor moves that subtree under ordinary partial
    /// custody; siblings remain attached to the original live root.
    pub(super) fn constructor_operand(
        &mut self,
        value: CheckedStructuralValueHandle,
        reference: checked_trees::types::TypeReferenceHandle,
    ) -> Result<StructuralArgument, LoweringError> {
        let node = self.structural_value_node(value);
        if let CheckedStructuralValueKind::Projection {
            source,
            path,
            type_identity,
        } = node.kind
            && matches!(
                structural_values(self.checked).nodes.get(source).kind,
                CheckedStructuralValueKind::Place(_) | CheckedStructuralValueKind::Call { .. }
            )
        {
            let projected = self.projected_root(source, path, type_identity)?;
            return Ok(StructuralArgument {
                place: projected.root,
                path: projected.lowered_path(self.type_ids, self.structural_types)?,
                access: StructuralAccess::Owned,
            });
        }
        // A fresh operand has its own continuation, not the enclosing
        // selection's result/residual slots. Incoming owners remain live
        // until their actual consuming operation or outer selection edge.
        let sources = std::mem::take(&mut self.sources);
        let owners = std::mem::take(&mut self.owners);
        let parent_reference = std::mem::replace(&mut self.type_reference, reference);
        let result = self
            .value(value, None)
            .and_then(|place| {
                if !self.completion_claims.is_empty()
                    && self.operations.operations.iter().any(|operation| {
                        operation
                            .result
                            .structural()
                            .is_some_and(|result| result.place == place)
                    })
                {
                    Self::bind_completion_claims(self.operations, place, &self.completion_claims)?;
                }
                Ok(place)
            })
            .and_then(|place| self.retain_checked_memberships(place, node.qualification_decisions));
        self.type_reference = parent_reference;
        self.sources = sources;
        self.owners = owners;
        result.map(|place| StructuralArgument {
            place,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        })
    }

    pub(super) fn structural_value_node(
        &self,
        value: CheckedStructuralValueHandle,
    ) -> CheckedStructuralValue {
        structural_values(self.checked).nodes.get(value).clone()
    }

    /// Every structured literal's member list lives in the same record-field
    /// span; each caller names its own staleness.
    pub(super) fn structural_value_fields(
        &self,
        fields: HandleSpan<checked_trees::CheckedStructuralRecordField>,
        stale: &'static str,
    ) -> Result<Vec<checked_trees::CheckedStructuralRecordField>, LoweringError> {
        Ok(structural_values(self.checked)
            .record_fields
            .span(fields)
            .ok_or(LoweringError::Unsupported(stale))?
            .to_vec())
    }

    /// Private control flow remaps saved scalar values. Read their final slots
    /// only after every later field evaluation has completed.
    pub(super) fn remap_saved_scalar_fields(
        &mut self,
        scalar_positions: Vec<(usize, usize)>,
        initialized: &mut [(usize, terminal_psi::RecordFieldInitializer)],
        source_count: usize,
    ) {
        for (field, position) in scalar_positions {
            let terminal_psi::RecordFieldValue::Scalar { value, .. } =
                &mut initialized[field].1.value
            else {
                unreachable!()
            };
            *value = self.values[position].id;
        }
        self.values.truncate(source_count);
    }

    pub(super) fn value(
        &mut self,
        value: CheckedStructuralValueHandle,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let node = self.structural_value_node(value);
        match node.kind {
            // A borrowed `&[T]` view lowers to `EstablishElementView` and a
            // `&mut [T]` view to `EstablishMutableElementView`: the collection
            // argument resolves against the current locals and signature
            // parameters exactly as a `&T` leaf does, and the fresh view result
            // joins the selection's block parameter under the view's loan.
            CheckedStructuralValueKind::BorrowedSliceView { source, region } => self
                .joined_element_view(
                    &source,
                    &region,
                    node.expression,
                    LoanAccess::Shared,
                    continuation,
                ),
            CheckedStructuralValueKind::MutableSliceView { source, region } => self
                .joined_element_view(
                    &source,
                    &region,
                    node.expression,
                    LoanAccess::Mutable,
                    continuation,
                ),
            CheckedStructuralValueKind::Reference { source } => {
                self.reference_value(&source, continuation)
            }
            CheckedStructuralValueKind::Record { .. } => {
                let place = self.record(value)?;

                self.completed(place, continuation)
            }
            CheckedStructuralValueKind::FixedArray { elements } => {
                let place = self.fixed_array(&elements, self.type_reference)?;

                self.completed(place, continuation)
            }
            CheckedStructuralValueKind::Call { .. } => {
                let mut matching = self.operand_calls.iter().filter(|call| call.value == value);
                let call = matching.next().ok_or(LoweringError::Unsupported(
                    "structural operand call missing",
                ))?;
                if matching.next().is_some() {
                    return unsupported("structural operand call is duplicated");
                }
                let declaration = self.emit_operand_call(call)?;
                let StructuralPlaceKind::OperationResult { .. } = declaration.kind else {
                    return unsupported("structural operand call has no result place");
                };
                self.completed(declaration.id, continuation)
            }
            CheckedStructuralValueKind::Projection {
                source,
                path,
                type_identity,
            } => {
                // Copied provenance observes its original backing; an owned
                // projection instead transfers one child on the selected edge
                // and disposes of the root's residual complement there. Only
                // the owned transfer requires a structural continuation.
                if let CheckedStructuralValueKind::CopiedStructuralPlace {
                    source: copied, ..
                } = &structural_values(self.checked).nodes.get(source).kind
                {
                    // A projection of a copy observes the selected subtree of
                    // its original backing. It neither transfers the root nor
                    // creates affine residuals. Source replay has rejoined the
                    // copied root independently; retain its complete path when
                    // appending the projection and publish one fresh leaf.
                    let mut argument = self.copied_place_argument(copied)?;
                    argument.path.extend(lower_structural_path(&path)?);
                    return self.copied_leaf_place(
                        &argument,
                        node.qualification_decisions,
                        continuation,
                    );
                }
                // A projection always needs an ownership edge for its kept
                // subtree and residual complement. At a value root there is
                // no enclosing dispatch to supply that edge; create the same
                // result continuation rather than turn the projection into
                // an alias over temporary storage.
                let owned_continuation;
                let is_root = continuation.is_none();
                let continuation = if let Some(continuation) = continuation {
                    continuation
                } else {
                    owned_continuation = self.root_value_continuation(self.values.clone())?;
                    &owned_continuation
                };
                let projected = self.projected_root(source, path, type_identity)?;
                if matches!(
                    structural_values(self.checked).nodes.get(source).kind,
                    CheckedStructuralValueKind::Place(_)
                ) && !self.source_at(projected.root).is_some()
                {
                    return unsupported(
                        "projected selection root is absent from its receipt sources",
                    );
                }
                self.complete_value(projected.root, Some(&projected), continuation)?;
                if is_root {
                    self.record_root_continuation(continuation);
                }
                Ok(continuation.place)
            }
            CheckedStructuralValueKind::ViewElementCopy { reads } => {
                self.view_element_copy(&reads, continuation)
            }
            CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } => {
                let argument = self.copied_place_argument(&source)?;
                self.copied_leaf_place(&argument, node.qualification_decisions, continuation)
            }
            CheckedStructuralValueKind::ZeroedScalarArray {
                element,
                element_count,
            } => {
                // The planner's synthesized zero has no authored operand to
                // replay — emit the verified literal leaf directly.
                let computation = self
                    .checked
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(element);
                let checked_trees::CheckedScalarComputationKind::Value(
                    checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
                ) = &computation.kind
                else {
                    return unsupported("zeroed scalar array element is not a literal zero");
                };
                let scalar_type = crate::emission::scalar_types::terminal_scalar_type(
                    computation.primitive_type,
                )?;
                let value = crate::emission::scalar_types::integer_value(literal, scalar_type)?;
                let zero = emit_scalar_leaf(
                    OperationKind::IntegerConstant { value },
                    scalar_type,
                    self.next_value,
                    self.operations,
                );
                self.values.push(ValueDeclaration {
                    qualifications: Default::default(),
                    id: zero,
                    scalar_type,
                });
                let (place, operation) = allocate_structural_result(
                    self.operations,
                    self.next_place,
                    self.structural_type,
                    self.multiplicity,
                    Vec::new(),
                    OperationKind::EstablishScalarArray {
                        elements: vec![
                            zero;
                            usize::try_from(element_count).map_err(|_| {
                                LoweringError::Unsupported(
                                    "zeroed scalar array leaf count exceeds usize",
                                )
                            })?
                        ],
                    },
                )?;
                self.temporary_places.push(operation_result_declaration(
                    place,
                    operation,
                    self.structural_type,
                ));
                self.completed(place, continuation)
            }
            CheckedStructuralValueKind::ScalarArray { elements } => {
                // Every leaf replays its own retained source computation in
                // authored row-major order, keyed by this array occurrence.
                // Private control flow may remap earlier saved values, so the
                // roster reads its final slots only after the last leaf.
                let source_count = self.values.len();
                for (ordinal, element) in elements.iter().enumerate() {
                    let evaluated = self.scalar(
                        CheckedScalarExpressionRole::StructuralArrayElement {
                            expression: node.expression,
                            element_ordinal: u32::try_from(ordinal).map_err(|_| {
                                LoweringError::Unsupported("scalar array leaf ordinal overflow")
                            })?,
                        },
                        *element,
                        source_count,
                    )?;
                    self.values.push(evaluated);
                }
                let leaves = self.values[source_count..]
                    .iter()
                    .map(|value| value.id)
                    .collect::<Vec<_>>();
                self.values.truncate(source_count);
                let (place, operation) = allocate_structural_result(
                    self.operations,
                    self.next_place,
                    self.structural_type,
                    self.multiplicity,
                    Vec::new(),
                    OperationKind::EstablishScalarArray { elements: leaves },
                )?;
                self.temporary_places.push(operation_result_declaration(
                    place,
                    operation,
                    self.structural_type,
                ));
                self.completed(place, continuation)
            }
            CheckedStructuralValueKind::ByteLiteralView { bytes } => {
                // The literal carries the memberships checking established
                // for this occurrence (`""` into a `&[u8] in Utf8` field);
                // Terminal replays its byte predicates against the octets.
                let qualifications =
                    self.checked_membership_domains(node.qualification_decisions)?;
                let place = crate::emission::structural_byte_sequence_store::emit_literal_view(
                    &bytes,
                    qualifications,
                    self.structural_types,
                    self.temporary_places,
                    self.next_place,
                    self.operations,
                )?;
                self.completed(place, continuation)
            }
            CheckedStructuralValueKind::ZeroedStructural { .. } => {
                let place = self.establish_zeroed()?;

                self.completed(place, continuation)
            }
            CheckedStructuralValueKind::Place(argument) => {
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } = &argument.source {
                    let domains = byte_literals::validate(
                        self.checked, self.type_reference, &argument,
                    )?;
                    let qualifications = domains.into_iter().map(|domain|
                        crate::terminal_identities::lookup_domain_id_for_carrier(
                            self.domain_ids, domain, self.structural_type,
                        )
                    ).collect::<Result<Vec<_>, _>>()?;
                    let (declaration, _) = crate::emission::structural_byte_sequence_store::emit_owned_literal(
                        bytes, self.structural_type, qualifications, self.structural_types,
                        self.temporary_places, self.next_place, self.next_value,
                        &mut self.calls.next_obligation_identity, self.operations,
                    )?;
                    let place = declaration.id;
                    self.temporary_places.push(declaration);
                    if let Some(continuation) = continuation {
                        self.complete_value(place, None, continuation)?;
                    }
                    return Ok(place);
                }
                if let Some(source) = self.copied_parameter(&argument)? {
                    return self.copied_leaf_place(
                        &source,
                        node.qualification_decisions,
                        continuation,
                    );
                }
                if self.sources.is_empty() && continuation.is_none() {
                    // An ordinary constructor consumes this whole source place,
                    // including a sum's active case. Source replay already checks
                    // its exact owned transfer. Only a selected value needs the
                    // per-arm source receipt and continuation handled below.
                    if let Some(place) = self.parameter_source(&argument)? {
                        return Ok(place);
                    }
                    return self
                        .structural_bindings()
                        .owned_argument(&argument)
                        .map(|binding| binding.place);
                }
                if !argument.path.is_empty()
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                {
                    return unsupported("owned selection requires whole owned sources");
                }
                let selected = match &argument.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } => crate::expression_preparation::bindings::structural_local_by_root(
                        &self.evaluation.structural_locals,
                        facts::PlaceRoot::Symbol(*symbol),
                    )
                    .map(|local| local.argument.place)
                    .or(self.parameter_source(&argument)?)
                    .ok_or(LoweringError::Unsupported(
                        "owned selection local place missing",
                    ))?,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        ..
                    } => self
                        .parameter_source(&argument)?
                        .ok_or(LoweringError::Unsupported(
                            "owned selection parameter place missing",
                        ))?,
                    _ => {
                        return unsupported(
                            "owned selection requires an established structural local or parameter",
                        );
                    }
                };
                if !self.source_at(selected).is_some() {
                    return unsupported("owned selection place is absent from its receipt sources");
                }
                let continuation = continuation.ok_or(LoweringError::Unsupported(
                    "direct owned place requires a structural continuation",
                ))?;
                self.complete_value(selected, None, continuation)?;
                Ok(continuation.place)
            }
            CheckedStructuralValueKind::Case(construction) => {
                let source = validation::scalar_case_constructor(
                    &self.checked.typed,
                    construction.expression,
                )
                .ok_or(LoweringError::Unsupported(
                    "structural case lost its authored constructor",
                ))?;
                let place = place_id(allocate_dense(self.next_place)?);
                let slot = crate::scalar_lowering::computations::cases::reserve(
                    self.checked,
                    construction.expression,
                    &source,
                    self.structural_types,
                    place,
                )?;
                let fields = crate::expression_preparation::computation_graph::fields(
                    self.checked,
                    &construction,
                )?
                .to_vec();
                let field_start = self.values.len();
                for (ordinal, field) in fields.iter().enumerate() {
                    let field_ordinal = u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("structural field ordinal overflow")
                    })?;
                    let value = self.scalar(
                        CheckedScalarExpressionRole::StructuralValueField {
                            expression: construction.expression,
                            field_ordinal,
                        },
                        field.value,
                        field_start,
                    )?;
                    self.values.push(value);
                }
                let completed = slot.construction(&self.values[field_start..])?;
                crate::scalar_lowering::computations::cases::emit(
                    &completed,
                    &self.values[field_start..],
                    self.next_value,
                    self.operations,
                    self.calls,
                )?;
                let declaration = crate::scalar_lowering::computations::cases::declarations(
                    &self.operations.operations,
                )
                .find(|declaration| declaration.id == place)
                .ok_or(LoweringError::Unsupported(
                    "structural case result declaration missing",
                ))?;
                self.temporary_places.push(declaration);
                self.values.truncate(field_start);
                self.completed(place, continuation)
            }
            CheckedStructuralValueKind::StructuralCase { .. } => {
                let place = self.structural_case(value)?;

                self.completed(place, continuation)
            }
            CheckedStructuralValueKind::Dispatch { subject, arms } => {
                self.dispatch(node.expression, subject, arms, continuation)
            }
        }
    }

    fn first_candidate(&self) -> Option<usize> {
        self.owners
            .iter()
            .position(|owner| self.source_at(owner.value.place).is_some())
    }

    /// Whole-value assignment uses ordinary owned edge transport. Returning the
    /// source place directly would alias unrestricted records instead of copying
    /// their backing, and would republish an earlier result's declaration.
    /// The existing block binder copies unrestricted payloads and transfers
    /// owned custody; source replay has already checked the exact operand.
    fn materialize_place(
        &mut self,
        source: PlaceId,
        expected_claims: &[terminal_psi::StructuralResultClaimBinding],
    ) -> Result<PlaceId, LoweringError> {
        // The checked move receipt and the already emitted source must name
        // the same complete claim set before binding its successor place.
        let mut bindings = self
            .operations
            .structural_values
            .iter()
            .filter(|(_, binding)| binding.place() == source);
        let binding = bindings.next().map(|(_, binding)| binding);
        if bindings.next().is_some() {
            return unsupported("owned move source has duplicate completed bindings");
        }
        let source_claims = if let Some(binding) = binding {
            binding.claims().to_vec()
        } else if let Some(result) = self
            .operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == source)
        {
            result.claims.clone()
        } else {
            if !self
                .evaluation
                .structural_root_parameters()
                .any(|parameter| parameter.place == source)
            {
                return unsupported("owned move has no source custody");
            }
            // Ingress identities have no operation result, including homes
            // transported through already emitted private blocks. Replay joined
            // the exact Transfer/Establish pair, and ClaimBindings resolved
            // those identities without minting at this move. The receiver
            // independently derives the live set from the actual incoming edge.
            expected_claims.to_vec()
        };
        let block = block_id(allocate_dense(self.next_block)?);
        let place = place_id(allocate_dense(self.next_place)?);
        if self.multiplicity == StructuralMultiplicity::Affine {
            // The return frontier orders operation results before block owners.
            // Transport surviving affine locals together so a moved result and
            // its older survivors retain their declaration-ordered cleanup.
            self.owners = prepare_owners(
                self.checked,
                self.machine,
                self.state,
                self.statement,
                self.structural_types,
                self.evaluation,
                self.operations,
            )?;
        }
        let mut remaining_owners = self
            .owners
            .iter()
            .filter(|owner| owner.value.place != source)
            .cloned()
            .collect::<Vec<_>>();
        let mut pass_through = Vec::new();
        let mut structural_parameters = Vec::new();
        for owner in &mut remaining_owners {
            let target = place_id(allocate_dense(self.next_place)?);
            let position = u32::try_from(structural_parameters.len()).map_err(|_| {
                LoweringError::Unsupported("owned move parameter count exceeds u32")
            })?;
            self.temporary_places
                .push(block_parameter_declaration(target, block, position));
            structural_parameters.push(structural_parameter_declaration(
                target,
                position,
                owner.value.structural_type,
                owner.value.multiplicity,
                StructuralAccess::Owned,
            ));
            pass_through.push((owner.value.place, target));
            owner.value.place = target;
        }
        let position = u32::try_from(structural_parameters.len())
            .map_err(|_| LoweringError::Unsupported("owned move parameter count exceeds u32"))?;
        structural_parameters.push(structural_parameter_declaration(
            place,
            position,
            self.structural_type,
            self.multiplicity,
            StructuralAccess::Owned,
        ));
        let parameters = fresh_value_declarations(self.next_value, self.values)?;
        self.temporary_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::BlockParameter { block, position },
        });
        let continuation = ValueContinuation {
            block,
            parameters,
            structural_parameters,
            place,
            remaining_owners,
            pass_through,
            residuals: Vec::new(),
        };
        self.complete_value(source, None, &continuation)?;
        self.arrival_claims.push((place, source_claims));
        self.start(block);
        if self.multiplicity != StructuralMultiplicity::Unrestricted {
            self.evaluation.structural_locals.retain(
                |crate::expression_preparation::bindings::StructuralLocalBinding {
                     argument,
                     ..
                 }| argument.place != source,
            );
        }
        if self.multiplicity == StructuralMultiplicity::Affine {
            for crate::expression_preparation::bindings::StructuralLocalBinding {
                argument, ..
            } in &mut self.evaluation.structural_locals
            {
                if let Some((_, target)) = continuation
                    .pass_through
                    .iter()
                    .find(|(source, _)| *source == argument.place)
                {
                    argument.place = *target;
                }
            }
            self.evaluation.structural_value_owners = continuation.remaining_owners;
            self.evaluation
                .selection_cleanups
                .push(argument_evaluation::SelectionCleanup {
                    selection: arena::Handle::invalid(),
                    selected: place,
                    sources: Vec::new(),
                    remaining: Vec::new(),
                    retired_reference_paths: Vec::new(),
                    pass_through: continuation.pass_through,
                    next_operation: self.operations.next_identity,
                });
        }
        *self.values = continuation.parameters.clone();
        self.evaluation.parameters = continuation.parameters;
        self.evaluation.block_structural_parameters = continuation.structural_parameters;
        Ok(place)
    }

    /// A `let view: &T = &place` establishment carries the same borrowed
    /// custody a selection arm does, but there is no dispatch to join through:
    /// the result still binds at a fresh block parameter so the sole incoming
    /// edge can present the exact borrowed referent place. No owner moves and
    /// no residual dies on that edge -- the referent's owner stays live under
    /// the shared loan the joined parameter records, exactly as it does for
    /// every arm of a borrowed selection.
    fn direct_borrow(&mut self, argument: StructuralArgument) -> Result<PlaceId, LoweringError> {
        let join = block_id(allocate_dense(self.next_block)?);
        let place = place_id(allocate_dense(self.next_place)?);
        let position = 0u32;
        self.temporary_places
            .push(block_parameter_declaration(place, join, position));
        let structural_parameters = vec![structural_parameter_declaration(
            place,
            position,
            self.structural_type,
            self.multiplicity,
            self.result_access,
        )];
        let parameters = fresh_value_declarations(self.next_value, self.values)?;
        let continuation = ValueContinuation {
            block: join,
            parameters,
            structural_parameters,
            place,
            remaining_owners: Vec::new(),
            pass_through: Vec::new(),
            residuals: Vec::new(),
        };
        self.complete_loan(argument, &continuation, LoanAccess::Shared)?;
        self.start(join);
        *self.values = continuation.parameters.clone();
        self.evaluation.parameters = continuation.parameters;
        self.evaluation.block_structural_parameters = continuation.structural_parameters;
        Ok(place)
    }

    /// Copy an `Unrestricted` leaf out of a live root into the continuation's
    /// owned place with one `StructuralLeafCopy`: the source subtree stays
    /// fully intact, so shared loans admit the observation where a move
    /// would vacate borrowed storage. Only the op-join path is needed — no
    /// case fan-out, no residual bookkeeping.
    /// Establish a fresh owned record from one view element's scalar fields.
    /// Each retained read is an ordinary element read through the view at the
    /// copy's selector, so each carries the view's own bound; the record then
    /// commits the leaves in declaration order. A field that is bounded,
    /// erased or structural is refused: a plain read carries none of the
    /// evidence establishing it would owe.
    pub(super) fn view_element_copy(
        &mut self,
        reads: &[checked_trees::CheckedScalarExpression],
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let shape =
            terminal_psi::find_structural_type(&self.structural_types[..], self.structural_type)
                .ok_or(LoweringError::Unsupported(
                    "view element copy structural type missing",
                ))?;
        let Some(fields) = shape.shape.record_fields() else {
            return unsupported("view element copy establishes a nonrecord");
        };
        if fields.len() != reads.len() {
            return unsupported("view element copy does not read every field once");
        }
        let bindings = self
            .evaluation
            .scalar_bindings
            .clone()
            .with_view_locals(&self.evaluation.view_locals);
        let mut initialized = Vec::with_capacity(fields.len());
        for (field, read) in fields.iter().zip(reads) {
            let checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
                element_path,
                ..
            } = read
            else {
                return unsupported("view element copy field is not an element read");
            };
            if !matches!(
                field.field_type,
                terminal_psi::StructuralFieldType::Scalar(_)
                    | terminal_psi::StructuralFieldType::IeeeFloat(_)
            ) {
                return unsupported("view element copy reads only plain scalar fields");
            }
            if field.relevance.is_erased()
                || element_path.as_slice()
                    != [checked_trees::CheckedScalarReadProjection::Static(
                        checked_trees::CheckedStructuralPredicatePathSegment::Field(
                            field.identity.clone(),
                        ),
                    )]
            {
                return unsupported("view element copy reordered or renamed a field");
            }
            let lowered = bindings.expression(&self.checked.facts.proven_scalar_indexes, read)?;
            let value = crate::emission::operation_emission::expressions::emit_direct_expression(
                &lowered,
                self.values,
                self.next_value,
                self.operations,
            );
            initialized.push(terminal_psi::RecordFieldInitializer {
                field: field.id,
                value: terminal_psi::RecordFieldValue::Scalar {
                    value,
                    range_obligation: None,
                },
            });
        }
        let place = place_id(allocate_dense(self.next_place)?);
        self.temporary_places.push(super::record::emit_completed(
            place,
            self.structural_type,
            self.multiplicity,
            initialized,
            &fields,
            self.operations,
        )?);
        let Some(continuation) = continuation else {
            return Ok(place);
        };
        self.complete_value(place, None, continuation)?;
        Ok(continuation.place)
    }

    fn copied_leaf_place(
        &mut self,
        argument: &StructuralArgument,
        qualification_decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let path = self.copied_place_path(argument)?;
        // Ordinary records and bytes retain the same occurrence-relative
        // source roster. Receiving annotations never manufacture membership.
        let source_contract = self.completed_memberships(argument.place);
        let (mut qualifications, projected_qualifications) = source_contract
            .map(|(whole, projected)| {
                terminal_psi::StructuralPathQualification::select(whole, projected, &path)
            })
            .unwrap_or_default();
        // The checker proved these exact memberships at this operand's
        // evaluation point, before a later constructor child may mutate the
        // source home. This is decision transport, not a second proof replay.
        let decisions = self
            .checked
            .facts
            .qualifications
            .value_formations
            .span(qualification_decisions)
            .ok_or(LoweringError::Unsupported(
                "copied membership decisions are stale",
            ))?;
        for decision in decisions {
            qualifications.push(crate::terminal_identities::lookup_domain_id_for_carrier(
                self.domain_ids,
                decision.semantic_domain,
                self.structural_type,
            )?);
        }
        qualifications.sort();
        qualifications.dedup();
        let place = place_id(allocate_dense(self.next_place)?);
        let operation = self.operations.allocate();
        self.operations.push(terminal_psi::Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: self.structural_type,
                multiplicity: self.multiplicity,
                qualifications,
                projected_qualifications,
                claims: Vec::new(),
            }),
            kind: OperationKind::StructuralLeafCopy {
                source: argument.place,
                path,
            },
        });
        self.temporary_places.push(operation_result_declaration(
            place,
            operation,
            self.structural_type,
        ));
        let Some(continuation) = continuation else {
            // At the value root the fresh place publishes directly — the copy
            // consumes nothing, so there is no custody edge to join.
            return Ok(place);
        };
        self.complete_value(place, None, continuation)?;
        Ok(continuation.place)
    }

    /// The scalar namespace augmented with every structural observation
    /// this emission can name — the same bindings shape structural reads,
    /// views, and argument resolution share.
    fn structural_bindings(&self) -> ScalarBindings {
        ScalarBindings::new(self.values.iter().map(|value| value.scalar_type))
            .with_structural_parameters(&self.evaluation.structural_parameters)
            .with_structural_locals(&self.evaluation.structural_locals)
            .with_structural_observations(self.structural_types)
    }

    pub(super) fn scalar(
        &mut self,
        role: CheckedScalarExpressionRole,
        value: checked_trees::CheckedScalarComputationHandle,
        source_count: usize,
    ) -> Result<ValueDeclaration, LoweringError> {
        let value = self.evaluation.source_value(
            self.checked,
            self.machine,
            self.state,
            self.statement,
            role,
            &checked_trees::CheckedCallScalarArgument::Computation(value),
            source_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        Ok(value)
    }

    /// Push the `Jump` terminator naming `edge` — every field transfers
    /// from the edge itself and each route supplies its cleanup roster.
    /// Case-driven control flow splits on one Boolean condition: emit the
    /// test, close the current block on it, remember the fallback block's
    /// value tail, and resume at the selected arm.
    pub(super) fn boolean_case_split(
        &mut self,
        kind: OperationKind,
        fallback: &mut Option<(BlockId, Vec<ValueDeclaration>)>,
    ) -> Result<(), LoweringError> {
        let condition = value_id(allocate_dense(self.next_value)?);
        let id = self.operations.allocate();
        self.operations.push(terminal_psi::Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: scalar_result_declaration(condition, ScalarType::Boolean),
            kind,
        });
        let selected = block_id(allocate_dense(self.next_block)?);
        let next = block_id(allocate_dense(self.next_block)?);
        let when_true = self.edge(selected, Vec::new(), Vec::new())?;
        let when_false = self.edge(next, Vec::new(), Vec::new())?;
        self.finish(Terminator::Conditional {
            condition,
            when_true,
            when_false,
        });
        *fallback = Some((next, self.values.clone()));
        self.start(selected);
        Ok(())
    }

    fn finish_jump(
        &mut self,
        edge: SuccessorEdge,
        cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
    ) {
        self.finish(Terminator::Jump {
            edge: edge.edge,
            target: edge.target,
            arguments: edge.arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: edge.structural_arguments,
            cleanup_actions,
        });
    }

    fn edge(
        &mut self,
        target: BlockId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
    ) -> Result<SuccessorEdge, LoweringError> {
        Ok(SuccessorEdge {
            edge: edge_id(allocate_dense(self.next_edge)?),
            target,
            arguments,
            structural_arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        })
    }

    fn finish(&mut self, terminator: Terminator) {
        self.evaluation.blocks.push(Block {
            id: self.evaluation.current,
            parameters: std::mem::take(&mut self.evaluation.parameters),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: std::mem::take(&mut self.evaluation.block_structural_parameters),
            operations: self.operations[self.evaluation.operation_start..].to_vec(),
            terminator,
        });
    }

    fn start(&mut self, block: BlockId) {
        self.evaluation.current = block;
        self.evaluation.operation_start = self.operations.len();
        self.operations.byte_lengths.clear();
        self.operations.field_byte_lengths.clear();
    }

    /// Resume the case-fallback arm's stashed value roster at its entry
    /// block, when one was recorded.
    fn restore_fallback(&mut self, fallback: Option<(BlockId, Vec<ValueDeclaration>)>) {
        if let Some((next, values)) = fallback {
            self.start(next);
            *self.values = values;
        }
    }

    /// Push an evaluated scalar onto the SSA roster, request the declared
    /// field's range-evidence obligation when its type requires one, and
    /// return the terminal initializer value. The verifier reconstructs the
    /// declared range against the final SSA initializer; the identity
    /// requests that proof — it is not an assertion construction is valid.
    /// `positions` records the saved-value remap for private control flow.
    pub(super) fn scalar_field_value(
        &mut self,
        field_type: &terminal_psi::StructuralFieldType,
        evaluated: ValueDeclaration,
        positions: &mut Vec<(usize, usize)>,
        field_index: usize,
    ) -> Result<terminal_psi::RecordFieldValue, LoweringError> {
        // Construction establishes storage before it can be lent or observed.
        // An atomic cell accepts its exact scalar initializer here, without
        // becoming an ordinary scalar field or acquiring event authority.
        // Records, selected cases and omitted-field zeroes share this check.
        if field_type.scalar_type() != Some(evaluated.scalar_type) {
            return unsupported(
                "field initializer requires its exact scalar carrier and range evidence",
            );
        }
        positions.push((field_index, self.values.len()));
        self.values.push(evaluated);
        let range_obligation = if terminal_semantics::scalar_field_requires_evidence(field_type) {
            Some(obligation_id(allocate_dense(
                &mut self.calls.next_obligation_identity,
            )?))
        } else {
            None
        };
        Ok(terminal_psi::RecordFieldValue::Scalar {
            value: evaluated.id,
            range_obligation,
        })
    }

    /// Root selection takes its result from the first candidate owner; every
    /// other owner crosses the same continuation partition used by nested
    /// selections. Dispatch and projection therefore retain identical cleanup
    /// and result-arrival obligations without aliasing temporary storage.
    fn root_value_continuation(
        &mut self,
        values: Vec<ValueDeclaration>,
    ) -> Result<ValueContinuation, LoweringError> {
        let first_candidate = self.first_candidate();
        let remaining_owners = self
            .owners
            .iter()
            .enumerate()
            .filter(|(position, _)| Some(*position) != first_candidate)
            .map(|(_, owner)| owner.clone())
            .collect();
        self.joined_value_continuation(values, remaining_owners)
    }

    /// Mint fresh join identities and partition remaining owners into
    /// pass-through and residual places. The trailing structural parameter
    /// carries the selected result's own access, independently of its siblings.
    fn joined_value_continuation(
        &mut self,
        values: Vec<ValueDeclaration>,
        mut remaining_owners: Vec<argument_evaluation::StructuralValueOwner>,
    ) -> Result<ValueContinuation, LoweringError> {
        let join = block_id(allocate_dense(self.next_block)?);
        let joined_values = fresh_value_declarations(self.next_value, &values)?;
        let mut structural_parameters = Vec::new();
        let (pass_through, residuals) = self.partition_owner_continuation(
            join,
            &mut remaining_owners,
            &mut structural_parameters,
        )?;
        let place = structural_parameters
            .last()
            .ok_or(LoweringError::Unsupported(
                "structural continuation has no result",
            ))?
            .place;
        Ok(ValueContinuation {
            block: join,
            parameters: joined_values,
            structural_parameters,
            place,
            remaining_owners,
            pass_through,
            residuals,
        })
    }

    fn partition_owner_continuation(
        &mut self,
        join: BlockId,
        remaining_owners: &mut Vec<argument_evaluation::StructuralValueOwner>,
        structural_parameters: &mut Vec<StructuralParameterDeclaration>,
    ) -> Result<(Vec<(PlaceId, PlaceId)>, Vec<PlaceId>), LoweringError> {
        let mut pass_through = Vec::new();
        let mut residuals = Vec::new();
        for position in 0..remaining_owners.len() + 1 {
            let (structural_type, multiplicity) = remaining_owners
                .get(position)
                .map_or((self.structural_type, self.multiplicity), |owner| {
                    (owner.value.structural_type, owner.value.multiplicity)
                });
            let is_result_slot = position == remaining_owners.len();
            let owner = remaining_owners.get_mut(position);
            let position = u32::try_from(position).map_err(|_| {
                LoweringError::Unsupported("owned selection parameter count exceeds u32")
            })?;
            let place = place_id(allocate_dense(self.next_place)?);
            if let Some(owner) = owner {
                if self.source_at(owner.value.place).is_some() {
                    owner.symbol = symbols::SymbolHandle::invalid();
                    residuals.push(place);
                } else {
                    pass_through.push((owner.value.place, place));
                }
                owner.value.place = place;
            }
            self.temporary_places
                .push(block_parameter_declaration(place, join, position));
            structural_parameters.push(structural_parameter_declaration(
                place,
                position,
                structural_type,
                multiplicity,
                if is_result_slot {
                    self.result_access
                } else {
                    StructuralAccess::Owned
                },
            ));
        }
        Ok((pass_through, residuals))
    }

    /// Bookkeep a root selection continuation: install the join block's
    /// value and structural parameters on the evaluation state, record the
    /// selection cleanup for the live sources, then repoint structural
    /// locals through the continuation's pass-through mapping.
    fn record_root_continuation(&mut self, continuation: &ValueContinuation) {
        self.start(continuation.block);
        *self.values = continuation.parameters.clone();
        self.evaluation.parameters = continuation.parameters.clone();
        self.evaluation.block_structural_parameters = continuation.structural_parameters.clone();
        if self.sources.is_empty() {
            return;
        }
        self.evaluation
            .selection_cleanups
            .push(argument_evaluation::SelectionCleanup {
                selection: self
                    .checked
                    .facts
                    .flow
                    .ownership
                    .owned_selection_at(self.state, self.statement)
                    .map(|(handle, _)| handle)
                    .unwrap_or_default(),
                selected: continuation.place,
                sources: self
                    .owners
                    .iter()
                    .rev()
                    .filter(|owner| self.source_at(owner.value.place).is_some())
                    .map(|owner| owner.value.place)
                    .collect(),
                remaining: self
                    .owners
                    .iter()
                    .filter(|owner| self.source_at(owner.value.place).is_some())
                    .skip(1)
                    .zip(&continuation.residuals)
                    .map(|(owner, receiving)| (owner.value.place, *receiving))
                    .collect(),
                retired_reference_paths: Vec::new(),
                pass_through: continuation.pass_through.clone(),
                next_operation: self.operations.next_identity,
            });
        self.evaluation.structural_value_owners = continuation.remaining_owners.clone();
        self.evaluation.structural_locals.retain(
            |crate::expression_preparation::bindings::StructuralLocalBinding {
                 argument, ..
             }| {
                !self
                    .sources
                    .iter()
                    .any(|source| source.place == argument.place)
            },
        );
        for crate::expression_preparation::bindings::StructuralLocalBinding { argument, .. } in
            &mut self.evaluation.structural_locals
        {
            if let Some((_, target)) = continuation
                .pass_through
                .iter()
                .find(|(source, _)| *source == argument.place)
            {
                argument.place = *target;
            }
        }
    }
}

/// The claims a completed place holds where it was established: on its
/// producing operation's result, or on the binding a control join published
/// for a value that completed there.
fn completed_place_claims(
    operations: &OperationBuffer,
    place: PlaceId,
) -> impl Iterator<Item = terminal_psi::StructuralResultClaimBinding> + '_ {
    operations
        .structural_values
        .iter()
        .map(|(_, root)| (root.place(), root.claims()))
        .chain(
            operations
                .operations
                .iter()
                .filter_map(|operation| operation.result.structural())
                .map(|produced| (produced.place, produced.claims.as_slice())),
        )
        .filter(move |(candidate, _)| *candidate == place)
        .flat_map(|(_, claims)| claims)
        .cloned()
}
