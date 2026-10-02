//! Stage call ranges at their authored argument positions.
//!
//! Ordinary and composed bodies provide the current scalar/storage bindings;
//! view_ranges owns collection resolution, descriptor establishment and bounds
//! replay. Neither caller result type nor storage origin selects another route.
use super::view_ranges::{self, Range, ViewRangeSite};
use super::{
    CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError, PlaceId,
    StructuralPlaceDeclaration, StructuralTypeId, ValueDeclaration, allocate_dense,
    literal_argument_places, lookup_type_id, place_id, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::expression_preparation::bindings::ScalarBindings;
use checked_trees::{CheckedSubsliceSite, CheckedUnitStructuralArgumentPlan};

pub(super) fn arguments(
    operation: &CheckedUnitEffectOperationPlan,
) -> &[CheckedUnitStructuralArgumentPlan] {
    operation.call_structural_arguments()
}

pub(super) use super::view_ranges::range;

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
    let Some(call) = operation.call() else {
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
    if !matches!(
        argument.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
            | checked_trees::CheckedStructuralAccess::MutableBorrow
    ) {
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

/// The current bindings and type namespace a staged range reads.
pub(super) struct StagingContext<'a> {
    pub(super) state: symbols::SymbolHandle,
    pub(super) structural_types: &'a [terminal_psi::StructuralTypeDeclaration],
    pub(super) type_ids: &'a [(String, StructuralTypeId)],
    /// The scalar namespace the call's authored operands read.
    pub(super) bindings: &'a ScalarBindings,
}

/// Emit the call's `ordinal`th structural argument, a range over a descriptor
/// or array backing. Every place it
/// establishes joins `temporaries`; the narrowed view's place is returned.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
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
    let (argument, _, site) = locate(checked, context.state, operation, ordinal)?;
    let structural_type = lookup_type_id(context.type_ids, &argument.type_identity)?;
    let destination = place_id(allocate_dense(next_place)?);
    let storage = view_ranges::storage(
        checked,
        context.state,
        site.statement,
        argument,
        context.structural_types,
    )?;
    let whole = storage
        .requires_establishment()
        .then(|| allocate_dense(next_place).map(place_id))
        .transpose()?;
    let (source, establishment) = storage.bind(checked, context.state, context.bindings, whole)?;
    if let Some(establishment) = establishment {
        temporaries.push(view_ranges::establish(&establishment, operations));
    }
    let declaration = view_ranges::emit(
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
    )?;
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
