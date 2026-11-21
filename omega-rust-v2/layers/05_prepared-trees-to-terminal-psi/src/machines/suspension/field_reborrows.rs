//! A reborrow can inherit backing from a known incoming loan. The projected
//! subject and the type-wide floors on its path still constrain suspension.
use crate::{DataId, LoweredData, LoweringError, LoweringTypes, PlaceProjection, Site, TypeShape};

pub(super) fn check_backing(
    referent: DataId,
    path: &[PlaceProjection],
    types: &LoweringTypes,
    data: &[LoweredData],
    site: &Site,
) -> Result<(), LoweringError> {
    let unknown = || LoweringError::new(site, "suspension needs the exact backing and carry evidence of the live structural place");
    let record = |id: DataId| {
        let schema = &data[id.get() - 1];
        if schema
            .carry
            .as_ref()
            .is_some_and(|carry| !carry.allows_suspension)
        {
            Err(LoweringError::new(site, "parked field reborrow has suspension-forbidden backing"))
        } else {
            Ok(schema)
        }
    };
    let mut subject = data[referent.get() - 1].carrier;
    for step in path {
        let (Some(TypeShape::Nominal(id)), PlaceProjection::Field(field)) = (types.get(subject), step) else {
            return Err(unknown());
        };
        let selected = record(*id)?
            .fields
            .iter()
            .find(|f| f.declaration == *field)
            .ok_or_else(unknown)?;
        subject = selected.carrier;
    }
    // A descriptor within the selected subject or an unknown active case
    // needs more evidence than the incoming record field alone supplies.
    let mut pending = vec![subject];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if !seen.insert(ty) {
            continue;
        }
        match types.get(ty) {
            Some(TypeShape::PrimitiveScalar(_)) => {}
            Some(TypeShape::FixedArray { element, .. }) => pending.push(*element),
            Some(TypeShape::Nominal(id)) => {
                let schema = record(*id)?;
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
