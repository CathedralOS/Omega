//! Source-bound byte ranges are evaluated at their authored call positions.
//! The range replay and its Terminal operation are shared with every other
//! view-narrowing site through `view_ranges`; this module only schedules the
//! call's byte-range arguments and resolves their source places.
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
use crate::expression_preparation::bindings::view_locals;
use checked_trees::{
    CheckedStorageRoot, CheckedSubsliceSite, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

pub(super) fn arguments(
    operation: &CheckedUnitEffectOperationPlan,
) -> &[CheckedUnitStructuralArgumentPlan] {
    operation.call_structural_arguments()
}

pub(super) fn contains(operation: &CheckedUnitEffectOperationPlan) -> bool {
    arguments(operation).iter().any(|argument| {
        matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
        )
    })
}

/// Emit the call's `ordinal`th structural argument, an exclusive byte range
/// over an established byte view, at its authored call position.
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
    let argument = arguments(operation)
        .get(ordinal)
        .ok_or(LoweringError::Unsupported(
            "subslice has no checked argument",
        ))?;
    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
        root,
        expression,
        start,
        end,
    } = &argument.source
    else {
        return unsupported("subslice schedule names a different argument kind");
    };
    let coordinate = match operation {
        CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
        | CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. } => *coordinate,
        _ => return unsupported("subslice schedule has no call coordinate"),
    };
    let authored = crate::emission::call_source_custody::authored::locate_source(
        checked, plan.state, coordinate,
    )?;
    if authored
        .structural_arguments
        .get(ordinal)
        .is_none_or(|(_, source)| source != expression)
    {
        return unsupported("subslice disagrees with its authored call argument");
    }
    let structural_type = lookup_type_id(type_ids, &argument.type_identity)?;
    if !argument.path.is_empty()
        || argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow
    {
        return unsupported("subslice source or result changed immutable byte-view custody");
    }
    let source = match *root {
        CheckedStorageRoot::Parameter {
            index: parameter_index,
        } => {
            let parameter =
                parameters
                    .get(parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "subslice source parameter is absent",
                    ))?;
            let source_parameter = plan
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported("subslice source plan is absent"))?;
            let (_, state) =
                crate::expression_preparation::source_custody::authored_state(checked, plan.state)?;
            let symbol = checked
                .state_parameters(state)
                .get(source_parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "subslice source has no authored parameter",
                ))?
                .symbol;
            if parameter.access != StructuralAccess::SharedBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return unsupported(
                    "subslice source or result changed immutable byte-view custody",
                );
            }
            ViewRangeSource {
                symbol,
                place: parameter.place,
                structural_type: parameter.structural_type,
                family: ViewFamily::Bytes,
            }
        }
        CheckedStorageRoot::ViewLocal { symbol } => {
            let local = view_locals::resolve(view_locals, symbol)?;
            if local.carrier != view_locals::ViewCarrier::Bytes {
                return unsupported("byte subslice source local is not a byte view");
            }
            ViewRangeSource {
                symbol,
                place: local.place,
                structural_type: local.structural_type,
                family: ViewFamily::Bytes,
            }
        }
    };
    let bindings = crate::expression_preparation::bindings::ScalarBindings::new(values.len())
        .with_structural_parameters(structural_parameters)
        .with_view_locals(view_locals);
    let argument_ordinal = u32::try_from(ordinal)
        .map_err(|_| LoweringError::Unsupported("subslice argument ordinal exceeds u32"))?;
    view_ranges::emit(
        checked,
        ViewRangeSite {
            state: plan.state,
            statement: coordinate.statement_index,
            site: CheckedSubsliceSite::CallArgument {
                call_ordinal: coordinate.call_ordinal,
                argument_ordinal,
            },
            expression: *expression,
            retained: Some((start, end)),
        },
        source,
        structural_type,
        place_id(allocate_dense(next_place)?),
        &bindings,
        values,
        next_value,
        operations,
    )
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
        } else if matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
        ) {
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
