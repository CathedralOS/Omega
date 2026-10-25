use super::super::super::{
    BlockId, ScalarType, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
    StructuralTypeDeclaration, SuccessorEdge, ValueId, block_id,
};
use super::super::{
    Block, CheckedScalarExpressionRole, LoweringError, Multiplicity, OperationKind,
    OperationResult, PlaceId, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, Terminator, ValueDeclaration,
    allocate_dense, argument_evaluation, edge_id, lookup_type_id, obligation_id,
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

mod completed_projection;
mod completed_storage;
mod completions;
mod content_transformations;
mod copied_place;
mod dispatch;
mod element_views;
mod parameters;
mod quotient_calls;
mod reference;
mod residual_complement;

/// Checked value production is independent of a source local's publication.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueDestination {
    /// Establish the receiving declaration's value, including owned transfer.
    Binding,
    /// Retain an evaluated operand; its eventual consumer owns any transfer.
    Operand,
}

pub(crate) struct ValueProduction<'source> {
    pub(crate) value: CheckedStructuralValueHandle,
    pub(crate) owner: facts::PlaceRoot,
    pub(crate) destination: ValueDestination,
    /// A scalar consumer reserves its receiving home before graph emission.
    pub(crate) statement: u32,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) multiplicity: Multiplicity,
    pub(crate) binding_type_reference: checked_trees::types::TypeReferenceHandle,
    pub(crate) access: StructuralAccess,
    pub(crate) selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
    pub(crate) calls: &'source [checked_trees::CheckedStructuralValueCall],
}

/// A producer returns custody and optional fresh storage. Its consumer alone
/// publishes its local ordinal or lends the complete binding to its consumer.
pub(crate) struct CompletedValue {
    pub(crate) argument: StructuralArgument,
    pub(crate) binding: crate::emission::operation_emission::buffer::StructuralBinding,
    pub(crate) declaration: Option<StructuralPlaceDeclaration>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit<'source>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    production: ValueProduction<'source>,
    environment: &mut super::super::operation_frame::OperationEnvironment<'source>,
    calls: &mut CallEmissionContext<'_>,
    evaluation: &mut argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<CompletedValue, LoweringError> {
    let checked = environment.checked;
    let machine = environment.machine;
    let state = environment.state;
    let structural_types = environment.structural_types;
    let type_ids = environment.type_ids;
    let domain_ids = environment.callees.domain_ids;
    let domain_declarations = environment.callees.domain_declarations;
    let value = &production.value;
    let operand_calls = production.calls;
    let structural_type = production.structural_type;
    let multiplicity = crate::terminal_identities::structural_multiplicity(production.multiplicity);
    let occurrence = checked.facts.values.structural_values.nodes.get(*value);
    if occurrence.machine != machine
        || occurrence.state != state
        || occurrence.statement_index != production.statement
    {
        return unsupported("structural producer has a different checked source coordinate");
    }
    let prepared_claims = crate::emission::claim_bindings::prepare_establishment_claims(
        identities,
        checked,
        machine,
        state,
        production.statement as usize,
        production.owner,
        occurrence.type_reference,
        structural_type,
        structural_types,
    )?;
    let result_claims = crate::emission::claim_bindings::emit_establishment_claims(
        &prepared_claims,
        &mut calls.claims,
    )?;
    let authored_state = crate::declarations::state_declaration(checked, state)?.1;
    let sources = production
        .selection
        .is_valid()
        .then(|| {
            checked
                .facts
                .flow
                .ownership
                .owned_selections
                .get(production.selection)
        })
        .map(|receipt| {
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
                // Only residual owners need this physical cleanup roster.
                // Whole moves still resolve through the complete receipt.
                .filter(|source| source.retains_storage)
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
                    if declaration.access != StructuralAccess::Owned {
                        return Err(LoweringError::Unsupported(
                            "owned selection parameter source lacks owned custody",
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
    // A referent alias observes storage through the selection's address join.
    // A materialized descriptor instead transfers its own affine lifetime
    // resource, even when the source binding has shared referent access.
    // Statement construction records that carrier multiplicity before
    // lowering; local access alone would lend the descriptor itself while
    // every incoming arm transfers it owned. Keep those two custody domains
    // distinct without choosing a route from the initializer's source shape.
    let borrowed_alias = match &structural_values(checked).nodes.get(*value).kind {
        CheckedStructuralValueKind::Reference {
            source,
            source_value,
            ..
        } if !source_value.is_valid()
            && lookup_type_id(type_ids, &source.type_identity)? == structural_type =>
        {
            Some(
                crate::expression_preparation::bindings::structural_paths::terminal_access(
                    source.access,
                ),
            )
        }
        _ => None,
    };
    // Canonical view occurrences retain their direct descriptor role. Stored
    // descriptor values instead own the arriving words, with their own copy or
    // affine lifetime. Their backing remains borrowed in either representation.
    let view_loan = structural_types
        .iter()
        .find_map(|declaration| {
            if declaration.id != structural_type {
                return None;
            }
            Some(borrowed_view_join_access(
                &declaration.shape,
                production.multiplicity,
            ))
        })
        .flatten();
    let stored_descriptor_value = structural_types.iter().any(|declaration| {
        declaration.id == structural_type
            && terminal_semantics::stored_reference_access(&declaration.shape).is_some()
    });
    let result_access = if let Some(access) = borrowed_alias {
        // Equal source/destination carriers transport the existing loan. They
        // do not acquire owned custody merely because mutable access is affine.
        access
    } else if stored_descriptor_value {
        // A materialized descriptor crosses by value. Shared copies and
        // exclusive transfers keep their own receiving home; borrowing that
        // home would outlive the source's completed normal-death cut.
        StructuralAccess::Owned
    } else if let Some(access) = view_loan {
        access
    } else if production.access == StructuralAccess::SharedBorrow
        && production.multiplicity == Multiplicity::Unrestricted
    {
        StructuralAccess::SharedBorrow
    } else {
        StructuralAccess::Owned
    };
    if result_access == StructuralAccess::SharedBorrow
        && !sources.is_empty()
        && view_loan != Some(StructuralAccess::SharedBorrow)
    {
        return unsupported("borrowed selection cannot carry owned residual custody");
    }
    let owners = if sources.is_empty() {
        Vec::new()
    } else {
        let owners = prepare_owners(
            checked,
            machine,
            state,
            production.statement,
            production.selection,
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
            return Err(LoweringError::MissingSelectionSources {
                machine: checked.symbols.display_path(machine, "::"),
                state: checked.symbols.display_path(state, "::"),
                statement_index: production.statement,
                source_places: sources.iter().map(|source| source.place).collect(),
                live_places: owners.iter().map(|owner| owner.value.place).collect(),
            });
        }
        owners
    };
    let mut emission = Emission {
        produced_root: *value,
        checked,
        machine,
        state,
        statement: production.statement,
        selection: production.selection,
        structural_type,
        multiplicity,
        structural_types,
        type_ids,
        domain_ids,
        domain_declarations,
        binding_type_reference: production.binding_type_reference,
        type_reference: occurrence.type_reference,
        environment,
        calls,
        operand_calls,
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
        selection_route: Vec::new(),
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
    // Copyable children require their own snapshots; an alias would reread
    // or consume their parent at a later use. Only non-copy children join here.
    // A statement that carries an
    // owned-selection receipt keeps the selection route, because that receipt
    // retains its sources through the selected edge and an alias has no
    // edge to carry it. A receiving binding over a computed source transfers
    // its subtree; retaining an operand instead preserves even a computed
    // owner's home until the actual consumer and contextual completion.
    // Whole-value views retain the selected storage geometry. Collect their
    // checked memberships separately: the exact source-claim join below must
    // precede any value formation that publishes those memberships.
    let mut storage_value = *value;
    let mut storage_memberships = Vec::new();
    loop {
        let node = structural_values(checked).nodes.get(storage_value);
        let CheckedStructuralValueKind::Projection { source, path, .. } = &node.kind else {
            break;
        };
        if !path.is_empty() {
            break;
        }
        storage_memberships.push((node.qualification_decisions, node.expression));
        storage_value = *source;
    }
    // Operand retention is not a receiving declaration. An already owned
    // place keeps its physical home until the actual consumer transfers it;
    // silently moving it here would strand its checked cleanup on the old
    // home. Unrestricted reads still take their ordinary snapshot producer.
    let storage_projection = match structural_values(checked)
        .nodes
        .get(storage_value)
        .kind
        .clone()
    {
        CheckedStructuralValueKind::Projection {
            source,
            logical_root_type_identity,
            path,
            type_identity,
            projection_evaluations,
            copies_selected_value: false,
            ..
        } if !path.is_empty()
            && (production.destination == ValueDestination::Operand
                || matches!(
                    structural_values(checked).nodes.get(source).kind,
                    CheckedStructuralValueKind::Place(_)
                )) =>
        {
            Some((
                source,
                path,
                projection_evaluations,
                type_identity,
                logical_root_type_identity,
            ))
        }
        CheckedStructuralValueKind::Place(argument)
            if production.destination == ValueDestination::Operand
                && checked.type_multiplicity(
                    structural_values(checked)
                        .nodes
                        .get(storage_value)
                        .type_reference,
                ) != Multiplicity::Unrestricted
                && multiplicity != StructuralMultiplicity::Unrestricted =>
        {
            Some((
                storage_value,
                Vec::new(),
                HandleSpan::empty(),
                argument.type_identity,
                Default::default(),
            ))
        }
        _ => None,
    };
    let projected_storage = if emission.sources.is_empty()
        && let Some((
            source,
            path,
            projection_evaluations,
            type_identity,
            logical_root_type_identity,
        )) = storage_projection
    {
        let projected =
            emission.projection_source(identities, source, path, logical_root_type_identity)?;
        let alias = emission.complete_projection_source(
            identities,
            structural_values(checked)
                .nodes
                .get(storage_value)
                .expression,
            &projected,
            projection_evaluations,
            type_identity,
            StructuralAccess::Owned,
        )?;
        let alias_path = alias.path;
        // The leaf's claim is published where its root was established: the
        // alias names that exact place and path, so the consuming operation
        // resolves the binding already on the root's result. An alias has no
        // result of its own, so a claim this statement minted fresh would
        // have nowhere to land; require the root to carry each one.
        // The root's claim roster is on its producing operation's result,
        // or -- when its value completed at a control join -- on the
        // completed binding the join published for it.
        let published_claims = emission
            .evaluation
            .current_claims
            .source(&StructuralArgument {
                place: projected.root,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            })?
            .ok_or(LoweringError::Unsupported(
                "projected alias has no current root custody namespace",
            ))?;
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
        Some(alias)
    } else {
        None
    };
    let place = if let Some(alias) = projected_storage {
        let first_new_place = *emission.environment.next_place;
        let mut argument = StructuralArgument {
            place: alias.place(),
            path: alias.path().to_vec(),
            access: StructuralAccess::Owned,
        };
        let projected = structural_values(checked).nodes.get(storage_value);
        storage_memberships.push((projected.qualification_decisions, projected.expression));
        for (decisions, expression) in storage_memberships.into_iter().rev() {
            argument = emission.retain_checked_memberships(
                identities,
                argument,
                decisions,
                expression,
                first_new_place,
            )?;
        }
        if argument.place == alias.place() {
            return Ok(CompletedValue {
                argument,
                binding: alias,
                declaration: None,
            });
        }
        // The qualification transferred the selected child into its own home.
        // Its original parent still owns every unselected sibling.
        argument.place
    } else if emission.sources.is_empty()
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
            }
    {
        emission.direct_element_view(
            identities,
            source,
            region,
            structural_values(checked).nodes.get(*value).expression,
            loan,
        )?
    } else {
        let first_new_place = *emission.environment.next_place;
        let source = emission.value(identities, *value, None)?;
        // Image-backed descriptors are established storage, not operation
        // results. Copy the selected unrestricted carrier without copying
        // or taking ownership of its bytes, retaining the operand coordinate.
        let source = if emission
            .environment
            .private_places
            .iter()
            .any(|declaration| {
                declaration.id == source.place
                    && matches!(
                        declaration.kind,
                        StructuralPlaceKind::ByteSequenceLiteral { .. }
                    )
            }) {
            let place = emission.copied_leaf_place(
                &StructuralArgument {
                    access: StructuralAccess::SharedBorrow,
                    ..source
                },
                structural_values(checked)
                    .nodes
                    .get(*value)
                    .qualification_decisions,
            )?;
            StructuralArgument::whole(place, StructuralAccess::Owned)
        } else {
            source
        };
        // Binding publication consumes the same checked occurrence as a
        // constructor operand. Retain its established facts before forming
        // the receiving declaration, without changing the source contract.
        let node = structural_values(checked).nodes.get(*value);
        let source = emission.retain_checked_memberships(
            identities,
            source,
            node.qualification_decisions,
            node.expression,
            first_new_place,
        )?;
        // Existing storage needs an arrival owned by this binding; fresh
        // producers already published one. Use the same allocation frontier
        // as constructor capture, independent of the source expression shape.
        let binding = emission.value_binding_parameter(identities, &source, source.place, 0)?;
        let (source_whole, source_projected) =
            emission
                .completed_memberships(source.place)
                .ok_or(LoweringError::Unsupported(
                    "binding source has no exact membership declaration",
                ))?;
        let (source_whole, source_projected) = terminal_psi::StructuralPathQualification::select(
            source_whole,
            source_projected,
            &source.path,
        );
        let distinct_contract = binding.qualifications != source_whole
            || binding.projected_qualifications != source_projected;
        if source.place.get() < first_new_place || distinct_contract {
            emission.materialize_place(identities, source, *value, first_new_place)?
        } else {
            source.place
        }
    };
    // Private arm producers are not authored result ordinals. Publish exactly
    // one completed place for this binding, whether a direct producer or join.
    // A call-rooted producer's place was catalogued by the call emitter as its
    // own call result, so it claims no temporary row and yields no declaration
    // for the caller to republish.
    let declaration = emission
        .environment
        .private_places
        .iter()
        .position(|item| item.id == place)
        .map(|position| emission.environment.private_places.remove(position));
    let published_declaration = emission.published_places.iter().find(|row| row.id == place);
    let completed_declaration =
        declaration
            .as_ref()
            .or(published_declaration)
            .ok_or(LoweringError::Unsupported(
                "structural value place was not declared",
            ))?;
    let arrival_claims = emission
        .operations
        .arrival_claims
        .iter()
        .find(|(arrival, _)| *arrival == place)
        .map(|(_, claims)| claims.clone())
        .unwrap_or_default();
    let completed = completed_storage::completed_result(
        place,
        completed_declaration,
        emission.operations,
        emission.evaluation.structural_root_parameters(),
        &arrival_claims,
    )?;
    if emission.result_access == StructuralAccess::Owned && declaration.is_some() {
        emission.operations.register_structural_materialization(
            place,
            state,
            production.statement,
        )?;
    }
    Ok(CompletedValue {
        argument: StructuralArgument::whole(place, emission.result_access),
        binding: completed.into(),
        declaration,
    })
}

/// Loan access a borrowed-view join may publish for this carrier.
///
/// Only canonical view occurrences keep access on the receiving edge. A
/// materialized pointer or view descriptor instead crosses as its owned value
/// (a shared copy or exclusive transfer), independently of backing ownership.
/// A true referent alias is classified separately before this query.
fn borrowed_view_join_access(
    shape: &StructuralTypeShape,
    multiplicity: Multiplicity,
) -> Option<StructuralAccess> {
    match shape {
        StructuralTypeShape::ElementView { access: None, .. } => {
            (multiplicity == Multiplicity::Unrestricted).then_some(StructuralAccess::SharedBorrow)
        }
        StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView { access: None }
            | terminal_psi::ByteSequenceCarrier::BoundedView { access: None, .. },
        ) => match multiplicity {
            Multiplicity::Unrestricted => Some(StructuralAccess::SharedBorrow),
            Multiplicity::Affine => Some(StructuralAccess::MutableBorrow),
            Multiplicity::Linear => None,
        },
        _ => None,
    }
}

/// Allocate a fresh dense id for each carried value declaration.
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
    selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
    structural_types: &[StructuralTypeDeclaration],
    evaluation: &argument_evaluation::Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<argument_evaluation::StructuralValueOwner>, LoweringError> {
    // A checked selection keeps its exact candidate and survivor inventory.
    // Ordinary single-value materialization instead updates Evaluation's live
    // frontier at the consuming edge without reconstructing unrelated owners.
    let source_parameters =
        checked.state_parameters(crate::declarations::state_declaration(checked, state)?.1);
    let available = |symbol: symbols::SymbolHandle,
                     formation: u32|
     -> Result<bool, LoweringError> {
        if !symbol.is_valid() {
            return Ok(true);
        }
        // A completed descriptor release can retire an actual source home
        // without an Owned permission transfer. Its emitted-prefix binding
        // correspondence remains authoritative after removing that local.
        if evaluation
            .retired_structural_bindings
            .iter()
            .any(|binding| binding.symbol == symbol)
        {
            return Ok(false);
        }
        // Parameter slots are ordering coordinates, not statement formations:
        // state-entry owners precede every statement, including statement zero.
        let entry = source_parameters
            .iter()
            .any(|parameter| parameter.symbol == symbol);
        for (_, receipt) in checked.facts.flow.ownership.owned_selections.iter() {
            if receipt.machine == machine
                && receipt.state == state
                && matches!(
                    receipt.evaluation,
                    checked_trees::FlowPermissionEvaluation::Statement
                        | checked_trees::FlowPermissionEvaluation::TransitionGuard
                )
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
        // A historical extraction is not a permanent hole. Fold the checked
        // storage events in evaluation order: establishment or exclusive-call
        // restoration reseats its subtree, while transfer/disposal removes it.
        // Claim identities are resolved separately at the consuming operand.
        let mut vacancies = Vec::new();
        for (_, event) in checked.facts.flow.ownership.permissions.iter() {
            if event.machine_symbol != machine
                || event.state_symbol != state
                || event.root != facts::PlaceRoot::Symbol(symbol)
                || event.access != language_semantics::PermissionAccess::Owned
            {
                continue;
            }
            // A target completes this state's edge. Its transfers cannot be
            // history for a sibling target later in authored statement order.
            // Only ordinary statements and observed fallthrough guards precede
            // this value on the same execution path.
            if !matches!(
                event.evaluation,
                checked_trees::FlowPermissionEvaluation::Statement
                    | checked_trees::FlowPermissionEvaluation::TransitionGuard
            ) {
                continue;
            }
            let ordinal = match event.source {
                language_semantics::PermissionEventSource::Statement { statement_index }
                | language_semantics::PermissionEventSource::Call {
                    statement_index, ..
                } => statement_index,
                _ => continue,
            };
            if (!entry && ordinal <= formation as usize) || ordinal >= statement as usize {
                continue;
            }
            let path = checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments);
            match event.kind {
                language_semantics::PermissionEventKind::Establish
                | language_semantics::PermissionEventKind::Restore => {
                    vacancies.retain(|vacancy: &Vec<_>| !vacancy.starts_with(path));
                }
                language_semantics::PermissionEventKind::Transfer
                | language_semantics::PermissionEventKind::Consume
                | language_semantics::PermissionEventKind::AffineDrop => {
                    if !vacancies.iter().any(|vacancy| path.starts_with(vacancy)) {
                        vacancies.retain(|vacancy| !vacancy.starts_with(path));
                        vacancies.push(path.to_vec());
                    }
                }
            }
        }
        if vacancies.iter().any(Vec::is_empty) {
            return Ok(false);
        }
        if !vacancies.is_empty() {
            if residual_complement::is_empty(
                checked,
                state,
                statement as usize,
                symbol,
                &vacancies,
            )? {
                return Ok(false);
            }
            return unsupported(
                "selection pass-through of partially moved roots requires retained residual frontier correspondence",
            );
        }
        Ok(true)
    };
    let mut owners: Vec<argument_evaluation::StructuralValueOwner> = Vec::new();
    for owner in &evaluation.structural_value_owners {
        // Anonymous completed operands have no authored symbol to query.
        // Actual whole-owned consumption retires their unique emitted home,
        // just as it does named homes; borrowing or a child transfer does not.
        if available(owner.symbol, owner.statement)? {
            // Survivor identity is the emitted home. An anonymous call row and
            // the local that names that home are one owner; transferring both
            // consumes the home on the first argument and refuses the second.
            if let Some(existing) = owners
                .iter_mut()
                .find(|existing| existing.value.place == owner.value.place)
            {
                if !existing.symbol.is_valid() && owner.symbol.is_valid() {
                    existing.symbol = owner.symbol;
                    existing.statement = owner.statement;
                }
                continue;
            }
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
        let local = match checked.facts.state_locals.named(state, symbol) {
            Some(local) => local,
            None => {
                let parameter = source_parameters
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)
                    .ok_or(LoweringError::Unsupported(
                        "selection home has no checked source declaration",
                    ))?;
                // The home catalogue also contains addressable scalar ingress.
                // Checked unrestricted custody needs no affine survivor, even
                // when a mutable parameter has an emitted structural home.
                // Never invent a local identity or discard an affine/linear
                // parameter here; selected owned ingress joins separately below.
                if crate::reference_custody::binding_multiplicity(checked, parameter.type_reference)
                    == Multiplicity::Unrestricted
                {
                    continue;
                }
                return unsupported("selection home requires an owned ingress frontier");
            }
        };
        if !available(symbol, local.statement_index)? {
            continue;
        }
        if let Some(position) = owners
            .iter()
            .position(|owner| owner.symbol == symbol || owner.value.place == argument.place)
        {
            // The named local adopts an anonymous row for this same home.
            // A second valid symbol would be two locals claiming one place.
            if owners[position].value.place == argument.place
                && owners[position].symbol.is_valid()
                && owners[position].symbol != symbol
            {
                return unsupported("affine home is owned by two locals");
            }
            if !owners[position].symbol.is_valid() {
                owners[position].symbol = symbol;
                owners[position].statement = local.statement_index;
            }
            continue;
        }
        if crate::reference_custody::binding_multiplicity(checked, local.declared_type)
            != Multiplicity::Affine
        {
            continue;
        }
        if local.statement_index >= statement
            || !argument.path.is_empty()
            || argument.access != StructuralAccess::Owned
            || !crate::content_classes::stored_value_contents(checked, local.declared_type)
        {
            return unsupported("selection survivor requires whole owned custody");
        }
        let value = operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == argument.place)
            .ok_or(LoweringError::Unsupported(
                "selection survivor requires an exact emitted operation-result frontier",
            ))?;
        if value.multiplicity
            != crate::terminal_identities::structural_multiplicity(
                checked.type_multiplicity(local.declared_type),
            )
            || !structural_types
                .iter()
                .any(|declaration| declaration.id == value.structural_type)
        {
            return unsupported("selection survivor disagrees with its declared owned carrier");
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
    if selection.is_valid() {
        let receipt = checked.facts.flow.ownership.owned_selections.get(selection);
        for (position, parameter) in source_parameters.iter().enumerate() {
            if !checked
                .facts
                .flow
                .ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .iter()
                .any(|source| {
                    source.symbol == parameter.symbol
                        && source.statement_ordinal == position as u32
                        && source.retains_storage
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
            if evaluation
                .retired_structural_homes
                .contains(&declaration.place)
            {
                continue;
            }
            if declaration.access != StructuralAccess::Owned
                || declaration.multiplicity != StructuralMultiplicity::Affine
                || checked.type_multiplicity(parameter.type_reference) != Multiplicity::Affine
                || !crate::content_classes::stored_value_contents(checked, parameter.type_reference)
            {
                return unsupported("selection parameter source lacks exact owned custody");
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
        if crate::reference_custody::binding_multiplicity(checked, local.declared_type)
            == Multiplicity::Affine
            && available(local.symbol, local.statement_index)?
            && !owners.iter().any(|owner| owner.symbol == local.symbol)
        {
            return Err(LoweringError::MissingSelectionOwner {
                machine: checked.symbols.display_path(machine, "::"),
                state: checked.symbols.display_path(state, "::"),
                statement_index: statement,
                local: checked.symbols.display_path(local.symbol, "::"),
            });
        }
    }
    Ok(owners)
}

pub(super) struct Emission<'a, 'b, 'calls> {
    produced_root: CheckedStructuralValueHandle,
    pub(super) checked: &'a CheckedTrees,
    pub(super) machine: symbols::SymbolHandle,
    pub(super) state: symbols::SymbolHandle,
    pub(super) statement: u32,
    selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
    pub(super) structural_type: StructuralTypeId,
    pub(super) multiplicity: StructuralMultiplicity,
    pub(super) structural_types: &'a [StructuralTypeDeclaration],
    pub(super) type_ids: &'a [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    pub(super) domain_ids: &'a [crate::terminal_identities::StructuralDomainBinding],
    pub(super) domain_declarations: &'a [terminal_psi::StructuralDomainDeclaration],
    type_reference: checked_trees::types::TypeReferenceHandle,
    binding_type_reference: checked_trees::types::TypeReferenceHandle,
    pub(super) environment: &'b mut super::super::operation_frame::OperationEnvironment<'a>,
    pub(super) calls: &'b mut CallEmissionContext<'calls>,
    pub(super) operand_calls: &'a [checked_trees::CheckedStructuralValueCall],
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
    selection_route: Vec<crate::emission::claim_bindings::selections::SelectedDecision>,
}

#[derive(Clone, Copy)]
pub(super) struct CompletionOccurrence {
    value: CheckedStructuralValueHandle,
    first_new_place: u64,
}

/// One value's normal-death work, owned by its actual production cut.
/// A producer that must finish before a private edge returns the same
/// acknowledgement; an enclosing scope does not perform that death again.
pub(super) enum ValueCompletion {
    Awaiting {
        argument: StructuralArgument,
        occurrence: CompletionOccurrence,
    },
    Complete {
        argument: StructuralArgument,
        receipt: NormalDeathReceipt,
    },
}

/// This acknowledges normal death only. It supplies no liveness, access,
/// loan or claim authority; ordinary capture and edge checks still own those.
pub(super) struct NormalDeathReceipt {
    occurrence: CompletionOccurrence,
    source: StructuralArgument,
    block: BlockId,
}

impl ValueCompletion {
    fn into_argument(self) -> StructuralArgument {
        match self {
            Self::Awaiting { argument, .. } | Self::Complete { argument, .. } => argument,
        }
    }
}

pub(super) struct ValueContinuation {
    block: BlockId,
    parameters: Vec<ValueDeclaration>,
    structural_parameters: Vec<StructuralParameterDeclaration>,
    place: PlaceId,
    remaining_owners: Vec<argument_evaluation::StructuralValueOwner>,
    claim_destinations: Vec<language_semantics::PermissionClaimIdentity>,
    claims: Vec<terminal_psi::StructuralResultClaimBinding>,
    common_claims: Option<Vec<terminal_psi::StructuralResultClaimBinding>>,
    current_claims: Option<crate::emission::current_claims::CurrentClaimNames>,
    /// Memberships are the intersection of completed incoming values, never
    /// facts inferred from the receiving annotation. An empty first roster is
    /// distinct from a join that has not received an edge yet.
    has_incoming_memberships: bool,
}

pub(super) struct FallbackContinuation {
    block: BlockId,
    values: Vec<ValueDeclaration>,
    current_claims: crate::emission::current_claims::CurrentClaimNames,
    sources: argument_evaluation::StructuralSourceFrontier,
}

/// The completed source and exact projection, before its consumer chooses
/// copying or transfer. Only the selected path crosses the continuation; the
/// original contextual owner retains its siblings until normal death.
pub(super) struct ProjectionSource {
    pub(super) root: PlaceId,
    /// The already lowered walk from `root`'s storage to the checked source
    /// the projection reads. It is empty unless that source is itself a
    /// projected owned alias, which holds no storage of its own.
    pub(super) root_path: Vec<terminal_psi::StructuralPathSegment>,
    pub(super) path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub(super) root_type_identity: typed_trees::type_identity::TypeIdentityId,
    pub(super) logical_root_type_identity: typed_trees::type_identity::TypeIdentityId,
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
        direct_establishments: &[semantic_vocabulary::ClaimId],
    ) -> Result<(), LoweringError> {
        let (operation, kind, produced) = operations
            .operations
            .iter_mut()
            .find_map(|operation| match &mut operation.result {
                OperationResult::Structural(result) if result.place == place => {
                    Some((operation.id, &operation.kind, result))
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "owned completion has no operation result",
            ))?;
        if !produced.claims.is_empty() && produced.claims != claims {
            return unsupported("owned completion changes its established claim identities");
        }
        produced.claims = claims.to_vec();
        if !matches!(
            kind,
            OperationKind::EstablishRecord { .. }
                | OperationKind::EstablishStructuralCase { .. }
                | OperationKind::EstablishStructuralArray { .. }
                | OperationKind::EstablishZeroedStructural { .. }
        ) {
            // Calls classify introduction versus transport from their exact
            // input claim roster; rebinding their result cannot introduce again.
            return Ok(());
        }
        // Recursive literal children establish their own root claims. The
        // containing construction only transports those names under its paths.
        // The roster comes from exact checked establishment provenance, not
        // the result type or whether a namespace name was already reserved.
        for binding in claims.iter().filter(|binding| {
            binding.path.is_empty() && direct_establishments.contains(&binding.claim)
        }) {
            let introduction = crate::emission::claim_bindings::ClaimIntroduction {
                operation,
                claim: binding.claim,
            };
            if !operations.claim_introductions.contains(&introduction) {
                operations.claim_introductions.push(introduction);
            }
        }
        Ok(())
    }

    /// Execute this call's retained preparation only when the value evaluator
    /// reaches its node. The ordinary argument schedule interleaves scalar
    /// observations and structural producers in authored formal order.
    fn emit_operand_call(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
        let mut staged_selectors = vec![Vec::new(); planned.len()];
        let mut staged_structural_arguments = vec![Vec::new(); planned.len()];
        let mut staged_ranges = vec![Vec::new(); planned.len()];
        let mut source_count = None;
        let mut completed = None;
        for step in schedule {
            let (index, staged) = match step {
                Step::CaptureBefore(statement) => {
                    self.evaluation.prepare_storage_before(
                        identities,
                        self.checked,
                        self.state,
                        statement,
                        self.structural_types,
                        self.domain_declarations,
                        self.values,
                        self.environment.next_place,
                        self.next_value,
                        self.operations,
                        self.environment.private_places,
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
                        identities,
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
                        self.environment,
                    )?;
                    staged_values[operation].push(self.values.len());
                    self.values.push(value);
                    continue;
                }
                Step::Selector {
                    operation,
                    argument,
                    segment,
                } => {
                    let count = source_count.ok_or(LoweringError::Unsupported(
                        "structural selector has no source prefix",
                    ))?;
                    let value = self.evaluation.call_selector(
                        identities,
                        self.checked,
                        self.machine,
                        self.state,
                        &planned[operation],
                        argument,
                        segment,
                        count,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.operations,
                        self.calls,
                        self.environment,
                    )?;
                    staged_selectors[operation].push(self.values.len());
                    self.values.push(value);
                    continue;
                }
                Step::Structural {
                    operation,
                    argument,
                    complete,
                } => {
                    let count = source_count.ok_or(LoweringError::Unsupported(
                        "structural actual has no source prefix",
                    ))?;
                    let call = planned[operation]
                        .call()
                        .ok_or(LoweringError::Unsupported("structural actual has no call"))?;
                    let offset = call.structural_arguments[..argument]
                        .iter()
                        .map(|source| source.call_selector_positions().count())
                        .sum::<usize>();
                    let selectors = staged_selectors[operation][offset..].to_vec();
                    if staged_structural_arguments[operation].is_empty() {
                        staged_structural_arguments[operation] =
                            vec![None; call.structural_arguments.len()];
                    }
                    let previous = staged_structural_arguments[operation][argument].take();
                    let captured = self.environment.capture_argument(
                        identities,
                        &planned[operation],
                        argument,
                        &selectors,
                        previous,
                        complete,
                        count,
                        self.evaluation,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.calls,
                        self.operations,
                    )?;
                    staged_structural_arguments[operation][argument] = Some(captured);
                    continue;
                }
                Step::Endpoint {
                    operation,
                    argument,
                } => {
                    let count = source_count.ok_or(LoweringError::Unsupported(
                        "structural endpoint operand has no source prefix",
                    ))?;
                    self.evaluation.window_endpoints(
                        identities,
                        self.checked,
                        self.machine,
                        self.state,
                        &planned[operation],
                        argument,
                        count,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.operations,
                        self.calls,
                        self.environment,
                    )?;
                    continue;
                }
                Step::Subslice { operation, ordinal } => {
                    let count = source_count.ok_or(LoweringError::Unsupported(
                        "structural range operand has no source prefix",
                    ))?;
                    let bindings = self.evaluation.contextual_bindings();
                    let context = super::super::byte_subslices::StagingContext {
                        state: self.state,
                        structural_types: self.structural_types,
                        type_ids: self.type_ids,
                        bindings: &bindings,
                    };
                    let place = super::super::byte_subslices::emit(
                        identities,
                        self.checked,
                        &context,
                        &planned[operation],
                        ordinal,
                        self.evaluation,
                        count,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.operations,
                        self.calls,
                        self.environment,
                    )?;
                    staged_ranges[operation].push((ordinal, place));
                    continue;
                }
                Step::Operation(index) | Step::Call(index) if source_count.is_some() => {
                    (index, true)
                }
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
                    identities,
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
                    self.environment,
                )?
            } else {
                None
            };
            let selectors = staged_selectors[index]
                .iter()
                .map(|slot| {
                    self.values
                        .get(*slot)
                        .copied()
                        .ok_or(LoweringError::Unsupported(
                            "structural selector lost its retained value",
                        ))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let range_places = super::super::byte_subslices::argument_places(
                operation.structural_operands(),
                &[],
                &mut 0,
                &staged_ranges[index],
            )?;
            let declaration = self.environment.emit_operand(
                identities,
                operation,
                evaluated.as_deref(),
                staged.then_some(selectors.as_slice()),
                (!staged_structural_arguments[index].is_empty())
                    .then_some(staged_structural_arguments[index].as_slice()),
                &range_places,
                count,
                self.evaluation,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.calls,
                self.operations,
            )?;
            if index + 1 == planned.len() {
                completed = declaration;
            }
        }
        for completion in call.completions().iter() {
            // Completion is an ordinary checked operation stream. In
            // particular, nominal backing cleanup follows descriptor release
            // through the same frame that emitted the operand preparations.
            self.environment.emit_operand(
                identities,
                completion,
                None,
                None,
                None,
                &[],
                source_count.unwrap_or(self.values.len()),
                self.evaluation,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.calls,
                self.operations,
            )?;
        }
        let completed = completed.ok_or(LoweringError::Unsupported(
            "structural operand call did not publish its result",
        ))?;
        // A nested call's result has its own source occurrence even when a
        // projection becomes the authored local. An ordinary call publishes
        // an operation result; selected leaves publish one completed arrival.
        // Retain that exact home and custody instead of choosing a leaf or
        // reconstructing the producer from the receiving local.
        let result =
            call.operation()
                .structural_result_binding()
                .ok_or(LoweringError::Unsupported(
                    "structural operand call lost its checked result",
                ))?;
        let binding = self
            .operations
            .structural_values
            .iter()
            .find(|(ordinal, _)| *ordinal == result.binding_ordinal)
            .map(|(_, binding)| binding)
            .ok_or(LoweringError::Unsupported(
                "structural operand call has no completed binding",
            ))?;
        if binding.place() != completed.id || !binding.path().is_empty() {
            return unsupported("structural operand call completion names different storage");
        }
        let result = completed_storage::completed_result(
            completed.id,
            &completed,
            self.operations,
            self.evaluation.structural_root_parameters(),
            binding.claims(),
        )?;
        // Enclosing value completion must see the call's authenticated claim
        // frontier even when its storage is the dispatch continuation.
        let source_expression = self.structural_value_node(call.value).expression;
        let structural_type = result.structural_type;
        self.operations
            .arrival_claims
            .push((completed.id, result.claims));
        crate::expression_preparation::bindings::retain_completed_structural_homes(
            &mut self.evaluation.structural_locals,
            &[
                crate::expression_preparation::bindings::StructuralLocalBinding {
                    result_binding_ordinal: None,
                    root: facts::PlaceRoot::Expression(source_expression),
                    argument: StructuralArgument::whole(completed.id, StructuralAccess::Owned),
                    structural_type,
                },
            ],
        )?;
        self.published_places.push(completed);
        Ok(completed)
    }

    /// Evaluate one child under its actual stored carrier and owned access.
    /// Its producer publishes its own checked claims; the containing operation
    /// rebases those completed names under its field or case path.
    pub(super) fn child_constructor_operand(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        child_type: StructuralTypeId,
        value: CheckedStructuralValueHandle,
        reference: checked_trees::types::TypeReferenceHandle,
    ) -> Result<super::operand_claims::CompletedOperand, LoweringError> {
        let parent_type = self.structural_type;
        let parent_multiplicity = self.multiplicity;
        // Each child completes independently of the containing receiver's
        // annotation. Restore it even when child formation refuses.
        let parent_binding = std::mem::take(&mut self.binding_type_reference);
        // A constructor owns each stored operand, including a loan descriptor.
        // The enclosing result may instead be a borrowed alias; that access
        // describes its destination, not the custody of its contained values.
        let parent_access = std::mem::replace(&mut self.result_access, StructuralAccess::Owned);
        self.structural_type = child_type;
        self.multiplicity = crate::terminal_identities::structural_multiplicity(
            crate::reference_custody::result_multiplicity(self.checked, reference),
        );
        let argument = self.constructor_operand(identities, value, reference);
        self.structural_type = parent_type;
        self.multiplicity = parent_multiplicity;
        self.result_access = parent_access;
        self.binding_type_reference = parent_binding;
        argument
    }

    /// Complete one constructor operand without erasing an owned projection.
    /// The consuming constructor moves that subtree under ordinary partial
    /// custody; siblings remain attached to the original live root.
    pub(super) fn constructor_operand(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: CheckedStructuralValueHandle,
        reference: checked_trees::types::TypeReferenceHandle,
    ) -> Result<super::operand_claims::CompletedOperand, LoweringError> {
        let first_new_place = *self.environment.next_place;
        let node = self.structural_value_node(value);
        // A projection of existing storage moves its child in place; the
        // owner keeps its residual. An anonymous call result has no owner to
        // keep it, so its ordinary contextual completion owns the temporary
        // residual at the consuming boundary.
        if let CheckedStructuralValueKind::Projection {
            source,
            logical_root_type_identity,
            path,
            type_identity,
            projection_evaluations,
            copies_selected_value: false,
        } = node.kind
            && !path.is_empty()
            && matches!(
                structural_values(self.checked).nodes.get(source).kind,
                CheckedStructuralValueKind::Place(_)
            )
        {
            let projected =
                self.projection_source(identities, source, path, logical_root_type_identity)?;
            let argument = self.complete_projection_source(
                identities,
                node.expression,
                &projected,
                projection_evaluations,
                type_identity,
                StructuralAccess::Owned,
            )?;
            let argument = self.retain_checked_memberships(
                identities,
                argument,
                node.qualification_decisions,
                node.expression,
                first_new_place,
            )?;
            return self.capture_operand(identities, argument, node.expression, first_new_place);
        }
        // A fresh operand has its own continuation, not the enclosing
        // selection's result/residual slots. Incoming owners remain live
        // until their actual consuming operation or outer selection edge.
        let sources = std::mem::take(&mut self.sources);
        let owners = std::mem::take(&mut self.owners);
        let parent_reference = std::mem::replace(&mut self.type_reference, reference);
        let result = self.value(identities, value, None).and_then(|argument| {
            self.retain_checked_memberships(
                identities,
                argument,
                node.qualification_decisions,
                node.expression,
                first_new_place,
            )
        });
        self.type_reference = parent_reference;
        self.sources = sources;
        self.owners = owners;
        result.and_then(|argument| {
            self.capture_operand(identities, argument, node.expression, first_new_place)
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

    /// A produced operand retains its storage coordinate through value
    /// completion. Existing locals can name a subtree; only a fresh result
    /// or completed join is necessarily a whole place.
    pub(super) fn value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        value: CheckedStructuralValueHandle,
        continuation: Option<&mut ValueContinuation>,
    ) -> Result<StructuralArgument, LoweringError> {
        let node = self.structural_value_node(value);
        // Recursive value nodes are checked in this one statement's namespace.
        // Authenticate that coordinate before retaining any physical birth;
        // private block ordinals are not source statement or state identities.
        if node.machine != self.machine
            || node.state != self.state
            || node.statement_index != self.statement
        {
            return unsupported("structural operand has a different checked source coordinate");
        }
        if value != self.produced_root
            && structural_values(self.checked)
                .root_for_value(value)
                .is_some_and(|root| root.captured_copy.is_valid())
        {
            // A saved copy is a completed producer, never an expression to
            // execute again. Individual reads copy a projected subtree from
            // its exact handle; another root at the same source occurrence
            // cannot replace this birth or its current execution frame.
            let captured = self
                .evaluation
                .saved_value_binding(value, self.operations)?;
            return Ok(StructuralArgument {
                access: StructuralAccess::SharedBorrow,
                ..captured.argument.clone()
            });
        }
        let requires_completion = self.content_transformation(&node)?.is_some()
            || self
                .checked
                .facts
                .flow
                .terminal_unit_effects
                .cleanup_control
                .value_completion(self.machine, self.state, self.statement, node.expression)
                .is_some();
        if requires_completion {
            // The result is secured before its normal death and before the
            // external consumer's edge. A producer may already have completed
            // that same operation before its own private edge.
            let produced = self.raw_value(identities, value, None)?;
            let (completed, receipt) = self.finish_value_completion(identities, value, produced)?;
            return self
                .finish_value_arrival(completed, receipt, continuation)
                .map(ValueCompletion::into_argument);
        }
        // With no checked normal-death or content transaction there is no
        // extra completion work between this producer and its consumer.
        self.raw_value(identities, value, continuation)
            .map(ValueCompletion::into_argument)
    }

    fn raw_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: CheckedStructuralValueHandle,
        continuation: Option<&mut ValueContinuation>,
    ) -> Result<ValueCompletion, LoweringError> {
        let node = self.structural_value_node(value);
        let completion = CompletionOccurrence {
            value,
            first_new_place: *self.environment.next_place,
        };
        let place = match node.kind {
            CheckedStructuralValueKind::NativeCodeLayout { selected_state } => {
                let (selected, _) =
                    validation::find_machine_by_entry_state(&self.checked.typed, selected_state)
                        .ok_or(LoweringError::Unsupported(
                            "native layout query lost its exact selected executable entry",
                        ))?;
                let machine = crate::terminal_identities::lookup_machine_id(
                    self.calls.machine_ids,
                    selected.symbol,
                )?;
                let place = place_id(allocate_dense(self.environment.next_place)?);
                let operation = self.operations.allocate();
                self.operations.push(terminal_psi::Operation {
                    id: operation,
                    static_reach_binding: None,
                    suspension_crossing: None,
                    result:
                        crate::emission::operation_emission::buffer::structural_result_declaration(
                            place,
                            self.structural_type,
                            StructuralMultiplicity::Unrestricted,
                        ),
                    kind: OperationKind::EstablishNativeCodeLayout { machine },
                });
                self.environment
                    .private_places
                    .push(StructuralPlaceDeclaration {
                        id: place,
                        kind: StructuralPlaceKind::OperationResult {
                            producer: operation,
                            structural_type: self.structural_type,
                        },
                    });
                self.completed(identities, place, completion, continuation)
            }

            CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                let place = self.quotient_call(identities, value, &arguments)?;
                self.completed(identities, place, completion, continuation)
            }
            CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                let child = self.structural_value_node(source);
                let source_type = lookup_type_id(
                    self.type_ids,
                    &validation::stored_structural_type_identity(
                        &self.checked.typed,
                        child.type_reference,
                        &[],
                        &[],
                    ),
                )?;
                let completed = self.child_constructor_operand(
                    identities,
                    source_type,
                    source,
                    child.type_reference,
                )?;
                if !completed.claims.is_empty()
                    || !completed.argument.path.is_empty()
                    || self.multiplicity != StructuralMultiplicity::Unrestricted
                {
                    return unsupported(
                        "quotient adaptation requires a whole custody-free copy value",
                    );
                }
                let place = place_id(allocate_dense(self.environment.next_place)?);
                let declaration = crate::emission::quotient_values::emit_adaptation(
                    value,
                    completed.argument.place,
                    source_type,
                    place,
                    self.structural_type,
                    self.operations,
                );
                self.environment.private_places.push(declaration);
                self.completed(identities, place, completion, continuation)
            }
            // A borrowed `&[T]` view lowers to `EstablishElementView` and a
            // `&mut [T]` view to `EstablishMutableElementView`: the collection
            // argument resolves against the current locals and signature
            // parameters exactly as a `&T` leaf does, and the fresh view result
            // joins the selection's block parameter under the view's loan.
            CheckedStructuralValueKind::BorrowedSliceView { source, region } => self
                .joined_element_view(
                    identities,
                    &source,
                    &region,
                    node.expression,
                    LoanAccess::Shared,
                    continuation,
                ),
            CheckedStructuralValueKind::MutableSliceView { source, region } => self
                .joined_element_view(
                    identities,
                    &source,
                    &region,
                    node.expression,
                    LoanAccess::Mutable,
                    continuation,
                ),
            CheckedStructuralValueKind::Reference {
                source,
                selectors,
                source_value,
                storage,
                backing_type,
            } => self.reference_value(
                identities,
                super::super::reference_results::Formation {
                    checked: self.checked,
                    state: node.state,
                    statement_index: node.statement_index as usize,
                    expression: node.expression,
                    backing_type,
                    qualification_decisions: node.qualification_decisions,
                    domain_ids: self.domain_ids,
                },
                &source,
                selectors,
                source_value,
                storage,
                completion,
                continuation,
            ),
            CheckedStructuralValueKind::Record { .. } => {
                let place = self.record(identities, value)?;

                self.complete_construction(
                    identities,
                    place,
                    node.qualification_decisions,
                    completion,
                    continuation,
                )
            }
            CheckedStructuralValueKind::FixedArray { elements } => {
                let place = self.fixed_array(identities, &elements, self.type_reference)?;

                self.complete_construction(
                    identities,
                    place,
                    node.qualification_decisions,
                    completion,
                    continuation,
                )
            }
            CheckedStructuralValueKind::Call { .. } => {
                let mut matching = self.operand_calls.iter().filter(|call| call.value == value);
                let call = matching.next().ok_or(LoweringError::Unsupported(
                    "structural operand call missing",
                ))?;
                if matching.next().is_some() {
                    return unsupported("structural operand call is duplicated");
                }
                let declaration = self.emit_operand_call(identities, call)?;
                self.completed(identities, declaration.id, completion, continuation)
            }
            CheckedStructuralValueKind::Projection {
                source,
                logical_root_type_identity,
                path,
                projection_evaluations,
                type_identity,
                copies_selected_value,
            } => {
                // A noncopy whole-value projection has no residual complement or new
                // storage. Evaluate its retained producer at this DAG position,
                // including only its selected arm, then use the ordinary value
                // continuation. Reborrowing a shared descriptor preserves its
                // actual geometry rather than reforming its backing's view.
                // A same-carrier qualification is this projection's own
                // checked membership, published on the produced value.
                if path.is_empty() && !copies_selected_value {
                    // The cast occurrence must complete before its consumer:
                    // passing the continuation into the operand would seal its
                    // jump with the unqualified source, stranding the checked
                    // membership copy after that edge.
                    let first_new_place = *self.environment.next_place;
                    let argument = self.value(identities, source, None)?;
                    let mut argument = self.retain_checked_memberships(
                        identities,
                        argument,
                        node.qualification_decisions,
                        node.expression,
                        first_new_place,
                    )?;
                    // The operand publishes storage; the checked projected
                    // value retains its own access at the continuation.
                    // Owning descriptor storage does not own its backing.
                    argument.access = self.result_access;
                    if let Some(continuation) = continuation {
                        self.complete_operand(
                            identities,
                            argument.clone(),
                            completion,
                            continuation,
                        )?;
                    }
                    return Ok(ValueCompletion::Awaiting {
                        argument,
                        occurrence: completion,
                    });
                }
                // Copy permission belongs to the selected value, independently
                // of its parent's multiplicity. Shared descriptors copy their
                // carrier while preserving the same backing obligations.
                if copies_selected_value {
                    let projected = self.projection_source(
                        identities,
                        source,
                        path,
                        logical_root_type_identity,
                    )?;
                    let argument = self.complete_projection_source(
                        identities,
                        node.expression,
                        &projected,
                        projection_evaluations,
                        type_identity,
                        StructuralAccess::SharedBorrow,
                    )?;
                    let place = self.copied_leaf_place(&argument, node.qualification_decisions)?;
                    let produced = ValueCompletion::Awaiting {
                        argument: StructuralArgument::whole(place, StructuralAccess::Owned),
                        occurrence: completion,
                    };
                    let (completed, receipt) =
                        self.finish_value_completion(identities, value, produced)?;
                    return self.finish_value_arrival(completed, receipt, continuation);
                }
                // A projection always needs an ownership edge for its kept
                // subtree. The original complement stays at its checked normal
                // death. At a value root there is
                // no enclosing dispatch to supply that edge; create the same
                // result continuation rather than turn the projection into
                // an alias over temporary storage.
                let mut owned_continuation;
                let is_root = continuation.is_none();
                let continuation = if let Some(continuation) = continuation {
                    continuation
                } else {
                    owned_continuation =
                        self.root_value_continuation(identities, value, self.values.clone())?;
                    &mut owned_continuation
                };
                let projected =
                    self.projection_source(identities, source, path, logical_root_type_identity)?;
                if matches!(
                    structural_values(self.checked).nodes.get(source).kind,
                    CheckedStructuralValueKind::Place(_)
                ) {
                    // projection_source resolved this exact checked plan to
                    // its existing owned home and full physical alias prefix.
                    // `sources` lists named residual cleanup owners only; an
                    // anonymous completed home is not a missing operand.
                    // Current names are a required association, not ownership
                    // authority: capture_operand below still authenticates
                    // the consuming receipt's logical source, full path and
                    // exact evaluated claim frontier before moving anything.
                    let existing = StructuralArgument {
                        place: projected.root,
                        path: projected.root_path.clone(),
                        access: StructuralAccess::Owned,
                    };
                    if self
                        .evaluation
                        .retired_structural_homes
                        .contains(&existing.place)
                        || self.evaluation.current_claims.source(&existing)?.is_none()
                    {
                        return unsupported(
                            "projected selection root has no current owned source association",
                        );
                    }
                }
                let selected = self.complete_projection_source(
                    identities,
                    node.expression,
                    &projected,
                    projection_evaluations,
                    type_identity,
                    StructuralAccess::Owned,
                )?;
                // Move first: the selected subtree must survive the exact
                // anonymous complement and home retirement at this completion.
                // Jumping the original projection directly would keep its
                // branch-private parent alive across the value join.
                let captured = self.capture_operand(
                    identities,
                    selected,
                    node.expression,
                    completion.first_new_place,
                )?;
                let (whole, projected_memberships) =
                    self.completed_memberships(captured.argument.place).ok_or(
                        LoweringError::Unsupported("projected move lost its source memberships"),
                    )?;
                let (qualifications, projected_qualifications) =
                    terminal_psi::StructuralPathQualification::select(
                        whole,
                        projected_memberships,
                        &captured.argument.path,
                    );
                let selected = self.move_completed_operand(
                    captured,
                    qualifications,
                    projected_qualifications,
                )?;
                let produced = ValueCompletion::Awaiting {
                    argument: selected,
                    occurrence: completion,
                };
                let (completed, receipt) =
                    self.finish_value_completion(identities, value, produced)?;
                let arrived =
                    self.finish_value_arrival(completed, receipt, Some(&mut *continuation))?;
                if is_root {
                    self.record_root_continuation(continuation)?;
                }
                return Ok(arrived);
            }
            CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } => {
                let argument = self.copied_place_argument(identities, node.expression, &source)?;
                let place = self.copied_leaf_place(&argument, node.qualification_decisions)?;
                self.completed(identities, place, completion, continuation)
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
                    self.environment.next_place,
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
                self.environment
                    .private_places
                    .push(operation_result_declaration(
                        place,
                        operation,
                        self.structural_type,
                    ));
                self.complete_construction(
                    identities,
                    place,
                    node.qualification_decisions,
                    completion,
                    continuation,
                )
            }
            CheckedStructuralValueKind::Scalar { value } => {
                let initialized = self.scalar(
                    identities,
                    CheckedScalarExpressionRole::StructuralScalarValue {
                        expression: node.expression,
                    },
                    value,
                    self.values.len(),
                )?;
                let obligation = super::super::primitive_locals::initialization_obligation(
                    self.structural_types,
                    self.structural_type,
                    &mut self.calls.next_obligation_identity,
                )?;
                let (place, declaration) = super::super::primitive_locals::push_established_local(
                    self.environment.next_place,
                    self.operations,
                    initialized,
                    self.structural_type,
                    super::super::primitive_locals::PrimitiveStorageCatalog {
                        types: self.structural_types,
                        domains: self.domain_declarations,
                        scalars: self.evaluation.shared_qualifications.catalog(),
                    },
                    obligation,
                )?;
                self.environment.private_places.push(declaration);
                self.completed(identities, place, completion, continuation)
            }
            CheckedStructuralValueKind::ScalarArray { elements } => {
                // Every leaf replays its own retained source computation in
                // authored row-major order, keyed by this array occurrence.
                // Private control flow may remap earlier saved values, so the
                // roster reads its final slots only after the last leaf.
                let source_count = self.values.len();
                for (ordinal, element) in elements.iter().enumerate() {
                    let evaluated = self.scalar(
                        identities,
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
                    self.environment.next_place,
                    self.structural_type,
                    self.multiplicity,
                    Vec::new(),
                    OperationKind::EstablishScalarArray { elements: leaves },
                )?;
                self.environment
                    .private_places
                    .push(operation_result_declaration(
                        place,
                        operation,
                        self.structural_type,
                    ));
                self.complete_construction(
                    identities,
                    place,
                    node.qualification_decisions,
                    completion,
                    continuation,
                )
            }
            CheckedStructuralValueKind::ByteLiteralView { bytes } => {
                // The literal carries the memberships checking established
                // for this occurrence (`""` into a `&[u8] in Utf8` field);
                // Terminal replays its byte predicates against the octets.
                let qualifications = self.checked_membership_domains(
                    node.qualification_decisions,
                    self.structural_type,
                )?;
                let declaration =
                    crate::emission::structural_byte_sequence_store::emit_call_literal(
                        identities,
                        &bytes,
                        self.structural_type,
                        qualifications,
                        self.structural_types,
                        self.environment.private_places,
                        self.environment.next_place,
                        self.next_value,
                        &mut self.calls.next_obligation_identity,
                        self.operations,
                    )?;
                self.completed(identities, declaration.id, completion, continuation)
            }
            CheckedStructuralValueKind::ZeroedStructural {
                projected_qualifications,
            } => {
                let projected = super::super::parameters::lower_projected_qualifications(
                    identities,
                    &projected_qualifications,
                    self.domain_ids,
                    self.structural_type,
                    self.structural_types,
                )?;
                let place = self.establish_zeroed()?;
                let result = self
                    .operations
                    .operations
                    .last_mut()
                    .and_then(|operation| match &mut operation.result {
                        OperationResult::Structural(result) => Some(result),
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "zeroed establishment has no result",
                    ))?;
                result.projected_qualifications = projected;

                self.complete_construction(
                    identities,
                    place,
                    node.qualification_decisions,
                    completion,
                    continuation,
                )
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
                    let (declaration, _) = crate::emission::structural_byte_sequence_store::emit_owned_literal(identities, bytes, self.structural_type, qualifications, self.structural_types, self.environment.private_places, self.environment.next_place, self.next_value, &mut self.calls.next_obligation_identity, self.operations)?;
                    let place = declaration.id;
                    self.environment.private_places.push(declaration);
                    self.completed(identities, place, completion, continuation)?;
                    return Ok(ValueCompletion::Awaiting {
                        argument: StructuralArgument { place, path: Vec::new(), access: StructuralAccess::Owned },
                        occurrence: completion,
                    });
                }
                if let Some(source) = self.copied_parameter(&argument)? {
                    let place = self.copied_leaf_place(&source, node.qualification_decisions)?;
                    self.completed(identities, place, completion, continuation)?;
                    return Ok(ValueCompletion::Awaiting {
                        argument: StructuralArgument {
                            place,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        },
                        occurrence: completion,
                    });
                }
                // Ownership is attached to the checked argument, independent
                // of whether its consumer is a local, constructor or join.
                // A continuation receives that same selected coordinate.
                let selected = if let Some(place) = self.parameter_source(&argument)? {
                    StructuralArgument {
                        place,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    }
                } else {
                    self.structural_bindings()
                        .owned_argument(identities, &argument)?
                };
                if let Some(continuation) = continuation {
                    self.complete_operand(identities, selected, completion, continuation)?;
                    Ok(continuation.place)
                } else {
                    return Ok(ValueCompletion::Awaiting {
                        argument: selected,
                        occurrence: completion,
                    });
                }
            }
            CheckedStructuralValueKind::Case(construction) => {
                let source = validation::scalar_case_constructor(
                    &self.checked.typed,
                    construction.expression,
                )
                .ok_or(LoweringError::Unsupported(
                    "structural case lost its authored constructor",
                ))?;
                let place = place_id(allocate_dense(self.environment.next_place)?);
                let slot = crate::scalar_lowering::computations::cases::reserve(
                    identities,
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
                        identities,
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
                self.environment.private_places.push(declaration);
                self.values.truncate(field_start);
                self.complete_construction(
                    identities,
                    place,
                    node.qualification_decisions,
                    completion,
                    continuation,
                )
            }
            CheckedStructuralValueKind::StructuralCase { .. } => {
                let place = self.structural_case(identities, value)?;

                self.complete_construction(
                    identities,
                    place,
                    node.qualification_decisions,
                    completion,
                    continuation,
                )
            }
            CheckedStructuralValueKind::Dispatch { subject, arms } => self.dispatch(
                identities,
                value,
                node.expression,
                subject,
                arms,
                continuation,
            ),
        }?;
        // A transported arm returns its continuation's future receiving place.
        // Only its actual operand completes here; the receiving namespace is
        // published after every predecessor has completed the continuation.
        Ok(ValueCompletion::Awaiting {
            argument: StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
            occurrence: completion,
        })
    }

    /// Value assignment uses ordinary owned edge transport. Returning the
    /// source place directly would alias unrestricted records instead of copying
    /// their backing, and would republish an earlier result's declaration.
    /// The existing block binder copies unrestricted payloads and transfers
    /// owned custody; source replay has already checked the exact operand.
    fn materialize_place(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        source: StructuralArgument,
        value: CheckedStructuralValueHandle,
        first_new_place: u64,
    ) -> Result<PlaceId, LoweringError> {
        // A whole-value ascription preserves its operand's storage. Capture
        // that input's consuming occurrence, before the completed ascription
        // itself is handed to a call or successor. Those are distinct claims
        // and coordinates even when both values share a physical carrier.
        let mut consumed_value = value;
        while let CheckedStructuralValueKind::Projection { source, path, .. } =
            &structural_values(self.checked)
                .nodes
                .get(consumed_value)
                .kind
            && path.is_empty()
        {
            consumed_value = *source;
        }
        let node = structural_values(self.checked).nodes.get(consumed_value);
        let source_claims = self
            .capture_operand(identities, source.clone(), node.expression, first_new_place)?
            .claims;
        let block = block_id(allocate_dense(self.next_block)?);
        let place = place_id(allocate_dense(self.environment.next_place)?);
        // Common owners retain their dominating homes. Forwarding their
        // bytes would invalidate addresses captured by surviving descriptors.
        // Only the actual moved value receives a new ownership binding. The
        // live evaluation keeps unrelated whole and partially moved homes;
        // complete_value commits just this operand to that same frontier.
        let mut structural_parameters = Vec::new();
        let position = u32::try_from(structural_parameters.len())
            .map_err(|_| LoweringError::Unsupported("owned move parameter count exceeds u32"))?;
        structural_parameters
            .push(self.value_binding_parameter(identities, &source, place, position)?);
        let parameters = fresh_value_declarations(self.next_value, self.values)?;
        self.environment
            .private_places
            .push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::BlockParameter { block, position },
            });
        let mut continuation = ValueContinuation {
            block,
            parameters,
            structural_parameters,
            place,
            remaining_owners: Vec::new(),
            claim_destinations: Vec::new(),
            claims: source_claims.clone(),
            common_claims: None,
            current_claims: None,
            has_incoming_memberships: false,
        };
        self.complete_value(
            super::operand_claims::CompletedOperand {
                argument: source.clone(),
                claims: source_claims.clone(),
            },
            &mut continuation,
        )?;
        self.seal_continuation_names(&mut continuation)?;
        self.operations.arrival_claims.push((place, source_claims));
        self.start(block);
        self.operations
            .register_structural_materialization(place, self.state, self.statement)?;
        if self.multiplicity != StructuralMultiplicity::Unrestricted {
            self.evaluation.structural_locals.retain(
                |crate::expression_preparation::bindings::StructuralLocalBinding {
                     argument,
                     ..
                 }| {
                    argument.place != source.place || !argument.path.starts_with(&source.path)
                },
            );
        }
        *self.values = continuation.parameters.clone();
        self.evaluation.parameters = continuation.parameters;
        self.evaluation.block_structural_parameters = continuation.structural_parameters;
        Ok(place)
    }

    /// A reference alias carries the same borrowed
    /// custody a selection arm does, but there is no dispatch to join through:
    /// the result still binds at a fresh block parameter so the sole incoming
    /// edge can present the exact borrowed referent place. No owner moves and
    /// no residual dies on that edge -- the referent's owner stays live under
    /// the checked loan the joined parameter records, exactly as it does for
    /// every arm of a borrowed selection.
    fn direct_borrow(&mut self, argument: StructuralArgument) -> Result<PlaceId, LoweringError> {
        let join = block_id(allocate_dense(self.next_block)?);
        let place = place_id(allocate_dense(self.environment.next_place)?);
        let position = 0u32;
        self.environment
            .private_places
            .push(block_parameter_declaration(place, join, position));
        let structural_parameters = vec![structural_parameter_declaration(
            place,
            position,
            self.structural_type,
            self.multiplicity,
            self.result_access,
        )];
        let parameters = fresh_value_declarations(self.next_value, self.values)?;
        let mut continuation = ValueContinuation {
            block: join,
            parameters,
            structural_parameters,
            place,
            remaining_owners: Vec::new(),
            claim_destinations: Vec::new(),
            claims: Vec::new(),
            common_claims: None,
            current_claims: None,
            has_incoming_memberships: false,
        };
        let loan = match argument.access {
            StructuralAccess::SharedBorrow => LoanAccess::Shared,
            StructuralAccess::MutableBorrow => LoanAccess::Mutable,
            _ => return unsupported("direct borrow requires readable loan access"),
        };
        self.complete_loan(argument, &mut continuation, loan)?;
        self.seal_continuation_names(&mut continuation)?;
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
    fn copied_leaf_place(
        &mut self,
        argument: &StructuralArgument,
        qualification_decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
    ) -> Result<PlaceId, LoweringError> {
        let path = self.copied_place_path(argument)?;
        // Ordinary records and bytes retain the same occurrence-relative
        // source roster. Receiving annotations never manufacture membership.
        let source_contract = self.completed_memberships(argument.place);
        let (mut qualifications, mut projected_qualifications) = source_contract
            .map(|(whole, projected)| {
                terminal_psi::StructuralPathQualification::select(whole, projected, &path)
            })
            .unwrap_or_default();
        terminal_semantics::present_structural_memberships(
            self.structural_types.iter(),
            self.domain_declarations.iter(),
            self.structural_type,
            &mut qualifications,
            &mut projected_qualifications,
        )
        .map_err(LoweringError::Unsupported)?;
        // The checker proved these exact memberships at this operand's
        // evaluation point, before a later constructor child may mutate the
        // source home. This is decision transport, not a second proof replay.
        let (membership_carrier, membership_path) = self.checked_membership_subject()?;
        let domains =
            self.checked_membership_domains(qualification_decisions, membership_carrier)?;
        if membership_path.is_empty() {
            qualifications.extend(domains);
        } else {
            terminal_psi::StructuralPathQualification::append_under(
                &domains,
                &[],
                membership_path,
                &mut projected_qualifications,
            );
        }
        qualifications.sort();
        qualifications.dedup();
        projected_qualifications.sort();
        projected_qualifications.dedup();
        let place = place_id(allocate_dense(self.environment.next_place)?);
        let first_operation = self.operations.operations.len();
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
        self.environment
            .private_places
            .push(operation_result_declaration(
                place,
                operation,
                self.structural_type,
            ));
        self.operations
            .register_structural_materialization(place, self.state, self.statement)?;
        // This copy is a new completed value, even when it carries no claims.
        // Publish its actual operation result before any receiving consumer;
        // this never revives the source or imports sibling-arm declarations.
        self.evaluation
            .current_claims
            .publish_operations(&self.operations.operations[first_operation..])?;
        Ok(place)
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        role: CheckedScalarExpressionRole,
        value: checked_trees::CheckedScalarComputationHandle,
        source_count: usize,
    ) -> Result<ValueDeclaration, LoweringError> {
        let value = self.evaluation.source_value(
            identities,
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
            self.environment,
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
        fallback: &mut Option<FallbackContinuation>,
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
        self.boolean_value_split(condition, fallback)
    }

    /// Split on an already evaluated predicate without manufacturing another
    /// evaluation or changing its operation's observation identity.
    pub(super) fn boolean_value_split(
        &mut self,
        condition: semantic_vocabulary::ValueId,
        fallback: &mut Option<FallbackContinuation>,
    ) -> Result<(), LoweringError> {
        let selected = block_id(allocate_dense(self.next_block)?);
        let next = block_id(allocate_dense(self.next_block)?);
        let when_true = self.edge(selected, Vec::new(), Vec::new())?;
        let when_false = self.edge(next, Vec::new(), Vec::new())?;
        self.finish(Terminator::Conditional {
            condition,
            when_true,
            when_false,
        });
        *fallback = Some(FallbackContinuation {
            block: next,
            values: self.values.clone(),
            current_claims: self.evaluation.current_claims.clone(),
            sources: self.evaluation.structural_source_frontier(),
        });
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
        self.operations.storage_observations.clear();
    }

    /// Resume the case-fallback arm's stashed value roster at its entry
    /// block, when one was recorded.
    fn restore_fallback(
        &mut self,
        fallback: Option<FallbackContinuation>,
    ) -> Result<(), LoweringError> {
        if let Some(fallback) = fallback {
            self.evaluation.current_claims = fallback.current_claims;
            self.evaluation
                .restore_structural_sources(fallback.sources)?;
            self.start(fallback.block);
            *self.values = fallback.values;
        }
        Ok(())
    }

    /// Push an evaluated scalar onto the SSA roster, request the declared
    /// field's range-evidence obligation when its type requires one, and
    /// return the terminal initializer value. The verifier reconstructs the
    /// declared range against the final SSA initializer; the identity
    /// requests that proof — it is not an assertion construction is valid.
    /// `positions` records the saved-value remap for private control flow.
    pub(super) fn scalar_field_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        field: &terminal_psi::StructuralFieldDeclaration,
        evaluated: ValueDeclaration,
        positions: &mut Vec<(usize, usize)>,
        field_index: usize,
    ) -> Result<terminal_psi::RecordFieldValue, LoweringError> {
        // Construction establishes storage before it can be lent or observed.
        // An atomic cell accepts its exact scalar initializer here, without
        // becoming an ordinary scalar field or acquiring event authority.
        // Records, selected cases and omitted-field zeroes share this check.
        if field.field_type.scalar_type() != Some(evaluated.scalar_type) {
            return unsupported(
                "field initializer requires its exact scalar carrier and range evidence",
            );
        }
        let receiving_type = self
            .evaluation
            .shared_qualifications
            .catalog()
            .fields
            .iter()
            .find(|row| row.field == field.id)
            .map(|row| row.value_type)
            .unwrap_or(evaluated.value_type());
        let evaluated = self.evaluation.receiving_value(
            identities,
            evaluated,
            receiving_type,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
            self.environment,
        )?;
        positions.push((field_index, self.values.len()));
        self.values.push(evaluated);
        let range_obligation =
            if terminal_semantics::scalar_field_requires_evidence(&field.field_type) {
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

    /// Transfer only the selected value. Every candidate keeps its original
    /// home until that actual edge transfers it or its checked death runs.
    /// The existing private-continuation specialization retains the different
    /// source frontiers; compressing N-1 survivors into positional slots would
    /// lose original declaration order around interleaved common owners.
    fn root_value_continuation(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: CheckedStructuralValueHandle,
        values: Vec<ValueDeclaration>,
    ) -> Result<ValueContinuation, LoweringError> {
        let join = block_id(allocate_dense(self.next_block)?);
        let parameters = fresh_value_declarations(self.next_value, &values)?;
        let place = place_id(allocate_dense(self.environment.next_place)?);
        self.environment
            .private_places
            .push(block_parameter_declaration(place, join, 0));
        let mut claims = Vec::new();
        for path in crate::emission::claim_bindings::completed_value_paths(self.checked, value)? {
            let claim = self
                .calls
                .claims
                .receive_completed_value_claim(value, &path)?;
            let path = lower_static_argument_path(
                identities,
                &path,
                self.structural_type,
                self.structural_types,
            )?;
            self.operations
                .block_entry_claims
                .push(terminal_psi::EntryClaim {
                    claim,
                    input: place,
                    path: path.clone(),
                });
            claims.push(terminal_psi::StructuralResultClaimBinding { claim, path });
        }
        claims.sort();
        Ok(ValueContinuation {
            block: join,
            parameters,
            structural_parameters: vec![structural_parameter_declaration(
                place,
                0,
                self.structural_type,
                self.multiplicity,
                self.result_access,
            )],
            place,
            remaining_owners: self.owners.clone(),
            claim_destinations: Vec::new(),
            claims,
            common_claims: None,
            current_claims: None,
            has_incoming_memberships: false,
        })
    }

    /// Bookkeep a root selection continuation: install the join block's
    /// value and structural parameters on the evaluation state, publish the
    /// completed custody roster and retain candidate homes. Both common and
    /// unselected owners keep their original storage through the continuation.
    fn seal_continuation_names(
        &mut self,
        continuation: &mut ValueContinuation,
    ) -> Result<(), LoweringError> {
        crate::emission::claim_bindings::seal_receiving_claims(
            self.operations,
            continuation.place,
            &mut continuation.claims,
            continuation.common_claims.as_deref(),
        )?;
        if let Some(current) = continuation.current_claims.take() {
            self.evaluation.current_claims = current;
        }
        self.evaluation
            .current_claims
            .publish(continuation.place, continuation.claims.clone())?;
        Ok(())
    }

    fn record_root_continuation(
        &mut self,
        continuation: &mut ValueContinuation,
    ) -> Result<(), LoweringError> {
        self.seal_continuation_names(continuation)?;
        self.start(continuation.block);
        if self.result_access == StructuralAccess::Owned {
            self.operations.register_structural_materialization(
                continuation.place,
                self.state,
                self.statement,
            )?;
        }
        *self.values = continuation.parameters.clone();
        self.evaluation.parameters = continuation.parameters.clone();
        self.evaluation.block_structural_parameters = continuation.structural_parameters.clone();
        // A join is a producer just as an operation is. Enclosing constructors
        // capture its checked completion roster before evaluating their next
        // operand; an empty roster is a completed value, not missing custody.
        // Publish before the source-owner early return: fresh alternatives
        // still have a result even when they have no residual input owners.
        self.operations
            .arrival_claims
            .push((continuation.place, continuation.claims.clone()));
        self.evaluation.structural_value_owners = continuation.remaining_owners.clone();
        if !self.selection.is_valid() {
            if self.sources.is_empty() {
                return Ok(());
            }
            return unsupported("selected source storage has no checked receipt");
        }
        let selection = self.selection;
        let receipt = self
            .checked
            .facts
            .flow
            .ownership
            .owned_selections
            .get(selection);
        // A checked fresh selection still has a join with an explicitly empty
        // named-source roster. Anonymous partial sources have already retired
        // their exact complements at their own value completions.
        let roster = self
            .checked
            .facts
            .flow
            .ownership
            .selection_sources
            .span_or_empty(receipt.sources);
        let mut sources = Vec::new();
        for owner in self
            .owners
            .iter()
            .rev()
            .filter(|owner| self.source_at(owner.value.place).is_some())
        {
            let mut matching = roster
                .iter()
                .enumerate()
                .filter(|(_, source)| source.symbol == owner.symbol);
            let (ordinal, _) = matching.next().ok_or(LoweringError::Unsupported(
                "selected source home lost its checked declaration",
            ))?;
            if matching.next().is_some() {
                return Err(LoweringError::Unsupported(
                    "selected source declaration is ambiguous",
                ));
            }
            sources.push(argument_evaluation::CapturedSelectionSource {
                source_ordinal: u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported(
                        "selected source ordinal exceeds its checked representation",
                    )
                })?,
                storage: argument_evaluation::StructuralValueStorage {
                    place: owner.value.place,
                    path: Vec::new(),
                    selected_type: owner.value.structural_type,
                },
            });
        }
        self.evaluation
            .selection_cleanups
            .push(argument_evaluation::SelectionCleanup {
                selection,
                sources,
                retired_reference_paths: Vec::new(),
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
        Ok(())
    }
}
