//! Projection from typed declaration storage into stable property evidence.

use crate::record::PackageReviewDataProperties;

pub(crate) const fn project_data_properties(
    properties: psi_typed_trees::data::DataProperties,
) -> PackageReviewDataProperties {
    PackageReviewDataProperties {
        multiplicity: properties.multiplicity,
        carry: properties.carry,
    }
}
