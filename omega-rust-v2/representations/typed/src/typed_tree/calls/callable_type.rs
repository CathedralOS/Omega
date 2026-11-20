use crate::{CrashCause, PauseEffects, RequirementSlot, ServiceApplication, Site, ValueType};

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct CallableType {
    pub requirement: Option<RequirementSlot>,
    pub parameters: Vec<ValueType>,
    pub result: ValueType,
    pub contract: Site,
    pub closed_runtime: bool,
    pub pause: PauseEffects,
    pub has_guarantee: bool,
    pub requires: bool,
    pub reach: Option<Vec<ServiceApplication>>,
    pub terminates: bool,
    pub crash_ceiling: Option<Vec<CrashCause>>,
}
