//! View ranges passed as call arguments (`f(view[a..b])`,
//! `f(self.items[a..b])`) are evaluated at their authored call positions.
//! The range replay and its Terminal operation are shared with every other
//! view-narrowing site through `view_ranges`; this module locates one call's
//! range arguments, resolves the places they narrow and stages them.
//!
//! Both Unit routes use `argument_schedule` to stage each range at its
//! authored argument position. The ordinary body calls `emit`; a composed
//! state uses `composed_control::literal_arguments::stage_range`, then
//! `emit_staged`, with that state's current scalar and storage bindings.
//! A terminal call in a guarded transition arm consumes the same schedule
//! inside its selected arm. (The module predates
//! element ranges and keeps its byte-era name; it stages both families.)
//! Both resolve sources through `view_source` and `root_parameter`, so a
//! range means the same view on either route:
//!
//! - a whole immutable view parameter of the state. A byte view parameter may
//!   carry primitive byte qualifications: they stay with the source, and the
//!   narrowed view is the unqualified subslice result Terminal defines;
//! - an immutable view local an earlier statement established;
//! - a fixed-array field below a readable structural parameter. The range has
//!   no view to narrow yet, so the route first establishes the whole field
//!   view (`view_ranges::establish_field_view`) into a fresh place and
//!   narrows that. Its extra place needs a declaration roster the route owns;
//!   only a composed state's private temporaries can take it today, so the
//!   ordinary route refuses a field range by name.
//!
//! The ordinary route also stages only byte ranges: it binds endpoints in a
//! namespace built from its parameter and view-local rosters alone, without
//! the element-view family of its parameters an element length observation
//! would need.
use super::super::{StructuralAccess, StructuralParameterDeclaration};
use super::view_ranges::{self, ViewRangeSite, ViewRangeSource};
use super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError,
    PlaceId, StructuralMultiplicity, StructuralPlaceDeclaration, StructuralTypeId,
    ValueDeclaration, allocate_dense, literal_argument_places, lookup_type_id, place_id,
    unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::view_subslice::ViewFamily;
use crate::expression_preparation::bindings::{ScalarBindings, view_locals};
use checked_trees::expression::ExpressionHandle;
use checked_trees::{
    CheckedScalarExpression, CheckedStorageRoot, CheckedSubsliceSite, CheckedUnitCallResult,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan,
};

pub(super) fn arguments(
    operation: &CheckedUnitEffectOperationPlan,
) -> &[CheckedUnitStructuralArgumentPlan] {
    operation.call_structural_arguments()
}

/// One range argument as its checked plan retains it.
pub(super) struct Range<'a> {
    pub(super) root: CheckedStorageRoot,
    pub(super) family: ViewFamily,
    pub(super) expression: ExpressionHandle,
    pub(super) start: &'a Option<CheckedScalarExpression>,
    pub(super) end: &'a Option<CheckedScalarExpression>,
}

/// The range `argument` narrows, of either view family.
pub(super) fn range(argument: &CheckedUnitStructuralArgumentPlan) -> Option<Range<'_>> {
    let (root, family, expression, start, end) = match &argument.source {
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root,
            expression,
            start,
            end,
        } => (*root, ViewFamily::Bytes, *expression, start, end),
        CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root,
            expression,
            start,
            end,
        } => (*root, ViewFamily::Elements, *expression, start, end),
        _ => return None,
    };
    Some(Range {
        root,
        family,
        expression,
        start,
        end,
    })
}

pub(super) fn contains(operation: &CheckedUnitEffectOperationPlan) -> bool {
    arguments(operation)
        .iter()
        .any(|argument| range(argument).is_some())
}

/// The call's `ordinal`th structural argument, a range, at its authored
/// call-argument site.
fn locate<'a>(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    operation: &'a CheckedUnitEffectOperationPlan,
    ordinal: usize,
) -> Result<
    (
        &'a CheckedUnitStructuralArgumentPlan,
        Range<'a>,
        ViewRangeSite<'a>,
    ),
    LoweringError,
> {
    let argument = arguments(operation)
        .get(ordinal)
        .ok_or(LoweringError::Unsupported(
            "subslice has no checked argument",
        ))?;
    let range = range(argument).ok_or(LoweringError::Unsupported(
        "subslice schedule names a different argument kind",
    ))?;
    // Boundary calls of any result and ordinary Unit calls; ordinary scalar
    // and structural calls are refused.
    let Some(call) = operation
        .call()
        .filter(|call| call.is_boundary() || *call.result() == CheckedUnitCallResult::Unit)
    else {
        return unsupported("subslice schedule has no call coordinate");
    };
    let coordinate = call.coordinate;
    let authored =
        crate::emission::call_source_custody::authored::locate_source(checked, state, coordinate)?;
    let (machine, _) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let signature = crate::emission::call_source_custody::authored::target_signature(
        checked,
        machine.symbol,
        authored.source_target,
    )?;
    let positions = super::call_parameters::structural_positions(checked, call, &signature)?;
    let authored_ordinal = positions
        .get(ordinal)
        .and_then(|formal_position| {
            authored
                .structural_arguments
                .iter()
                .position(|(position, _)| position == formal_position)
        })
        .ok_or(LoweringError::Unsupported(
            "subslice disagrees with its authored call argument",
        ))?;
    if authored
        .structural_arguments
        .get(authored_ordinal)
        .is_none_or(|(_, source)| *source != range.expression)
    {
        return unsupported("subslice disagrees with its authored call argument");
    }
    if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
        return unsupported("subslice source or result changed immutable view custody");
    }
    let argument_ordinal = u32::try_from(authored_ordinal)
        .map_err(|_| LoweringError::Unsupported("subslice argument ordinal exceeds u32"))?;
    let site = ViewRangeSite {
        state,
        statement: coordinate.statement_index,
        site: CheckedSubsliceSite::CallArgument {
            call_ordinal: coordinate.call_ordinal,
            argument_ordinal,
        },
        expression: range.expression,
        retained: Some((range.start, range.end)),
    };
    Ok((argument, range, site))
}

/// The caller's structural parameters as the emitting route holds them.
pub(super) struct SourceParameters<'a> {
    /// Checked plans: their positions name authored formals.
    pub(super) plans: &'a [CheckedUnitStructuralParameterPlan],
    /// Lowered declarations at the same dense indexes.
    pub(super) declarations: &'a [StructuralParameterDeclaration],
    /// The place currently holding a parameter's place. A composed state's
    /// argument evaluation can rebind it through a private join.
    pub(super) current: &'a dyn Fn(PlaceId) -> PlaceId,
}

/// The parameter at a range's root: its lowered declaration, current place
/// and authored name.
fn root_parameter<'a>(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    parameters: &SourceParameters<'a>,
    index: u32,
) -> Result<
    (
        &'a StructuralParameterDeclaration,
        PlaceId,
        symbols::SymbolHandle,
    ),
    LoweringError,
> {
    let declaration =
        parameters
            .declarations
            .get(index as usize)
            .ok_or(LoweringError::Unsupported(
                "subslice source parameter is absent",
            ))?;
    let plan = parameters
        .plans
        .get(index as usize)
        .ok_or(LoweringError::Unsupported("subslice source plan is absent"))?;
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let symbol = checked
        .state_parameters(authored)
        .get(plan.position as usize)
        .ok_or(LoweringError::Unsupported(
            "subslice source has no authored parameter",
        ))?
        .symbol;
    Ok((declaration, (parameters.current)(declaration.place), symbol))
}

/// The established view a range over a parameter or view local narrows.
fn view_source(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    range: &Range<'_>,
    parameters: &SourceParameters<'_>,
    view_locals: &[view_locals::ViewLocalBinding],
) -> Result<ViewRangeSource, LoweringError> {
    match range.root {
        CheckedStorageRoot::Parameter { index } => {
            let (parameter, place, symbol) = root_parameter(checked, state, parameters, index)?;
            // Primitive byte qualifications stay with the source parameter;
            // the narrowed view is an unqualified subslice result.
            if parameter.access != StructuralAccess::SharedBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || (range.family == ViewFamily::Elements && !parameter.qualifications.is_empty())
                || !parameter.projected_qualifications.is_empty()
            {
                return unsupported("subslice source or result changed immutable view custody");
            }
            Ok(ViewRangeSource {
                symbol,
                place,
                structural_type: parameter.structural_type,
                family: range.family,
            })
        }
        CheckedStorageRoot::ViewLocal { symbol } => {
            let local = view_locals::resolve(view_locals, symbol)?;
            if (range.family == ViewFamily::Bytes)
                != (local.carrier == view_locals::ViewCarrier::Bytes)
            {
                return unsupported("subslice source local changed its view family");
            }
            Ok(ViewRangeSource {
                symbol,
                place: local.place,
                structural_type: local.structural_type,
                family: range.family,
            })
        }
        CheckedStorageRoot::OwnedLocal { .. } => {
            unsupported("subslice source is an owned local, not a view")
        }
    }
}

/// Ordinary-route emission of the call's `ordinal`th structural argument, an
/// exclusive byte range over an established byte view, at its authored call
/// position.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
    ordinal: usize,
    parameters: &[StructuralParameterDeclaration],
    structural_parameters: &[(u32, StructuralParameterDeclaration)],
    view_locals: &[view_locals::ViewLocalBinding],
    values: &[ValueDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let (argument, range, site) = locate(checked, plan.state, operation, ordinal)?;
    if !argument.path.is_empty() {
        return unsupported("an ordinary body declares no whole-field view for a field range");
    }
    if range.family != ViewFamily::Bytes {
        return unsupported("an ordinary body binds no element-view namespace for a range");
    }
    let structural_type = lookup_type_id(type_ids, &argument.type_identity)?;
    let source = view_source(
        checked,
        plan.state,
        &range,
        &SourceParameters {
            plans: &plan.structural_parameters,
            declarations: parameters,
            current: &|place| place,
        },
        view_locals,
    )?;
    let bindings = ScalarBindings::new(values.iter().map(|value| value.scalar_type))
        .with_structural_parameters(structural_parameters)
        .with_view_locals(view_locals);
    view_ranges::emit(
        checked,
        site,
        source,
        structural_type,
        place_id(allocate_dense(next_place)?),
        &bindings,
        values,
        next_value,
        operations,
    )
}

/// The composed-route state a staged range reads.
pub(super) struct StagingContext<'a> {
    pub(super) state: symbols::SymbolHandle,
    pub(super) parameters: SourceParameters<'a>,
    pub(super) view_locals: &'a [view_locals::ViewLocalBinding],
    pub(super) structural_types: &'a [terminal_psi::StructuralTypeDeclaration],
    pub(super) type_ids: &'a [(String, StructuralTypeId)],
    /// The scalar namespace the call's authored operands read.
    pub(super) bindings: &'a ScalarBindings,
}

/// Composed-route emission of the call's `ordinal`th structural argument, a
/// range of either family over a view or a fixed-array field. Every place it
/// establishes joins `temporaries`; the narrowed view's place is returned.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_staged(
    checked: &CheckedTrees,
    context: &StagingContext<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    ordinal: usize,
    values: &[ValueDeclaration],
    temporaries: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<PlaceId, LoweringError> {
    let (argument, range, site) = locate(checked, context.state, operation, ordinal)?;
    let structural_type = lookup_type_id(context.type_ids, &argument.type_identity)?;
    let destination = place_id(allocate_dense(next_place)?);
    let declaration = if argument.path.is_empty() {
        let source = view_source(
            checked,
            context.state,
            &range,
            &context.parameters,
            context.view_locals,
        )?;
        view_ranges::emit(
            checked,
            site,
            source,
            structural_type,
            destination,
            context.bindings,
            values,
            next_value,
            operations,
        )?
    } else {
        let CheckedStorageRoot::Parameter { index } = range.root else {
            return unsupported("subslice field range is not rooted at a parameter");
        };
        let (owner, owner_place, symbol) =
            root_parameter(checked, context.state, &context.parameters, index)?;
        let (source, whole) = view_ranges::establish_field_view(
            owner,
            owner_place,
            symbol,
            &argument.path,
            structural_type,
            range.family,
            context.structural_types,
            place_id(allocate_dense(next_place)?),
            operations,
        )?;
        temporaries.push(whole);
        view_ranges::emit_over_field(
            checked,
            site,
            source,
            &argument.path,
            structural_type,
            destination,
            context.bindings,
            values,
            next_value,
            operations,
        )?
    };
    temporaries.push(declaration);
    Ok(destination)
}

pub(super) fn argument_places(
    arguments: &[CheckedUnitStructuralArgumentPlan],
    literals: &[StructuralPlaceDeclaration],
    next_literal: &mut usize,
    derived: &[(usize, PlaceId)],
) -> Result<Vec<PlaceId>, LoweringError> {
    let mut places = Vec::new();
    for (ordinal, argument) in arguments.iter().enumerate() {
        if argument.byte_sequence_literal().is_some() {
            places.extend(literal_argument_places(
                std::slice::from_ref(argument),
                literals,
                next_literal,
            )?);
        } else if range(argument).is_some() {
            places.push(
                derived
                    .iter()
                    .find_map(|(index, place)| (*index == ordinal).then_some(*place))
                    .ok_or(LoweringError::Unsupported(
                        "subslice was not evaluated at its argument position",
                    ))?,
            );
        }
    }
    Ok(places)
}
