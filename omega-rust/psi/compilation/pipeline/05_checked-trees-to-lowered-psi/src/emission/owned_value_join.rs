//! The selected edge transports one result and its exact surviving owners.
//! Unit emission and expression continuations share this positional complement;
//! inventory is never an unconditional move or disposal list.
use crate::lowering_error::LoweringError;
use semantic_vocabulary::{PlaceId, StructuralTypeId};

#[derive(Clone, Copy)]
pub(crate) struct Owner {
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
}

pub(crate) fn removed_owner(
    owners: &[Owner],
    sources: &[PlaceId],
    selected: PlaceId,
) -> Option<usize> {
    owners
        .iter()
        .position(|owner| sources.contains(&owner.place))
        .or_else(|| owners.iter().position(|owner| owner.place == selected))
}

pub(crate) struct Complement {
    pub(crate) places: Vec<PlaceId>,
    pub(crate) displaced: Option<PlaceId>,
}

pub(crate) fn selected_complement(
    owners: &[Owner],
    sources: &[PlaceId],
    selected: PlaceId,
    moved: &[PlaceId],
) -> Result<Complement, LoweringError> {
    // A constructed result may have consumed a source into its contents.
    // That transfer fills the selected slot without a second discard. The
    // existing continuation has one fewer owner slot; a different cardinality
    // cannot borrow or duplicate owners to fill it.
    let mut transferred = moved
        .iter()
        .copied()
        .filter(|place| sources.contains(place));
    let consumed = transferred.next();
    if transferred.next().is_some() {
        return Err(LoweringError::Unsupported(
            "selection has unequal residual ownership at its join",
        ));
    }
    let selected_source = if sources.contains(&selected) {
        selected
    } else {
        consumed.unwrap_or(selected)
    };
    let first = owners
        .iter()
        .position(|owner| sources.contains(&owner.place));
    let displaced = (!sources.is_empty() && !sources.contains(&selected_source))
        .then(|| first.map(|position| owners[position].place))
        .flatten();
    let removed = removed_owner(owners, sources, selected_source);
    let mut unselected = sources
        .iter()
        .copied()
        .filter(|place| *place != selected_source && Some(*place) != displaced);
    let mut places = Vec::new();
    for (position, owner) in owners.iter().enumerate() {
        if Some(position) == removed {
            continue;
        }
        let place = if sources.contains(&owner.place) {
            let residual = unselected.next().ok_or(LoweringError::Unsupported(
                "owned selection residual sources do not cover their slots",
            ))?;
            let actual = owners
                .iter()
                .find(|candidate| candidate.place == residual)
                .ok_or(LoweringError::Unsupported(
                    "owned selection residual source has no frontier type",
                ))?;
            if actual.structural_type != owner.structural_type {
                return Err(LoweringError::Unsupported(
                    "owned selection residual sources need uniform custody types",
                ));
            }
            residual
        } else {
            owner.place
        };
        places.push(place);
    }
    if unselected.next().is_some() {
        return Err(LoweringError::Unsupported(
            "owned selection has untransported residual candidates",
        ));
    }
    Ok(Complement { places, displaced })
}
