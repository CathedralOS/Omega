//! Replay one checked view range at its authored site, then emit it through
//! the shared view-subslice emitter.
//!
//! Every Unit route that narrows an established view -- a call argument
//! (`byte_subslices.rs`), a Unit-graph edge transfer
//! (`composed_control/state_graph/subslices.rs`) and a view local's `let`
//! (`operation_frame.rs`) -- independently replays the same facts before it
//! emits anything: the authored expression is an exclusive builtin range over
//! the bare name of the checked root, each present endpoint has exactly one
//! source-bound `u64` fact at the range's `CheckedSubsliceSite`, and an
//! endpoint the plan retains equals that fact. Only the lookup of the source
//! place differs by route, so each route resolves `ViewRangeSource` itself.
//!
//! A view local's range over a fixed-array field (`self.items[a..b]`) names
//! no established view: its route first establishes a whole element view of
//! the field and passes that place as the source, together with the checked
//! field path. `emit_over_field` then replays the authored member chain to
//! that exact field in place of the bare view name.
use super::{CheckedTrees, LoweringError, unsupported};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::view_subslice::{self, ViewFamily};
use crate::expression_preparation::bindings::ScalarBindings;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedSubsliceSite,
    CheckedUnitStructuralPathSegment,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralPlaceDeclaration, ValueDeclaration};

/// The established view a range narrows, as the emitting route found it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ViewRangeSource {
    /// The authored name the range's collection must spell.
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) family: ViewFamily,
}

/// Where the range sits and what the checked plan retained for it.
pub(crate) struct ViewRangeSite<'a> {
    /// The checked plan's state, whose scalar facts key the endpoints.
    pub(crate) state: symbols::SymbolHandle,
    pub(crate) statement: u32,
    pub(crate) site: CheckedSubsliceSite,
    pub(crate) expression: ExpressionHandle,
    /// Endpoints a call-argument or binding plan retains; a transfer retains
    /// none and replays its site's facts alone.
    pub(crate) retained: Option<(
        &'a Option<CheckedScalarExpression>,
        &'a Option<CheckedScalarExpression>,
    )>,
}

/// Replay the range at `site` and emit `source[start..end]` into
/// `destination`, whose type is the narrowed view's (always the source's).
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    checked: &CheckedTrees,
    site: ViewRangeSite<'_>,
    source: ViewRangeSource,
    result_type: StructuralTypeId,
    destination: PlaceId,
    bindings: &ScalarBindings,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    emit_ranged(
        checked,
        site,
        source,
        &[],
        result_type,
        destination,
        bindings,
        values,
        next_value,
        operations,
    )
}

/// `emit` for a range whose authored collection is the fixed-array field
/// `collection` below the parameter `source.symbol` names. `source.place` is
/// the whole element view the route already established over that field.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_over_field(
    checked: &CheckedTrees,
    site: ViewRangeSite<'_>,
    source: ViewRangeSource,
    collection: &[CheckedUnitStructuralPathSegment],
    result_type: StructuralTypeId,
    destination: PlaceId,
    bindings: &ScalarBindings,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    if collection.is_empty() {
        return unsupported("view subslice field collection has no field path");
    }
    emit_ranged(
        checked,
        site,
        source,
        collection,
        result_type,
        destination,
        bindings,
        values,
        next_value,
        operations,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_ranged(
    checked: &CheckedTrees,
    site: ViewRangeSite<'_>,
    source: ViewRangeSource,
    collection: &[CheckedUnitStructuralPathSegment],
    result_type: StructuralTypeId,
    destination: PlaceId,
    bindings: &ScalarBindings,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let (start, end) = endpoints(checked, &site, source.symbol, collection, bindings)?;
    if result_type != source.structural_type {
        return unsupported("view subslice changed its view type");
    }
    view_subslice::emit(
        source.family,
        source.place,
        start.as_ref(),
        end.as_ref(),
        destination,
        result_type,
        values,
        next_value,
        operations,
    )
}

/// The site's lowered endpoints after replaying its authored range.
fn endpoints(
    checked: &CheckedTrees,
    site: &ViewRangeSite<'_>,
    source_symbol: symbols::SymbolHandle,
    collection: &[CheckedUnitStructuralPathSegment],
    bindings: &ScalarBindings,
) -> Result<
    (
        Option<LoweredDirectExpression>,
        Option<LoweredDirectExpression>,
    ),
    LoweringError,
> {
    let (machine, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, site.state)?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(site.expression)
    else {
        return unsupported("view subslice has no authored indexed expression");
    };
    let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index) else {
        return unsupported("view subslice has no authored range");
    };
    let root = if collection.is_empty() {
        indexed.collection
    } else {
        let (root, path) =
            crate::expression_preparation::source_custody::structural::authored_collection_path(
                checked,
                machine,
                authored,
                indexed.collection,
            )?;
        if path != collection {
            return unsupported("view subslice field range lost its checked field path");
        }
        root
    };
    let ExpressionNode::Name(path) = checked.expression_table.expression(root) else {
        return unsupported("view subslice lost its exclusive range or exact source name");
    };
    let members = checked.expression_table.name_path_members(path.members);
    // A field range's root is the owning parameter; an attached `self`
    // spells the machine's receiver rather than the parameter's own symbol.
    let names_source = members.len() == 1
        && ((path.symbol == source_symbol && path.head_symbol == source_symbol)
            || (!collection.is_empty()
                && members[0].is_self_receiver()
                && checked
                    .state_parameters(authored)
                    .iter()
                    .any(|parameter| parameter.is_self && parameter.symbol == source_symbol)));
    if range.end_inclusive || !source_symbol.is_valid() || !names_source {
        return unsupported("view subslice lost its exclusive range or exact source name");
    }
    if let Some((start, end)) = site.retained
        && (range.start.is_valid() != start.is_some() || range.end.is_valid() != end.is_some())
    {
        return unsupported("view subslice endpoint presence changed");
    }
    if checked.facts.operators.uses.iter().any(|(_, selected)| {
        selected.expression == site.expression
            && (selected.spelling != language_core::OperatorSpelling::Range
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
    }) || !validation::has_builtin_subslice_meaning(
        &checked.typed,
        machine,
        Some(authored),
        site.expression,
    ) {
        return unsupported("view subslice no longer selects the builtin range");
    }
    let endpoint = |authored_endpoint: ExpressionHandle,
                    role: CheckedScalarExpressionRole,
                    retained: Option<&Option<CheckedScalarExpression>>|
     -> Result<Option<LoweredDirectExpression>, LoweringError> {
        if !authored_endpoint.is_valid() {
            return Ok(None);
        }
        let (binding, selected) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(site.state, site.statement, role)
            .ok_or(LoweringError::Unsupported(
                "view subslice endpoint has no source-bound scalar plan",
            ))?;
        if binding.expression != authored_endpoint
            || binding.destination.is_valid()
            || selected.primitive_type() != Some(checked_trees::types::PrimitiveType::U64)
            || retained.is_some_and(|retained| retained.as_ref() != Some(selected))
        {
            return unsupported("view subslice endpoint differs from its source-bound plan");
        }
        bindings
            .expression_at(checked, site.state, site.statement, role)
            .map(Some)
    };
    Ok((
        endpoint(
            range.start,
            CheckedScalarExpressionRole::SubsliceStart { site: site.site },
            site.retained.map(|(start, _)| start),
        )?,
        endpoint(
            range.end,
            CheckedScalarExpressionRole::SubsliceEnd { site: site.site },
            site.retained.map(|(_, end)| end),
        )?,
    ))
}

/// The immutable view local an `EstablishViewSubslice` binds: its result
/// statement must be that `let`, whose initializer is exactly the retained
/// range and whose declared type is the result's view type.
pub(crate) fn binding_local<'a>(
    checked: &'a CheckedTrees,
    state: symbols::SymbolHandle,
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
) -> Result<&'a checked_trees::statement::TableLocalData, LoweringError> {
    let checked_trees::CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source } =
        operation
    else {
        return unsupported("view subslice binding has no producer");
    };
    let (checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
        expression,
        ..
    }
    | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
        expression,
        ..
    }) = &source.source
    else {
        return unsupported("view subslice binding lost its range source");
    };
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(authored.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("view subslice binding lost its authored local");
    };
    // Only a range over a fixed-array field, rooted at the parameter owning
    // it, carries a path: the field projection to the array it views whole.
    let field_range = matches!(
        source.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root: checked_trees::CheckedStorageRoot::Parameter { .. },
            ..
        }
    );
    if local.is_mutable
        || !local.symbol.is_valid()
        || local.initial_value != *expression
        || (!source.path.is_empty() && !field_range)
        || source.access != checked_trees::CheckedStructuralAccess::SharedBorrow
        || source.type_identity != result.type_identity
        || result.multiplicity != language_semantics::Multiplicity::Unrestricted
    {
        return unsupported("view subslice binding changed its local, range or custody");
    }
    Ok(local)
}
