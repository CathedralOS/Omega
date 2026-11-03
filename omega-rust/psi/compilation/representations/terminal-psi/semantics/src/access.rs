//! Access views and placed RAM: lifecycles of whole-value placement at foreign
//! ingress, RAM correspondence and origins, access-view plans and referents,
//! and scalar accessor boundaries.

pub(crate) mod access_lifecycles;
pub(crate) mod access_ram_correspondence;
pub(crate) mod access_ram_origins;
pub(crate) mod access_view_plans;
pub(crate) mod access_view_referent;
pub(crate) mod accessor_scalar_boundaries;
