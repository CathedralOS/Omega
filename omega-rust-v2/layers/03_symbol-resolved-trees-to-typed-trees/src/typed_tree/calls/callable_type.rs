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
    pub reach: Option<Vec<ServiceApplication>>,
    pub terminates: bool,
    pub crash_ceiling: Option<Vec<CrashCause>>,
}
impl CallableType {
    pub(crate) fn admits_contract(
        &self,
        actual: &Self,
    ) -> bool {
        self.parameters == actual.parameters
            && self.result == actual.result
            && (self.requirement.is_none() || self.requirement == actual.requirement)
            && (!actual.pause.block || self.pause.block)
            && (!actual.pause.suspend || self.pause.suspend)
            && (!self.terminates || actual.terminates)
            && self.crash_ceiling.as_ref().is_none_or(|allowed| {
                actual
                    .crash_ceiling
                    .as_ref()
                    .is_some_and(|supplied| supplied.iter().all(|cause| allowed.contains(cause)))
            })
            && self.reach.as_ref().is_none_or(|allowed| {
                actual
                    .reach
                    .as_ref()
                    .is_some_and(|supplied| supplied.iter().all(|s| allowed.contains(s)))
            })
    }
}
