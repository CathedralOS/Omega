//! Narrow publication projection for the first optimized fragment cohort.
//!
//! The optimizer owns instruction selection and fragment validation. This
//! module only rejoins the already validated, return-only Unit fragments to
//! the established native object/image path. It deliberately does not infer
//! evidence for calls, frames, spills, effects, or scalar results. Admission
//! is based on the emitted fragment shape, not the optimization route that
//! produced it.

use omega_machine_code::{
    FunctionAppliedFrameProtocol, FunctionFragmentEmissionPlan, MachineCodeFunction,
    MachineCodePlan, SemanticCodeAttribution, SemanticCodeSite,
};
use omega_machine_emission::{
    StagedFunctionFragmentFrameApplication, StagedOptimizedFunctionFragmentEmission,
    stage_function_fragment_frame_application, validate_optimized_function_fragment_emission,
};

use super::optimized_fragment_unit_stack::unit_stack_evidence;
use omega_selected_instructions::MachineAlternativeFamily;
use psi_diagnostics::Diagnostic;

pub(super) struct OptimizedFragmentPublicationRequest<'request> {
    pub(super) has_provider_installation: bool,
    pub(super) has_boundary_settlements: bool,
    pub(super) boundary_application_coverage:
        Option<&'request omega_boundary_applications::TerminalBoundaryApplicationCoverage>,
    pub(super) optimized_plan: &'request omega_abstract_operations::AbstractOperationPlan,
    pub(super) terminal: psi_terminal::TerminalPsiIdentity,
    pub(super) validation: omega_optimization_core::OptimizedAbstractPlanProjectionIdentity,
    pub(super) final_unit: omega_optimization_core::OptimizationUnitIdentity,
}

pub(super) fn emit_return_only_optimized_fragments(
    physical: crate::StagedOptimizedVerifiedPhysicalPipeline,
    request: OptimizedFragmentPublicationRequest<'_>,
) -> Result<
    (
        MachineCodePlan,
        omega_native_artifact::NativePhysicalEvidenceScope,
    ),
    Vec<Diagnostic>,
> {
    if request.has_provider_installation || request.has_boundary_settlements {
        return Err(super::diagnostics::realization_error(
            "optimized fragment native publication",
            "the first return-only publication cohort admits no provider installation or boundary settlements",
        ));
    }
    let selected_lowering_completion = physical.selected_lowering_completion();
    let emitted = omega_machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .map_err(|error| {
        super::diagnostics::realization_error("optimized function-fragment emission", error)
    })?;
    let projected = ProjectedFragments::from_emission(emitted).map_err(|error| {
        super::diagnostics::realization_error(
            "optimized function-fragment frame application",
            error,
        )
    })?;
    let fragments = projected.emission();
    let plan = project_return_only_unit_fragments(&projected).map_err(|error| {
        super::diagnostics::realization_error("optimized fragment native publication", error)
    })?;
    let physical_evidence_scope = match (
        request.boundary_application_coverage,
        selected_lowering_completion,
    ) {
        (Some(coverage), Some(completion)) => {
            let fragment_manifest = fragments.manifest().record();
            let realization_manifest = fragments.function_relative_manifest().record();
            if realization_manifest.selected_lowering_completion != Some(completion) {
                return Err(super::diagnostics::realization_error(
                    "optimized physical-evidence projection",
                    "physical route and function-relative manifest disagree on selected-lowering completion",
                ));
            }
            let publication = omega_native_artifact::SelectedLoweringNativePublicationInput::new(
                fragment_manifest.selections,
                completion,
                fragments.pre_physical_manifest().record().identity,
                fragments.post_allocation_manifest().record().identity,
                realization_manifest.identity,
                fragments.custody().fragments(),
                fragments.custody().manifest(),
                &plan,
            );
            omega_native_artifact::NativePhysicalEvidenceScope::from_validated_selected_lowering_optimization(
                request.optimized_plan,
                request.terminal,
                request.validation,
                request.final_unit,
                coverage,
                publication,
            )
            .map_err(|error| {
                super::diagnostics::realization_error(
                    "optimized physical-evidence projection",
                    error,
                )
            })?
        }
        (Some(_), None) => {
            return Err(super::diagnostics::realization_error(
                "optimized physical-evidence projection",
                "the selected physical route has no admitted native evidence projection; no coverage was discarded",
            ));
        }
        (None, _) => omega_native_artifact::NativePhysicalEvidenceScope::Unavailable,
    };
    Ok((plan, physical_evidence_scope))
}

/// The emitted fragments plus, where the realization owns a target frame
/// protocol, its applied prologue and epilogue bytes. A route without a frame
/// protocol publishes the emitted fragments unchanged.
enum ProjectedFragments {
    Frameless(StagedOptimizedFunctionFragmentEmission),
    Framed(StagedFunctionFragmentFrameApplication),
}

impl ProjectedFragments {
    fn from_emission(
        emitted: StagedOptimizedFunctionFragmentEmission,
    ) -> Result<Self, omega_machine_emission::FunctionFragmentFrameApplicationError> {
        if emitted.source().frame_protocol().is_none() {
            return Ok(Self::Frameless(emitted));
        }
        stage_function_fragment_frame_application(emitted).map(Self::Framed)
    }

    const fn emission(&self) -> &StagedOptimizedFunctionFragmentEmission {
        match self {
            Self::Frameless(emitted) => emitted,
            Self::Framed(applied) => applied.source(),
        }
    }

    fn fragments(&self) -> &FunctionFragmentEmissionPlan {
        match self {
            Self::Frameless(emitted) => emitted.fragments(),
            Self::Framed(applied) => applied.fragments(),
        }
    }

    fn applied_frames(&self) -> &[FunctionAppliedFrameProtocol] {
        match self {
            Self::Frameless(_) => &[],
            Self::Framed(applied) => &applied.application().functions,
        }
    }
}

fn project_return_only_unit_fragments(
    projected: &ProjectedFragments,
) -> Result<MachineCodePlan, &'static str> {
    let staged = projected.emission();
    validate_optimized_function_fragment_emission(staged)
        .map_err(|_| "optimized fragment custody failed replay")?;
    let fragments = projected.fragments();
    let return_bytes: &[u8] = match (
        fragments.target.architecture,
        fragments.target.pointer_size,
        fragments.target.pointer_alignment,
    ) {
        (omega_target::Architecture::X86_64, 8, 8) => &[0xc3],
        (omega_target::Architecture::Aarch64, 8, 8) => &[0xc0, 0x03, 0x5f, 0xd6],
        _ => {
            return Err("optimized return-only publication requires an eight-byte pointer target");
        }
    };
    if !fragments.structural_unit_functions.is_empty() {
        return Err("optimized native publication does not yet admit structural Unit fragments");
    }

    let functions = fragments
        .functions
        .iter()
        .map(|fragment| {
            project_function(
                fragment,
                return_bytes,
                projected
                    .applied_frames()
                    .iter()
                    .find(|row| row.machine == fragment.machine),
                staged.source().frame_layout(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if functions.is_empty() {
        return Err("optimized native publication requires at least one function");
    }

    Ok(MachineCodePlan {
        psi: fragments.psi,
        target: fragments.target,
        entry: fragments.entry,
        functions,
    })
}

fn project_function(
    fragment: &omega_machine_code::FunctionFragment,
    return_bytes: &[u8],
    applied_frame: Option<&FunctionAppliedFrameProtocol>,
    frame_layout: Option<
        &omega_post_allocation_machine_to_frame_layout::ValidatedTargetFrameLayout,
    >,
) -> Result<MachineCodeFunction, &'static str> {
    let stack = unit_stack_evidence(fragment, return_bytes, applied_frame, frame_layout)?;
    // A framed function opens its block after the prologue and places the
    // return after the epilogue that restores the frame.
    let (block_offset, return_offset) = match applied_frame {
        Some(row) => {
            let [epilogue] = row.epilogues.as_slice() else {
                return Err("optimized framed Unit fragment requires one applied epilogue");
            };
            (
                row.prologue_byte_count,
                row.prologue_byte_count + epilogue.byte_count,
            )
        }
        None => (0, 0),
    };
    let [block] = fragment.blocks.as_slice() else {
        return Err(
            "optimized native publication currently admits one return-only block per function",
        );
    };
    let [instruction] = block.instructions.as_slice() else {
        return Err(
            "optimized native publication currently admits one return-only instruction per function",
        );
    };
    let omega_machine_code::FunctionFragmentControlProvenance::Return { psi_return_edge } =
        instruction.control
    else {
        return Err("optimized native publication currently admits only Unit returns");
    };
    if instruction.alternative.family != MachineAlternativeFamily::ReturnUnit
        || instruction.branch.is_some()
        || !instruction.provenance.operations.is_empty()
        || !instruction.provenance.values.is_empty()
        || !instruction.provenance.obligations.is_empty()
        || instruction.provenance.edges.as_slice() != [psi_return_edge]
        || !fragment.provenance.operations.is_empty()
        || fragment.provenance.edges.as_slice() != [psi_return_edge]
        || fragment.bytes.get(return_offset as usize..) != Some(return_bytes)
        || block.offset != block_offset
        || block.byte_count != fragment.byte_count.saturating_sub(block_offset)
        || instruction.offset != return_offset
        || instruction.bytes != return_bytes
        || usize::try_from(fragment.byte_count).ok() != Some(fragment.bytes.len())
    {
        return Err("optimized Unit return fragment is not an exact publication projection");
    }

    Ok(MachineCodeFunction {
        machine: fragment.machine,
        attachment: fragment.attachment,
        fixed_integer_scalar_abi: None,
        mixed_structural_scalar_abi: None,
        structural_call_scalar_return: None,
        unit_scalar_abi: None,
        provenance: fragment.provenance.clone(),
        bytes: fragment.bytes.clone(),
        x86_scalar_fma: Vec::new(),
        x86_scalar_fma_occurrences: Vec::new(),
        x86_floating_control: None,
        unit_stack: Some(stack),
        unit_parameter_homes: Vec::new(),
        unit_parameters: Vec::new(),
        scalar_stack: None,
        internal_calls: Vec::new(),
        foreign_calls: Vec::new(),
        internal_unit_calls: Vec::new(),
        internal_unit_scalar_calls: Vec::new(),
        installed_provider_unit_scalar_calls: Vec::new(),
        dynamic_calls: Vec::new(),
        stored_dynamic_calls: Vec::new(),
        dynamic_parameter_calls: Vec::new(),
        forwarded_dynamic_parameter_calls: Vec::new(),
        forwarded_dynamic_descriptor_calls: Vec::new(),
        unit_scalar_homes: Vec::new(),
        unit_integer_constants: Vec::new(),
        unit_affine_scalar_records: Vec::new(),
        unit_structural_scalar_field_stores: Vec::new(),
        unit_write_only_primitive_stores: Vec::new(),
        scalar_structural_scalar_field_stores: Vec::new(),
        unit_affine_cleanup: Some(omega_machine_code::UnitAffineCleanupRecord {
            psi_edge: psi_return_edge,
            structural_types: Vec::new(),
            locals: Vec::new(),
            actions: Vec::new(),
            code_offset: 0,
            byte_count: fragment.bytes.len(),
        }),
        scalar_affine_cleanup: None,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters: Vec::new(),
        scalar_structural_parameter_homes: Vec::new(),
        ranked_u32_countdown: None,
        semantic_code_attribution: vec![SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(psi_return_edge),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: fragment.bytes.len(),
        }],
        port_effects: Vec::new(),
        boundary_settlements: Vec::new(),
        structural_return: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_machine_code::{
        FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentControlProvenance,
        FunctionFragmentInstructionSpan,
    };
    use omega_selected_instructions::{
        MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionProvenance,
    };
    use omega_target_operations::TerminalPsiProvenance;
    use psi_core::{EdgeId, MachineId};

    const X86_RETURN: &[u8] = &[0xc3];
    const AARCH64_RETURN: &[u8] = &[0xc0, 0x03, 0x5f, 0xd6];

    fn return_fragment(return_bytes: &[u8]) -> FunctionFragment {
        let edge = EdgeId::new(7).expect("edge");
        let byte_count = return_bytes.len() as u64;
        FunctionFragment {
            machine: MachineId::new(3).expect("machine"),
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: vec![edge],
            },
            byte_count,
            bytes: return_bytes.to_vec(),
            blocks: vec![FunctionFragmentBlockSpan {
                block: SelectedBlockId(0),
                offset: 0,
                byte_count,
                instructions: vec![FunctionFragmentInstructionSpan {
                    instruction: SelectedInstructionId(0),
                    alternative: MachineAlternativeKey {
                        family: MachineAlternativeFamily::ReturnUnit,
                        variant: 0,
                    },
                    offset: 0,
                    bytes: return_bytes.to_vec(),
                    branch: None,
                    internal_machine_fixup: None,
                    provenance: SelectedInstructionProvenance {
                        operations: Vec::new(),
                        values: Vec::new(),
                        edges: vec![edge],
                        obligations: Vec::new(),
                        fuel: Vec::new(),
                    },
                    control: FunctionFragmentControlProvenance::Return {
                        psi_return_edge: edge,
                    },
                }],
            }],
        }
    }

    #[test]
    fn exact_return_projects_explicit_empty_unit_custody() {
        for return_bytes in [X86_RETURN, AARCH64_RETURN] {
            let projected =
                project_function(&return_fragment(return_bytes), return_bytes, None, None)
                    .expect("exact Unit return");
            assert_eq!(projected.bytes, return_bytes);
            let stack = projected.unit_stack.expect("Unit stack evidence");
            assert!(stack.frame.is_none());
            assert!(stack.aarch64_return_link.is_none());
            assert_eq!(stack.stack_alignment, 16);
            assert!(projected.unit_affine_cleanup.is_some());
            assert_eq!(projected.semantic_code_attribution.len(), 1);
        }
    }

    #[test]
    fn altered_return_family_or_bytes_rejects() {
        for return_bytes in [X86_RETURN, AARCH64_RETURN] {
            let mut wrong_family = return_fragment(return_bytes);
            wrong_family.blocks[0].instructions[0].alternative.family =
                MachineAlternativeFamily::ReturnI64;
            assert!(project_function(&wrong_family, return_bytes, None, None).is_err());

            let mut wrong_bytes = return_fragment(return_bytes);
            wrong_bytes.blocks[0].instructions[0].bytes[0] = 0x90;
            assert!(project_function(&wrong_bytes, return_bytes, None, None).is_err());
        }
    }

    #[test]
    fn one_architectures_return_encoding_rejects_under_the_other() {
        assert!(
            project_function(&return_fragment(X86_RETURN), AARCH64_RETURN, None, None).is_err()
        );
        assert!(
            project_function(&return_fragment(AARCH64_RETURN), X86_RETURN, None, None).is_err()
        );
    }
    /// An empty machine selected for optimization joins Terminal Psi to object
    /// custody. Only x86-64 is exercised: an authored empty machine resolves to
    /// `CanonicalFixedFrameBodyV1`, not the Unit baseline route, and that route
    /// takes the AAPCS64 leaf exemption, so its AArch64 function is still a
    /// bare `ret` the object boundary rejects. `TASKS_OPTIMIZER.md` owns that.
    #[test]
    fn optimized_empty_machine_reaches_object_custody() {
        {
            let (target, expected_bytes) = (omega_target::NativeTarget::linux_x64(), 1_usize);
            let checked = crate::tests::fixtures::checked_source::checked(
                "data Main {} machine Main::launch() {}",
            );
            let artifact =
                psi_checked_trees_to_terminal::produce_terminal_artifact(&checked, "Main::launch")
                    .expect("canonical empty-machine Terminal artifact");
            let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &psi_proof_admission::AdmissionProfile::default(),
            )
            .expect("verified abstract input");
            let abstract_program = crate::optimize_verified_abstract_input(
                input,
                crate::compiler_baseline_request_v1(
                    &omega_optimization_core::OptimizationSelections::default(),
                ),
            )
            .expect("complete abstract optimization");
            let post_terminal = abstract_program.selections().project_post_terminal();
            let optimized_target =
                omega_abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                    abstract_program,
                    target,
                )
                .expect("independently validated target lowering");
            let physical = crate::stage_optimized_verified_physical_pipeline(
                optimized_target,
                post_terminal.selections(),
            )
            .expect("optimized physical pipeline");
            let emitted = omega_machine_emission::stage_optimized_function_fragment_emission(
                physical.into_function_fragment_emission_source(),
            )
            .expect("optimized function-fragment emission");
            let projected =
                ProjectedFragments::from_emission(emitted).expect("applied frame protocol");
            let plan =
                project_return_only_unit_fragments(&projected).expect("return-only projection");

            let [function] = plan.functions.as_slice() else {
                panic!("one projected Unit function");
            };
            assert_eq!(function.bytes.len(), expected_bytes);
            let stack = function.unit_stack.expect("Unit stack evidence");
            assert_eq!(
                stack.aarch64_return_link.is_some(),
                target.architecture == omega_target::Architecture::Aarch64
            );
            assert_eq!(stack.frame.is_some(), stack.aarch64_return_link.is_some());

            let object = omega_image_emission::build_object_artifact(&plan)
                .expect("optimized empty machine reaches object custody");
            assert_eq!(object.target(), target);
            assert_eq!(object.text_bytes(), function.bytes.as_slice());
        }
    }
}
