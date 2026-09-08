//! Retained replay evidence for common-pipeline objects, not a second current IR.

use std::sync::Arc;

use object_file::StagedOptimizedRelocationFreeObjectContainer;

#[derive(Debug, Clone)]
pub(crate) struct FragmentReplay(pub(super) Arc<StagedOptimizedRelocationFreeObjectContainer>);

// Object value equality does not admit evidence. Publication always reconstructs
// the source relationship below, including the complete retained stage evidence.
impl PartialEq for FragmentReplay {
    fn eq(&self, other: &Self) -> bool {
        self.0.object() == other.0.object()
            && self.0.container() == other.0.container()
            && self.0.manifest().record() == other.0.manifest().record()
    }
}

impl Eq for FragmentReplay {}

pub(crate) fn validate(artifact: &crate::ObjectArtifact) -> Result<(), diagnostics::Diagnostic> {
    if artifact.fragment_replay.is_none()
        && artifact.functions().iter().any(|function| {
            function
            .unit_parameters
            .iter()
            .any(|parameter| parameter.access != terminal_psi::StructuralAccess::Owned)
            || function.unit_parameter_homes.iter().any(|home| {
                home.access != terminal_psi::StructuralAccess::Owned
                    || home.source.shape.class == calling_conventions::ValueClass::BorrowedReference
                    || matches!(
                        home.location,
                        machine_code::StructuralSourceLocation::IncomingBorrowedPointer { .. }
                    )
            })
            || function
                .internal_unit_calls
                .iter()
                .flat_map(|call| &call.arguments)
                .any(|argument| {
                    argument.access != terminal_psi::StructuralAccess::Owned
                        || argument.source.shape.class
                            == calling_conventions::ValueClass::BorrowedReference
                        || argument.destination.shape.class
                            == calling_conventions::ValueClass::BorrowedReference
                        || matches!(
                            argument.source_location,
                            machine_code::StructuralSourceLocation::IncomingBorrowedPointer { .. }
                        )
                })
        })
    {
        return Err(diagnostics::Diagnostic::error(
            "borrowed structural pointers require common-pipeline replay evidence",
        ));
    }
    if artifact.fragment_replay.is_none()
        && artifact
            .functions()
            .iter()
            .flat_map(|function| &function.internal_unit_calls)
            .flat_map(|call| &call.scalar_arguments)
            .any(|argument| {
                matches!(
                    argument.source,
                    machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall { .. }
                )
            })
    {
        return Err(diagnostics::Diagnostic::error(
            "selected Unit call requires common-pipeline replay evidence",
        ));
    }
    if artifact.fragment_replay.is_none()
        && artifact
            .functions()
            .iter()
            .any(|function| function.ranked_u32_countdown.is_some())
    {
        return Err(diagnostics::Diagnostic::error(
            "ranked body requires common-pipeline replay evidence",
        ));
    }
    if artifact.fragment_replay.is_none()
        && artifact.boundary_settlements().iter().any(|row| {
            row.settlement
            .runtime_scalar_arguments
            .iter()
            .any(|argument| {
                matches!(
                    argument.source,
                    machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary { .. }
                )
            })
        })
    {
        return Err(diagnostics::Diagnostic::error(
            "selected boundary output requires common-pipeline replay evidence",
        ));
    }
    if let Some(replay) = &artifact.fragment_replay {
        super::validate_function_fragment_object_artifact(&replay.0, artifact)
            .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    }
    Ok(())
}
