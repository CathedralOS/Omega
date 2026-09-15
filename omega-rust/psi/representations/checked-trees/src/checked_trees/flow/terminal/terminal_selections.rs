//! Terminal machine selections, signature eligibility and the debug plans
//! that name selected machines and states.

use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTerminalMachineSelections {
    pub machines: Vec<CheckedTerminalMachineSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTerminalMachineSelection {
    pub machine: SymbolHandle,
    pub name: String,
    pub signature: CheckedTerminalSignatureEligibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedTerminalSignatureEligibility {
    Eligible,
    Attached,
    FreeUnitEffect,
    Unsupported,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTerminalDebugPlans {
    pub machines: Vec<CheckedTerminalMachineDebugPlan>,
}

impl CheckedTerminalDebugPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&CheckedTerminalMachineDebugPlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTerminalMachineDebugPlan {
    pub machine: SymbolHandle,
    pub machine_span: Option<source::SourceSpan>,
    pub contract_span: Option<source::SourceSpan>,
    pub states: Vec<CheckedTerminalStateDebugPlan>,
    pub source_files: Vec<source::SourceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTerminalStateDebugPlan {
    pub state: SymbolHandle,
    pub state_span: Option<source::SourceSpan>,
    /// Dense scalar declaration order; structural and borrowed formals have
    /// places, not scalar ValueIds, and do not occupy this presentation lane.
    pub parameter_spans: Vec<Option<source::SourceSpan>>,
    pub transition_spans: Vec<source::SourceSpan>,
    pub operation_spans: Vec<source::SourceSpan>,
}
