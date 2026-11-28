//! Runtime capture borrows admitted artifact and selection custody, not the
//! executable abstract graph. Only the verified input can lend this projection;
//! its private fields preserve the association admission established.

use crate::VerifiedPsiOptimizationInput;
use psi_authorization::AdmittedTerminalArtifact;
use psi_scalar_applications::AdmittedScalarApplications;
use psi_target_layouts::TerminalTargetContext;
use semantic_vocabulary::MachineId;

#[derive(Clone, Copy)]
pub struct AdmittedRuntimeInput<'input> {
    artifact: &'input AdmittedTerminalArtifact,
    entry: MachineId,
    boundaries: &'input [terminal_psi::BoundaryMachineDeclaration],
    borrowed_storage: &'input runtime_abi::NativeBorrowedStorageLayouts,
    target: Option<&'input TerminalTargetContext>,
    scalar_applications: Option<&'input AdmittedScalarApplications>,
    native_scope: Option<&'input crate::NativeExecutionScope>,
}

impl VerifiedPsiOptimizationInput {
    /// Lend only the admitted facts runtime capture consumes before lowering
    /// takes ownership of the executable program.
    pub fn runtime_input(&self) -> AdmittedRuntimeInput<'_> {
        AdmittedRuntimeInput {
            artifact: &self.context.artifact,
            entry: self.plan.entry,
            boundaries: &self.plan.boundary_machines,
            borrowed_storage: self.borrowed_storage.layouts(),
            target: self.target_context.as_ref(),
            scalar_applications: self.scalar_applications.as_ref(),
            native_scope: self.native_execution_scope.as_deref(),
        }
    }
}

impl AdmittedRuntimeInput<'_> {
    pub(super) fn borrowed_storage(&self) -> &runtime_abi::NativeBorrowedStorageLayouts {
        self.borrowed_storage
    }

    pub(super) fn require_native_providers(
        &self,
        providers: &crate::ScopedProviderSelection,
        bindings: &native_binding_admission::RetainedNativeBindings,
    ) -> Result<&crate::NativeExecutionScope, String> {
        providers
            .authorization()
            .require_artifact(self.artifact.artifact())
            .map_err(|error| error.to_string())?;
        let scope = self
            .native_scope
            .ok_or("runtime provisions require the lowered native execution scope")?;
        if scope != providers.scope()
            || scope.source_native_bindings() != bindings.identity_digest()
        {
            return Err("runtime providers differ from their lowered artifact, selection or native bindings".into());
        }
        Ok(scope)
    }

    pub fn identity(&self) -> terminal_psi::TerminalPsiIdentity {
        self.artifact.artifact().view().identity()
    }

    pub fn manifest_identity(&self) -> [u8; 32] {
        *self.artifact.artifact().manifest().identity().as_bytes()
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
