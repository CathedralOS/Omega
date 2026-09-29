//! Exact source bindings and final custody of structural call results.

use super::super::admission::CallerView;
use super::super::parameters::expression_producer;
use super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, ExpressionNode,
    LoweringError, Multiplicity, StatementNode, unsupported,
};
use checked_trees::CheckedUnitStructuralResultBindingPlan;
use checked_trees::types::TypeReferenceNode;

/// Rejoin a whole named linear operand to its earlier successful call, not
/// merely another result with the same carrier type. Source claims retain the
/// input lineage across both calls; operation emission checks the same join in
/// the Terminal place/claim namespace.
fn validate_linear_result_consumer(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    argument_index: usize,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
) -> Result<(), LoweringError> {
    let (machine, state, operations) = (caller.machine, caller.state, caller.operations);
    let CheckedUnitEffectOperationPlan::StructuralCall {
        coordinate,
        structural_arguments,
        custody,
        result: consumer_result,
        ..
    } = operation
    else {
        return unsupported("linear result operand requires an ordinary structural call");
    };
    let argument = structural_arguments
        .get(argument_index)
        .ok_or(LoweringError::Unsupported(
            "linear result operand has no argument slot",
        ))?;
    let binding_ordinal =
        argument
            .source_structural_result_binding_ordinal()
            .ok_or(LoweringError::Unsupported(
                "linear result operand has no binding ordinal",
            ))?;
    let mut consumers = operations
        .iter()
        .enumerate()
        .filter(|(_, candidate)| *candidate == operation);
    let (consumer_index, _) = consumers.next().ok_or(LoweringError::Unsupported(
        "linear result consumer is absent",
    ))?;
    if consumers.next().is_some() {
        return unsupported("linear result consumer is ambiguous");
    }
    let source = producer(operations, binding_ordinal)?;
    let producer_index = source.operation_index;
    let producer = &operations[producer_index];
    let CheckedUnitEffectOperationPlan::StructuralCall {
        coordinate: producer_coordinate,
        target_state: producer_target_state,
        result,
        custody: produced,
        ..
    } = producer
    else {
        return unsupported("linear result operand has no ordinary producer");
    };
    if producer_index >= consumer_index
        || producer_coordinate.call_ordinal != 0
        || producer_coordinate.statement_index >= coordinate.statement_index
        || result.binding_ordinal >= consumer_result.binding_ordinal
        || source.discard
        || result.multiplicity != Multiplicity::Linear
        || parameter.multiplicity != Multiplicity::Linear
        || !argument.path.is_empty()
        || argument.access != checked_trees::CheckedStructuralAccess::Owned
        || parameter.access != argument.access
        || parameter.is_self
        || parameter.fused_service_erasure.is_some()
        || result.type_identity != argument.type_identity
        || parameter.type_identity != result.type_identity
        || parameter.qualifications != produced.result_qualifications
    {
        return unsupported("linear result operand has stale or incompatible producer custody");
    }
    super::validate_custody(checked, machine, state, producer)?;
    super::validate_custody(checked, machine, state, operation)?;
    let (_, producer_state) = crate::expression_preparation::source_custody::authored_state(
        checked,
        *producer_target_state,
    )?;
    let projected_qualifications = validation::structural_result_projected_qualifications(
        &checked.typed,
        producer_state.return_type,
    )
    .map_err(LoweringError::Unsupported)?;
    if parameter.projected_qualifications != projected_qualifications {
        return unsupported("linear result operand changes its projected qualification row");
    }
    crate::emission::call_source_custody::initializers::validate_structural(
        checked,
        machine,
        state,
        *producer_coordinate,
        result,
    )?;
    let (_, source_state) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(source_state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("linear result operand has no authored local");
    };
    // A `&mut` view local's qualifications are the referee's: the reference
    // presentation is custody, not a domain shell.
    let qualification_type = validation::reference_result_custody::mutable_borrowed_parts(
        &checked.typed,
        local.type_reference,
    )
    .map(|(referee, _)| referee)
    .unwrap_or(local.type_reference);
    if local.is_mutable
        || validation::structural_result_qualifications(&checked.typed, qualification_type)
            .map_err(LoweringError::Unsupported)?
            != produced.result_qualifications
        || validation::structural_result_projected_qualifications(
            &checked.typed,
            qualification_type,
        )
        .map_err(LoweringError::Unsupported)?
            != projected_qualifications
    {
        return unsupported("linear result local changes its producing qualification row");
    }
    let authored =
        crate::emission::call_source_custody::authored::locate_source(checked, state, *coordinate)?;
    let mut arguments = authored
        .structural_arguments
        .iter()
        .filter(|(position, _)| *position == parameter.position);
    let (_, expression) = arguments.next().ok_or(LoweringError::Unsupported(
        "linear result operand has no authored argument",
    ))?;
    if arguments.next().is_some()
        || !matches!(checked.expression_table.expression(*expression), ExpressionNode::Name(name)
        if name.symbol == local.symbol && name.head_symbol == local.symbol
            && checked.expression_table.name_path_members(name.members).len() == 1)
    {
        return unsupported("linear result operand does not name its exact producing local");
    }
    let transfers = custody
        .claim_transfers
        .iter()
        .filter(|transfer| transfer.argument_index as usize == argument_index)
        .collect::<Vec<_>>();
    // Claim identities survive the move, and each identity keeps its relative
    // path below the whole owner. Equal counts or equal aggregate types cannot
    // authorize exchanging two sibling claims between successive calls. Shared
    // custody replay above independently establishes canonical path order.
    if transfers.is_empty()
        || transfers.len() != produced.returned_claim_transfers.len()
        || transfers.len() != custody.returned_claim_transfers.len()
        || transfers
            .iter()
            .zip(&produced.returned_claim_transfers)
            .zip(&custody.returned_claim_transfers)
            .any(|((transfer, returned), continued)| {
                transfer.claim_identity != returned.caller_claim
                    || transfer.claim_identity != continued.caller_claim
                    || returned.path != continued.path
            })
    {
        return unsupported("linear result operand changes the returned input claim lineage");
    }
    for earlier in &operations[producer_index + 1..=consumer_index] {
        let arguments = match earlier {
            CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                structural_arguments,
                ..
            } => structural_arguments,
            _ => continue,
        };
        if arguments
            .iter()
            .enumerate()
            .any(|(argument_position, candidate)| {
                candidate.source_structural_result_binding_ordinal() == Some(binding_ordinal)
                    && candidate.access == checked_trees::CheckedStructuralAccess::Owned
                    && (earlier != operation || argument_position != argument_index)
            })
        {
            return unsupported("linear result operand has already been consumed");
        }
    }
    Ok(())
}

/// Rejoin a construction authored as a call argument to the call that reads
/// it: `f(Event::Insert { cents: 50 })` establishes the value at the call's
/// statement and moves it whole into the formal it was written for. The
/// construction's own source custody — the authored argument expression at
/// that formal position — is checked where the construction rejoins its
/// statement. This ties the two operations together, so the value cannot move
/// into another formal, another call, a borrowed or projected parameter, or
/// into two consumers.
fn validate_argument_construction_consumer(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    argument_index: usize,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
    source_position: u32,
) -> Result<(), LoweringError> {
    let (machine, state, operations) = (caller.machine, caller.state, caller.operations);
    let (coordinate, structural_arguments) = match operation {
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            structural_arguments,
            ..
        } => (coordinate, structural_arguments),
        _ => return unsupported("argument construction requires an ordinary or boundary call"),
    };
    let argument = structural_arguments
        .get(argument_index)
        .ok_or(LoweringError::Unsupported(
            "argument construction has no argument slot",
        ))?;
    let binding_ordinal =
        argument
            .source_structural_result_binding_ordinal()
            .ok_or(LoweringError::Unsupported(
                "argument construction has no binding ordinal",
            ))?;
    let mut consumers = operations
        .iter()
        .enumerate()
        .filter(|(_, candidate)| *candidate == operation);
    let (consumer_index, _) = consumers.next().ok_or(LoweringError::Unsupported(
        "argument construction consumer is absent",
    ))?;
    if consumers.next().is_some() {
        return unsupported("argument construction consumer is ambiguous");
    }
    let source = producer(operations, binding_ordinal)?;
    let Some(checked_trees::CheckedArrayConstructionSource::CallArgument {
        parameter_position,
        ..
    }) = source.construction_source
    else {
        return unsupported("argument operand is not its call's own construction");
    };
    if source.operation_index >= consumer_index
        || !source.precedes_consumer(*coordinate)
        || source.discard
        || parameter_position != source_position
        || !argument.path.is_empty()
        || argument.access != checked_trees::CheckedStructuralAccess::Owned
        || parameter.access != argument.access
        || parameter.is_self
        || parameter.fused_service_erasure.is_some()
        || !parameter.qualifications.is_empty()
        || source.result.multiplicity != parameter.multiplicity
        || argument.type_identity != source.result.type_identity
        || parameter.type_identity != source.result.type_identity
    {
        return unsupported("argument construction moves into another formal or custody");
    }
    super::validate_custody(checked, machine, state, operation)?;
    let owned_uses = operations
        .iter()
        .flat_map(|candidate| match candidate {
            CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                structural_arguments,
                ..
            } => structural_arguments.as_slice(),
            _ => &[],
        })
        .filter(|candidate| {
            candidate.source_structural_result_binding_ordinal() == Some(binding_ordinal)
        })
        .count();
    if owned_uses != 1 {
        return unsupported("argument construction is read by more than its own call");
    }
    Ok(())
}

struct Producer<'plan> {
    operation_index: usize,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    result: &'plan CheckedUnitStructuralResultBindingPlan,
    discard: bool,
    construction_source: Option<checked_trees::CheckedArrayConstructionSource>,
}

/// A bare reference result's retained leaf loan proves a projected owned
/// operand's subtree custody independently: `validate_custody` and the release
/// boundary replay that loan against typed evidence, and the carrier's
/// residual still dies at its own return drop. The projection must therefore
/// name declared field edges only; referent and index projections keep the
/// ordinary continuation-cleanup route.
fn loaned_record_projection(
    operation: &CheckedUnitEffectOperationPlan,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> bool {
    !argument.path.is_empty()
        && matches!(
            operation,
            CheckedUnitEffectOperationPlan::StructuralCall { custody, .. }
                if custody.reference_loan.is_valid()
        )
        && argument.path.iter().all(|segment| {
            matches!(
                segment,
                checked_trees::CheckedUnitStructuralPathSegment::Field(_)
            )
        })
}

/// Whether the loaned projections move every declared child out of an
/// anonymous carrier, leaving no residual for a dying continuation to own.
/// This mirrors the terminal frontier's `projected_moves::retires` over
/// checked type plans: fields must stay relevant and no-cleanup-shaped,
/// each structural child is moved whole or covered by deeper moved paths,
/// and leaf children need no row at all. A fully covered temporary's place
/// retires at the consuming call, so a longer operation schedule does not
/// need a separate residual continuation for it.
fn projected_moves_cover_result(
    types: &[checked_trees::CheckedUnitStructuralTypePlan],
    result_type: &str,
    moved_paths: &[&[checked_trees::CheckedUnitStructuralPathSegment]],
) -> bool {
    if moved_paths.is_empty()
        || moved_paths.iter().enumerate().any(|(index, path)| {
            path.is_empty()
                || moved_paths[..index]
                    .iter()
                    .any(|earlier| path.starts_with(earlier) || earlier.starts_with(path))
        })
    {
        return false;
    }
    projected_moves_cover_type(types, result_type, moved_paths)
}

fn projected_moves_cover_type(
    types: &[checked_trees::CheckedUnitStructuralTypePlan],
    current_type: &str,
    moved_paths: &[&[checked_trees::CheckedUnitStructuralPathSegment]],
) -> bool {
    let Some(declaration) = types
        .iter()
        .find(|declaration| declaration.identity == current_type)
    else {
        return false;
    };
    match &declaration.shape {
        checked_trees::CheckedUnitStructuralTypeShape::Record { fields } => {
            if fields.is_empty()
                || fields.iter().enumerate().any(|(index, field)| {
                    field.relevance.is_erased()
                        || !no_cleanup_residual_field(&field.field_type)
                        || fields[..index]
                            .iter()
                            .any(|earlier| earlier.identity == field.identity)
                })
            {
                return false;
            }
            let mut matched = 0_usize;
            for field in fields.iter().rev() {
                let matching = moved_paths
                    .iter()
                    .filter_map(|path| {
                        matches!(
                            path.first(),
                            Some(checked_trees::CheckedUnitStructuralPathSegment::Field(
                                identity
                            )) if identity == &field.identity
                        )
                        .then_some(&path[1..])
                    })
                    .collect::<Vec<_>>();
                matched += matching.len();
                let checked_trees::CheckedUnitStructuralFieldType::Structural { type_identity } =
                    &field.field_type
                else {
                    // A moved path may end at a no-cleanup leaf, but it may
                    // not continue through one.
                    if matching.iter().any(|tail| !tail.is_empty()) {
                        return false;
                    }
                    continue;
                };
                // An untouched structural child leaves a residual the carrier
                // must still own; covered children are moved whole or covered
                // by their own deeper moved paths.
                if matching.is_empty()
                    || !(matching.len() == 1 && matching[0].is_empty()
                        || matching.iter().all(|tail| !tail.is_empty())
                            && projected_moves_cover_type(types, type_identity, &matching))
                {
                    return false;
                }
            }
            matched == moved_paths.len()
        }
        checked_trees::CheckedUnitStructuralTypeShape::FixedArray {
            element_type_identity,
            length,
        } => {
            if *length == 0 || u128::from(*length) > moved_paths.len() as u128 {
                return false;
            }
            let mut matched = 0_usize;
            for index in (0..*length).rev() {
                let matching = moved_paths
                    .iter()
                    .filter_map(|path| {
                        matches!(
                            path.first(),
                            Some(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                                touched
                            )) if *touched == index
                        )
                        .then_some(&path[1..])
                    })
                    .collect::<Vec<_>>();
                matched += matching.len();
                if matching.is_empty()
                    || !(matching.len() == 1 && matching[0].is_empty()
                        || matching.iter().all(|tail| !tail.is_empty())
                            && projected_moves_cover_type(types, element_type_identity, &matching))
                {
                    return false;
                }
            }
            matched == moved_paths.len()
        }
        _ => false,
    }
}

/// `projected_moves`'s field admissibility at the checked level: only plain
/// no-cleanup leaves may sit beside moved subtrees. Reference and provider
/// carriers are `Structural` children, so an uncovered one still fails the
/// coverage check rather than inheriting leaf treatment.
fn no_cleanup_residual_field(field_type: &checked_trees::CheckedUnitStructuralFieldType) -> bool {
    matches!(
        field_type,
        checked_trees::CheckedUnitStructuralFieldType::Structural { .. }
            | checked_trees::CheckedUnitStructuralFieldType::BoundedInteger(_)
            | checked_trees::CheckedUnitStructuralFieldType::ByteSequence(
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
            )
            | checked_trees::CheckedUnitStructuralFieldType::PredicateByteSequence {
                carrier: checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. },
                ..
            }
            | checked_trees::CheckedUnitStructuralFieldType::Scalar(_)
    )
}

impl Producer<'_> {
    fn precedes_consumer(&self, coordinate: checked_trees::CheckedUnitCallCoordinate) -> bool {
        if let Some(checked_trees::CheckedArrayConstructionSource::CallArgument {
            call_ordinal,
            ..
        }) = self.construction_source
        {
            return self.result.statement_index == coordinate.statement_index
                && call_ordinal == coordinate.call_ordinal;
        }
        if self.coordinate.call_ordinal == 0 {
            self.result.statement_index < coordinate.statement_index
        } else {
            self.result.statement_index == coordinate.statement_index
                && coordinate.call_ordinal < self.coordinate.call_ordinal
        }
    }
}

fn producer(
    operations: &[CheckedUnitEffectOperationPlan],
    binding_ordinal: u32,
) -> Result<Producer<'_>, LoweringError> {
    let mut matches = operations
        .iter()
        .enumerate()
        .filter_map(|(operation_index, operation)| match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                discard_result_on_return,
                operand_source,
                ..
            } if result.binding_ordinal == binding_ordinal => Some(Producer {
                operation_index,
                coordinate: checked_trees::CheckedUnitCallCoordinate {
                    statement_index: result.statement_index,
                    call_ordinal: 0,
                },
                result,
                discard: *discard_result_on_return,
                construction_source: *operand_source,
            }),
            CheckedUnitEffectOperationPlan::EstablishScalarArray { source, result, .. }
                if result.binding_ordinal == binding_ordinal =>
            {
                Some(Producer {
                    operation_index,
                    coordinate: checked_trees::CheckedUnitCallCoordinate {
                        statement_index: result.statement_index,
                        call_ordinal: 0,
                    },
                    result,
                    discard: false,
                    construction_source: Some(*source),
                })
            }
            CheckedUnitEffectOperationPlan::EstablishByteSequenceLiteral { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. }
                if result.binding_ordinal == binding_ordinal =>
            {
                Some(Producer {
                    operation_index,
                    coordinate: checked_trees::CheckedUnitCallCoordinate {
                        statement_index: result.statement_index,
                        call_ordinal: 0,
                    },
                    result,
                    discard: false,
                    construction_source: None,
                })
            }
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                result,
                discard_result_on_return,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                result,
                discard_result_on_return,
                ..
            } if result.binding_ordinal == binding_ordinal => Some(Producer {
                operation_index,
                coordinate: *coordinate,
                result,
                discard: *discard_result_on_return,
                construction_source: None,
            }),
            _ => None,
        });
    let result = matches.next().ok_or(LoweringError::Unsupported(
        "Unit structural result argument has no exact producer binding",
    ))?;
    if matches.next().is_some() {
        return unsupported("Unit structural result argument has ambiguous producer bindings");
    }
    Ok(result)
}

/// An immutable local's shared view type supplies its access; naming it does
/// not move an owned result. The initializer's producer replays separately.
fn shared_view_binding(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Result<bool, LoweringError> {
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(authored.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return Ok(false);
    };
    let Some(carrier) =
        crate::expression_preparation::source_custody::structural::shared_slice_view_referent(
            checked,
            local.type_reference,
        )
    else {
        return Ok(false);
    };
    Ok(!local.is_mutable
        && local.symbol.is_valid()
        && result.multiplicity == Multiplicity::Unrestricted
        && checked.normalized_type_identity(carrier).as_str() == result.type_identity)
}

pub(crate) fn validate_usage(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let producer = producer(&caller.operations, result.binding_ordinal)?;
    if producer.result != result {
        return unsupported("Unit structural result binding disagrees with its producer");
    }
    let mut consumed = false;
    let mut disposed = false;
    let mut projected_paths = Vec::<&[checked_trees::CheckedUnitStructuralPathSegment]>::new();
    // Every projected move under a still return-dropped carrier must carry
    // its own proven leaf loan; a single unloaned projection revives the
    // ordinary residual-continuation requirement.
    let mut projected_residual_loaned = true;
    for (operation_index, operation) in
        caller
            .operations
            .iter()
            .enumerate()
            .flat_map(|(index, operation)| {
                operation
                    .with_value_calls()
                    .map(move |operation| (index, operation))
            })
    {
        if let CheckedUnitEffectOperationPlan::ReleaseReference {
            binding_ordinal, ..
        } = operation
            && *binding_ordinal == result.binding_ordinal
        {
            super::super::reference_results::validate_releases(checked, caller)?;
            if consumed
                || disposed
                || operation_index <= producer.operation_index
                || producer.discard
            {
                return unsupported("reference carrier is released after losing custody");
            }
            disposed = true;
            continue;
        }
        if let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: selected, ..
        } = operation
            && let Some((_, receipt)) = checked
                .facts
                .flow
                .ownership
                .owned_selection_at(caller.state, selected.statement_index)
        {
            // A selection carries both the chosen source and its complement.
            // This replaces each candidate's old unconditional death, without
            // claiming that every arm moves that candidate into the result.
            crate::expression_preparation::source_custody::structural::validate(
                checked,
                caller.machine,
                caller.state,
                operation,
            )?;
            let sources = checked
                .facts
                .flow
                .ownership
                .selection_sources
                .span(receipt.sources)
                .ok_or(LoweringError::Unsupported(
                    "selected result has stale source custody",
                ))?;
            // Only statement-established sources name a result binding:
            // parameter sources carry their authored position in
            // `statement_ordinal`, which is not a statement index.
            let result_local_symbol = checked
                .statement_table
                .statements(
                    crate::expression_preparation::source_custody::authored_state(
                        checked,
                        caller.state,
                    )?
                    .1
                    .statement_nodes,
                )
                .get(result.statement_index as usize)
                .and_then(|statement| match statement {
                    StatementNode::LocalData(local) => Some(local.symbol),
                    _ => None,
                });
            if let Some(source_ordinal) = sources.iter().position(|source| {
                (matches!(
                    source.provenance,
                    language_semantics::PermissionProvenance::Established {
                        source: language_semantics::PermissionEventSource::Statement {
                            statement_index,
                        },
                        ..
                    } if statement_index == result.statement_index as usize
                ) || (source.origin_selection.is_valid()
                    && source.statement_ordinal as usize == result.statement_index as usize))
                    && Some(source.symbol) == result_local_symbol
            }) {
                let source_handle = arena::Handle::from_parts(
                    receipt.sources.start().arena_index() + source_ordinal as u32,
                    receipt.sources.start().generation(),
                );
                // A whole-local move carries the source's own type into the
                // result. A source moved only through projected children keeps
                // its root type here; the projected leaf identity was replayed
                // against the recorded transfer path by source custody above.
                let whole = checked
                    .facts
                    .flow
                    .ownership
                    .selection_transfers
                    .span(receipt.transfers)
                    .is_some_and(|transfers| {
                        transfers.iter().any(|transfer| {
                            transfer.source == source_handle
                                && checked
                                    .facts
                                    .flow
                                    .ownership
                                    .segments
                                    .span_or_empty(transfer.path)
                                    .is_empty()
                        })
                    });
                if consumed
                    || disposed
                    || !projected_paths.is_empty()
                    || operation_index <= producer.operation_index
                    || selected.statement_index <= result.statement_index
                    || result.multiplicity != Multiplicity::Affine
                    || (whole && selected.type_identity != result.type_identity)
                {
                    return unsupported("selected result reuses an unavailable structural source");
                }
                consumed = true;
            }
        }
        if let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            value,
            result: destination,
            ..
        } = operation
            && checked
                .facts
                .flow
                .ownership
                .owned_selection_at(caller.state, destination.statement_index)
                .is_none()
        {
            let (_, state) = crate::expression_preparation::source_custody::authored_state(
                checked,
                caller.state,
            )?;
            if let Some(StatementNode::LocalData(local)) = checked
                .statement_table
                .statements(state.statement_nodes)
                .get(result.statement_index as usize)
            {
                let values = &checked.facts.values.structural_values;
                let mut pending = vec![*value];
                let mut visited = Vec::new();
                while let Some(value) = pending.pop() {
                    if !values.nodes.is_valid(value) || visited.contains(&value) {
                        return unsupported(
                            "record consumption has stale or repeated value custody",
                        );
                    }
                    visited.push(value);
                    let node = values.nodes.get(value);
                    match &node.kind {
                        checked_trees::CheckedStructuralValueKind::Place(argument)
                            if argument.source == (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: local.symbol }) => {
                            if !matches!(checked.expression_table.expression(node.expression), ExpressionNode::Name(name) if name.symbol == local.symbol && name.head_symbol == local.symbol && name.members.count() == 1)
                                || argument.access != checked_trees::CheckedStructuralAccess::Owned
                                || !argument.path.is_empty() || argument.type_identity != result.type_identity
                                || consumed || disposed || !projected_paths.is_empty()
                                || operation_index <= producer.operation_index
                                || destination.statement_index <= result.statement_index {
                                return unsupported("record child lost its exact prior owned local");
                            }
                            consumed = result.multiplicity != Multiplicity::Unrestricted;
                        }
                        checked_trees::CheckedStructuralValueKind::Record { fields, .. }
                        | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                            for field in values.record_fields.span(*fields).ok_or(LoweringError::Unsupported("record consumption field roster missing"))? {
                                if let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) = field.value { pending.push(value); }
                            }
                        }
                        checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => pending.extend(values.dispatch_arms.span(*arms).ok_or(LoweringError::Unsupported("record consumption arm roster missing"))?.iter().map(|arm| arm.value)),
                        _ => {}
                    }
                }
            }
        }
        if let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            coordinate,
            affine_discards,
        } = operation
        {
            let source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            };
            let immediately_discarded = operation_index.checked_sub(1)
                == Some(producer.operation_index)
                && matches!(&caller.operations[producer.operation_index],
                    CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate: call, .. }
                    | CheckedUnitEffectOperationPlan::StructuralCall { coordinate: call, .. }
                        if call == coordinate && call.call_ordinal == 0);
            if immediately_discarded {
                if consumed
                    || disposed
                    || producer.discard
                    || !projected_paths.is_empty()
                    || !matches!(affine_discards.as_slice(), [discard]
                        if discard.source == source && discard.path.is_empty()
                            && discard.type_identity == result.type_identity)
                {
                    return unsupported(
                        "discarded boundary result lost its immediate whole cleanup",
                    );
                }
                disposed = true;
                continue;
            }
            // Empty complements still end the temporary's lifetime. Its exact
            // consumer identifies that owner even when no residual row exists.
            let owns_continuation = operation_index
                .checked_sub(1)
                .and_then(|previous| caller.operations.get(previous))
                .is_some_and(|operation| {
                    matches!(operation,
                    CheckedUnitEffectOperationPlan::CallUnit { structural_arguments, .. }
                        if structural_arguments.iter().any(|argument| argument.source == source))
                });
            if !owns_continuation {
                if affine_discards
                    .iter()
                    .any(|discard| discard.source == source)
                {
                    return unsupported("call continuation substituted its result owner");
                }
                continue;
            }
            let (_, state) = crate::expression_preparation::source_custody::authored_state(
                checked,
                caller.state,
            )?;
            let reference_record_end = validation::reference_result_custody::local_record_loans(
                &checked.typed,
                &checked.facts,
                caller.machine,
                state,
                result.statement_index,
            )
            .is_some_and(|loans| {
                !loans.is_empty()
                    && loans.iter().all(|(_, loan)| {
                        validation::reference_result_custody::release_statement(
                            &checked.facts,
                            caller.machine,
                            caller.state,
                            *loan,
                        ) == coordinate.statement_index.checked_add(1)
                    })
            });
            if consumed
                || disposed
                || producer.discard
                || (!reference_record_end
                    && (producer.coordinate.call_ordinal == 0
                        || producer.coordinate.statement_index != coordinate.statement_index))
                || operation_index <= producer.operation_index
                || if projected_paths.is_empty() {
                    !matches!(affine_discards.as_slice(), [discard]
                        if discard.source == source && discard.path.is_empty() && discard.type_identity == result.type_identity)
                } else {
                    // One continuation may carry several temporaries; this
                    // owner's rows must all be nonempty residual paths. An
                    // empty complement leaves no row at all.
                    affine_discards
                        .iter()
                        .any(|discard| discard.source == source && discard.path.is_empty())
                }
            {
                return unsupported("call continuation does not own this result remainder");
            }
            disposed = true;
            continue;
        }
        // A field replacement's store moves the whole produced value into the
        // hole its move-out opened: the value's one consuming use. A copy
        // value is copied there and keeps owing nothing.
        if let CheckedUnitEffectOperationPlan::StoreStructuralField { value, .. } = operation
            && value.source_structural_result_binding_ordinal() == Some(result.binding_ordinal)
        {
            if consumed
                || disposed
                || operation_index <= producer.operation_index
                || !value.path.is_empty()
                || value.access != checked_trees::CheckedStructuralAccess::Owned
                || value.type_identity != result.type_identity
            {
                return unsupported(
                    "Unit structural result is consumed before production or twice",
                );
            }
            consumed = result.multiplicity != Multiplicity::Unrestricted;
            continue;
        }
        let (CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            structural_arguments,
            ..
        }) = operation
        else {
            continue;
        };
        for argument in structural_arguments {
            if argument.source_structural_result_binding_ordinal() != Some(result.binding_ordinal) {
                continue;
            }
            let source_order = producer.precedes_consumer(*coordinate);
            if consumed || disposed || operation_index <= producer.operation_index || !source_order
            {
                return unsupported(
                    "Unit structural result is consumed before production or twice",
                );
            }
            if matches!(
                argument.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
                    | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
            ) && (matches!(
                operation,
                CheckedUnitEffectOperationPlan::CallUnit { .. }
                    | CheckedUnitEffectOperationPlan::ScalarCall { .. }
                    | CheckedUnitEffectOperationPlan::StructuralCall { .. }
            ) || matches!(
                operation,
                CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                    | CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
                    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
            )) && producer.coordinate.call_ordinal == 0
            {
                if projected_paths.iter().any(|earlier| {
                    earlier.starts_with(&argument.path) || argument.path.starts_with(earlier)
                }) {
                    return unsupported("local receiver borrows an already moved field");
                }
                // Exact declaration/path/access and the captured loan are
                // independently rejoined by validate_consumer. A loan keeps
                // this whole result's original ownership and cleanup debt.
                continue;
            }
            if result.multiplicity == Multiplicity::Unrestricted {
                // Copies retain their source after either call family. Exact
                // boundary type eligibility is rejoined by validate_consumer;
                // this walk checks the produced value's use and lifetime.
                if !matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::CallUnit { .. }
                        | CheckedUnitEffectOperationPlan::ScalarCall { .. }
                        | CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                ) || argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || !argument.path.is_empty()
                    || argument.type_identity != result.type_identity
                {
                    return unsupported(
                        "unrestricted array result requires a whole owned call argument",
                    );
                }
                continue;
            }
            if !argument.path.is_empty() {
                let loaned = loaned_record_projection(operation, argument);
                if !(matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. }) || loaned)
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || result.multiplicity != Multiplicity::Affine
                    || projected_paths.iter().any(|earlier| {
                        earlier.starts_with(&argument.path) || argument.path.starts_with(earlier)
                    })
                {
                    return unsupported(
                        "Unit result projection overlaps or lacks owned call custody",
                    );
                }
                projected_paths.push(&argument.path);
                projected_residual_loaned &= loaned;
                continue;
            }
            if !projected_paths.is_empty()
                || !matches!(
                    argument.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                )
                || argument.type_identity != result.type_identity
                || result.multiplicity != Multiplicity::Affine
            {
                return unsupported(
                    "Unit structural result use is not a whole affine move or shared borrow",
                );
            }
            if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                && producer.coordinate.call_ordinal != 0
                && (producer.coordinate.call_ordinal != 1
                    || producer.discard
                    || coordinate.call_ordinal != 0
                    || !matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { scalar_arguments, structural_arguments, .. }
                        if scalar_arguments.is_empty() && structural_arguments.len() == 1)
                    || !matches!(caller.operations.get(operation_index + 1),
                        Some(CheckedUnitEffectOperationPlan::CallContinuationCleanup { coordinate: cleanup, affine_discards })
                            if cleanup == coordinate && affine_discards.len() == 1
                                && affine_discards[0].source == argument.source))
            {
                return unsupported("anonymous shared result has no dying Unit call continuation");
            }
            consumed = argument.access == checked_trees::CheckedStructuralAccess::Owned;
        }
    }
    if let Some(returned) = &caller.structural_result
        && returned.source
            == (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            })
    {
        if consumed
            || disposed
            || !projected_paths.is_empty()
            || returned.type_identity != result.type_identity
            || returned.multiplicity != result.multiplicity
        {
            return unsupported("structural completion reuses or changes its owned result");
        }
        consumed = result.multiplicity != Multiplicity::Unrestricted;
    }
    // Unrestricted values carry no disposal debt. Argument transport is still
    // checked above; completing with a result is rejoined to source separately.
    if result.multiplicity == Multiplicity::Unrestricted {
        return if producer.discard || consumed || disposed || !projected_paths.is_empty() {
            unsupported("unrestricted result acquired affine transfer or disposal custody")
        } else {
            Ok(())
        };
    }
    if (projected_paths.is_empty() && producer.discard == (consumed || disposed))
        || (!projected_paths.is_empty() && producer.discard && !projected_residual_loaned)
        || (producer.coordinate.call_ordinal != 0
            && !consumed
            && projected_paths.is_empty()
            && !disposed)
        || (producer.coordinate.call_ordinal != 0
            && !projected_paths.is_empty()
            && !disposed
            // A loaned projection may carry the whole carrier: when every
            // structural child is moved out, no residual remains for a
            // continuation to own, and the terminal frontier retires the
            // empty shell at the consuming call.
            && !(projected_residual_loaned
                && projected_moves_cover_result(
                    &checked.facts.flow.terminal_unit_effects.structural_types,
                    &result.type_identity,
                    &projected_paths,
                ))
            && !matches!(
                caller.operations.as_slice(),
                [_, _, CheckedUnitEffectOperationPlan::Complete { .. }]
            ))
    {
        return unsupported(
            "Unit structural result cleanup disagrees with its final consuming use",
        );
    }
    Ok(())
}

pub(crate) fn validate_consumer(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    target_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    target_entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
) -> Result<(), LoweringError> {
    let (coordinate, target_machine, target_state, structural_arguments, claim_transfers) =
        match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                claim_transfers,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                claim_transfers,
                ..
            } => (
                coordinate,
                *target_machine,
                *target_state,
                structural_arguments,
                claim_transfers.as_slice(),
            ),
            // A structural call keeps its transfers in its custody record.
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                custody,
                ..
            } => (
                coordinate,
                *target_machine,
                *target_state,
                structural_arguments,
                custody.claim_transfers.as_slice(),
            ),
            CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                completion_receipts,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                completion_receipts,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                completion_receipts,
                ..
            } => (
                coordinate,
                *target_machine,
                *target_state,
                structural_arguments,
                completion_receipts.as_slice(),
            ),
            _ => {
                return unsupported("structural result use requires an ordinary or boundary call");
            }
        };
    // A borrowed receiver the callee leaves ambient on its attachment carrier
    // still occupies its authored slot among the call's structural arguments;
    // it owns no forwarded formal, so the walk skips that position. The gap is
    // real only when the arguments actually carry one extra slot — a roster
    // that forwards self keeps argument and parameter arity equal.
    let ambient_receiver =
        crate::expression_preparation::source_custody::authored_state(checked, target_state)
            .ok()
            .and_then(|(_, callee_state)| {
                checked
                    .state_parameters(callee_state)
                    .iter()
                    .position(|parameter| {
                        parameter.is_self
                            && matches!(
                                checked
                                    .type_reference_table
                                    .type_reference(parameter.type_reference),
                                TypeReferenceNode::Reference { .. }
                            )
                    })
                    .filter(|_| !target_parameters.iter().any(|parameter| parameter.is_self))
            })
            .filter(|_| structural_arguments.len() == target_parameters.len() + 1);
    if structural_arguments.len()
        != target_parameters.len() + usize::from(ambient_receiver.is_some())
    {
        return unsupported("Unit structural argument arity disagrees with its target");
    }
    let operation_index = caller
        .operations
        .iter()
        .position(|candidate| {
            candidate
                .with_value_calls()
                .any(|candidate| candidate == operation)
        })
        .ok_or(LoweringError::Unsupported(
            "structural result consumer has no operation position",
        ))?;
    let authored = crate::emission::call_source_custody::authored::locate_source(
        checked,
        caller.state,
        *coordinate,
    )?;
    let authored_nested = authored
        .structural_arguments
        .iter()
        .any(|(_, expression)| expression_producer(checked, *expression).is_some());
    validate_nested_execution_order(checked, caller, coordinate.statement_index, authored_nested)?;
    let (source_machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller.state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let signature = crate::emission::call_source_custody::authored::target_signature(
        checked,
        caller.machine,
        authored.source_target,
    )?;
    // Boundary plans omit the provider-selecting receiver from ABI positions.
    // Operand constructions retain authored positions, including that receiver.
    // Reuse the source partition that already joins literal boundary arguments.
    let boundary_source_positions = if authored.boundary {
        Some(
            crate::emission::call_source_custody::literal_arguments::structural_positions(
                checked,
                &signature,
                structural_arguments.len(),
            )?,
        )
    } else {
        None
    };
    let mut forwarded_parameters = target_parameters.iter();
    let mut parameter_position = 0usize;
    // `index` keeps the argument's authored call position (claim transfers,
    // boundary positions, consumer callbacks); `parameter_index` is the
    // forwarded formal's position in the callee roster (entry claims).
    for (index, argument) in structural_arguments.iter().enumerate() {
        if ambient_receiver == Some(index) {
            continue;
        }
        let parameter = forwarded_parameters
            .next()
            .ok_or(LoweringError::Unsupported(
                "structural result consumer lost a target parameter",
            ))?;
        let parameter_index = parameter_position;
        parameter_position += 1;
        let source_position = boundary_source_positions
            .as_ref()
            .map(|positions| positions[index])
            .unwrap_or(parameter.position);
        let boundary_array_argument = authored.boundary
            && signature
                .parameters
                .get(source_position as usize)
                .is_some_and(|source| {
                    validation::is_closed_primitive_array_type(
                        &checked.typed,
                        source.type_reference,
                    )
                });
        let expression = authored
            .structural_arguments
            .iter()
            .find_map(|(position, expression)| {
                (*position == source_position).then_some(*expression)
            });
        let binding_ordinal = argument.source_structural_result_binding_ordinal();
        if let Some(expression) = expression
            && super::super::reference_results::validate_consumer(
                checked,
                caller,
                *coordinate,
                argument,
                parameter,
                expression,
            )?
        {
            if authored.boundary
                || parameter.is_self
                || target_entry_claims
                    .iter()
                    .any(|claim| claim.parameter_index as usize == parameter_index)
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
            {
                return unsupported(
                    "reference result consumer acquired unrelated boundary or claim custody",
                );
            }
            continue;
        }
        if parameter.is_self
            && binding_ordinal.is_some()
            && matches!(
                argument.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
            )
        {
            crate::emission::call_source_custody::projected_receivers::validate(
                checked,
                caller.machine,
                caller.state,
                caller.operations,
                caller.structural_parameters,
                operation,
                target_parameters,
            )?;
            if target_entry_claims
                .iter()
                .any(|claim| claim.parameter_index as usize == parameter_index)
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
            {
                return unsupported("plain local receiver acquired transferred claims");
            }
            continue;
        }
        if let Some(source_index) = argument.source_parameter_index() {
            let source = caller
                .structural_parameters
                .get(source_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural argument source parameter is absent",
                ))?;
            let source_parameter = checked
                .state_parameters(state)
                .get(source.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural argument source position is absent",
                ))?;
            // Whole array parameters retain the same owned payload at ordinary
            // and boundary calls. The boundary's authored-position partition
            // above omits only its service receiver; it does not change which
            // caller parameter supplies this value. Rejoin that exact source
            // and custody here, then use shared structural argument emission.
            if validation::is_closed_primitive_array_type(&checked.typed, source_parameter.type_reference)
                && (!argument.path.is_empty()
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || source.access != argument.access
                    || parameter.access != argument.access
                    || source.multiplicity != Multiplicity::Unrestricted
                    || parameter.multiplicity != Multiplicity::Unrestricted
                    || source.type_identity != argument.type_identity
                    || parameter.type_identity != argument.type_identity
                    || !source.qualifications.is_empty()
                    || !parameter.qualifications.is_empty()
                    || source.fused_service_erasure.is_some()
                    || parameter.fused_service_erasure.is_some()
                    || source_parameter.is_self
                    || parameter.is_self
                    || !expression.is_some_and(|expression| matches!(
                        checked.expression_table.expression(expression), ExpressionNode::Name(name)
                            if name.symbol == source_parameter.symbol
                                && name.head_symbol == source_parameter.symbol
                                && checked.expression_table.name_path_members(name.members).len() == 1))
                    || target_entry_claims.iter().any(|claim| claim.parameter_index as usize == parameter_index)
                    || claim_transfers.iter().any(|transfer| transfer.argument_index as usize == index))
            {
                return unsupported("primitive array parameter requires exact whole owned transport");
            }
        }
        if binding_ordinal.is_none()
            && expression
                .is_some_and(|expression| expression_producer(checked, expression).is_some())
        {
            return unsupported(
                "nested structural argument has no expression-owned producer binding",
            );
        }
        validate_operand_producers(
            checked,
            caller,
            coordinate,
            &authored,
            source_machine,
            statements,
            argument,
            source_position,
            expression,
            binding_ordinal,
        )?;
        let Some(binding_ordinal) = binding_ordinal else {
            continue;
        };
        let producer = producer(caller.operations, binding_ordinal)?;
        let result = producer.result;
        let source_order = producer.precedes_consumer(*coordinate);
        let shared_view_argument = shared_view_binding(checked, caller.state, result)?
            && argument.path.is_empty()
            && argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow;
        // A whole linear result moved into this call transfers its producer's
        // live claim: the call's completion receipts publish that claim at
        // this argument rather than leaving the operand claim-free. Target
        // qualifications stay admissible — checking already joined the
        // bound local's domains to the formal's exact requirement.
        let linear_result_move = result.multiplicity == Multiplicity::Linear
            && argument.path.is_empty()
            && argument.access == checked_trees::CheckedStructuralAccess::Owned
            && argument.type_identity == result.type_identity
            && parameter.access == checked_trees::CheckedStructuralAccess::Owned
            && parameter.multiplicity == Multiplicity::Linear
            && parameter.type_identity == argument.type_identity
            && !parameter.is_self
            && parameter.fused_service_erasure.is_none()
            && claim_transfers
                .iter()
                .any(|transfer| transfer.argument_index as usize == index);
        // A claim-carrying `self` formal is the terminal consumer instead:
        // it carries no completion receipt, so the callee's entry-claim plan
        // and the caller's consume event must jointly prove that exact
        // result custody moves into it.
        if parameter.is_self {
            claimed_result_consumer(
                checked,
                caller,
                operation,
                operation_index,
                *coordinate,
                authored.source_target,
                target_machine,
                target_state,
                index,
                argument,
                parameter,
                &producer,
                target_entry_claims,
                claim_transfers,
            )?;
            continue;
        }
        // A construction authored as this call's own argument moves whole
        // into the formal it was written for, and into no other consumer.
        if matches!(
            producer.construction_source,
            Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. })
        ) {
            validate_argument_construction_consumer(
                checked,
                caller,
                operation,
                index,
                parameter,
                source_position,
            )?;
        }
        // A linear result chained into the next structural call keeps its
        // exact returned claim lineage across both calls.
        if linear_result_move
            && matches!(
                operation,
                CheckedUnitEffectOperationPlan::StructuralCall { .. }
            )
        {
            validate_linear_result_consumer(checked, caller, operation, index, parameter)?;
        }
        if (producer.discard
            && argument.access == checked_trees::CheckedStructuralAccess::Owned
            && !loaned_record_projection(operation, argument))
            || producer.operation_index >= operation_index
            || !source_order
            || matches!(operation, CheckedUnitEffectOperationPlan::StructuralCall { result: consumer, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result: consumer, .. }
                if result.binding_ordinal >= consumer.binding_ordinal)
        {
            return unsupported("Unit structural result argument has invalid claim-free custody");
        }
        // Whole local arrays lend their backing without transferring ownership.
        // The source/ordinal join above identifies the declaration; this check
        // independently rejoins its loan and fixed-array/slice presentation.
        if whole_local_array_loan(
            checked,
            caller,
            *coordinate,
            authored.source_target,
            result,
            argument,
            signature
                .parameters
                .get(source_position as usize)
                .ok_or(LoweringError::Unsupported(
                    "local array loan has no authored formal",
                ))?
                .type_reference,
        )? {
            if argument.access != parameter.access
                || parameter.type_identity != argument.type_identity
                || parameter.multiplicity != Multiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || parameter.fused_service_erasure.is_some()
                || target_entry_claims
                    .iter()
                    .any(|claim| claim.parameter_index as usize == parameter_index)
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
            {
                return unsupported("local array loan changes the target custody");
            }
            continue;
        }
        // A live result binding may lend one of its record's leaves, fixed
        // elements, or its whole extent through an exclusive borrow: the
        // binding is not consumed, the projected path (empty for a
        // whole-place loan) is already rejoined against the authored operand
        // above, and the borrowed formal carries no claims.
        let scalar_leaf_borrow = matches!(
            result.multiplicity,
            Multiplicity::Affine | Multiplicity::Unrestricted
        ) && matches!(
            operation,
            CheckedUnitEffectOperationPlan::CallUnit { .. }
        ) && argument.path.iter().all(|segment| {
            matches!(
                segment,
                checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                    | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                    | checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange { .. }
            )
        }) && matches!(
            argument.access,
            checked_trees::CheckedStructuralAccess::MutableBorrow
                | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
        ) && argument.access == parameter.access
            && !parameter.is_self
            && parameter.type_identity == argument.type_identity
            && parameter.multiplicity == Multiplicity::Unrestricted
            && parameter.qualifications.is_empty()
            && parameter.fused_service_erasure.is_none()
            && target_entry_claims
                .iter()
                .all(|claim| claim.parameter_index as usize != parameter_index)
            && claim_transfers
                .iter()
                .all(|transfer| transfer.argument_index as usize != index);
        if !linear_result_move
            && !scalar_leaf_borrow
            && (!matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) || (result.multiplicity == Multiplicity::Unrestricted
                && (!matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::CallUnit { .. }
                        | CheckedUnitEffectOperationPlan::ScalarCall { .. }
                        | CheckedUnitEffectOperationPlan::StructuralCall { .. }
                ) && !boundary_array_argument
                    && !shared_view_argument
                    || !argument.path.is_empty()
                    || !matches!(
                        argument.access,
                        checked_trees::CheckedStructuralAccess::Owned
                            | checked_trees::CheckedStructuralAccess::SharedBorrow
                    )))
                || (argument.path.is_empty() && argument.type_identity != result.type_identity)
                || parameter.type_identity != argument.type_identity
                || (!argument.path.is_empty()
                    && (!(matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
                        || loaned_record_projection(operation, argument))
                        || argument.access != checked_trees::CheckedStructuralAccess::Owned))
                || !matches!(
                    argument.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                )
                || argument.access != parameter.access
                || parameter.multiplicity
                    != if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                        Multiplicity::Unrestricted
                    } else {
                        result.multiplicity
                    }
                || !parameter.qualifications.is_empty()
                || parameter.fused_service_erasure.is_some()
                || target_entry_claims
                    .iter()
                    .any(|claim| claim.parameter_index as usize == parameter_index)
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index))
        {
            return unsupported("Unit structural result argument has invalid claim-free custody");
        }
    }
    Ok(())
}

/// Reconstruct a whole local array loan independently of its argument plan.
fn whole_local_array_loan(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    target: symbols::SymbolHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    formal: checked_trees::types::TypeReferenceHandle,
) -> Result<bool, LoweringError> {
    use checked_trees::CheckedStructuralAccess;
    if !argument.path.is_empty()
        || !matches!(
            argument.access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        )
        || result.multiplicity != Multiplicity::Unrestricted
    {
        return Ok(false);
    }
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller.state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return Ok(false);
    };
    let TypeReferenceNode::FixedArray { element_type, .. } = checked
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return Ok(false);
    };
    if !validation::is_closed_primitive_array_type(&checked.typed, local.type_reference) {
        return Ok(false);
    }
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = checked.type_reference_table.type_reference(formal)
    else {
        return unsupported("local array loan has no reference formal");
    };
    let expected_access = match access {
        language_core::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
        language_core::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        _ => return unsupported("local array loan has unsupported formal access"),
    };
    let presentation_matches = match checked.type_reference_table.type_reference(*referee) {
        TypeReferenceNode::Slice {
            element_type: target_element,
        } => {
            checked.normalized_type_identity(*element_type)
                == checked.normalized_type_identity(*target_element)
        }
        TypeReferenceNode::FixedArray { .. } => {
            checked.normalized_type_identity(local.type_reference)
                == checked.normalized_type_identity(*referee)
        }
        _ => false,
    };
    // Bounded byte views retain the reference shell in their carrier identity;
    // slices and ordinary structural borrows identify the referent instead.
    let presentation_type = if matches!(
        checked.type_reference_table.type_reference(*referee),
        TypeReferenceNode::FixedArray { .. }
    ) && checked.primitive_type_reference(*element_type)
        == Some(checked_trees::types::PrimitiveType::U8)
    {
        formal
    } else {
        *referee
    };
    if !presentation_matches
        || argument.access != expected_access
        || (expected_access == CheckedStructuralAccess::MutableBorrow && !local.is_mutable)
        || checked.normalized_type_identity(presentation_type).as_str() != argument.type_identity
        || checked
            .normalized_type_identity(local.type_reference)
            .as_str()
            != result.type_identity
    {
        return unsupported("local array loan changes backing, presentation, or access");
    }
    let borrow = &checked.facts.borrow;
    let state = borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == caller.machine && state.state_symbol == caller.state)
        .ok_or(LoweringError::Unsupported(
            "local array loan has no borrow state",
        ))?;
    let mut calls = borrow
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|call| {
            call.statement_index == coordinate.statement_index as usize
                && call.call_ordinal == coordinate.call_ordinal as usize
                && call.target_symbol == target
        });
    let call = calls.next().ok_or(LoweringError::Unsupported(
        "local array loan has no captured call",
    ))?;
    if calls.next().is_some() {
        return unsupported("local array loan has ambiguous captured calls");
    }
    let mut accesses = borrow
        .argument_accesses
        .span_or_empty(call.accesses)
        .iter()
        .filter(|access| {
            access.root_symbol == local.symbol && borrow.access_segments(access).is_empty()
        })
        .peekable();
    let expected_kind = if expected_access == CheckedStructuralAccess::SharedBorrow {
        checked_trees::BorrowAccessKind::Read
    } else {
        checked_trees::BorrowAccessKind::Mutable
    };
    if accesses.peek().is_none() || accesses.any(|access| access.kind != expected_kind) {
        return unsupported("local array loan differs from captured argument access");
    }
    Ok(true)
}

/// Check one structural argument against every producer the caller plans,
/// in both directions: a construction, call result or local names this
/// argument's result binding exactly when the authored operand is that
/// producer, and the named producer's custody and access agree with the
/// formal. An authored result therefore cannot be replaced by a same-typed
/// parameter or construction-local plan.
fn validate_operand_producers(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    coordinate: &checked_trees::CheckedUnitCallCoordinate,
    authored: &crate::emission::call_source_custody::authored::AuthoredCall,
    source_machine: &checked_trees::machine::Machine,
    statements: &[StatementNode],
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    source_position: u32,
    expression: Option<checked_trees::expression::ExpressionHandle>,
    binding_ordinal: Option<u32>,
) -> Result<(), LoweringError> {
    for candidate in caller.operations {
        if let CheckedUnitEffectOperationPlan::EstablishScalarArray {
            source:
                source @ checked_trees::CheckedArrayConstructionSource::CallArgument {
                    call_ordinal,
                    parameter_position,
                },
            result,
            ..
        } = candidate
        {
            // A literal has no local symbol. Its call occurrence and formal
            // position own the constructor even when it has no leaf values.
            let (source_expression, source_type) =
                crate::expression_preparation::source_custody::array_sources::construction_expression(
                    checked,
                    caller.machine,
                    caller.state,
                    result.statement_index,
                    *source,
                )
                .ok_or(LoweringError::Unsupported(
                    "array argument constructor lost its authored occurrence",
                ))?;
            let names_result = result.statement_index == coordinate.statement_index
                && *call_ordinal == coordinate.call_ordinal
                && *parameter_position == source_position
                && expression == Some(source_expression);
            if names_result != (binding_ordinal == Some(result.binding_ordinal)) {
                return unsupported(
                    "array argument does not rejoin its exact constructor occurrence",
                );
            }
            if names_result
                && (argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || !argument.path.is_empty()
                    || result.multiplicity != Multiplicity::Unrestricted
                    || !validation::is_closed_primitive_array_type(&checked.typed, source_type)
                    || checked
                        .typed
                        .normalized_type_identity(source_type)
                        .into_string()
                        != result.type_identity)
            {
                return unsupported("array argument constructor lost whole owned custody");
            }
            continue;
        }
        if let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            operand_source:
                Some(checked_trees::CheckedArrayConstructionSource::CallArgument {
                    call_ordinal,
                    parameter_position,
                }),
            result,
            value,
            ..
        } = candidate
        {
            // A construction or copied-place operand has no local symbol;
            // its call occurrence and authored formal position own the
            // constructor, exactly like a literal array operand.
            if !checked
                .facts
                .values
                .structural_values
                .nodes
                .is_valid(*value)
            {
                return unsupported("case argument operand has a stale value handle");
            }
            let node = checked.facts.values.structural_values.nodes.get(*value);
            let (construction_expression, construction_type) = match &node.kind {
                checked_trees::CheckedStructuralValueKind::Case(constructor) => {
                    (constructor.expression, constructor.type_reference)
                }
                checked_trees::CheckedStructuralValueKind::Record { .. }
                | checked_trees::CheckedStructuralValueKind::StructuralCase { .. }
                | checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. } => (
                    node.expression,
                    checked
                        .facts
                        .values
                        .structural_values
                        .root_for_expression(caller.state, result.statement_index, node.expression)
                        .ok_or(LoweringError::Unsupported(
                            "record argument operand lost its checked root",
                        ))?
                        .type_reference,
                ),
                _ => {
                    return unsupported(
                        "structural argument operand is not a construction or copy",
                    );
                }
            };
            let names_result = result.statement_index == coordinate.statement_index
                && *call_ordinal == coordinate.call_ordinal
                && *parameter_position == source_position
                && expression == Some(construction_expression);
            if names_result != (binding_ordinal == Some(result.binding_ordinal)) {
                return unsupported(
                    "case argument does not rejoin its exact constructor occurrence",
                );
            }
            // The call takes the fresh value whole: unrestricted or affine
            // plain contents (numeric domains allowed) need no cleanup
            // beyond the transfer itself; linear values keep claim rules.
            let copied_array = result.multiplicity == Multiplicity::Unrestricted
                && validation::is_closed_primitive_array_type(&checked.typed, construction_type);
            let whole_owned_contents = copied_array
                || result.multiplicity != Multiplicity::Linear
                    && validation::has_plain_owned_contents_with_numeric_constraints(
                        &checked.typed,
                        construction_type,
                    );
            if names_result
                && ((authored.boundary && !copied_array)
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || !argument.path.is_empty()
                    || !whole_owned_contents
                    || checked
                        .typed
                        .normalized_type_identity(construction_type)
                        .into_string()
                        != result.type_identity)
            {
                return unsupported("case argument constructor lost whole owned custody");
            }
            continue;
        }
        match candidate {
            CheckedUnitEffectOperationPlan::EstablishByteSequenceLiteral { .. } => {
                super::super::literal_locals::validate(checked, caller.state, candidate)?;
            }
            CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. } => {
                super::super::view_ranges::binding_local(checked, caller.state, candidate)?;
            }
            _ => {}
        }
        let (producer_coordinate, source_site, result) = match candidate {
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate: producer_coordinate,
                source_site,
                result,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate: producer_coordinate,
                source_site,
                result,
                ..
            } => (*producer_coordinate, *source_site, result),
            CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishByteSequenceLiteral { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => (
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index: result.statement_index,
                    call_ordinal: 0,
                },
                None,
                result,
            ),
            _ => continue,
        };
        if producer_coordinate.call_ordinal == 0
            && matches!(
                statements.get(result.statement_index as usize),
                Some(StatementNode::Expression(_))
            )
        {
            if binding_ordinal == Some(result.binding_ordinal) {
                return unsupported(
                    "terminal structural result cannot supply an earlier call operand",
                );
            }
            continue;
        }
        // `place = value` over a borrowed structural field stores the
        // produced value whole into the window its move-out opened
        // (`StoreStructuralField`); the field, not a local, holds it.
        if producer_coordinate.call_ordinal == 0
            && matches!(
                statements.get(result.statement_index as usize),
                Some(StatementNode::Assignment(_))
            )
        {
            if binding_ordinal == Some(result.binding_ordinal) {
                return unsupported("stored field replacement value cannot supply a later operand");
            }
            continue;
        }
        if producer_coordinate.call_ordinal == 0
            && matches!(
                statements.get(result.statement_index as usize),
                Some(StatementNode::Call(_))
            )
        {
            crate::emission::call_source_custody::initializers::validate_discarded_structural(
                checked,
                caller.machine,
                caller.state,
                producer_coordinate,
                result,
            )?;
            if binding_ordinal == Some(result.binding_ordinal) {
                return unsupported("discarded boundary result cannot supply a later operand");
            }
            continue;
        }
        let names_result = if producer_coordinate.call_ordinal == 0 {
            let Some(StatementNode::LocalData(local)) =
                statements.get(result.statement_index as usize)
            else {
                return unsupported("Unit structural result producer has no authored local");
            };
            // A `&T` producer keeps the authored reference on its local but
            // joins the referent's carrier: compare the slice/record the
            // view names, not the borrow shell. A `&mut` producer's carrier
            // is its `ref-mut` type whole — the custody the result spells.
            let view_carrier = crate::expression_preparation::source_custody::structural::shared_slice_view_referent(
                checked,
                local.type_reference,
            );
            // The producer roster includes every local, even while rejoining
            // another operand. Copy cases carry the same plain-owned custody
            // as arrays and records; their exact carrier is still checked below.
            if !local.symbol.is_valid()
                || (result.multiplicity == Multiplicity::Unrestricted
                    && !(validation::has_plain_owned_contents_with_numeric_constraints(
                        &checked.typed,
                        local.type_reference,
                    ) || view_carrier.is_some()))
                || checked
                    .typed
                    .normalized_type_identity(view_carrier.unwrap_or(
                        crate::unit::attached_unit::parameters::structural_carrier_type(
                            checked,
                            local.type_reference,
                        )?,
                    ))
                    .into_string()
                    != result.type_identity
            {
                return unsupported(
                    "Unit structural result producer disagrees with its authored local",
                );
            }
            match expression.map(|expression| {
                super::super::parameters::source_path(
                    checked,
                    source_machine,
                    Some(
                        crate::expression_preparation::source_custody::authored_state(
                            checked,
                            caller.state,
                        )?
                        .1,
                    ),
                    local.type_reference,
                    expression,
                )
            }) {
                Some(Ok((root, path, access))) if root == local.symbol => {
                    // A borrowed-view local can only ever be lent shared;
                    // its authored access names the view's only custody.
                    let authored_access = access.unwrap_or(if view_carrier.is_some() {
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                    } else {
                        checked_trees::CheckedStructuralAccess::Owned
                    });
                    if path != argument.path || authored_access != argument.access {
                        return unsupported(
                            "Unit result projection disagrees with its authored path",
                        );
                    }
                    true
                }
                _ => false,
            }
        } else {
            let source = crate::emission::call_source_custody::authored::locate_source(
                checked,
                caller.state,
                producer_coordinate,
            )?;
            if source.source_site != source_site {
                return unsupported("nested structural producer has a different authored source");
            }
            let Some(checked_trees::NominalMachineUseSite::Expression(source_expression)) =
                source.source_site
            else {
                return unsupported("anonymous producer has no expression-owned source");
            };
            let matches = producer_coordinate.statement_index == coordinate.statement_index
                && expression.and_then(|expression| expression_producer(checked, expression))
                    == Some(source_expression);
            if matches {
                let signature = crate::emission::call_source_custody::authored::target_signature(
                    checked,
                    source_machine.symbol,
                    source.source_target,
                )?;
                if result.multiplicity == Multiplicity::Unrestricted
                    && !validation::has_plain_owned_contents_with_numeric_constraints(
                        &checked.typed,
                        signature.return_type,
                    )
                {
                    return unsupported(
                        "unrestricted structural operand has no plain owned producer",
                    );
                }
                let (root, path, access) = super::super::parameters::source_place_path(
                    checked,
                    source_machine,
                    Some(
                        crate::expression_preparation::source_custody::authored_state(
                            checked,
                            caller.state,
                        )?
                        .1,
                    ),
                    signature.return_type,
                    expression.unwrap(),
                )?;
                if root != facts::PlaceRoot::Expression(source_expression)
                    || path != argument.path
                    || access.unwrap_or(checked_trees::CheckedStructuralAccess::Owned)
                        != argument.access
                {
                    return unsupported(
                        "anonymous result projection disagrees with its authored source",
                    );
                }
                if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                    super::shared_temporary::validate(
                        checked,
                        caller,
                        producer_coordinate,
                        *coordinate,
                        source_expression,
                    )?;
                }
            }
            matches
        };
        if names_result != (binding_ordinal == Some(result.binding_ordinal)) {
            return unsupported(if producer_coordinate.call_ordinal == 0 {
                "Unit structural result argument does not rejoin its exact authored local"
            } else {
                "Unit structural result argument does not rejoin its exact authored source"
            });
        }
        if names_result
            && producer_coordinate.call_ordinal == 0
            && expression.is_none_or(|expression| {
                // The operand's authored access mirrors its source: a
                // borrowed-view local carries shared custody in its type,
                // not a borrow expression.
                let authored_access = named_result_operand(checked, expression).1;
                let authored_access =
                    if authored_access == checked_trees::CheckedStructuralAccess::Owned
                        && matches!(
                            statements.get(result.statement_index as usize),
                            Some(StatementNode::LocalData(local))
                                if crate::expression_preparation::source_custody::structural::shared_slice_view_referent(
                                    checked,
                                    local.type_reference,
                                )
                                .is_some()
                        )
                    {
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                    } else {
                        authored_access
                    };
                authored_access != argument.access
            })
        {
            return unsupported(
                "Unit structural result access disagrees with its authored operand",
            );
        }
    }
    Ok(())
}

/// One linear structural result moved whole into a claim-carrying `self`
/// formal. The claim plan is the evidence rather than a bypass: the callee
/// must publish exactly one whole-value `StateEntry` claim on this parameter
/// owned by the target state, and the caller's own consume event at this
/// exact call must retire precisely the claim the producing statement
/// established. A claim consumed twice, established elsewhere, or attached to
/// a different parameter is still a rejection.
#[allow(clippy::too_many_arguments)]
fn claimed_result_consumer(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    operation_index: usize,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    source_target: symbols::SymbolHandle,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    argument_index: usize,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
    producer: &Producer<'_>,
    target_entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
    claim_transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
) -> Result<(), LoweringError> {
    let result = producer.result;
    if producer.discard
        || producer.operation_index >= operation_index
        || !producer.precedes_consumer(coordinate)
        || matches!(
            operation,
            CheckedUnitEffectOperationPlan::StructuralCall { result: consumer, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result: consumer, .. }
                if result.binding_ordinal >= consumer.binding_ordinal
        )
        || result.multiplicity != Multiplicity::Linear
        || parameter.multiplicity != Multiplicity::Linear
        || !parameter.is_self
        || !argument.path.is_empty()
        || argument.access != checked_trees::CheckedStructuralAccess::Owned
        || parameter.access != argument.access
        || argument.type_identity != result.type_identity
        || parameter.type_identity != argument.type_identity
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || parameter.fused_service_erasure.is_some()
        || claim_transfers
            .iter()
            .any(|transfer| transfer.argument_index as usize == argument_index)
    {
        return unsupported("Unit structural result argument has invalid claim-carrying custody");
    }
    let mut entry_claims = target_entry_claims
        .iter()
        .filter(|claim| claim.parameter_index as usize == argument_index);
    let Some(entry_claim) = entry_claims.next() else {
        return unsupported("Unit structural result claim custody has no exact callee entry claim");
    };
    if entry_claims.next().is_some()
        || !entry_claim.path.is_empty()
        || entry_claim.carry != language_semantics::CarryPolicy::STRICT
        || !matches!(
            entry_claim.claim_identity,
            language_semantics::PermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source: language_semantics::PermissionEventSource::StateEntry,
                ..
            } if machine_symbol == target_machine && state_symbol == target_state
        )
    {
        return unsupported("Unit structural result claim custody has no exact callee entry claim");
    }
    // The consuming call must retire the exact claim the producing statement
    // established for this result: one owned linear consume at this authored
    // call occurrence carrying a statement-established caller claim.
    let call_source = language_semantics::PermissionEventSource::Call {
        statement_index: coordinate.statement_index as usize,
        call_ordinal: coordinate.call_ordinal as usize,
        target_symbol: source_target,
    };
    let consumed = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == caller.machine
                && event.state_symbol == caller.state
                && event.source == call_source
                && event.kind == language_semantics::PermissionEventKind::Consume
                && event.access == language_semantics::PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
                && checked
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty())
            .then_some(event)
        })
        .collect::<Vec<_>>();
    let [consumed] = consumed.as_slice() else {
        return unsupported(
            "Unit structural result claim custody lost its exact caller consume event",
        );
    };
    let language_semantics::PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source: language_semantics::PermissionEventSource::Statement { statement_index },
        ..
    } = consumed.claim_identity
    else {
        return unsupported(
            "Unit structural result claim custody lost its exact caller consume event",
        );
    };
    if machine_symbol != caller.machine
        || state_symbol != caller.state
        || statement_index != result.statement_index as usize
        || consumed.provenance
            != (language_semantics::PermissionProvenance::Established {
                machine_symbol: caller.machine,
                state_symbol: caller.state,
                source: language_semantics::PermissionEventSource::Statement { statement_index },
            })
    {
        return unsupported(
            "Unit structural result claim custody lost its exact caller consume event",
        );
    }
    // The producing statement must have minted this claim identity, and this
    // consume must be its only retirement: a second consume, transfer, or
    // drop of the same identity spends the custody twice.
    let lifecycle = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == caller.machine
                && event.state_symbol == caller.state
                && event.claim_identity == consumed.claim_identity)
                .then_some(event)
        })
        .collect::<Vec<_>>();
    let establishes = lifecycle
        .iter()
        .filter(|event| {
            event.kind == language_semantics::PermissionEventKind::Establish
                && event.access == language_semantics::PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
                && event.source
                    == language_semantics::PermissionEventSource::Statement { statement_index }
        })
        .count();
    let retirements = lifecycle
        .iter()
        .filter(|event| event.kind != language_semantics::PermissionEventKind::Establish)
        .count();
    if establishes != 1 || retirements != 1 {
        return unsupported(
            "Unit structural result claim custody was not established and consumed exactly once",
        );
    }
    // The bound local's producing statement establishes the whole claim
    // frontier the consume retires: a sibling mint at the same coordinate —
    // or an establish event re-minting an identity established elsewhere —
    // leaves claim custody this consume never carries.
    let (_, source_state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller.state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(source_state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("Unit structural result claim custody has no authored local");
    };
    if checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == caller.machine
                && event.state_symbol == caller.state
                && event.source
                    == language_semantics::PermissionEventSource::Statement { statement_index }
                && event.kind == language_semantics::PermissionEventKind::Establish
                && event.access == language_semantics::PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
                && event.root == facts::PlaceRoot::Symbol(local.symbol)
        })
        .count()
        != 1
    {
        return unsupported(
            "Unit structural result claim custody was not established and consumed exactly once",
        );
    }
    for candidate in caller
        .operations
        .iter()
        .flat_map(|operation| operation.with_value_calls())
    {
        let arguments = match candidate {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                structural_arguments,
                ..
            } => structural_arguments,
            _ => continue,
        };
        if arguments.iter().enumerate().any(|(position, argument)| {
            argument.source_structural_result_binding_ordinal() == Some(result.binding_ordinal)
                && argument.access == checked_trees::CheckedStructuralAccess::Owned
                && !(candidate == operation && position == argument_index)
        }) {
            return unsupported(
                "Unit structural result claim custody was not established and consumed exactly once",
            );
        }
    }
    if caller.structural_result.is_some_and(|returned| {
        returned.source
            == (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            })
    }) {
        return unsupported(
            "Unit structural result claim custody was not established and consumed exactly once",
        );
    }
    Ok(())
}

fn named_result_operand(
    checked: &CheckedTrees,
    expression: checked_trees::expression::ExpressionHandle,
) -> (
    checked_trees::expression::ExpressionHandle,
    checked_trees::CheckedStructuralAccess,
) {
    if let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(expression) {
        let access = match borrow.access {
            language_core::ReferenceAccess::Shared => {
                checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            language_core::ReferenceAccess::Mutable => {
                checked_trees::CheckedStructuralAccess::MutableBorrow
            }
            language_core::ReferenceAccess::WriteOnly => {
                checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
            }
        };
        (borrow.target, access)
    } else {
        (expression, checked_trees::CheckedStructuralAccess::Owned)
    }
}

fn validate_nested_execution_order(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    statement_index: u32,
    authored_nested: bool,
) -> Result<(), LoweringError> {
    if !authored_nested
        && !caller.operations.iter().any(|operation| {
            matches!(operation,
        CheckedUnitEffectOperationPlan::StructuralCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. }
            if coordinate.statement_index == statement_index && coordinate.call_ordinal != 0)
        })
    {
        return Ok(());
    }
    let expected = crate::emission::call_source_custody::authored::nested::authored_postorder(
        checked,
        caller.state,
        statement_index,
    )?;
    let mut actual = Vec::new();
    for operation in caller.operations {
        let coordinate = match operation {
            CheckedUnitEffectOperationPlan::StructuralCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
            | CheckedUnitEffectOperationPlan::ScalarCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. }
                if coordinate.statement_index == statement_index =>
            {
                coordinate
            }
            _ => continue,
        };
        actual.push(coordinate.call_ordinal);
    }
    if !actual
        .iter()
        .copied()
        .eq(expected.iter().filter_map(|(ordinal, expression)| {
            if *ordinal != 0
                && let ExpressionNode::Call(call) = checked.expression_table.expression(*expression)
                && crate::expression_preparation::source_custody::authored_state(
                    checked,
                    call.target_symbol,
                )
                .is_ok_and(|(_, target)| {
                    checked
                        .primitive_type_reference(target.return_type)
                        .is_some()
                })
            {
                None
            } else {
                Some(*ordinal)
            }
        }))
    {
        return unsupported(
            "nested structural operations disagree with authored argument execution order",
        );
    }
    Ok(())
}
