//! Exact source bindings and final custody of structural call results.

use super::super::admission::CallerView;
use super::super::parameters::expression_producer;
use super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, ExpressionNode,
    LoweringError, Multiplicity, StatementNode, unsupported,
};
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitStructuralResultBindingPlan,
};

/// The ordinary call with a Unit result an operation performs, if any.
fn ordinary_unit_call(operation: &CheckedUnitEffectOperationPlan) -> Option<&CheckedUnitCallPlan> {
    operation
        .call()
        .filter(|call| !call.is_boundary() && *call.result() == CheckedUnitCallResult::Unit)
}

/// Whether a call's result can retain no loan of its operands:
/// nothing, a scalar, or a structural result that mints no reference loan
/// or returned claim and whose callee declares contents holding no
/// reference, slice or cleanup owner. The declared return type is read from
/// the callee itself rather than from the checked plan's result shape, so a
/// loan lent into such a call provably ends with it.
fn loan_free_call(checked: &CheckedTrees, operation: &CheckedUnitEffectOperationPlan) -> bool {
    let Some(call) = operation.call() else {
        return false;
    };
    let binding = match call.result() {
        CheckedUnitCallResult::Unit | CheckedUnitCallResult::Scalar(_) => return true,
        CheckedUnitCallResult::Structural { binding, .. } => binding,
    };
    // A Unit/scalar boundary result cannot retain a loan any more than an
    // ordinary result can. Structural foreign retention still requires its
    // dedicated custody protocol, not the ordinary callee inspection below.
    if call.is_boundary() {
        return false;
    }
    call.structural_custody().is_some_and(|custody| {
        !custody.reference_loan.is_valid() && custody.returned_claim_transfers.is_empty()
    }) && crate::expression_preparation::source_custody::authored_state(checked, call.target.state)
        .is_ok_and(|(_, callee_state)| {
            let returned = callee_state.return_type;
            checked.typed.normalized_type_identity(returned).as_str() == binding.type_identity
                && ((matches!(
                    binding.multiplicity,
                    Multiplicity::Affine | Multiplicity::Unrestricted
                ) && validation::has_plain_owned_contents_with_numeric_constraints(
                    &checked.typed,
                    returned,
                )) || (binding.multiplicity == Multiplicity::Unrestricted
                    && validation::is_closed_primitive_array_type(&checked.typed, returned)))
        })
}

/// The structural result binding of a call, ordinary or boundary.
fn call_structural_result(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<&CheckedUnitStructuralResultBindingPlan> {
    operation.call()?.structural_result()
}

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
    let Some((consumer, consumer_result, custody)) = operation
        .call()
        .filter(|call| !call.is_boundary())
        .and_then(|call| Some((call, call.structural_result()?, call.structural_custody()?)))
    else {
        return unsupported("linear result operand requires an ordinary structural call");
    };
    let coordinate = &consumer.coordinate;
    let argument =
        consumer
            .structural_arguments
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
    let Some((producer_call, result, produced)) = producer
        .call()
        .and_then(|call| Some((call, call.structural_result()?, call.structural_custody()?)))
    else {
        return unsupported("linear result operand has no ordinary producer");
    };
    let producer_coordinate = &producer_call.coordinate;
    let producer_target_state = &producer_call.target.state;
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
        let Some(call) = earlier.call() else {
            continue;
        };
        if call
            .structural_arguments
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
    let Some(call) = operation.call() else {
        return unsupported("argument construction requires an ordinary or boundary call");
    };
    let coordinate = call.coordinate;
    let argument =
        call.structural_arguments
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
    // A copied operand keeps the declaration's exact qualification roster.
    // This source-side join does not establish those predicates: Terminal
    // independently reconstructs them from the copied field's declaration.
    // Fresh constructors remain on their existing qualification-free route.
    let copied_qualifications = match &operations[source.operation_index] {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. }
            if checked
                .facts
                .values
                .structural_values
                .nodes
                .is_valid(*value)
                && matches!(
                    checked
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .get(*value)
                        .kind,
                    checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
                ) =>
        {
            let expression = checked
                .facts
                .values
                .structural_values
                .nodes
                .get(*value)
                .expression;
            checked
                .facts
                .values
                .structural_values
                .root_for_expression(state, source.result.statement_index, expression)
                .filter(|root| root.machine == machine)
                .and_then(|root| {
                    validation::structural_result_qualifications(
                        &checked.typed,
                        root.type_reference,
                    )
                    .ok()
                })
                .is_some_and(|qualifications| qualifications == parameter.qualifications)
        }
        _ => false,
    };
    if source.operation_index >= consumer_index
        || !source.precedes_consumer(coordinate)
        || source.discard
        || parameter_position != source_position
        || !argument.path.is_empty()
        || argument.access != checked_trees::CheckedStructuralAccess::Owned
        || parameter.access != argument.access
        || parameter.is_self
        || parameter.fused_service_erasure.is_some()
        || (!parameter.qualifications.is_empty() && !copied_qualifications)
        || source.result.multiplicity != parameter.multiplicity
        || argument.type_identity != source.result.type_identity
        || parameter.type_identity != source.result.type_identity
    {
        return unsupported("argument construction moves into another formal or custody");
    }
    super::validate_custody(checked, machine, state, operation)?;
    let owned_uses = operations
        .iter()
        .flat_map(CheckedUnitEffectOperationPlan::call_structural_arguments)
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
        && operation
            .call()
            .and_then(CheckedUnitCallPlan::structural_custody)
            .is_some_and(|custody| custody.reference_loan.is_valid())
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
            | checked_trees::CheckedUnitStructuralFieldType::RefinedScalar { .. }
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
            CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. }
            | CheckedUnitEffectOperationPlan::MoveStructuralField { result, .. }
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
            // A declared structural local's establishment is the same kind
            // of producer: its binding publishes the home member operations
            // fill, and its authored discard flag rides with the result.
            CheckedUnitEffectOperationPlan::EstablishStructuralLocal {
                result,
                discard_result_on_return,
                ..
            } if result.binding_ordinal == binding_ordinal => Some(Producer {
                operation_index,
                coordinate: checked_trees::CheckedUnitCallCoordinate {
                    statement_index: result.statement_index,
                    call_ordinal: 0,
                },
                result,
                discard: *discard_result_on_return,
                construction_source: None,
            }),
            // Any call with a structural result, ordinary or boundary.
            CheckedUnitEffectOperationPlan::Call(call) => {
                let result = call
                    .structural_result()
                    .filter(|result| result.binding_ordinal == binding_ordinal)?;
                Some(Producer {
                    operation_index,
                    coordinate: call.coordinate,
                    result,
                    discard: call.discard_result_on_return()?,
                    construction_source: None,
                })
            }
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
                // A returning consumer can retire its argument temporaries
                // without retiring the distinct value it just returned.
                && affine_discards.iter().any(|discard| discard.source == source)
                && caller.operations[producer.operation_index]
                    .call()
                    .is_some_and(|call| {
                        call.structural_result().is_some()
                            && call.coordinate == *coordinate
                            && call.coordinate.call_ordinal == 0
                    });
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
                .and_then(CheckedUnitEffectOperationPlan::call)
                .filter(|call| !call.is_boundary() && call.coordinate == *coordinate)
                .is_some_and(|call| {
                    call.structural_arguments
                        .iter()
                        .any(|argument| argument.source == source)
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
                    let mut owned_discards = affine_discards
                        .iter()
                        .filter(|discard| discard.source == source);
                    !matches!(owned_discards.next(), Some(discard)
                        if discard.path.is_empty() && discard.type_identity == result.type_identity)
                        || owned_discards.next().is_some()
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
        let Some(call) = operation.call() else {
            continue;
        };
        let coordinate = call.coordinate;
        for argument in &call.structural_arguments {
            if argument.source_structural_result_binding_ordinal() != Some(result.binding_ordinal) {
                continue;
            }
            let source_order = producer.precedes_consumer(coordinate);
            if consumed || disposed || operation_index <= producer.operation_index || !source_order
            {
                return unsupported(
                    "Unit structural result is consumed before production or twice",
                );
            }
            // Either call family, ordinary or boundary, may borrow the result.
            if matches!(
                argument.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
                    | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
            ) && producer.coordinate.call_ordinal == 0
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
                if argument.access != checked_trees::CheckedStructuralAccess::Owned
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
                if !(ordinary_unit_call(operation).is_some() || loaned)
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
            // A linear result moves whole like an affine one: the consuming edge
            // carries the claim and checking already proved who retires it, so
            // only the multiplicity differs.
            if !projected_paths.is_empty()
                || !matches!(
                    argument.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                )
                || argument.type_identity != result.type_identity
                || !matches!(
                    result.multiplicity,
                    Multiplicity::Affine | Multiplicity::Linear
                )
            {
                return unsupported(
                    "Unit structural result use is not a whole owned move or shared borrow",
                );
            }
            if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                && producer.coordinate.call_ordinal != 0
                && (producer.discard
                    || call.is_boundary()
                    || !matches!(caller.operations.get(operation_index + 1),
                        Some(CheckedUnitEffectOperationPlan::CallContinuationCleanup { coordinate: cleanup, affine_discards })
                            if *cleanup == coordinate
                                && affine_discards.iter().filter(|discard| discard.source == argument.source).count() == 1))
            {
                return unsupported("anonymous shared result has no exact dying call continuation");
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
    // A `&mut` reference result's custody is the loan its producing call's
    // custody row records, never an owned consume or discard: retirement is
    // the loan's release — the explicit `ReleaseReference` the loop already
    // counted as `disposed`, or the state exit when the last use is the
    // final statement. Edge retirement still owns the carrier place, so its
    // return discard is required; an explicit release already retired both
    // and must not carry one.
    if let Some(custody) = caller.operations[producer.operation_index]
        .call()
        .and_then(|call| call.structural_custody())
        && custody.reference_loan.is_valid()
    {
        if consumed || !projected_paths.is_empty() || (disposed && producer.discard) {
            return unsupported("reference result acquired owned cleanup custody");
        }
        if !disposed {
            // Edge retirement: without a release operation the loan ends with
            // the carrier place's own return discard, which the checker must
            // therefore have kept on the result.
            if !producer.discard {
                return unsupported("reference result omits its carrier disposal");
            }
            let boundary = validation::reference_result_custody::release_boundary(
                &checked.typed,
                &checked.facts,
                caller.machine,
                caller.state,
                custody.reference_loan,
            )
            .ok_or(LoweringError::Unsupported(
                "reference result has no exact weakening boundary",
            ))?;
            let (_, authored) = crate::expression_preparation::source_custody::authored_state(
                checked,
                caller.state,
            )?;
            if (boundary as usize)
                < checked
                    .statement_table
                    .statements(authored.statement_nodes)
                    .len()
            {
                return unsupported("reference result omits its loan release");
            }
        }
        return Ok(());
    }

    // Affine custody belongs to the result carrier, not necessarily to its
    // backing storage. A mutable view descriptor retires under these same
    // exact transfer/disposal rules; its source loan is checked separately.
    // Shared descriptors already followed the unrestricted path above. A
    // view-shaped type alone can neither waive nor forbid carrier cleanup.
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
    let Some(call) = operation.call() else {
        return unsupported("structural result use requires an ordinary or boundary call");
    };
    // Retained loans are admitted by the same reconstructed source/outcome
    // custody as other structural calls, not by recognizing the caller's
    // storage spelling. The loan's release is checked by validate_usage.
    super::validate_custody(checked, caller.machine, caller.state, operation)?;
    let loan_is_accounted = loan_free_call(checked, operation)
        || (!call.is_boundary()
            && call
                .structural_custody()
                .is_some_and(|custody| custody.reference_loan.is_valid()));
    // A structural call keeps its transfers in its custody record; a boundary
    // call's are its completion receipts.
    let (coordinate, target_machine, target_state, structural_arguments, claim_transfers) = (
        call.coordinate,
        call.target.machine,
        call.target.state,
        &call.structural_arguments,
        call.claim_transfers(),
    );
    if structural_arguments.len() != target_parameters.len() {
        return unsupported("Unit structural argument arity disagrees with its target");
    }
    let operation_index = caller
        .operations
        .iter()
        .position(|candidate| candidate == operation)
        .ok_or(LoweringError::Unsupported(
            "structural result consumer has no operation position",
        ))?;
    let authored = crate::emission::call_source_custody::authored::locate_source(
        checked,
        caller.state,
        coordinate,
    )?;
    super::super::argument_schedule::build_operations(
        checked,
        caller.machine,
        caller.state,
        caller.operations,
    )?;
    let (source_machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller.state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let signature = crate::emission::call_source_custody::authored::target_signature(
        checked,
        caller.machine,
        authored.source_target,
    )?;
    let retained = super::super::call_parameters::structural_parameters(checked, call)?;
    if retained != target_parameters {
        return unsupported("structural result consumer changed its retained target parameters");
    }
    let source_positions =
        super::super::call_parameters::structural_positions(checked, call, &signature)?;
    // Claims use dense runtime slots; constructions retain authored positions.
    // The declaration projection joins them without manufacturing a receiver slot.
    for (index, ((argument, parameter), source_position)) in structural_arguments
        .iter()
        .zip(target_parameters)
        .zip(source_positions)
        .enumerate()
    {
        let parameter_index = index;
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
                checked, caller, coordinate, argument, parameter, expression,
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
            &coordinate,
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
        let source_order = producer.precedes_consumer(coordinate);
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
        // A receiver with no linear claim uses ordinary owned-value custody;
        // being spelled `self` does not create a claim-bearing obligation.
        if parameter.is_self && parameter.multiplicity == Multiplicity::Linear {
            claimed_result_consumer(
                checked,
                caller,
                operation,
                operation_index,
                coordinate,
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
        let argument_construction = matches!(
            producer.construction_source,
            Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. })
        );
        if argument_construction {
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
            && operation
                .call()
                .is_some_and(|call| !call.is_boundary() && call.structural_result().is_some())
        {
            validate_linear_result_consumer(checked, caller, operation, index, parameter)?;
        }
        if (producer.discard
            && argument.access == checked_trees::CheckedStructuralAccess::Owned
            && !loaned_record_projection(operation, argument))
            || producer.operation_index >= operation_index
            || !source_order
            || call_structural_result(operation)
                .is_some_and(|consumer| result.binding_ordinal >= consumer.binding_ordinal)
        {
            return unsupported("Unit structural result argument has invalid claim-free custody");
        }
        // Whole local arrays lend their backing without transferring ownership.
        // The source/ordinal join above identifies the declaration; this check
        // independently rejoins its loan and fixed-array/slice presentation.
        if whole_local_array_loan(
            checked,
            caller,
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
        // A whole bounded owner lends its own carrier or a shared live-byte
        // view. Boundary joins stay confined to shared byte formals with Unit
        // or scalar results; source capture and claim checks remain required.
        if whole_local_bounded_owner_loan(
            checked,
            caller,
            result,
            argument,
            parameter,
            signature
                .parameters
                .get(source_position as usize)
                .ok_or(LoweringError::Unsupported(
                    "bounded owner loan has no authored formal",
                ))?
                .type_reference,
        )? {
            let boundary_shared_view = authored.boundary
                && parameter.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                && signature
                    .parameters
                    .get(source_position as usize)
                    .is_some_and(|formal| {
                        shared_byte_view_referent(checked, formal.type_reference).is_some()
                    })
                && operation.call().is_some_and(|call| {
                    matches!(
                        call.result(),
                        CheckedUnitCallResult::Unit | CheckedUnitCallResult::Scalar(_)
                    )
                });
            if (authored.boundary && !boundary_shared_view)
                || parameter.is_self
                || parameter.fused_service_erasure.is_some()
                || target_entry_claims
                    .iter()
                    .any(|claim| claim.parameter_index as usize == parameter_index)
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
                || (!boundary_shared_view && !loan_free_call(checked, operation))
            {
                return unsupported("bounded owner loan changes the target custody");
            }
            continue;
        }
        // Lending a whole local or a projected subtree does not consume its
        // owner. The authored path was rejoined above; a loan either ends at
        // this call or survives in independently reconstructed result custody.
        // Stored-reference crossings retain that same call custody. The path
        // replay preserves their descriptors; Terminal independently checks
        // each live permission instead of recognizing a returned-record form.
        let lent_access = match argument.access {
            checked_trees::CheckedStructuralAccess::MutableBorrow
            | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
            | checked_trees::CheckedStructuralAccess::SharedBorrow => true,
            checked_trees::CheckedStructuralAccess::Owned => false,
        };
        let borrowed_storage = matches!(
            result.multiplicity,
            Multiplicity::Affine | Multiplicity::Unrestricted
        ) && loan_is_accounted
            && argument.path.iter().all(|segment| {
                matches!(
                    segment,
                    checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                        | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                        | checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange { .. }
                        | checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                        | checked_trees::CheckedUnitStructuralPathSegment::Referent
                )
            })
            && lent_access
            && argument.access == parameter.access
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
            && !borrowed_storage
            && (!matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) || (result.multiplicity == Multiplicity::Unrestricted
                && (call.is_boundary() && !boundary_array_argument && !shared_view_argument
                    || !argument.path.is_empty()
                    || !matches!(
                        argument.access,
                        checked_trees::CheckedStructuralAccess::Owned
                            | checked_trees::CheckedStructuralAccess::SharedBorrow
                    )))
                || (argument.path.is_empty() && argument.type_identity != result.type_identity)
                || parameter.type_identity != argument.type_identity
                || (!argument.path.is_empty()
                    && (!(ordinary_unit_call(operation).is_some()
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
                // Qualification obligations belong to the emitted call's
                // semantic check, independently of this source custody join.
                // In particular, a subslice's primitive byte predicates are
                // derived from its exact producer without result annotations.
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
    // Common call observation replay authenticates the exact ordered loan
    // roster for every result family, including construction member calls.
    Ok(true)
}

/// Reconstruct a whole bounded byte owner loan (`parse(&text)` with
/// `text: [u8; 8] in Utf8`) independently of its argument plan: the operand
/// is the authored local itself, the formal names the same bounded carrier
/// or its shared live-byte view, every formal qualification is
/// declared on the local. Common source replay authenticates captured access.
/// A bounded owner is never a raw fixed array: its live length is its own.
fn whole_local_bounded_owner_loan(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    result: &CheckedUnitStructuralResultBindingPlan,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
    formal: checked_trees::types::TypeReferenceHandle,
) -> Result<bool, LoweringError> {
    use checked_trees::CheckedStructuralAccess;
    if !argument.path.is_empty() || result.multiplicity != Multiplicity::Unrestricted {
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
    let Some(capacity) =
        validation::bounded_byte_buffer_capacity(&checked.typed, local.type_reference)
    else {
        return Ok(false);
    };
    // The formal's access is the loan; its referent names the owner.
    let mut referent = formal;
    let mut formal_access = None;
    loop {
        match checked.type_reference_table.type_reference(referent) {
            TypeReferenceNode::Constrained { base_type, .. } => referent = *base_type,
            TypeReferenceNode::Reference {
                access, referee, ..
            } if formal_access.is_none() => {
                formal_access = Some(*access);
                referent = *referee;
            }
            _ => break,
        }
    }
    let expected_access = match formal_access {
        Some(language_core::ReferenceAccess::Shared) => CheckedStructuralAccess::SharedBorrow,
        Some(language_core::ReferenceAccess::Mutable) => CheckedStructuralAccess::MutableBorrow,
        _ => return unsupported("bounded owner loan has unsupported formal access"),
    };
    let formal_capacity = match checked.type_reference_table.type_reference(referent) {
        TypeReferenceNode::FixedArray {
            element_type,
            length: checked_trees::types::FixedArrayLength::Literal(length),
        } if checked.primitive_type_reference(*element_type)
            == Some(checked_trees::types::PrimitiveType::U8) =>
        {
            Some(*length)
        }
        _ => None,
    };
    let shared_view = shared_byte_view_referent(checked, formal).is_some();
    let mut declared =
        validation::structural_result_qualifications(&checked.typed, local.type_reference)
            .map_err(LoweringError::Unsupported)?;
    declared.sort_by_key(|domain| domain.0);
    declared.dedup();
    if (!shared_view && formal_capacity != Some(capacity))
        || argument.access != expected_access
        || parameter.access != expected_access
        || (expected_access == CheckedStructuralAccess::MutableBorrow && !local.is_mutable)
        || parameter.multiplicity != Multiplicity::Unrestricted
        || (!shared_view && argument.type_identity != result.type_identity)
        || parameter.type_identity != argument.type_identity
        || checked.normalized_type_identity(referent).as_str() != argument.type_identity
        || !parameter
            .qualifications
            .iter()
            .all(|domain| declared.contains(domain))
        || !parameter.projected_qualifications.is_empty()
    {
        return unsupported("bounded owner loan changes backing, presentation, or access");
    }
    // Common call observation replay authenticates the exact ordered loan
    // roster for every result family, including construction member calls.
    Ok(true)
}

fn shared_byte_view_referent(
    checked: &CheckedTrees,
    formal: checked_trees::types::TypeReferenceHandle,
) -> Option<checked_trees::types::TypeReferenceHandle> {
    let view = validation::slice_view_shell(&checked.typed, formal, &[])?;
    let TypeReferenceNode::Slice { element_type } =
        checked.type_reference_table.type_reference(view.slice)
    else {
        return None;
    };
    (view.access == language_core::ReferenceAccess::Shared
        && checked.primitive_type_reference(*element_type)
            == Some(checked_trees::types::PrimitiveType::U8))
    .then_some(view.slice)
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
    let shared_byte_formal = crate::emission::call_source_custody::authored::target_signature(
        checked,
        caller.machine,
        authored.source_target,
    )?
    .parameters
    .get(source_position as usize)
    .is_some_and(|formal| shared_byte_view_referent(checked, formal.type_reference).is_some());
    // A computed descriptor or operand construction (including a copy) owns
    // its produced value. The local it reads supplies storage, not that
    // value's result binding. Argument staging replays a range's exact source
    // and endpoints; constructions rejoin their occurrence below. Keep the
    // complete roster check so neither can impersonate a local initializer
    // or another call's snapshot.
    let operand_has_own_producer = super::super::view_ranges::range(argument).is_some()
        || binding_ordinal
            .map(|ordinal| producer(caller.operations, ordinal))
            .transpose()?
            .is_some_and(|producer| {
                matches!(
                    producer.construction_source,
                    Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. })
                )
            });
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
                    )
                // A copy preserves existing field predicates; unlike a
                // constructor it does not mint them. Source replay still
                // checks the exact path, type and unrestricted snapshot.
                || (matches!(node.kind,
                        checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. })
                    && result.multiplicity == Multiplicity::Unrestricted
                    && validation::has_plain_owned_contents_with_represented_constraints(
                        &checked.typed, construction_type,
                    ));
            // Source custody validates the complete qualified copied type.
            // The result identity instead names its storage carrier, as it
            // does for structural parameters; predicates remain separate.
            let identity_type = if matches!(
                node.kind,
                checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
            ) {
                crate::unit::attached_unit::structural_carrier_type(checked, construction_type)?
            } else {
                construction_type
            };
            if names_result
                && ((authored.boundary && !copied_array)
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || !argument.path.is_empty()
                    || !whole_owned_contents
                    || checked
                        .typed
                        .normalized_type_identity(identity_type)
                        .into_string()
                        != result.type_identity)
            {
                return unsupported("case argument constructor lost whole owned custody");
            }
            continue;
        }
        match candidate {
            CheckedUnitEffectOperationPlan::EstablishByteSequence { .. } => {
                super::super::byte_locals::validate(checked, caller.state, candidate)?;
            }
            CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. } => {
                super::super::view_ranges::binding_site(checked, caller.state, candidate)?;
            }
            _ => {}
        }
        let (producer_coordinate, source_site, result) = match candidate {
            // Any call with a structural result retains its source site.
            CheckedUnitEffectOperationPlan::Call(call) => {
                let Some(result) = call.structural_result() else {
                    continue;
                };
                (call.coordinate, call.source_site().flatten(), result)
            }
            CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. }
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
        // A completing transition mints its result on its own terminal
        // statement; no operand inside the body can follow it. Its custody is
        // rejoined by the completion and source-custody validators instead.
        if producer_coordinate.call_ordinal == 0
            && matches!(
                statements.get(result.statement_index as usize),
                Some(StatementNode::Transition(_))
            )
        {
            if binding_ordinal == Some(result.binding_ordinal) {
                return unsupported("completing transition result cannot supply a later operand");
            }
            continue;
        }
        // The authored formal determines whether a bare whole bounded owner
        // lends shared rather than moves. Both operand checks below consume
        // this same decision; the exact root, path and captured access remain
        // independently checked before the result may be used.
        let implicit_shared_owner = producer_coordinate.call_ordinal == 0
            && shared_byte_formal
            && argument.path.is_empty()
            && matches!(statements.get(result.statement_index as usize),
                Some(StatementNode::LocalData(local))
                    if validation::bounded_byte_buffer_capacity(
                        &checked.typed, local.type_reference,
                    ).is_some());
        let names_result = if producer_coordinate.call_ordinal == 0 {
            let Some(StatementNode::LocalData(local)) =
                statements.get(result.statement_index as usize)
            else {
                return unsupported("Unit structural result producer has no authored local");
            };
            // A `&T` producer keeps the authored reference on its local but
            // joins the referent's carrier: compare the slice/record the
            // view names, not the borrow shell. A `&mut` producer's carrier
            // peels the same way — view/named referees join the view or
            // record carrier, primitives keep the `ref-mut` shell.
            let view_carrier = crate::expression_preparation::source_custody::structural::shared_slice_view_referent(
                checked,
                local.type_reference,
            )
            .or(
                crate::expression_preparation::source_custody::structural::mutable_view_carrier(
                    checked,
                    local.type_reference,
                ),
            );
            // The producer roster includes every local, even while rejoining
            // another operand. Copy cases carry the same plain-owned custody
            // as arrays and records; represented byte predicates stay attached
            // to the established home, and its exact carrier is checked below.
            if !local.symbol.is_valid()
                || (result.multiplicity == Multiplicity::Unrestricted
                    && !(validation::has_plain_owned_contents_with_represented_constraints(
                        &checked.typed,
                        local.type_reference,
                    ) || view_carrier.is_some()
                        // A bounded byte owner (`[u8; N] in Utf8`) keeps its
                        // declared domains; its carrier is the fixed array.
                        || validation::bounded_byte_buffer_capacity(
                            &checked.typed,
                            local.type_reference,
                        )
                        .is_some()))
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
            match expression
                .filter(|_| !operand_has_own_producer)
                .map(|expression| {
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
                    let authored_access =
                        access.unwrap_or(if view_carrier.is_some() || implicit_shared_owner {
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
            let matches = !operand_has_own_producer
                && producer_coordinate.statement_index == coordinate.statement_index
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
                    && !(validation::bounded_byte_buffer_capacity(
                        &checked.typed,
                        signature.return_type,
                    )
                    .is_some()
                        && validation::has_plain_owned_contents_with_represented_constraints(
                            &checked.typed,
                            signature.return_type,
                        ))
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
        // Source-path replay above owns access as well as identity. A second
        // syntax-only pass would mistake a stored reference for owned storage.
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
        || call_structural_result(operation)
            .is_some_and(|consumer| result.binding_ordinal >= consumer.binding_ordinal)
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
        let arguments = candidate.call_structural_arguments();
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
