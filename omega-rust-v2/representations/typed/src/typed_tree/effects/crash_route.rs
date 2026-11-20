use crate::{CrashCause, Site};

/// The index belongs to the forest that typed the crash clause.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrashRouteRef {
    Local(usize),
    Library(usize),
}

#[derive(Debug, Eq, PartialEq)]
pub struct CrashRoute {
    pub clause: Site,
    pub contract: Site,
    pub cause: CrashCause,
    pub guard: Option<Site>,
}
