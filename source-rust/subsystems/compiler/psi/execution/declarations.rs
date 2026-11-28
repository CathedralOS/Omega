use crate::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionDeclarations {
    pub dynamic_dispatch: TerminalDynamicDispatchCatalog,
    pub callables: crate::TerminalCallableCatalog,
    pub unoptimized_behavior: Vec<crate::UnoptimizedMachineBehavior>,
    /// Declaration contracts with no executable body, ordered by ContractId.
    pub declaration_contracts: Vec<crate::MachineContract>,
    pub suspension_call_plan_count: u32,
    pub suspension_call_sites: Vec<TerminalSuspensionCallSite>,
    pub suspension_call_plans: Vec<TerminalSuspensionCallPlan>,
    pub task_activation_demands: Vec<TerminalTaskActivationDemand>,
}
