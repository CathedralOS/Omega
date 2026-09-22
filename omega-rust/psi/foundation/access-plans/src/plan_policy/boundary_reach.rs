//! The set of boundary services a placement may reach.

use crate::placements::schema_correspondence::normalized_identity;
use std::collections::BTreeSet;

normalized_identity!(BoundaryServiceReachId, "boundary-service reach identity");

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct BoundaryReach {
    services: BTreeSet<BoundaryServiceReachId>,
}

impl BoundaryReach {
    pub fn from_services(services: impl IntoIterator<Item = BoundaryServiceReachId>) -> Self {
        Self {
            services: services.into_iter().collect(),
        }
    }

    pub fn services(&self) -> impl ExactSizeIterator<Item = BoundaryServiceReachId> + '_ {
        self.services.iter().copied()
    }

    pub fn contains(&self, service: BoundaryServiceReachId) -> bool {
        self.services.contains(&service)
    }

    pub fn contains_all(&self, required: &Self) -> bool {
        required.services.is_subset(&self.services)
    }

    pub fn intersection(&self, other: &Self) -> Self {
        Self {
            services: self
                .services
                .intersection(&other.services)
                .copied()
                .collect(),
        }
    }
}
