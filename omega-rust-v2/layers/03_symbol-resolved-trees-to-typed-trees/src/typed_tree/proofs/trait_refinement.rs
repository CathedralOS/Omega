use crate::{CrashCause, ServiceApplication, Site};
use omega_resolver::DeclarationId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitRefinement {
    pub site: Site,
    pub schema: DeclarationId,
    pub prohibits_block: bool,
    pub prohibits_suspend: bool,
    pub reach: Option<Vec<ServiceApplication>>,
    pub crash_ceiling: Option<Vec<CrashCause>>,
    pub terminates: bool,
}
