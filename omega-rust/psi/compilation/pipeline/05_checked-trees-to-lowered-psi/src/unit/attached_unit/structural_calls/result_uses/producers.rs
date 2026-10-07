//! Producer-side machinery for structural result validation: the producing
//! `Producer` record, projected-move coverage, residual-field cleanup
//! freedom, and the whole-local array/owner loan shapes a consumer may cite.
use super::super::super::admission::CallerView;
use super::super::{
    CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError, Multiplicity, StatementNode,
    unsupported,
};
use super::loan_free_call;
use checked_trees::types::TypeReferenceNode;
use checked_trees::{CheckedUnitCallPlan, CheckedUnitStructuralResultBindingPlan};

pub(super) struct Producer<'plan> {
    pub(super) operation_index: usize,
    pub(super) coordinate: checked_trees::CheckedUnitCallCoordinate,
    pub(super) result: &'plan CheckedUnitStructuralResultBindingPlan,
    pub(super) discard: bool,
    pub(super) construction_source: Option<checked_trees::CheckedArrayConstructionSource>,
}

/// A completed anonymous owner can be lent only through a loan-free call.
/// Affine owners die at the recorded normal continuation, independently of
/// whether a nested invocation or an argument construction established them.
/// The checked disposition owns the exact complement: hooks and maximal
/// residual subtrees need not look like one whole-root trivial discard.
pub(super) fn validate_shared_temporary_continuation(
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
pub(super) fn loaned_record_projection(
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
pub(super) fn projected_moves_cover_result(
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

pub(super) fn projected_moves_cover_type(
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
pub(super) fn no_cleanup_residual_field(
    field_type: &checked_trees::CheckedUnitStructuralFieldType,
) -> bool {
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
    pub(super) fn is_argument_temporary(&self) -> bool {
        self.coordinate.call_ordinal != 0
            || matches!(
                self.construction_source,
                Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. })
            )
    }

    pub(super) fn precedes_consumer(
        &self,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
    ) -> bool {
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

pub(super) fn producer(
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
pub(super) fn shared_view_binding(
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

pub(super) fn whole_local_array_loan(
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
pub(super) fn whole_local_bounded_owner_loan(
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

pub(super) fn shared_byte_view_referent(
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
