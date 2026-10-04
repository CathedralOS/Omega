//! Replay the retained named catalog's joins, not the compiler proofs that
//! originally closed its layouts and selected conformance applications.

use super::super::{
    BoundaryNativeParameterOrigin, BoundaryNativeParameterShape, MaterializedBoundarySignature,
};
use super::BoundaryCallbackLayoutEntry;
use calling_conventions::{NativeCallbackDemand, NativePlace};

pub(in crate::calling_policy_plans) fn validate(
    signature: &MaterializedBoundarySignature,
    realization_demands: &[NativeCallbackDemand],
) -> Result<(), String> {
    if signature.callback_demands != realization_demands
        || realization_demands
            .windows(2)
            .any(|pair| pair[0].destination >= pair[1].destination)
    {
        return Err("callback layout catalog lost its exact ordered demand context".to_owned());
    }
    let mut entries = signature.callback_layout_catalog.iter();
    for demand in realization_demands {
        if !matches!(demand.destination, NativePlace::Field { .. }) {
            continue;
        }
        let entry = entries.next().ok_or_else(|| {
            "callback layout catalog is missing a declared private field demand".to_owned()
        })?;
        if entry.destination != demand.destination
            || entry.terminal_slot().requirement != demand.requirement
        {
            return Err("callback layout catalog changed its exact field demand".to_owned());
        }
        validate_entry(signature, entry)?;
    }
    if entries.next().is_some() {
        return Err(
            "callback layout catalog contains an undeclared private field demand".to_owned(),
        );
    }
    Ok(())
}

fn validate_entry(
    signature: &MaterializedBoundarySignature,
    entry: &BoundaryCallbackLayoutEntry,
) -> Result<(), String> {
    let NativePlace::Field { parameter, .. } = &entry.destination else {
        return Err("callback layout catalog contains a non-field destination".to_owned());
    };
    let native_index = usize::try_from(entry.native_ordinal)
        .map_err(|_| "callback layout catalog native ordinal is out of range")?;
    let native = signature
        .native_parameters
        .get(native_index)
        .ok_or_else(|| "callback layout catalog has no exact native parameter".to_owned())?;
    let formal_index = usize::try_from(entry.formal_ordinal)
        .map_err(|_| "callback layout catalog formal ordinal is out of range")?;
    if native.identity != *parameter
        || native.native_ordinal != entry.native_ordinal
        || native.origin
            != (BoundaryNativeParameterOrigin::SemanticFormal {
                formal_ordinal: entry.formal_ordinal,
            })
        || signature.parameters.get(formal_index).is_none_or(|shape| {
            native.shape != BoundaryNativeParameterShape::Semantic(*shape)
                || usize::from(*shape) >= signature.shapes.len()
        })
        || !native.layout_data_symbol.is_valid()
        || native.layout_data_symbol != entry.root_layout().data_symbol
        || signature
            .native_parameters
            .iter()
            .enumerate()
            .any(|(index, other)| {
                index != native_index
                    && (other.identity == native.identity
                        || other.native_ordinal == native.native_ordinal
                        || other.origin == native.origin)
            })
    {
        return Err(
            "callback layout catalog changed its native or semantic parameter association"
                .to_owned(),
        );
    }
    entry
        .layout
        .validate(&entry.destination, signature.native_target)
}
