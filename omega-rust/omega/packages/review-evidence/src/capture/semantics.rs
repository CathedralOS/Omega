//! Fail-closed translation from checked compiler semantics into portable review shapes.

pub(super) mod conformances;
pub(super) mod declarations;
pub(super) mod facts;
pub(super) mod services;
pub(super) mod signatures;
pub(super) mod types;

pub(super) mod encoding;

#[cfg(test)]
mod tests;
