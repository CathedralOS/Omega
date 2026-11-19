use crate::{CrashCause, Site};

#[derive(Debug, Eq, PartialEq)]
pub struct CrashRoute {
    pub clause: Site,
    pub contract: Site,
    pub cause: CrashCause,
    pub guard: Option<Site>,
}
