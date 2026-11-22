//! A reborrow can inherit backing from a known incoming loan. The path from its
//! referent to the selected subject may cross plain record fields and explicit
//! `Referent` steps. Crossing a descriptor stored in a reached record makes
//! that stored loan the parent of the rest of the path. The checker follows
//! the path to verify the backing and access of the selected subject.
use crate::{DataId, LoweredData, LoweringError, LoweringTypes, PlaceProjection, Site, TypeShape};

pub(super) fn check_backing(
    referent: DataId,
    path: &[PlaceProjection],
    types: &LoweringTypes,
    data: &[LoweredData],
    site: &Site,
) -> Result<(), LoweringError> {
    let unknown = || LoweringError::new(site, "suspension needs the exact backing of the live structural place");
    let record = |id: DataId| &data[id.get() - 1];
    let mut subject = data[referent.get() - 1].carrier;
    for step in path {
        subject = match (types.get(subject), step) {
            (Some(TypeShape::Nominal(id)), PlaceProjection::Field(field)) => {
                record(*id)
                    .fields
                    .iter()
                    .find(|f| f.declaration == *field)
                    .ok_or_else(unknown)?
                    .carrier
            }
            // The reached field stores a descriptor; crossing it continues in
            // that loan's referent record.
            (Some(TypeShape::NominalReference { referent, .. }), PlaceProjection::Referent) => data[referent.get() - 1].carrier,
            _ => return Err(unknown()),
        };
    }
    // A descriptor within the selected subject or an unknown active case
    // needs more evidence than the path to it supplies.
    let mut pending = vec![subject];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if !seen.insert(ty) {
            continue;
        }
        match types.get(ty) {
            Some(TypeShape::PrimitiveScalar(_) | TypeShape::QualifiedScalar { .. }) => {}
            Some(TypeShape::FixedArray { element, .. }) => pending.push(*element),
            Some(TypeShape::Nominal(id)) => {
                let schema = record(*id);
                if !schema.cases.is_empty() {
                    return Err(unknown());
                }
                pending.extend(schema.fields.iter().map(|f| f.carrier));
            }
            _ => return Err(unknown()),
        }
    }
    Ok(())
}
