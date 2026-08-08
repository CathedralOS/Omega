use psi_symbols::SymbolHandle;
use psi_typed_trees::types::PrimitiveType;

/// Stable machine identities and names used to select the bootstrap terminal
/// producer without reopening the typed machine table.
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
    pub machine_span: Option<psi_source::SourceSpan>,
    pub contract_span: Option<psi_source::SourceSpan>,
    pub states: Vec<CheckedTerminalStateDebugPlan>,
    pub source_files: Vec<psi_source::SourceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTerminalStateDebugPlan {
    pub state: SymbolHandle,
    pub state_span: Option<psi_source::SourceSpan>,
    pub parameter_spans: Vec<Option<psi_source::SourceSpan>>,
    pub transition_spans: Vec<psi_source::SourceSpan>,
    pub operation_spans: Vec<psi_source::SourceSpan>,
}

/// Source-handle-free control plans accepted by the bootstrap terminal-Psi
/// scalar producer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarGraphPlans {
    pub machines: Vec<CheckedScalarMachineGraph>,
}

impl CheckedScalarGraphPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&CheckedScalarMachineGraph> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarMachineGraph {
    pub machine: SymbolHandle,
    pub states: Vec<CheckedScalarStateGraph>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarStateGraph {
    pub state: SymbolHandle,
    pub parameter_types: Vec<PrimitiveType>,
    pub result_type: PrimitiveType,
    pub terminator: CheckedScalarStateTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarStateTerminator {
    Return {
        statement_ordinal: u32,
    },
    Crash {
        statement_ordinal: u32,
    },
    Jump(CheckedScalarSuccessor),
    Conditional {
        guard_statement_ordinal: u32,
        when_true: CheckedScalarSuccessor,
        when_false: CheckedScalarSuccessor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarSuccessor {
    pub statement_ordinal: u32,
    pub target: SymbolHandle,
    pub argument_count: u32,
}
