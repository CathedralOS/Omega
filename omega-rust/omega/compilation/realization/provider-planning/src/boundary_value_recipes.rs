//! Source attribution beside native boundary storage, never inside layout.
//!
//! Psi receives only source type correspondence. Native layout reports each
//! exact recipe ordinal and shape root; this owner rejoins that occurrence to
//! the same explicit Build selection roster for package review and publication.
//! A geometry report grants no representation or boundary authority.

use representation_planning::OpaqueRepresentationSelection;

pub(crate) fn representation_types(
    selections: &[OpaqueRepresentationSelection],
) -> Vec<checked_trees::typed_trees::boundary_values::BoundaryRepresentationType> {
    selections
        .iter()
        .map(
            |selection| checked_trees::typed_trees::boundary_values::BoundaryRepresentationType {
                opaque: selection.opaque(),
                carrier: selection.carrier_type(),
            },
        )
        .collect()
}
