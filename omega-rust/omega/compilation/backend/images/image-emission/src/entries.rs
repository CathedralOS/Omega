//! How execution enters an emitted image: `physical_entry` binds a selected
//! arrival to its unchanged body, `callable_entry_transport` joins native
//! entry transport to that body, the `hosted_*` modules provide hosted entry
//! storage, receivers and Unit completion adapters, and `foreign_adapter`
//! builds the fixed-frame adapter for one native import.

pub(crate) mod callable_entry_transport;
pub(crate) mod foreign_adapter;
pub(crate) mod hosted_entry_storage;
pub(crate) mod hosted_receiver;
pub(crate) mod hosted_unit_entry;
pub(crate) mod physical_entry;
