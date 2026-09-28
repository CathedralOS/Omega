//! Transition blocks: `parse_transition` reads a block's transitions with
//! their `guards`, and `targets` reads and copies each transition target.

pub(crate) mod guards;
pub(crate) mod parse_transition;
pub(crate) mod targets;
