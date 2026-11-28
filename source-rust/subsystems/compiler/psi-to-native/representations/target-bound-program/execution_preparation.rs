//! Keep the selected worker and result fields together for native lowering.
use language_core::sealed_intrinsics::ActivationOperation;
use semantic_vocabulary::BoundaryMachineId;
use terminal_semantics::calls::owned_activation::{PreparationShape, prepare_shape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedWorker {
    artifact: terminal_psi::TerminalPsiIdentity,
    shape: PreparationShape,
}

impl PreparedWorker {
    pub fn is_preparation(boundary: &terminal_psi::BoundaryMachineDeclaration) -> bool {
        boundary.declaration.as_ref().is_some_and(|declaration| {
            matches!(
                declaration.execution,
                terminal_psi::CheckedBoundaryExecution::OwnedActivation(
                    ActivationOperation::Prepare
                )
            )
        })
    }

    pub fn for_boundary(
        artifact: terminal_codec::TerminalArtifactView<'_>,
        id: BoundaryMachineId,
    ) -> Result<Self, &'static str> {
        let module = artifact.module();
        let boundary = module
            .boundary_machine(id)
            .ok_or("native call names an absent boundary")?;
        Ok(Self {
            artifact: artifact.identity(),
            shape: prepare_shape(module, boundary)?,
        })
    }

    pub const fn shape(&self) -> PreparationShape {
        self.shape
    }

    pub(crate) fn matches(&self, artifact: terminal_psi::TerminalPsiIdentity) -> bool {
        self.artifact == artifact
    }
}
