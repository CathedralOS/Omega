//! One declaration header for runtime, native and static contract questions.
//! Open static bounds exist only in this checking model, never a runtime ABI.

pub use terminal_semantics::calls::callable_refinement::CallableCrashPolicy;

#[derive(Debug, Clone)]
pub struct CallableHeader {
    pub scalar_parameters: Vec<terminal_psi::ValueDeclaration>,
    pub parameter_order: Vec<terminal_psi::BoundaryParameterKind>,
    pub structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    pub result: terminal_psi::TerminalMachineResult,
    pub proof: terminal_psi::MachineContract,
    pub crash_policy: CallableCrashPolicy,
}
