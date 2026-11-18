//! Facts carried with the current machine form after source graphs are dropped.
//!
//! Each row states a particular publication or verification obligation. There
//! are no blocks, executable operation sequences, source artifacts or callbacks.

use verified_optimization_input::publication::SemanticEmissionFacts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionFacts {
    pub termination: Result<(), String>,
    pub faults: Result<machine_code::fault_origins::PhysicalFaultOrigins, String>,
    pub cpu: machine_code::cpu_origins::PhysicalCpuOrigins,
    pub internal_calls: machine_code::internal_call_plans::PhysicalInternalCallPlans,
    pub boundary_settlements: machine_code::boundary_settlements::PhysicalBoundarySettlements,
    pub contracts: std::sync::Arc<selected_instructions::SelectedProgramContracts>,
    pub port_writes: Result<Vec<machine_code::function_metadata::PhysicalPortWrite>, &'static str>,
    pub temporary_stack: Vec<machine_code::function_metadata::FunctionTemporaryStack>,
    pub semantic: SemanticEmissionFacts,
    pub stack: machine_code::physical_stack::PhysicalStackProgram,
    pub calls: machine_code::call_origins::PhysicalCallOrigins,
    pub image_origins: Result<machine_code::image_origins::ImageAddressOrigins, &'static str>,
    pub foreign_calls: Result<
        machine_code::foreign_call_plans::PhysicalForeignCallPlans,
        machine_code::foreign_call_plans::ForeignCallPlanError,
    >,
}

impl std::ops::Deref for EmissionFacts {
    type Target = SemanticEmissionFacts;
    fn deref(&self) -> &Self::Target {
        &self.semantic
    }
}
