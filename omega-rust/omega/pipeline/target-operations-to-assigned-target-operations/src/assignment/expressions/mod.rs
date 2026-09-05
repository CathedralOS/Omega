//! Optimizer module role: stage group. Expression assignment descends through frame construction, typed trees,
//! and independent parameter-location discovery.

pub(super) mod boolean;
pub(super) mod frame;
pub(super) mod integer;
pub(super) mod parameters;
