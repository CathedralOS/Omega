//! Join possible descriptor storage from actual incoming edges. Case paths
//! guard conditional payloads: an absent slot on one arm is not a missing loan.
//! A shared slot must retain its carrier and referent types; its possible
//! origins join monotonically. Runtime pointer selection remains edge-owned.

use super::*;

pub(in super::super) fn merge_arrivals(
    function: &AbstractFunction,
    left: &mut BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>,
    right: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>,
) -> Result<(), LoweringError> {
    for (key, incoming) in right {
        let Some(existing) = left.get_mut(key) else {
            left.insert(key.clone(), incoming.clone());
            continue;
        };
        if existing.carrier_type != incoming.carrier_type || existing.referent != incoming.referent
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        if existing.root != incoming.root {
            existing.root = combine_candidate_origins(
                function,
                vec![existing.root.clone(), incoming.root.clone()],
            )?;
        }
    }
    Ok(())
}
