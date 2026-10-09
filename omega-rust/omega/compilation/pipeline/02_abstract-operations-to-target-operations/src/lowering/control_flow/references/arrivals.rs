//! Selected parameters retain every incoming occurrence; other loans agree
//! exactly. This is a checked edge correspondence, not backing-based identity
//! coalescing. A missing incoming leaf or an unbound parent cannot disappear
//! through an intersection of live state.

use super::*;

pub(in super::super) fn merge_arrivals(
    function: &AbstractFunction,
    target: semantic_vocabulary::BlockId,
    left: &mut BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceCustody>,
    right: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceCustody>,
) -> Result<(), LoweringError> {
    if !left.keys().eq(right.keys()) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    for (key, incoming) in right {
        let existing = left
            .get_mut(key)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if existing == incoming {
            continue;
        }
        let parameter = function
            .block_entries
            .iter()
            .find(|entry| entry.block == target)
            .and_then(|entry| {
                entry
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == key.0)
            })
            .filter(|parameter| parameter.access == StructuralAccess::Owned)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let contract = ReferenceCarrierContract::Parameter(parameter.clone());
        if existing.identity != *key
            || incoming.identity != *key
            || existing.contract != contract
            || incoming.contract != contract
            || existing.referent != incoming.referent
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        if existing.root != incoming.root {
            let mut origins = existing.root.origins();
            origins.extend(incoming.root.origins());
            origins.sort();
            origins.dedup();
            existing.root = ReferenceRoot::Alternatives(origins);
        }
        existing.parents.extend_from_slice(&incoming.parents);
        existing.parents.sort();
        existing.parents.dedup();
        existing.formations.extend_from_slice(&incoming.formations);
        existing
            .formations
            .sort_by(|left, right| left.identity.cmp(&right.identity));
        // A formation identity names one exact source declaration. Conflicting
        // rows are corruption, not two alternatives to select between.
        if existing.formations.windows(2).any(|pair| {
            pair[0].identity == pair[1].identity && pair[0].contract != pair[1].contract
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        existing.formations.dedup();
    }
    Ok(())
}
