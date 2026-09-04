//! Canonical semantic-value encoders, grouped by the value families they encode.

pub(super) mod callables;
pub(super) mod contracts;
pub(super) mod crashes;
pub(super) mod declarations;
pub(super) mod effects;
pub(super) mod expressions;
pub(super) mod identity;
pub(super) mod providers;
pub(super) mod quotients;

#[cfg(test)]
mod tests;
