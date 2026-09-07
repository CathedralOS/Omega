//! Rejoin anonymous partial-result permissions without reconstructing cleanup.

use super::*;
use checked_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use language_semantics::{
    PermissionAccess, PermissionEventKind, PermissionEventSource, PermissionProvenance,
};

pub(super) fn validate(
    checked: &CheckedTrees,
    partial: &CheckedPartialAffineUnitCleanupMachinePlan,
) -> Result<(), LoweringError> {
    let plan = &partial.machine;
    let [
        producer,
        consumer,
        CheckedUnitEffectOperationPlan::ReturnUnit { .. },
    ] = plan.operations.as_slice()
    else {
        return unsupported("anonymous partial permissions have no exact call schedule");
    };
    let (CheckedUnitEffectOperationPlan::StructuralCall { coordinate, .. }
    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. }) = producer
    else {
        return unsupported("anonymous partial permissions have no structural producer");
    };
    let authored_producer =
        crate::call_source_custody::authored::locate_source(checked, plan.state, *coordinate)?;
    let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
        authored_producer.source_site
    else {
        return unsupported("anonymous partial permissions have no expression root");
    };
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate: consumer_coordinate,
        structural_arguments,
        ..
    } = consumer
    else {
        return unsupported("anonymous partial permissions have no Unit consumer");
    };
    let [argument] = structural_arguments.as_slice() else {
        return unsupported("anonymous partial permissions have ambiguous projected operands");
    };
    let authored_consumer = crate::call_source_custody::authored::locate_source(
        checked,
        plan.state,
        *consumer_coordinate,
    )?;
    let signature = crate::call_source_custody::authored::target_signature(
        checked,
        plan.machine,
        authored_producer.source_target,
    )?;
    let mut states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == plan.machine && state.state_symbol == plan.state
        });
    let (_, state) = states.next().ok_or(LoweringError::Unsupported(
        "anonymous partial permissions have no captured state",
    ))?;
    if states.next().is_some() {
        return unsupported("anonymous partial permissions have ambiguous captured states");
    }
    let calls =
        checked
            .facts
            .flow
            .control
            .calls
            .span(state.calls)
            .ok_or(LoweringError::Unsupported(
                "anonymous partial permissions have a stale call span",
            ))?;
    if calls.len() != 2 {
        return unsupported("anonymous partial permissions have extra captured calls");
    }
    let call_source = |coordinate: checked_trees::CheckedUnitCallCoordinate,
                       target: symbols::SymbolHandle|
     -> Result<PermissionEventSource, LoweringError> {
        let mut matches = calls.iter().filter(|call| {
            call.statement_index == coordinate.statement_index as usize
                && call.call_ordinal == coordinate.call_ordinal as usize
        });
        let call = matches.next().ok_or(LoweringError::Unsupported(
            "anonymous partial permissions have no captured call",
        ))?;
        if matches.next().is_some() || call.target_symbol != target {
            return unsupported("anonymous partial permission call identity drifted");
        }
        Ok(PermissionEventSource::Call {
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
            target_symbol: call.target_symbol,
        })
    };
    let producer_source = call_source(*coordinate, authored_producer.source_target)?;
    let consumer_source = call_source(*consumer_coordinate, authored_consumer.source_target)?;
    let provenance = PermissionProvenance::Established {
        machine_symbol: plan.machine,
        state_symbol: plan.state,
        source: producer_source,
    };
    let mut events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == plan.machine
                && event.state_symbol == plan.state
                && event.root == facts::PlaceRoot::Expression(expression)
        });
    for index in 0..partial.residual_affine_discards.len() + 2 {
        let (kind, source, path) = match index {
            0 => (PermissionEventKind::Establish, producer_source, &[][..]),
            1 => (
                PermissionEventKind::Transfer,
                consumer_source,
                argument.path.as_slice(),
            ),
            _ => (
                PermissionEventKind::AffineDrop,
                consumer_source,
                partial.residual_affine_discards[index - 2].path.as_slice(),
            ),
        };
        let (_, event) = events.next().ok_or(LoweringError::Unsupported(
            "anonymous partial permission sequence is incomplete",
        ))?;
        if event.kind != kind
            || event.source != source
            || event.provenance != provenance
            || event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.obligation_live
        {
            return unsupported("anonymous partial permission ownership or order drifted");
        }
        let segments = checked
            .facts
            .flow
            .ownership
            .segments
            .span(event.segments)
            .ok_or(LoweringError::Unsupported(
                "anonymous partial permission path span is stale",
            ))?;
        validate_path(checked, signature.return_type, segments, path)?;
    }
    if events.next().is_some() {
        return unsupported("anonymous partial permission sequence has extra events");
    }
    Ok(())
}

/// Invert retained field identities through their exact source declaration.
/// The caller already reconstructs the complement; this only rejoins its paths.
fn validate_path(
    checked: &CheckedTrees,
    mut reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
    path: &[CheckedUnitStructuralPathSegment],
) -> Result<(), LoweringError> {
    if segments.len() != path.len() {
        return unsupported("anonymous partial permission path length drifted");
    }
    for (segment, expected) in segments.iter().zip(path) {
        if !reference.is_valid() {
            return unsupported("anonymous partial permission path has no source type");
        }
        match (
            segment,
            expected,
            checked.type_reference_table.type_reference(reference),
        ) {
            (
                facts::PlaceSegment::Field { symbol },
                CheckedUnitStructuralPathSegment::Field(identity),
                TypeReferenceNode::Named { symbol: owner, .. }
                | TypeReferenceNode::Generic {
                    base_symbol: owner, ..
                },
            ) => {
                let mut owners = checked
                    .data_definitions()
                    .iter()
                    .filter(|data| data.symbol == *owner);
                let data = owners.next().ok_or(LoweringError::Unsupported(
                    "anonymous partial permission field owner is absent",
                ))?;
                if owners.next().is_some() {
                    return unsupported("anonymous partial permission field owner is ambiguous");
                }
                let mut fields =
                    checked
                        .data_members(data)
                        .iter()
                        .filter_map(|member| match member {
                            checked_trees::data::DataMember::Field(field)
                                if field.symbol == *symbol =>
                            {
                                Some(field)
                            }
                            _ => None,
                        });
                let field = fields.next().ok_or(LoweringError::Unsupported(
                    "anonymous partial permission field substituted its owner",
                ))?;
                let actual = field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned());
                if fields.next().is_some() || actual != *identity {
                    return unsupported("anonymous partial permission field identity drifted");
                }
                reference = field.type_reference;
            }
            (
                facts::PlaceSegment::FixedIndex { index },
                CheckedUnitStructuralPathSegment::FixedIndex(expected),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                },
            ) if u64::try_from(*index).ok() == Some(*expected) && *index < *length => {
                reference = *element_type;
            }
            _ => {
                return unsupported(
                    "anonymous partial permission path disagrees with its source type",
                );
            }
        }
    }
    Ok(())
}
