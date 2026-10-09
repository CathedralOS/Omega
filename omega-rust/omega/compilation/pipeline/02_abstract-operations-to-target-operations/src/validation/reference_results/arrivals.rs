//! Independently join actual selected parameter leaves. Every other live key
//! agrees exactly. Complete incoming backing alternatives survive selection;
//! the target's retained result rows never supply the expected provenance.

use super::*;

pub(super) fn merge_arrivals(
    function: &AbstractFunction,
    target: BlockId,
    left: &mut BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    right: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
) -> Option<()> {
    if !left.keys().eq(right.keys()) {
        return None;
    }
    for (key, incoming) in right {
        let current = left.get_mut(key)?;
        if current == incoming {
            continue;
        }
        let parameter = function
            .block_entry(target)?
            .structural_parameters
            .iter()
            .find(|parameter| {
                parameter.place == key.0 && parameter.access == StructuralAccess::Owned
            })?;
        if current.structural_type != parameter.structural_type
            || incoming.structural_type != parameter.structural_type
            || current.multiplicity != parameter.multiplicity
            || incoming.multiplicity != parameter.multiplicity
        {
            return None;
        }
        let mut origins = current.root.origins();
        origins.extend(incoming.root.origins());
        origins.sort();
        origins.dedup();
        current.root = Root::Alternatives(origins);
    }
    Some(())
}
