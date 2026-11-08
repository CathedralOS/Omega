//! Binding a chosen plan: `selected_binding` binds it and admits its policy,
//! then `installation_reach`, `qualification_grants` and `provider_instances`
//! complete its installation closure, its grants and its checked instances.

pub(crate) mod installation_reach;
pub(crate) mod provider_instances;
pub(crate) mod qualification_grants;
pub(crate) mod selected_binding;
