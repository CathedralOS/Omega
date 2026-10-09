//! Optimizer module role: executable entrance. Independently replays fixed-view copy work and seals exact transformed-plan custody.

mod apply;
mod copy_constraint;
mod leaf_destination;
mod roots;
mod seal;
mod site;
mod source_exit;
mod transformation;
mod usage;

pub(crate) use seal::seal_validation;
