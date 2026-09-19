//! A selected owner retains its original alternatives, not a new establishment.
//! Rejoin receipts to actual declarations and occurrences before emission can
//! carry the selected owner and its ordered complement through a continuation.
use super::{
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, StatementNode, SymbolHandle,
    unsupported,
};
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, FlowOwnedSelectionReceipt,
};
use language_semantics::{
    Multiplicity, PermissionClaimIdentity, PermissionEventSource, PermissionProvenance,
};

pub(super) fn validate_receipt(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    receipt: &FlowOwnedSelectionReceipt,
) -> Result<(), LoweringError> {
    let (owner, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let statements = checked.statement_table.statements(authored.statement_nodes);
    let (expression, destination, reference) = match statements.get(statement as usize) {
        Some(StatementNode::LocalData(local)) if !local.is_mutable => {
            (local.initial_value, local.symbol, local.type_reference)
        }
        Some(StatementNode::Expression(expression))
            if statement as usize + 1 == statements.len() =>
        {
            (*expression, SymbolHandle::invalid(), authored.return_type)
        }
        _ => return unsupported("selected ownership has no exact value destination"),
    };
    if owner.symbol != machine
        || receipt.machine != machine
        || receipt.state != state
        || receipt.statement_ordinal != statement
        || receipt.expression != expression
        || receipt.destination != destination
        || receipt.type_reference != reference
        || receipt.death != PermissionEventSource::StateExit
        || checked.type_multiplicity(reference) != Multiplicity::Affine
        // A whole affine carrier with `[linear]` members joins through the
        // same receipt: the claim set on each transfer names the discharged
        // frontier exactly, so the destination's plain-storage rule widens to
        // the linear-tolerant carrier rule.
        || !validation::has_linear_owned_contents(&checked.typed, reference)
        || !matches!(
            checked.expression_table.expression(expression),
            ExpressionNode::Match(_)
        )
    {
        return unsupported("selected ownership substituted its destination or death edge");
    }
    let ownership = &checked.facts.flow.ownership;
    let sources =
        ownership
            .selection_sources
            .span(receipt.sources)
            .ok_or(LoweringError::Unsupported(
                "selected ownership has a stale source span",
            ))?;
    let transfers = ownership
        .selection_transfers
        .span(receipt.transfers)
        .ok_or(LoweringError::Unsupported(
            "selected ownership has a stale transfer span",
        ))?;
    if transfers.is_empty()
        || (sources.is_empty() && transfers.iter().any(|transfer| transfer.source.is_valid()))
    {
        return unsupported("selected ownership omitted its source alternatives");
    }
    let parameters = checked.state_parameters(authored);
    let mut previous: Option<(bool, u32)> = None;
    for (ordinal, source) in sources.iter().enumerate() {
        // A source row names either an established local (statement ordinal)
        // or an owned parameter (authored position). Parameters precede every
        // statement local in establishment order, so the receipt keeps them
        // after all local rows in descending order.
        let parameter = parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == source.symbol);
        let (source_reference, is_parameter) = if let Some((position, parameter)) = parameter {
            if parameter.is_self
                || parameter.is_const
                || parameter.is_mutable
                || position != source.statement_ordinal as usize
            {
                return unsupported(
                    "selected ownership changed its exact source or declaration order",
                );
            }
            (parameter.type_reference, true)
        } else {
            let Some(StatementNode::LocalData(local)) =
                statements.get(source.statement_ordinal as usize)
            else {
                return unsupported("selected ownership source has no local declaration");
            };
            if source.symbol != local.symbol || local.is_mutable || !local.initial_value.is_valid()
            {
                return unsupported(
                    "selected ownership changed its exact source or declaration order",
                );
            }
            (local.type_reference, false)
        };
        let key = (!is_parameter, source.statement_ordinal);
        if previous.is_some_and(|previous| key >= previous) {
            return unsupported("selected ownership changed its exact source or declaration order");
        }
        previous = Some(key);
        let source_handle =
            arena::Handle::from_parts(
                receipt
                    .sources
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("selected source ordinal overflow")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "selected source handle overflow",
                    ))?,
                receipt.sources.start().generation(),
            );
        // A whole-leaf source carries the result type at its root. A source
        // moved only through projected children instead keeps its own root
        // type; the projected leaf type is replayed on each transfer below.
        let whole = transfers.iter().any(|transfer| {
            transfer.source == source_handle
                && ownership.segments.span_or_empty(transfer.path).is_empty()
        });
        let projected = !whole
            && transfers.iter().all(|transfer| {
                transfer.source != source_handle
                    || !ownership.segments.span_or_empty(transfer.path).is_empty()
            });
        if (whole
            && checked.normalized_type_identity(source_reference)
                != checked.normalized_type_identity(reference))
            || (projected
                && !(checked.type_multiplicity(source_reference) == Multiplicity::Affine
                    && validation::has_linear_owned_contents(&checked.typed, source_reference)))
            || source.claim_identity != PermissionClaimIdentity::Unknown
            || sources[..ordinal]
                .iter()
                .any(|prior| prior.symbol == source.symbol)
        {
            return unsupported("selected ownership changed its exact source or declaration order");
        }
        if is_parameter {
            // A parameter's ownership enters at state entry; it can never
            // originate from a prior selection statement.
            if source.origin_selection.is_valid()
                || source.provenance
                    != (PermissionProvenance::Established {
                        machine_symbol: machine,
                        state_symbol: state,
                        source: PermissionEventSource::StateEntry,
                    })
            {
                return unsupported("selected ownership minted or lost source provenance");
            }
            if !transfers
                .iter()
                .any(|transfer| transfer.source == source_handle)
            {
                return unsupported("selected ownership added an unauthored residual owner");
            }
            continue;
        }
        if source.origin_selection.is_valid() {
            if !ownership.owned_selections.is_valid(source.origin_selection)
                || ownership
                    .owned_selection_at(state, source.statement_ordinal)
                    .is_none_or(|(handle, _)| handle != source.origin_selection)
            {
                return unsupported("selected ownership lost its prior selection origin");
            }
            let prior = ownership.owned_selections.get(source.origin_selection);
            if prior.machine != machine
                || prior.state != state
                || prior.statement_ordinal != source.statement_ordinal
                || prior.destination != source.symbol
                || source.provenance != PermissionProvenance::Unknown
            {
                return unsupported("selected ownership substituted its prior result origin");
            }
            // A prior selection's destination arrives on a join block
            // parameter. The terminal verifier resolves a block parameter's
            // declared root type from its target block's parameter roster, so
            // projected moves out of that root carry the same partial-affine
            // residual custody as signature-parameter and operation-result
            // roots.
        } else if source.provenance
            != (PermissionProvenance::Established {
                machine_symbol: machine,
                state_symbol: state,
                source: PermissionEventSource::Statement {
                    statement_index: source.statement_ordinal as usize,
                },
            })
            || ownership
                .owned_selection_at(state, source.statement_ordinal)
                .is_some()
        {
            return unsupported("selected ownership minted or lost source provenance");
        }
        if !transfers
            .iter()
            .any(|transfer| transfer.source == source_handle)
        {
            return unsupported("selected ownership added an unauthored residual owner");
        }
    }
    for (ordinal, transfer) in transfers.iter().enumerate() {
        if !transfer.source_arm.is_valid()
            || transfers[..ordinal]
                .iter()
                .any(|prior| prior.expression == transfer.expression)
        {
            return unsupported("selected ownership reused or substituted a transfer occurrence");
        }
        if !transfer.source.is_valid() {
            // A call's structural product holds no local roster entry: the
            // product is emitted on its own arm and its residual complement
            // dies on that edge. The leaf must be an exact projection whose
            // root is the producing call.
            if ownership.segments.span_or_empty(transfer.path).is_empty()
                || projected_call_root(checked, transfer.expression).is_none()
            {
                return unsupported("selected ownership added an unauthored product transfer");
            }
            continue;
        }
        if !ownership.selection_sources.is_valid(transfer.source)
            || transfer.source.generation() != receipt.sources.start().generation()
            || transfer
                .source
                .arena_index()
                .checked_sub(receipt.sources.start().arena_index())
                .is_none_or(|offset| offset as usize >= sources.len())
        {
            return unsupported("selected ownership reused or substituted a transfer occurrence");
        }
        let source = ownership.selection_sources.get(transfer.source);
        // A field leaf inside a record arm fronts its own member type rather
        // than the result type, so the claim frontier replays from the leaf's
        // declared type.
        let leaf_reference = validation::expression_result_type_reference(
            &checked.typed,
            owner,
            authored,
            transfer.expression,
        )
        .ok_or(LoweringError::Unsupported(
            "selected ownership leaf has no declared type",
        ))?;
        let authored =
            validation::affine_owned_value_source(&checked.typed, transfer.expression, reference)
                .or_else(|| projected_local_root(checked, transfer.expression));
        if authored != Some(source.symbol) {
            return unsupported("selected ownership changed the authored source place");
        }
        // The consumed claim set is the leaf type's whole linear frontier
        // under the recorded moved path, replayed independently: a whole
        // carrier names every claim its children carry, a plain leaf names
        // none, and each row's provenance must agree with the source's own
        // establishment evidence when that evidence is known. A field leaf
        // inside a record arm fronts its own member type rather than the
        // result type, so the frontier replays from the leaf's declared type.
        let leaf_path = ownership.segments.span_or_empty(transfer.path);
        let expected_claims = validation::linear_claim_frontier(&checked.typed, leaf_reference);
        let claims = ownership
            .selection_transfer_claims
            .span_or_empty(transfer.claims);
        if claims.len() != expected_claims.len() {
            return unsupported("selected ownership changed its consumed claim set");
        }
        for (claim, expected) in claims.iter().zip(expected_claims.iter()) {
            let claim_path = ownership.segments.span_or_empty(claim.path);
            if claim_path.len() != leaf_path.len() + expected.path.len()
                || !claim_path.starts_with(leaf_path)
                || claim_path[leaf_path.len()..] != expected.path[..]
            {
                return unsupported("selected ownership changed its consumed claim set");
            }
            if source.provenance != PermissionProvenance::Unknown
                && claim.provenance != source.provenance
            {
                return unsupported("selected ownership claim disagrees with its source origin");
            }
        }
    }
    Ok(())
}

/// Walk an exact member/fixed-index chain to its whole-local name root. The
/// leaf-side structural checker is authoritative for path, type, and custody;
/// this root lookup only names the roster source a projected transfer must
/// carry.
fn projected_local_root(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let mut cursor = expression;
    loop {
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
            ExpressionNode::Name(name)
                if name.symbol.is_valid()
                    && name.head_symbol == name.symbol
                    && checked
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1 =>
            {
                return Some(name.symbol);
            }
            _ => return None,
        }
    }
}

/// The producing call occurrence beneath an exact member/fixed-index chain,
/// when the projected root is a call's structural product rather than a local.
fn projected_call_root(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut cursor = expression;
    loop {
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
            ExpressionNode::Call(_) => return Some(cursor),
            _ => return None,
        }
    }
}

pub(super) fn validate_leaf(
    checked: &CheckedTrees,
    receipt: &FlowOwnedSelectionReceipt,
    expression: ExpressionHandle,
    source_arm: arena::Handle<checked_trees::expression::TableMatchArm>,
    place: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let ownership = &checked.facts.flow.ownership;
    let transfer = ownership
        .owned_selection_transfer(receipt, expression)
        .ok_or(LoweringError::Unsupported(
            "selected place has no unique transfer receipt",
        ))?;
    if !ownership.selection_sources.is_valid(transfer.source) {
        return unsupported("selected place has a stale source owner");
    }
    let source = ownership.selection_sources.get(transfer.source);
    if transfer.source_arm != source_arm
        || !source_plan_matches(checked, receipt.state, source.symbol, &place.source)
        || !place.path.is_empty()
        || place.access != checked_trees::CheckedStructuralAccess::Owned
        || place.type_identity
            != checked
                .normalized_type_identity(receipt.type_reference)
                .as_str()
    {
        return unsupported("selected place changed its transfer, type or access");
    }
    Ok(())
}

/// The checked source plan a selected leaf carries for its roster root. A
/// local keeps its `StructuralLocal` symbol; a parameter arrives either as
/// the case-source `StructuralLocal` form or as the record-place `Parameter`
/// index, which counts the authored parameter list filtered to non-const,
/// non-primitive entries.
pub(super) fn source_plan_matches(
    checked: &CheckedTrees,
    state: SymbolHandle,
    symbol: SymbolHandle,
    plan: &CheckedUnitStructuralArgumentSourcePlan,
) -> bool {
    let (_, authored) =
        match crate::expression_preparation::source_custody::authored_state(checked, state) {
            Ok(authored) => authored,
            Err(_) => return false,
        };
    let parameters = checked.state_parameters(authored);
    if parameters
        .iter()
        .all(|parameter| parameter.symbol != symbol)
    {
        return *plan == (CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol });
    }
    let filtered = parameters
        .iter()
        .filter(|parameter| {
            !parameter.is_const
                && checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .position(|parameter| parameter.symbol == symbol)
        .and_then(|index| u32::try_from(index).ok());
    match plan {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: plan_symbol,
        } => *plan_symbol == symbol,
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            Some(*parameter_index) == filtered
        }
        _ => false,
    }
}

/// The rebuilt root of one projected selection leaf: its authored occurrence
/// and declared/result type.
pub(super) struct ProjectionRoot {
    pub(super) expression: ExpressionHandle,
    pub(super) reference: checked_trees::types::TypeReferenceHandle,
}

/// Replay one projected selection leaf against the authored expression. The
/// exact member/fixed-index chain is rebuilt through each receiver's declared
/// type and must equal both the recorded transfer path (fact segments) and
/// the retained node path (checked segments); the projected leaf type is the
/// receipt's result type. Returns the root occurrence so the caller can
/// validate its place or producing call.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_projection(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    receipt: &FlowOwnedSelectionReceipt,
    expression: ExpressionHandle,
    source_arm: arena::Handle<checked_trees::expression::TableMatchArm>,
    source_node: &checked_trees::CheckedStructuralValue,
    path: &[CheckedUnitStructuralPathSegment],
    type_identity: &str,
) -> Result<ProjectionRoot, LoweringError> {
    let ownership = &checked.facts.flow.ownership;
    let transfer = ownership
        .owned_selection_transfer(receipt, expression)
        .ok_or(LoweringError::Unsupported(
            "projected leaf has no unique transfer receipt",
        ))?;
    if transfer.source_arm != source_arm || path.is_empty() {
        return unsupported("projected leaf changed its transfer arm or path");
    }
    let mut segments = Vec::new();
    let mut checked_path = Vec::new();
    let mut cursor = expression;
    let root_expression = loop {
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return unsupported("projected selection cannot move through a case");
                }
                let receiver = validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(state),
                    member.receiver,
                )
                .and_then(|reference| {
                    validation::unwrapped_type_reference(&checked.typed, reference)
                })
                .ok_or(LoweringError::Unsupported(
                    "projected selection receiver has no declared type",
                ))?;
                let TypeReferenceNode::Named { symbol, .. } =
                    checked.type_reference_table.type_reference(receiver)
                else {
                    return unsupported("projected selection receiver is not a named record");
                };
                let owner = checked
                    .data_definitions()
                    .iter()
                    .find(|owner| owner.symbol == *symbol)
                    .ok_or(LoweringError::Unsupported(
                        "projected selection field owner is absent",
                    ))?;
                let field = validation::exact_data_member_field(
                    &checked.typed,
                    owner,
                    member.member_symbol,
                    member.member.as_str(),
                    None,
                )
                .ok_or(LoweringError::Unsupported(
                    "projected selection field disagrees with its declaration",
                ))?;
                if field.relevance.is_erased() {
                    return unsupported("projected selection cannot select an erased field");
                }
                segments.push(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
                checked_path.push(CheckedUnitStructuralPathSegment::Field(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ));
                cursor = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) =
                    checked.expression_table.expression(indexed.index)
                else {
                    return unsupported("projected selection requires a literal fixed index");
                };
                let index = index
                    .value_bignum()
                    .and_then(|value| value.to_u64())
                    .ok_or(LoweringError::Unsupported(
                        "projected selection index exceeds u64",
                    ))?;
                let reference = validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(state),
                    indexed.collection,
                )
                .and_then(|reference| {
                    validation::unwrapped_type_reference(&checked.typed, reference)
                })
                .ok_or(LoweringError::Unsupported(
                    "projected selection index has no declared collection",
                ))?;
                let TypeReferenceNode::FixedArray {
                    length: checked_trees::types::FixedArrayLength::Literal(length),
                    ..
                } = checked.type_reference_table.type_reference(reference)
                else {
                    return unsupported("projected selection index has no literal array length");
                };
                if usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index >= *length)
                {
                    return unsupported("projected selection index is out of bounds");
                }
                segments.push(facts::PlaceSegment::FixedIndex {
                    index: usize::try_from(index).map_err(|_| {
                        LoweringError::Unsupported("projected selection index exceeds usize")
                    })?,
                });
                checked_path.push(CheckedUnitStructuralPathSegment::FixedIndex(index));
                cursor = indexed.collection;
            }
            ExpressionNode::Name(_) | ExpressionNode::Call(_) => break cursor,
            _ => {
                return unsupported("projected selection root is not a local or call product");
            }
        }
    };
    segments.reverse();
    checked_path.reverse();
    if segments.is_empty()
        || checked_path.as_slice() != path
        || ownership.segments.span_or_empty(transfer.path) != segments.as_slice()
    {
        return unsupported("projected selection changed its authored path");
    }
    let leaf_reference =
        validation::expression_result_type_reference(&checked.typed, machine, state, expression)
            .ok_or(LoweringError::Unsupported(
                "projected selection leaf has no declared type",
            ))?;
    // The projected leaf type is the moved child's own declared carrier: the
    // arm-value leaf's carrier is the result type, while a record-field leaf
    // fronts its member type. Either way the retained node's type identity
    // pins it; the receipt's result type constrains only arm-level leaves.
    if checked.normalized_type_identity(leaf_reference).as_str() != type_identity {
        return unsupported("projected selection changed its moved child type");
    }
    let root_reference = validation::expression_result_type_reference(
        &checked.typed,
        machine,
        state,
        root_expression,
    )
    .ok_or(LoweringError::Unsupported(
        "projected selection root has no declared type",
    ))?;
    if checked.type_multiplicity(root_reference) != Multiplicity::Affine
        || !validation::has_linear_owned_contents(&checked.typed, root_reference)
    {
        return unsupported("projected selection root is not an affine owner");
    }
    let symbol = match checked.expression_table.expression(root_expression) {
        ExpressionNode::Name(name)
            if name.symbol.is_valid()
                && name.head_symbol == name.symbol
                && checked
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    == 1 =>
        {
            Some(name.symbol)
        }
        ExpressionNode::Call(_) => None,
        _ => {
            return unsupported("projected selection root is not a local or call product");
        }
    };
    if source_node.expression != root_expression {
        return unsupported("projected selection substituted its root occurrence");
    }
    // The source plan must carry the root exactly: a whole-owned local place
    // with the roster source on the transfer, or a producing call holding no
    // roster entry at all.
    match &source_node.kind {
        checked_trees::CheckedStructuralValueKind::Place(argument) => {
            let Some(symbol) = symbol else {
                return unsupported("projected local root lost its symbol");
            };
            if !ownership.selection_sources.is_valid(transfer.source)
                || ownership.selection_sources.get(transfer.source).symbol != symbol
                || !source_plan_matches(checked, state.symbol, symbol, &argument.source)
                || !argument.path.is_empty()
                || argument.access != checked_trees::CheckedStructuralAccess::Owned
                || argument.type_identity
                    != checked.normalized_type_identity(root_reference).as_str()
            {
                return unsupported("projected selection changed its local root place");
            }
        }
        checked_trees::CheckedStructuralValueKind::Call { .. } => {
            if transfer.source.is_valid() {
                return unsupported("projected selection product held a roster source");
            }
        }
        _ => {
            return unsupported("projected selection requires a place or call root");
        }
    }
    Ok(ProjectionRoot {
        expression: root_expression,
        reference: root_reference,
    })
}
