//! Replay backing geometry and audited function ABI lookup against retained stage 02 output.
//! Production reads the current program and the facts the driver carried;
//! only validators may enter this module, and only with stage 02's output
//! retained for that audit (`RetainedTargetOperations`).

use super::placed_reference::backing_geometry;
use super::{AbstractFunction, Error, MachineId};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use target_operations::TargetUnitOperation;

fn original_function(
    original: &crate::RetainedTargetOperations,
    machine: MachineId,
) -> Result<&AbstractFunction, Error> {
    let mut rows = original
        .target_for_correspondence()
        .optimized()
        .plan()
        .functions
        .iter()
        .filter(|row| row.machine == machine);
    match (rows.next(), rows.next()) {
        (Some(row), None) => Ok(row),
        _ => Err(Error::Mismatch("audit has no unique original function")),
    }
}

/// The optimized function and stage 02's lowered function for `machine`.
/// Only the replay validators call this; ordinary emission reads the
/// prepared plan.
pub(in crate::function_fragments) fn audited_function<'a>(
    _source: &'a StagedOptimizedRelocationFreeObjectContainer,
    original: &'a crate::RetainedTargetOperations,
    machine: MachineId,
) -> Result<(&'a AbstractFunction, &'a target_operations::TargetFunction), Error> {
    let abstracted = original_function(original, machine)?;
    let mut targeted = crate::retained_working_program::target_operations(original)
        .functions
        .iter()
        .filter(|function| function.machine == machine);
    match (targeted.next(), targeted.next()) {
        (Some(targeted), None) => Ok((abstracted, targeted)),
        _ => Err(Error::Mismatch(
            "shared function has no unique current source",
        )),
    }
}

pub(in crate::function_fragments) fn establish_geometry(
    replay: &crate::FragmentReplay,
    original: &crate::RetainedTargetOperations,
) -> Result<(), super::Error> {
    let physical = &replay
        .original_encoding()
        .ok_or(Error::Mismatch(
            "original physical instructions were not retained (ReplayUnavailable)",
        ))?
        .physical;
    let abstracted = original.target_for_correspondence().optimized().plan();
    let targeted = crate::retained_working_program::target_operations(original);
    let selected = physical;
    if !targeted.functions.iter().any(|function| {
        function.graph.operations().any(|operation| {
            matches!(
                operation,
                TargetUnitOperation::EstablishAccessReference { .. }
            )
        })
    }) {
        return Ok(());
    }
    let graph = backing_geometry::completed_graph(abstracted, targeted).ok_or(
        super::Error::Mismatch("placed formation has no complete reference-origin graph"),
    )?;
    let mut geometry =
        backing_geometry::BackingGeometry::new(abstracted, selected, targeted, &graph);
    for function in &targeted.functions {
        for operation in function.graph.operations() {
            if let TargetUnitOperation::EstablishAccessReference {
                psi_operation,
                result_home,
                source,
                correspondence,
                ..
            } = operation
            {
                geometry
                    .proves(
                        function.machine,
                        result_home.place(),
                        source,
                        correspondence,
                    )
                    .map_err(|reason| super::Error::AccessBackingGeometry {
                        machine: function.machine,
                        operation: *psi_operation,
                        reason,
                    })?;
            }
        }
    }
    Ok(())
}

impl crate::FragmentReplay {
    pub fn verify_access_geometry(&self) -> Result<(), diagnostics::Diagnostic> {
        let original = self.target_source().ok_or_else(|| {
            diagnostics::Diagnostic::error(
                "target-operation audit requires retained target operations",
            )
        })?;
        establish_geometry(self, original)
            .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))
    }
}
