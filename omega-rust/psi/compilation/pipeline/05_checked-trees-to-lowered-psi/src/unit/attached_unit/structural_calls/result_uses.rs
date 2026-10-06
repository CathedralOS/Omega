//! Exact source bindings and final custody of structural call results.

mod inventory;
pub(crate) use inventory::with_expression_consumers as result_use_operations;

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
/// reference or slice. Nominal cleanup travels with the owned result without
/// retaining a loan. The declared return type is read from the callee itself
/// rather than from the checked plan's result shape, so a loan lent into such
/// a call provably ends with it.
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
    }) && crate::declarations::state_declaration(checked, call.target.state).is_ok_and(
        |(_, callee_state)| {
            let returned = callee_state.return_type;
            checked.typed.normalized_type_identity(returned).as_str() == binding.type_identity
                && ((matches!(
                    binding.multiplicity,
                    Multiplicity::Affine | Multiplicity::Unrestricted
                ) && validation::has_cleanup_owned_contents(&checked.typed, returned))
                    || (binding.multiplicity == Multiplicity::Unrestricted
                        && crate::content_classes::closed_primitive_array(checked, returned)))
        },
    )
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
    let (state, operations) = (caller.state, caller.operations);
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
        || parameter.fused_service_erasure.is_some()
        || result.type_identity != argument.type_identity
        || parameter.type_identity != result.type_identity
        || parameter.qualifications != produced.result_qualifications
    {
        return unsupported("linear result operand has stale or incompatible producer custody");
    }
    let (_, producer_state) =
        crate::declarations::state_declaration(checked, *producer_target_state)?;
    let projected_qualifications = validation::structural_result_projected_qualifications(
        &checked.typed,
        producer_state.return_type,
    )
    .map_err(LoweringError::Unsupported)?;
    if parameter.projected_qualifications != projected_qualifications {
        return unsupported("linear result operand changes its projected qualification row");
    }
    let (_, source_state) = crate::declarations::state_declaration(checked, state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(source_state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("linear result operand has no authored local");
    };
    // A `&mut` view local's qualifications are the referee's: the reference
    // presentation is custody, not a domain shell.
    let qualification_type =
        crate::reference_custody::mutable_borrow_parts(checked, local.type_reference)
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
    let authored = crate::call_sites::locate(checked, state, *coordinate)?;
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
/// statement and supplies the formal it was written for. The
/// construction's own source custody — the authored argument expression at
/// that formal position — is checked where the construction rejoins its
/// statement. This ties the two operations together, so the value cannot move
/// into another formal, another call, a projected parameter, or two consumers.
/// Sharing keeps the completed owner through its exact normal continuation;
/// the borrowed formal's unrestricted multiplicity is not the owner's.
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
    // Every established operand carries its declared value contract. The
    // producer establishes those predicates; this consuming edge only joins
    // the exact retained root and formal roster, independent of evaluator kind.
    let declared_qualifications = match &operations[source.operation_index] {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. }
            if checked
                .facts
                .values
                .structural_values
                .nodes
                .is_valid(*value) =>
        {
            let values = &checked.facts.values.structural_values;
            let expression = values.nodes.get(*value).expression;
            values
                .root_for_expression(state, source.result.statement_index, expression)
                .filter(|root| root.machine == machine && root.root == *value)
                .and_then(|root| {
                    validation::structural_result_contract(&checked.typed, root.type_reference).ok()
                })
                .is_some_and(|contract| contract.qualifications == parameter.qualifications)
        }
        _ => parameter.qualifications.is_empty(),
    };
    if source.operation_index >= consumer_index
        || !source.precedes_consumer(coordinate)
        || source.discard
        || parameter_position != source_position
        || !argument.path.is_empty()
        || !matches!(
            argument.access,
            checked_trees::CheckedStructuralAccess::Owned
                | checked_trees::CheckedStructuralAccess::SharedBorrow
        )
        || parameter.access != argument.access
        || parameter.fused_service_erasure.is_some()
        || !declared_qualifications
        || parameter.multiplicity
            != if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                Multiplicity::Unrestricted
            } else {
                source.result.multiplicity
            }
        || argument.type_identity != source.result.type_identity
        || parameter.type_identity != source.result.type_identity
    {
        return unsupported("argument construction moves into another formal or custody");
    }
    if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
        validate_shared_temporary_continuation(
            checked,
            operations,
            consumer_index,
            &source,
            argument,
        )?;
    }
    let consumer_uses = operations
        .iter()
        .flat_map(CheckedUnitEffectOperationPlan::call_structural_arguments)
        .filter(|candidate| {
            candidate.source_structural_result_binding_ordinal() == Some(binding_ordinal)
        })
        .count();
    if consumer_uses != 1 {
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

/// A completed anonymous owner can be lent only through a loan-free call.
/// Affine owners die at the recorded normal continuation, independently of
/// whether a nested invocation or an argument construction established them.
/// The checked disposition owns the exact complement: hooks and maximal
/// residual subtrees need not look like one whole-root trivial discard.
fn validate_shared_temporary_continuation(
    checked: &CheckedTrees,
    operations: &[CheckedUnitEffectOperationPlan],
    consumer_index: usize,
    producer: &Producer<'_>,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let operation = operations
        .get(consumer_index)
        .ok_or(LoweringError::Unsupported(
            "shared temporary lost its consumer operation",
        ))?;
    let call = operation.call().ok_or(LoweringError::Unsupported(
        "shared temporary has no consuming call",
    ))?;
    if producer.discard || call.is_boundary() || !loan_free_call(checked, operation) {
        return unsupported("anonymous shared result has no exact dying call continuation");
    }
    match producer.result.multiplicity {
        Multiplicity::Unrestricted => Ok(()),
        Multiplicity::Affine
            if matches!(operations.get(consumer_index + 1),
            Some(CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate), cleanup_actions,
            }) if *coordinate == call.coordinate
                && cleanup_actions.iter().any(|action| action.source() == &argument.source)) =>
        {
            Ok(())
        }
        _ => unsupported("anonymous shared result has no exact dying call continuation"),
    }
}

/// A bare reference result's retained leaf loan proves a projected owned
/// operand's subtree custody independently: checking recorded that loan in
/// the call's custody, the release boundary rejoins it to typed evidence, and
/// the carrier's residual still dies at its own return drop. The projection must therefore
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
    fn is_argument_temporary(&self) -> bool {
        self.coordinate.call_ordinal != 0
            || matches!(
                self.construction_source,
                Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. })
            )
    }

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
    let (_, authored) = crate::declarations::state_declaration(checked, state)?;
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
    operations: &[CheckedUnitEffectOperationPlan],
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let producer = producer(operations, result.binding_ordinal)?;
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
        operations
            .iter()
            .enumerate()
            .flat_map(|(index, operation)| {
                operation
                    .with_value_calls()
                    .map(move |operation| (index, operation))
            })
    {
        if let CheckedUnitEffectOperationPlan::ReleaseReference {
            binding_ordinal,
            path,
            defer_to_exit: false,
            ..
        } = operation
            && *binding_ordinal == result.binding_ordinal
        {
            // Checking has selected this exact lifetime boundary. A leaf
            // release removes only its custody, leaving scalar siblings and
            // the owner's scheduled residual cleanup available.
            if path.is_empty() {
                disposed = true;
            } else {
                projected_paths.push(path);
            }
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
                    crate::declarations::state_declaration(checked, caller.state)?
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
            let (_, state) = crate::declarations::state_declaration(checked, caller.state)?;
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
                        checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                            pending.extend(elements.iter().rev().copied());
                        }
                        checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => pending.extend(values.dispatch_arms.span(*arms).ok_or(LoweringError::Unsupported("record consumption arm roster missing"))?.iter().map(|arm| arm.value)),
                        _ => {}
                    }
                }
            }
        }
        if let CheckedUnitEffectOperationPlan::ContinuationCleanup {
            boundary,
            cleanup_actions,
        } = operation
        {
            if matches!(
                boundary,
                checked_trees::CheckedUnitCleanupBoundary::StatementEntry { .. }
            ) {
                let source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    };
                if !cleanup_actions
                    .iter()
                    .any(|discard| *discard.source() == source)
                {
                    continue;
                }
                disposed = true;
                continue;
            }
            let checked_trees::CheckedUnitCleanupBoundary::Call(coordinate) = boundary else {
                return unsupported("cleanup has no supported semantic boundary");
            };
            let source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            };
            let immediately_discarded = operation_index.checked_sub(1)
                == Some(producer.operation_index)
                // A returning consumer can retire its argument temporaries
                // without retiring the distinct value it just returned.
                && cleanup_actions.iter().any(|discard| *discard.source() == source)
                && operations[producer.operation_index]
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
                    || !matches!(cleanup_actions.as_slice(), [discard]
                        if *discard.source() == source && discard.path().is_empty()
                            && discard.type_identity() == result.type_identity)
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
                .and_then(|previous| operations.get(previous))
                .and_then(CheckedUnitEffectOperationPlan::call)
                .filter(|call| !call.is_boundary() && call.coordinate == *coordinate)
                .is_some_and(|call| {
                    call.structural_arguments
                        .iter()
                        .any(|argument| argument.source == source)
                });
            if !owns_continuation {
                continue;
            }
            if consumed
                || disposed
                || producer.discard
                || !producer.is_argument_temporary()
                || producer.coordinate.statement_index != coordinate.statement_index
                || operation_index <= producer.operation_index
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
            ) && !producer.is_argument_temporary()
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
                    argument.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                ) || !argument.path.is_empty()
                    || argument.type_identity != result.type_identity
                {
                    return unsupported(
                        "unrestricted result requires a whole owned or shared call operand",
                    );
                }
                if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                    validate_shared_temporary_continuation(
                        checked,
                        operations,
                        operation_index,
                        &producer,
                        argument,
                    )?;
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
                && producer.is_argument_temporary()
            {
                validate_shared_temporary_continuation(
                    checked,
                    operations,
                    operation_index,
                    &producer,
                    argument,
                )?;
            }
            consumed = argument.access == checked_trees::CheckedStructuralAccess::Owned;
        }
    }
    let mut returned_carrier = false;
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
        returned_carrier = true;
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
    // A reference result either transfers its live carrier through completion
    // or retires the producing call's exact loan. Retirement is an explicit
    // ReleaseReference, or state exit when the last use is the final statement.
    // Edge retirement still owns the carrier place and requires its discard;
    // an explicit release or a return transfer must not also discard it.
    if let Some(custody) = operations[producer.operation_index]
        .call()
        .and_then(|call| call.structural_custody())
        && custody.reference_loan.is_valid()
    {
        if returned_carrier {
            // Completion forwards the exact carrier and its outstanding loan;
            // it does not retire either as an owned consume or disposal.
            if producer.discard || disposed || !projected_paths.is_empty() {
                return unsupported("returned reference also acquired disposal custody");
            }
            return Ok(());
        }
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
                operations,
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
    // Retained loans are admitted by the call's recorded custody, as other
    // structural calls are, not by recognizing the caller's storage
    // spelling. The loan's release is checked by validate_usage.
    let loan_is_accounted = loan_free_call(checked, operation)
        || (!call.is_boundary()
            && call
                .structural_custody()
                .is_some_and(|custody| custody.reference_loan.is_valid()));
    // A structural call keeps its transfers in its custody record; a boundary
    // call's are its completion receipts.
    let (coordinate, structural_arguments, claim_transfers) = (
        call.coordinate,
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
    let authored = crate::call_sites::locate(checked, caller.state, coordinate)?;
    super::super::argument_schedule::build_operations(
        checked,
        caller.machine,
        caller.state,
        caller.operations,
    )?;
    let (source_machine, state) = crate::declarations::state_declaration(checked, caller.state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let signature =
        crate::declarations::target_signature(checked, caller.machine, authored.source_target)?;
    super::super::call_parameters::structural_parameters(checked, call)?;
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
                    crate::content_classes::closed_primitive_array(checked, source.type_reference)
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
            // Whole owned array transport must rejoin the exact source and
            // target payload. A loan's endpoint type and access intentionally
            // differ from its enclosing array: the ordinary borrow and typed
            // projection checks own that custody instead. The authored-position
            // partition above identifies the caller's source in either case.
            if argument.access == checked_trees::CheckedStructuralAccess::Owned
                && crate::content_classes::closed_primitive_array(checked, source_parameter.type_reference)
                && (!argument.path.is_empty()
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
            && parameter.fused_service_erasure.is_none()
            && claim_transfers
                .iter()
                .any(|transfer| transfer.argument_index as usize == index);
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
    let (_, state) = crate::declarations::state_declaration(checked, caller.state)?;
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
    if !crate::content_classes::closed_primitive_array(checked, local.type_reference) {
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
    let (_, state) = crate::declarations::state_declaration(checked, caller.state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return Ok(false);
    };
    let Some(capacity) =
        crate::content_classes::bounded_byte_capacity(checked, local.type_reference)
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
    authored: &crate::call_sites::AuthoredCall,
    source_machine: &checked_trees::machine::Machine,
    statements: &[StatementNode],
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    source_position: u32,
    expression: Option<checked_trees::expression::ExpressionHandle>,
    binding_ordinal: Option<u32>,
) -> Result<(), LoweringError> {
    // A shared argument observes the owned target of an explicit borrow; an
    // implicit shared argument already names that same value. Keep the shell
    // for every other access so this cannot turn an exclusive loan into one.
    let owner_expression =
        expression.map(
            |expression| match checked.expression_table.expression(expression) {
                ExpressionNode::Borrow(borrow)
                    if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                        && borrow.access == language_core::ReferenceAccess::Shared =>
                {
                    borrow.target
                }
                _ => expression,
            },
        );
    let shared_byte_formal =
        crate::declarations::target_signature(checked, caller.machine, authored.source_target)?
            .parameters
            .get(source_position as usize)
            .is_some_and(|formal| {
                shared_byte_view_referent(checked, formal.type_reference).is_some()
            });
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
            if names_result
                && (argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || !argument.path.is_empty()
                    || result.multiplicity != Multiplicity::Unrestricted
                    || !crate::content_classes::closed_primitive_array(checked, source_type)
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
            // The checked value root owns this argument occurrence independently
            // of its evaluator kind. Arrays, cases, records and selected values
            // share the same exact root/statement/owner join; their evaluators
            // retain the distinct establishment and transfer operations.
            let values = &checked.facts.values.structural_values;
            if !values.nodes.is_valid(*value) {
                return unsupported("structural argument operand has a stale value handle");
            }
            let node = values.nodes.get(*value);
            let root = values
                .root_for_expression(caller.state, result.statement_index, node.expression)
                .filter(|root| root.machine == caller.machine && root.root == *value)
                .ok_or(LoweringError::Unsupported(
                    "structural argument operand lost its exact checked root",
                ))?;
            let construction_expression = root.expression;
            let construction_type = root.type_reference;
            let names_result = result.statement_index == coordinate.statement_index
                && *call_ordinal == coordinate.call_ordinal
                && *parameter_position == source_position
                && owner_expression == Some(construction_expression);
            // The call takes the fresh value whole, including its disposition
            // obligation. The consuming edge proves transfer; a nominal hook
            // belongs to whichever owner eventually disposes the value.
            let copied_array = result.multiplicity == Multiplicity::Unrestricted
                && crate::content_classes::closed_primitive_array(checked, construction_type);
            let whole_owned_contents = copied_array
                || result.multiplicity != Multiplicity::Linear
                    && validation::has_structural_construction_contents(
                        &checked.typed,
                        construction_type,
                    )
                // Represented predicates belong to the declared value, not
                // to a particular producer kind. Establishment remains with
                // that producer; whole transfer does not mint memberships.
                || (result.multiplicity == Multiplicity::Unrestricted
                    && crate::content_classes::plain_owned_with_represented_constraints(checked, construction_type));
            let contract =
                validation::structural_result_contract(&checked.typed, construction_type)
                    .map_err(LoweringError::Unsupported)?;
            if names_result != (binding_ordinal == Some(result.binding_ordinal)) {
                return unsupported("constructed operand names another completed owner");
            }
            if names_result
                && ((authored.boundary && !copied_array)
                    || !matches!(
                        argument.access,
                        checked_trees::CheckedStructuralAccess::Owned
                            | checked_trees::CheckedStructuralAccess::SharedBorrow
                    )
                    || !argument.path.is_empty()
                    || !whole_owned_contents
                    || checked
                        .typed
                        .normalized_type_identity(contract.carrier)
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
                    if crate::content_classes::bounded_byte_capacity(checked, local.type_reference).is_some());
        let names_result = if producer_coordinate.call_ordinal == 0 {
            let Some(StatementNode::LocalData(local)) =
                statements.get(result.statement_index as usize)
            else {
                return unsupported("Unit structural result producer has no authored local");
            };
            // The element-declared view spelling (`let v: u8 = coll[a..b]`)
            // binds the subslice's own result type, not the authored element;
            // a declared view keeps its spelling. Without an authored state
            // the declaration is taken as written.
            let binding_type = crate::declarations::state_declaration(
                checked,
                caller.state,
            )
            .map(|(machine, state)| {
                crate::expression_preparation::source_custody::structural::declared_view_binding_type(
                    checked,
                    machine,
                    state,
                    local.type_reference,
                    local.initial_value,
                )
            })
            .unwrap_or(local.type_reference);
            // A `&T` producer keeps the authored reference on its local but
            // joins the referent's carrier: compare the slice/record the
            // view names, not the borrow shell. A `&mut` producer's carrier
            // peels the same way — view/named referees join the view or
            // record carrier, primitives keep the `ref-mut` shell.
            let view_carrier = crate::expression_preparation::source_custody::structural::shared_slice_view_referent(
                checked,
                binding_type,
            )
            .or(
                crate::expression_preparation::source_custody::structural::mutable_view_carrier(
                    checked,
                    binding_type,
                ),
            );
            // The producer roster includes every local, even while rejoining
            // another operand. Copy cases carry the same plain-owned custody
            // as arrays and records; represented byte predicates stay attached
            // to the established home, and its exact carrier is checked below.
            if !local.symbol.is_valid()
                || (result.multiplicity == Multiplicity::Unrestricted
                    && !(crate::content_classes::plain_owned_with_represented_constraints(checked,
                        local.type_reference) || view_carrier.is_some()
                        // A bounded byte owner (`[u8; N] in Utf8`) keeps its
                        // declared domains; its carrier is the fixed array.
                        || crate::content_classes::bounded_byte_capacity(checked,
                            local.type_reference)
                        .is_some()))
                || checked
                    .typed
                    .normalized_type_identity(view_carrier.unwrap_or(
                        crate::unit::attached_unit::parameters::structural_carrier_type(
                            checked,
                            binding_type,
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
                        Some(crate::declarations::state_declaration(checked, caller.state)?.1),
                        local.type_reference,
                        expression,
                    )
                }) {
                Some(Ok((root, path, access))) if root == local.symbol => {
                    // A bare view local lends shared, or exclusively when the
                    // view itself is mutable and the formal asks for that.
                    let authored_access =
                        access.unwrap_or(if view_carrier.is_some() || implicit_shared_owner {
                            checked_trees::CheckedStructuralAccess::SharedBorrow
                        } else {
                            checked_trees::CheckedStructuralAccess::Owned
                        });
                    let exclusive_view = access.is_none()
                        && argument.access == checked_trees::CheckedStructuralAccess::MutableBorrow
                        && crate::expression_preparation::source_custody::structural::mutable_view_carrier(
                            checked,
                            binding_type,
                        )
                        .is_some();
                    if path != argument.path
                        || (authored_access != argument.access && !exclusive_view)
                    {
                        return unsupported(
                            "Unit result projection disagrees with its authored path",
                        );
                    }
                    true
                }
                _ => false,
            }
        } else {
            let source = crate::call_sites::locate(checked, caller.state, producer_coordinate)?;
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
                let signature = crate::declarations::target_signature(
                    checked,
                    source_machine.symbol,
                    source.source_target,
                )?;
                if result.multiplicity == Multiplicity::Unrestricted
                    && !crate::content_classes::plain_owned_with_numeric_constraints(
                        checked,
                        signature.return_type,
                    )
                    && !(crate::content_classes::bounded_byte_capacity(
                        checked,
                        signature.return_type,
                    )
                    .is_some()
                        && crate::content_classes::plain_owned_with_represented_constraints(
                            checked,
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
                    Some(crate::declarations::state_declaration(checked, caller.state)?.1),
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
