//! Projection from typed declaration storage into stable property evidence.

use crate::package_evidence::record::PackageReviewDataProperties;

pub(crate) const fn project_data_properties(
    properties: symbol_resolved_trees_to_typed_trees::typed_trees::data::DataProperties,
) -> PackageReviewDataProperties {
    PackageReviewDataProperties {
        multiplicity: properties.multiplicity,
        carry: properties.carry,
    }
}
