// Review-evidence projection for representation trust boundaries across unbound
// opaque declarations, consumer-side physical demands, and producer-side
// representation availability.

pub(super) mod physical_contract;
mod policy;

pub(crate) use policy::project_checked_representation_policy;
pub(crate) use policy::rederive_selections;
