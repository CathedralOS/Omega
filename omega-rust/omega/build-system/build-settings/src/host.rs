/*
Host import adapters for an evaluated Build program.

A running Build machine interacts with the compiler through three bounded host
surfaces: declaration and provider binding effects (`bindings`), captured-input
and output filesystem calls (`filesystem`), and project/dependency/log host
requests (`requests`). Each adapter authenticates its compiler-issued facet or
activation ticket before recording or executing an effect.
*/

pub mod bindings;
pub(crate) mod filesystem;
pub mod requests;
