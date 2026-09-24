use semantic_vocabulary::{PlaceId, StructuralTypeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoreDestination {
    Initialize {
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    Assign {
        place: PlaceId,
    },
}
