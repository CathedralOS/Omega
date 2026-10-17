//! Stage call ranges at their authored argument positions.
//!
//! Ordinary and composed bodies provide the current scalar/storage bindings;
//! view_ranges owns collection resolution, descriptor establishment and
//! narrowing. Neither caller result type nor storage origin selects another route.
use super::view_ranges::{self, ViewRangeSite};
use super::{
    CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError, PlaceId,
    StructuralPlaceDeclaration, StructuralTypeId, ValueDeclaration, allocate_dense,
    literal_argument_places, lookup_type_id, place_id, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::expression_preparation::bindings::ScalarBindings;
use checked_trees::CheckedUnitStructuralArgumentPlan;

pub(super) fn arguments(
    operation: &CheckedUnitEffectOperationPlan,
) -> &[CheckedUnitStructuralArgumentPlan] {
    operation.structural_operands()
}

pub(super) use super::view_ranges::range;

pub(super) fn contains(operation: &CheckedUnitEffectOperationPlan) -> bool {
    arguments(operation)
        .iter()
        .any(|argument| range(argument).is_some())
}

/// The call's `ordinal`th structural argument, a range, with its endpoints
/// keyed at the call-argument site stage 04 recorded on it.
fn locate<'a>(
    state: symbols::SymbolHandle,
    operation: &'a CheckedUnitEffectOperationPlan,
    ordinal: usize,
) -> Result<(&'a CheckedUnitStructuralArgumentPlan, ViewRangeSite<'a>), LoweringError> {
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
    if !matches!(
        argument.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
            | checked_trees::CheckedStructuralAccess::MutableBorrow
    ) {
        return unsupported("subslice source or result changed immutable view custody");
    }
    let site = ViewRangeSite {
        state,
        statement: call.coordinate.statement_index,
        range,
    };
    Ok((argument, site))
}

/// The current bindings and type namespace a staged range reads.
pub(super) struct StagingContext<'a> {
    pub(super) state: symbols::SymbolHandle,
    pub(super) structural_types: &'a [terminal_psi::StructuralTypeDeclaration],
    pub(super) type_ids: &'a [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    /// The scalar namespace the call's authored operands read.
    pub(super) bindings: &'a ScalarBindings,
}

/// Emit the call's `ordinal`th structural argument, a range over a descriptor
/// or array backing. Every place it
/// establishes joins `temporaries`; the narrowed view's place is returned.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    context: &StagingContext<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    ordinal: usize,
    evaluation: &mut super::argument_evaluation::Evaluation,
    source_value_count: usize,
    values: &mut Vec<ValueDeclaration>,
    temporaries: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut crate::emission::operation_emission::calls::CallEmissionContext<'_>,
) -> Result<PlaceId, LoweringError> {
    let (argument, site) = locate(context.state, operation, ordinal)?;
    let structural_type = lookup_type_id(context.type_ids, &argument.type_identity)?;
    let destination = place_id(allocate_dense(next_place)?);
    let storage = view_ranges::storage(
        identities,
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
    let (source, establishment) = storage.bind(
        identities,
        checked,
        context.state,
        context.bindings,
        whole,
        context.structural_types,
    )?;
    if let Some(establishment) = establishment {
        temporaries.push(view_ranges::establish(
            &establishment,
            values,
            next_value,
            operations,
        )?);
    }
    let declaration = view_ranges::emit(
        identities,
        checked,
        site,
        source,
        structural_type,
        destination,
        evaluation,
        source_value_count,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        calls,
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
