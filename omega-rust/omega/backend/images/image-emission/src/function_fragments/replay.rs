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

pub(crate) fn has_free_unit_entry(
    artifact: &crate::ObjectArtifact,
) -> Result<bool, diagnostics::Diagnostic> {
    let Some(replay) = &artifact.fragment_replay else {
        return Ok(false);
    };
    let (function, _) = super::source::function(&replay.0, artifact.entry)
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    Ok(matches!(
        function.result,
        abstract_operations::AbstractFunctionResult::Unit
    ) && function.entry_claims.is_empty()
        && function.parameters.is_empty()
        && function.structural_parameters.is_empty())
}

pub(crate) fn validate(artifact: &crate::ObjectArtifact) -> Result<(), diagnostics::Diagnostic> {
    if artifact.fragment_replay.is_none()
        && artifact.functions().iter().any(|function| {
            function.unit_parameters.len() != function.unit_parameter_homes.len()
                || function.scalar_structural_parameters.len()
                    != function.scalar_structural_parameter_homes.len()
                || function
                    .mixed_structural_scalar_abi
                    .as_ref()
                    .is_some_and(|abi| {
                        abi.structural_parameters.len()
                            != function.scalar_structural_parameter_homes.len()
                    })
        })
    {
        return Err(diagnostics::Diagnostic::error(
            "structural parameters without homes require common-pipeline replay evidence",
        ));
    }
    let missing_borrowed_replay = artifact.fragment_replay.is_none()
        && artifact.functions().iter().any(|function| {
            function
            .unit_parameters
            .iter()
            .chain(&function.scalar_structural_parameters)
            .any(|parameter| parameter.access != terminal_psi::StructuralAccess::Owned)
            || function
                .unit_parameter_homes
                .iter()
                .chain(&function.scalar_structural_parameter_homes)
                .any(|home| {
                    home.access != terminal_psi::StructuralAccess::Owned
                        || home.source.shape.class
                            == calling_conventions::ValueClass::BorrowedReference
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
                        || argument.source.placement().is_none_or(|placement| {
                            placement.shape.class
                                == calling_conventions::ValueClass::BorrowedReference
                        })
                        || argument.destination.shape.class
                            == calling_conventions::ValueClass::BorrowedReference
                        || matches!(
                            argument.source_location,
                            machine_code::StructuralSourceLocation::IncomingBorrowedPointer { .. }
                        )
                })
        });
    if missing_borrowed_replay {
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
                    machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary { .. } | machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. }
                )
            })
        })
    {
        return Err(diagnostics::Diagnostic::error(
            "selected boundary output requires common-pipeline replay evidence",
        ));
    }
    if artifact.fragment_replay.is_none()
        && artifact.boundary_settlements().iter().any(|row| {
            matches!(
                row.settlement.realization,
                target_operations::BoundaryRealization::HostedReadByte(_)
            )
        })
    {
        return Err(diagnostics::Diagnostic::error(
            "hosted byte input requires common-pipeline cleanup replay evidence",
        ));
    }
    if let Some(replay) = &artifact.fragment_replay {
        super::validate_function_fragment_object_artifact(&replay.0, artifact)
            .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    }
    Ok(())
}
