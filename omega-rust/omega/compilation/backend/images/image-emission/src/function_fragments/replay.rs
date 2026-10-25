//! Retained replay evidence for common-pipeline objects, not a second current IR.

use std::sync::Arc;

use object_file::StagedOptimizedRelocationFreeObjectContainer;

#[derive(Debug, Clone)]
pub(crate) struct FragmentReplay(pub(crate) Arc<StagedOptimizedRelocationFreeObjectContainer>);

/// Audit the retained target producer independently of instruction selection.
/// Corpus or retained-artifact inspection can call this explicitly; object
/// construction does not re-run the producing compile's source custody analysis.
pub fn verify_retained_target_operations(
    artifact: &crate::ObjectArtifact,
) -> Result<(), diagnostics::Diagnostic> {
    for private in &artifact.private_functions {
        verify_retained_target_operations(&private.body)?;
    }
    let replay = artifact.fragment_replay.as_ref().ok_or_else(|| {
        diagnostics::Diagnostic::error("target-operation audit requires retained pipeline input")
    })?;
    let current = replay
        .0
        .source()
        .source()
        .source()
        .source()
        .optimized_target();
    target_operations_to_selected_instructions::verify_target_operation_plan(
        current.target_operations(),
        current.optimized(),
    )
    .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))
}

impl FragmentReplay {
    pub(crate) fn terminal_module(&self) -> &terminal_psi::TerminalModule {
        self.0.verified_input().context().module()
    }

    pub(crate) fn target_machine_selections(
        &self,
    ) -> Option<&terminal_installation::AdmittedTargetMachineSelections> {
        self.0.verified_input().target_machine_selections()
    }

    /// Join the retained executable input and its admitted deployment to the
    /// immutable portable root. Absence describes a targetless input, not a
    /// missing member to substitute; the object owner permits only exact
    /// ordinary roots in that case.
    pub(crate) fn selected_terminal_entry(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        entry: semantic_vocabulary::MachineId,
        native_target: target::NativeTarget,
    ) -> Result<Option<semantic_vocabulary::MachineId>, &'static str> {
        let identity = artifact.view().identity();
        let input = self.0.verified_input();
        if input.plan().psi != identity
            || input.context().module() != artifact.semantic_module()
            || input.plan().entry != entry
        {
            return Err("native object entry differs from its admitted executable input");
        }
        let Some(selections) = input.target_machine_selections() else {
            return Ok(None);
        };
        let profile = target::TargetProfile::from_canonical_target_name(selections.target())
            .map_err(|_| "native object entry has an unknown admitted deployment tag")?;
        if profile.native_realization() != Some(native_target) {
            return Err("native object target differs from its admitted deployment profile");
        }
        selections
            .entry(
                identity,
                artifact.semantic_module(),
                terminal_psi::TargetMachineCallable::Machine(entry),
            )
            .map(Some)
            .map_err(|_| "native object entry has no executable member at its admitted deployment")
    }
}

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


pub(crate) fn entry_source(
    artifact: &crate::ObjectArtifact,
) -> Result<
    (
        &abstract_operations::AbstractFunction,
        &target_operations::TargetFunction,
    ),
    diagnostics::Diagnostic,
> {
    let replay = artifact.fragment_replay.as_ref().ok_or_else(|| {
        diagnostics::Diagnostic::error("hosted receiver requires current fragment replay")
    })?;
    super::source::function(&replay.0, artifact.entry)
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))
}

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
    if artifact.requires_graph_storage_replay && artifact.fragment_replay.is_none() {
        return Err(diagnostics::Diagnostic::error(
            "graph storage and aggregate results require retained physical replay evidence",
        ));
    }
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
                                || matches!(
                                    home.source.shape.class,
                                    calling_conventions::ValueClass::BorrowedReference
                                        | calling_conventions::ValueClass::BorrowedScalarReference
                                )
                                || matches!(
                            home.location,
                            machine_code::StructuralSourceLocation::IncomingBorrowedPointer { .. }
                        )
                        }) || function.call_arguments().any(|argument| {
                    argument.access != terminal_psi::StructuralAccess::Owned
                        || argument.source.placement().is_none_or(|placement| {
                            matches!(
                                placement.shape.class,
                                calling_conventions::ValueClass::BorrowedReference
                                    | calling_conventions::ValueClass::BorrowedScalarReference
                            )
                        })
                        || matches!(
                            argument.destination.shape.class,
                            calling_conventions::ValueClass::BorrowedReference
                                | calling_conventions::ValueClass::BorrowedScalarReference
                        )
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
