//! Optimizer module role: executable entrance. Independently replays fixed-view copy work and seals exact transformed-plan custody.
mod seal;
pub(crate) use seal::seal_validation;
