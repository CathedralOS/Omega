//! The bodies a selection runs: Psi's `checked_bodies`, available
//! `target_bodies` and the direct calls they reach, pre-optimization
//! `callback_roots`, and the numeric meaning `external_execution` retains.

pub(crate) mod callback_roots;
pub(crate) mod checked_bodies;
pub(crate) mod external_execution;
pub(crate) mod target_bodies;
