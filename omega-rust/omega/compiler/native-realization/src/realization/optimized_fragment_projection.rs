//! Product evidence joins for the shared fragment-to-object publication path.

use diagnostics::Diagnostic;

pub(super) struct OptimizedFragmentPublicationRequest<'request> {
    pub(super) has_provider_installation: bool,
    pub(super) has_boundary_settlements: bool,
    pub(super) boundary_application_coverage:
        Option<&'request boundary_applications::TerminalBoundaryApplicationCoverage>,
    pub(super) optimized_plan: &'request abstract_operations::AbstractOperationPlan,
    pub(super) terminal: terminal_psi::TerminalPsiIdentity,
    pub(super) validation: optimization_core::OptimizedAbstractPlanProjectionIdentity,
    pub(super) final_unit: optimization_core::OptimizationUnitIdentity,
}

pub(super) fn emit_optimized_fragments(
    physical: crate::StagedOptimizedVerifiedPhysicalPipeline,
    request: OptimizedFragmentPublicationRequest<'_>,
) -> Result<
    (
        image_emission::ObjectArtifact,
        native_artifact::NativePhysicalEvidenceScope,
    ),
    Vec<Diagnostic>,
> {
    if request.has_provider_installation || request.has_boundary_settlements {
        return Err(super::diagnostics::realization_error(
            "fragment object publication",
            "shared fragment publication does not yet admit provider installation or boundary settlements",
        ));
    }
    let source = stage_fragment_object(physical)?;
    let object =
        image_emission::build_function_fragment_object_artifact(&source).map_err(|error| {
            super::diagnostics::realization_error("fragment object publication", error)
        })?;
    let scope = match request.boundary_application_coverage {
        Some(coverage) => {
            native_artifact::NativePhysicalEvidenceScope::from_validated_fragment_publication(
                request.optimized_plan,
                request.terminal,
                request.validation,
                request.final_unit,
                coverage,
                &source,
                &object,
            )
            .map_err(|error| {
                super::diagnostics::realization_error(
                    "fragment physical-evidence projection",
                    error,
                )
            })?
        }
        None => native_artifact::NativePhysicalEvidenceScope::Unavailable,
    };
    Ok((object, scope))
}

/// Empty and selected phases publish through the same frame/text/object owners.
pub(super) fn stage_fragment_object(
    physical: crate::StagedOptimizedVerifiedPhysicalPipeline,
) -> Result<object_file::StagedOptimizedRelocationFreeObjectContainer, Vec<Diagnostic>> {
    let emitted = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .map_err(|error| super::diagnostics::realization_error("function-fragment emission", error))?;
    let text: object_file::StagedOptimizedObjectTextSectionSource = if emitted
        .source()
        .frame_protocol()
        .is_some()
    {
        let applied = machine_emission::stage_function_fragment_frame_application(emitted)
            .map_err(|error| {
                super::diagnostics::realization_error("function-fragment frame application", error)
            })?;
        machine_emission::stage_optimized_fixed_frame_text_section(applied)
            .map_err(|error| super::diagnostics::realization_error("framed text placement", error))?
            .into()
    } else {
        machine_emission::stage_optimized_relocation_free_text_section(emitted)
            .map_err(|error| super::diagnostics::realization_error("text placement", error))?
            .into()
    };
    object_file::stage_optimized_relocation_free_object_container(text)
        .map_err(|error| super::diagnostics::realization_error("fragment object placement", error))
}

#[cfg(test)]
mod tests {
    /// The compiler reaches object custody through the backend's public
    /// fragment publication entry, without constructing native records itself.
    #[test]
    fn return_publication_preserves_frames_with_selected_lowering() {
        for (target, expected_bytes) in [
            (target::NativeTarget::windows_x64(), 1_usize),
            (target::NativeTarget::linux_x64(), 1),
            (target::NativeTarget::linux_arm64(), 20),
            (target::NativeTarget::macos_arm64(), 20),
        ] {
            for selections in [
                optimization_core::OptimizationSelections::default(),
                optimization_core::OptimizationSelections::new([
                    optimization_core::Optimization::SelectedIncomingU12ExactAddImmediate,
                ])
                .expect("selected lowering"),
            ] {
                let checked = crate::tests::fixtures::checked_source::checked(
                    "data Main {} machine Main::launch() {}",
                );
                let artifact =
                    terminal_production::produce_terminal_artifact(&checked, "Main::launch")
                        .expect("canonical empty-machine Terminal artifact");
                let input =
                    terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
                        artifact.semantic_bytes(),
                        artifact.proof_bytes(),
                        &proof_admission::AdmissionProfile::default(),
                    )
                    .expect("verified abstract input");
                let abstract_program = crate::optimize_verified_abstract_input(
                    input,
                    crate::compiler_baseline_request_v1(&selections),
                )
                .expect("complete abstract optimization");
                let post_terminal = abstract_program.selections().project_post_terminal();
                let optimized_target =
                    abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                        abstract_program,
                        target,
                    )
                    .expect("independently validated target lowering");
                let physical = crate::stage_optimized_verified_physical_pipeline(
                    optimized_target,
                    post_terminal.selections(),
                )
                .expect("optimized physical pipeline");
                let source = physical.into_function_fragment_emission_source();
                let source = if !selections.is_empty()
                    && target.architecture == target::Architecture::Aarch64
                {
                    let machine_emission::FunctionFragmentReplayInputs::SelectedLowering(
                        mut realization,
                    ) = source.into_replay_for_test()
                    else {
                        panic!("selected-lowering evidence must remain selected-lowering evidence");
                    };
                    let frame = realization.frame_mut_for_test().take();
                    assert!(frame.is_some());
                    assert!(machine_emission::validate_selected_lowering_function_relative_realization_custody(
                        &realization,
                    ).is_err(), "dropping required return-address custody must reject");
                    *realization.frame_mut_for_test() = frame;
                    machine_emission::validate_selected_lowering_function_relative_realization_custody(
                        &realization,
                    ).expect("restored exact frame rejoins selected execution");
                    (*realization).into()
                } else {
                    source
                };
                let emitted = machine_emission::stage_optimized_function_fragment_emission(source)
                    .expect("optimized function-fragment emission");
                let publication = machine_emission::publish_function_fragments(emitted)
                    .expect("native fragment publication");
                let plan = publication.plan();

                let [function] = plan.functions.as_slice() else {
                    panic!("one projected Unit function");
                };
                assert_eq!(function.bytes.len(), expected_bytes);
                let stack = function.unit_stack.expect("Unit stack evidence");
                assert_eq!(
                    stack.aarch64_return_link.is_some(),
                    target.architecture == target::Architecture::Aarch64
                );
                assert_eq!(stack.frame.is_some(), stack.aarch64_return_link.is_some());

                let object = image_emission::build_object_artifact(plan)
                    .expect("optimized empty machine reaches object custody");
                assert_eq!(object.target(), target);
                assert_eq!(object.text_bytes(), function.bytes.as_slice());
            }
        }
    }
}
