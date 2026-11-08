//! Choosing whole providers: `selection` is the one choice policy for
//! invocation and canonical declarations, `provider_selection` chooses and then
//! admits complete correspondence and composition, `declarations` keeps exact
//! correspondence apart from candidate lookup, `independent_components` closes
//! `Independent` selections over their components, and `provider_demands`
//! judges prepared demand while keeping candidate order.

pub(crate) mod declarations;
pub(crate) mod independent_components;
pub(crate) mod provider_demands;
pub(crate) mod provider_selection;
pub(crate) mod selection;
