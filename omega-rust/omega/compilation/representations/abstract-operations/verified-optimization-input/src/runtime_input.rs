//! Runtime capture borrows admitted artifact and selection custody, not the
//! executable abstract graph. Only the verified input can lend this projection;
//! its private fields preserve the association admission established.

use crate::VerifiedPsiOptimizationInput;
use semantic_vocabulary::MachineId;
use terminal_installation::{
    AdmittedScalarApplications, AuthorizedTerminalArtifact, TerminalTargetContext,
};

#[derive(Clone, Copy)]
pub struct AdmittedRuntimeInput<'input> {
    artifact: &'input AuthorizedTerminalArtifact,
    entry: MachineId,
    boundaries: &'input [terminal_psi::BoundaryMachineDeclaration],
    target: Option<&'input TerminalTargetContext>,
    scalar_applications: Option<&'input AdmittedScalarApplications>,
}

impl VerifiedPsiOptimizationInput {
    /// Lend only the admitted facts runtime capture consumes before lowering
    /// takes ownership of the executable program.
    pub fn runtime_input(&self) -> AdmittedRuntimeInput<'_> {
        AdmittedRuntimeInput {
            artifact: &self.context.artifact,
            entry: self.plan.entry,
            boundaries: &self.plan.boundary_machines,
            target: self.target_context.as_ref(),
            scalar_applications: self.scalar_applications.as_ref(),
        }
    }
}

impl AdmittedRuntimeInput<'_> {
    pub fn identity(&self) -> terminal_psi::TerminalPsiIdentity {
        self.artifact.artifact().view().identity()
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn boundary_machines(&self) -> &[terminal_psi::BoundaryMachineDeclaration] {
        self.boundaries
    }

    pub fn module(&self) -> &terminal_psi::TerminalModule {
        self.artifact.artifact().semantic_module()
    }

    pub const fn target_context(&self) -> Option<&TerminalTargetContext> {
        self.target
    }

    pub const fn scalar_applications(&self) -> Option<&AdmittedScalarApplications> {
        self.scalar_applications
    }
}
