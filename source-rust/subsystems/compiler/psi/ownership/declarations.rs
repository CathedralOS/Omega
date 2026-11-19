use crate::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessDeclarations {
    pub access_view_inputs: Vec<TerminalAccessViewInput>,
    pub access_view_plans: Vec<TerminalAccessViewPlan>,
    pub access_lifecycle_plans: Vec<TerminalAccessLifecyclePlan>,
    pub reborrow_restored_call_uses: Vec<TerminalReborrowRestoredCallUse>,
}
