//! Keep the selected worker and result fields together for native lowering.
use language_core::sealed_intrinsics::ActivationOperation;
use semantic_vocabulary::BoundaryMachineId;
use terminal_semantics::calls::owned_activation::{PreparationShape, prepare_shape};
use terminal_semantics::calls::owned_activation::{StepShape, step_shape};

pub(super) type StepEntries = Vec<(BoundaryMachineId, StepShape)>;

/// Ordinary opaque declarations supply no layout. A core preparation vouches
/// for its ready owner, and only a matching step vouches for its parked owner.
pub(super) fn owner_homes(
    module: &terminal_psi::TerminalModule,
    reach: &native_execution_reach::ExecutionReach,
    target: target::NativeTarget,
    prepared: &[PreparedWorker],
    parent: Option<&crate::TargetBoundReach>,
) -> Result<(Vec<runtime_abi::ExecutionOwnerHome>, StepEntries), &'static str> {
    let mut homes = parent.map_or_else(Vec::new, |p| p.value_storage().execution_owners.to_vec());
    let home = |ty| runtime_abi::ExecutionOwnerHome::for_target(ty, target)
        .ok_or("the activation owner has no native address geometry");
    for worker in prepared {
        homes.push(home(worker.shape().owner_type)?);
    }
    let mut steps = Vec::new();
    for id in reach.boundaries() {
        let boundary = module.boundary_machine(*id).ok_or("reached activation boundary is absent")?;
        if !boundary.declaration.as_ref().is_some_and(|d| d.execution ==
            terminal_psi::CheckedBoundaryExecution::OwnedActivation(ActivationOperation::StepFixed)) {
            continue;
        }
        let shape = match parent.and_then(|p| p.step(*id)) {
            Some(inherited) => *inherited,
            None => {
                let [owner] = boundary.structural_parameters.as_slice() else {
                    return Err("native step does not consume one prepared owner");
                };
                let mut matched = prepared.iter().map(PreparedWorker::shape)
                    .filter(|w| w.owner_type == owner.structural_type);
                let (Some(worker), None) = (matched.next(), matched.next()) else {
                    return Err("native step needs the unique preparation of its owner");
                };
                step_shape(module, boundary, worker.selected.preparation)?
            }
        };
        homes.push(home(shape.suspended_owner)?);
        steps.push((*id, shape));
    }
    homes.sort_unstable_by_key(|home| home.ty());
    homes.dedup();
    Ok((homes, steps))
}

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
