//! What a running Build program may call: Build binding effects and product
//! queries (`bindings`), filesystem access over its admitted captured-input
//! scope (`filesystem`), and host requests (`requests`).

pub mod bindings;
pub(crate) mod filesystem;
pub mod requests;
