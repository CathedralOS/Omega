//! Facts carried with the current machine form after source graphs are dropped.
//!
//! Each row states a particular publication or verification obligation. There
//! are no blocks, executable operation sequences, source artifacts or callbacks.

use verified_optimization_input::publication::SemanticEmissionFacts;
pub use verified_optimization_input::publication::target_replay::{
    BodyPublication, RetainedTargetOperations,
};

/// Target lowering closes private references and semantic obligations. Selection
/// joins the installations for that body. All physical stages retain this record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBodyFacts {
    semantic: BodyPublication<SemanticEmissionFacts>,
    private_references: Vec<function_identity::MachineFunctionIdentity>,
    installations: std::sync::Arc<installation_admission::PlannedInstallationResources>,
}

impl NativeBodyFacts {
    pub fn retains_stage_inputs(&self) -> bool {
        self.semantic.original_for_audit().is_some()
    }

    pub fn retain_selected_body_facts(
        semantic: BodyPublication<SemanticEmissionFacts>,
        private_references: Vec<function_identity::MachineFunctionIdentity>,
        installations: installation_admission::PlannedInstallationResources,
    ) -> Self {
        Self {
            semantic,
            private_references,
            installations: std::sync::Arc::new(installations),
        }
    }

    pub fn into_framed_body_facts(
        self,
        actual_references: &[function_identity::MachineFunctionIdentity],
    ) -> Result<
        (
            BodyPublication<SemanticEmissionFacts>,
            std::sync::Arc<installation_admission::PlannedInstallationResources>,
        ),
        &'static str,
    > {
        if self.private_references != actual_references {
            return Err("physical selection changed the closed native private-body references");
        }
        Ok((self.semantic, self.installations))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionFacts {
    pub semantic: SemanticEmissionFacts,
    pub physical: PhysicalEmissionFacts,
}

/// Machine facts derived together from the allocated instructions and their frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEmissionFacts {
    pub termination: Result<(), String>,
    pub faults: Result<machine_code::fault_origins::PhysicalFaultOrigins, String>,
    pub cpu: machine_code::cpu_origins::PhysicalCpuOrigins,
    pub internal_calls: machine_code::internal_call_plans::PhysicalInternalCallPlans,
    pub boundary_settlements: machine_code::boundary_settlements::PhysicalBoundarySettlements,
    pub contracts: std::sync::Arc<selected_instructions::SelectedProgramContracts>,
    pub port_writes: Result<Vec<machine_code::function_metadata::PhysicalPortWrite>, &'static str>,
    pub temporary_stack: Vec<machine_code::function_metadata::FunctionTemporaryStack>,
    pub stack: machine_code::physical_stack::PhysicalStackProgram,
    pub calls: std::sync::Arc<machine_code::call_origins::PhysicalCallOrigins>,
    pub image_origins: Result<machine_code::image_origins::ImageAddressOrigins, &'static str>,
    pub foreign_calls: Result<
        std::sync::Arc<machine_code::foreign_call_plans::PhysicalForeignCallPlans>,
        machine_code::foreign_call_plans::ForeignCallPlanError,
    >,
}

/// Installation authority and emission facts travel with the body after frame binding.
#[derive(Debug, PartialEq, Eq)]
pub struct BoundBodyContracts {
    pub facts: EmissionFacts,
    pub installations: std::sync::Arc<installation_admission::BoundInstallationAuthority>,
}

impl BoundBodyContracts {
    pub fn needs_installed_branch_checks(&self) -> bool {
        self.installations
            .installed_entry_execution()
            .is_some_and(|execution| !execution.scope().fatal_entries().is_empty())
    }

    /// Settle installation and semantic agreement while the binding stage owns both.
    pub fn encoding_subject(
        &self,
        frame: std::sync::Arc<machine_code::TargetFrameLayoutPlan>,
    ) -> Result<machine_code::program_context::BoundEncodingSubject, &'static str> {
        let facts = &self.facts;
        let entry = self.installations.program();
        let calls = &facts.physical.calls;
        let contracts = &facts.physical.contracts;
        let roots = contracts
            .execution()
            .ok_or("encoded program is missing its execution facts")?
            .roots();
        if !self.installations.matches_admission(
            facts.semantic.admission.installed_interrupts(),
            facts.semantic.admission.native_callbacks(),
        ) || !self
            .installations
            .matches_frame(calls.physical_identity, &frame)
            || (entry.psi, entry.machine) != (roots.psi, roots.entry)
            || (calls.psi, calls.target) != (entry.psi, entry.target)
            || (facts.physical.stack.psi, facts.physical.stack.target) != (entry.psi, entry.target)
            || facts.physical.foreign_calls.as_ref().is_ok_and(|foreign| {
                (foreign.psi, foreign.target, foreign.physical_identity)
                    != (entry.psi, entry.target, calls.physical_identity)
            })
            || (facts.semantic.program.psi, facts.semantic.program.entry)
                != (entry.psi, entry.machine)
        {
            return Err("bound body contracts differ from the encoded program or its frame");
        }
        Ok(machine_code::program_context::BoundEncodingSubject {
            contracts: contracts.identity_bytes(),
            target: entry.target,
            physical: calls.physical_identity,
            frame,
        })
    }
}
