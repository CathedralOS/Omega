//! Narrow publication projection for the first optimized fragment cohort.
//!
//! The optimizer owns instruction selection and fragment validation. This
//! module only rejoins the already validated, return-only Unit fragments to
//! the established native object/image path. It deliberately does not infer
//! evidence for calls, frames, spills, effects, or scalar results. Admission
//! is based on the emitted fragment shape, not the optimization route that
//! produced it.

use omega_machine_code::{
    Aarch64ReturnLinkEvidence, FunctionAppliedFrameProtocol, FunctionFragmentEmissionPlan,
    MachineCodeFunction, MachineCodePlan, SemanticCodeAttribution, SemanticCodeSite,
    StackAdjustmentPair, UnitStackEvidence,
};
use omega_optimization_pipeline::{
    ReturnAddressFrameCustody, StagedFunctionFragmentFrameApplication,
    StagedOptimizedFunctionFragmentEmission, stage_function_fragment_frame_application,
    validate_optimized_function_fragment_emission,
};
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
    physical: omega_optimization_pipeline::StagedOptimizedVerifiedPhysicalPipeline,
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
    let emitted = omega_optimization_pipeline::stage_optimized_function_fragment_emission(
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
    ) -> Result<Self, omega_optimization_pipeline::FunctionFragmentFrameApplicationError> {
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
    frame_layout: Option<&omega_optimization_pipeline::ValidatedTargetFrameLayout>,
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

/// Derive the object boundary's Unit stack evidence from the applied frame
/// bytes and the target-owned geometry that produced them. A frameless route
/// carries neither, which is the x86-64 Unit leaf shape.
fn unit_stack_evidence(
    fragment: &omega_machine_code::FunctionFragment,
    return_bytes: &[u8],
    applied_frame: Option<&FunctionAppliedFrameProtocol>,
    frame_layout: Option<&omega_optimization_pipeline::ValidatedTargetFrameLayout>,
) -> Result<UnitStackEvidence, &'static str> {
    let Some(applied) = applied_frame else {
        return Ok(UnitStackEvidence {
            frame: None,
            aarch64_return_link: None,
            stack_alignment: 16,
        });
    };
    let layout = frame_layout
        .ok_or("optimized framed Unit fragment has no target frame layout")?
        .plan()
        .functions
        .iter()
        .find(|row| row.machine == fragment.machine)
        .ok_or("optimized framed Unit fragment has no target frame layout row")?;
    let ReturnAddressFrameCustody::SavedLinkRegister {
        frame_offset_bytes, ..
    } = layout.return_address
    else {
        return Err("optimized framed Unit fragment does not save its return address");
    };
    saved_link_unit_stack_evidence(
        layout.frame_size_bytes,
        frame_offset_bytes,
        applied,
        fragment.byte_count,
        return_bytes.len(),
    )
}

/// The AAPCS64 saved-link protocol allocates the frame and stores the link in
/// the prologue, then reloads and releases in the epilogue that precedes the
/// return. An empty Unit body puts the epilogue immediately after the
/// prologue, so the function is prologue, epilogue, return.
fn saved_link_unit_stack_evidence(
    frame_size_bytes: u64,
    link_offset_bytes: u64,
    applied: &FunctionAppliedFrameProtocol,
    function_byte_count: u64,
    return_byte_count: usize,
) -> Result<UnitStackEvidence, &'static str> {
    const ADJUSTMENT_BYTES: u64 = 4;
    const LINK_BYTES: u64 = 4;
    let [epilogue] = applied.epilogues.as_slice() else {
        return Err("optimized framed Unit fragment requires one applied epilogue");
    };
    if applied.prologue_function_offset != 0
        || applied.prologue_byte_count != ADJUSTMENT_BYTES + LINK_BYTES
        || epilogue.byte_count != LINK_BYTES + ADJUSTMENT_BYTES
        || epilogue.function_offset != applied.prologue_byte_count
        || epilogue.function_offset + epilogue.byte_count + return_byte_count as u64
            != function_byte_count
    {
        return Err("optimized framed Unit fragment is not the exact saved-link protocol shape");
    }
    let byte_size = u32::try_from(frame_size_bytes)
        .map_err(|_| "optimized framed Unit frame size is not encodable")?;
    let frame_byte_offset = u32::try_from(link_offset_bytes)
        .map_err(|_| "optimized framed Unit link offset is not encodable")?;
    Ok(UnitStackEvidence {
        frame: Some(StackAdjustmentPair {
            byte_size,
            allocation_offset: 0,
            allocation_byte_count: ADJUSTMENT_BYTES as usize,
            release_offset: (epilogue.function_offset + LINK_BYTES) as usize,
            release_byte_count: ADJUSTMENT_BYTES as usize,
        }),
        aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
            frame_byte_offset,
            store_offset: ADJUSTMENT_BYTES as usize,
            load_offset: epilogue.function_offset as usize,
        }),
        stack_alignment: 16,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_machine_code::{
        FunctionAppliedFrameEpilogue, FunctionFragment, FunctionFragmentBlockSpan,
        FunctionFragmentControlProvenance, FunctionFragmentInstructionSpan,
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
    /// The exact framed AArch64 Unit function the optimized route emits: a
    /// sixteen-byte frame whose link slot sits at offset zero, the epilogue
    /// immediately after the prologue, and the return last.
    /// `omega-image-emission` pins the matching bytes and evidence at its
    /// object boundary.
    #[test]
    fn saved_link_evidence_matches_the_object_boundary_geometry() {
        let applied = FunctionAppliedFrameProtocol {
            machine: MachineId::new(1).expect("nonzero machine"),
            prologue_function_offset: 0,
            prologue_byte_count: 8,
            epilogues: vec![FunctionAppliedFrameEpilogue {
                block: SelectedBlockId(1),
                return_instruction: SelectedInstructionId(1),
                psi_return_edge: EdgeId::new(7).expect("nonzero edge"),
                function_offset: 8,
                byte_count: 8,
            }],
        };
        let evidence = saved_link_unit_stack_evidence(16, 0, &applied, 20, AARCH64_RETURN.len())
            .expect("exact saved-link protocol");
        assert_eq!(
            evidence.frame,
            Some(StackAdjustmentPair {
                byte_size: 16,
                allocation_offset: 0,
                allocation_byte_count: 4,
                release_offset: 12,
                release_byte_count: 4,
            })
        );
        assert_eq!(
            evidence.aarch64_return_link,
            Some(Aarch64ReturnLinkEvidence {
                frame_byte_offset: 0,
                store_offset: 4,
                load_offset: 8,
            })
        );
        assert_eq!(evidence.stack_alignment, 16);

        let mut short_epilogue = applied.clone();
        short_epilogue.epilogues[0].byte_count = 4;
        assert!(saved_link_unit_stack_evidence(16, 0, &short_epilogue, 20, 4).is_err());

        let mut detached_epilogue = applied;
        detached_epilogue.epilogues[0].function_offset = 12;
        assert!(saved_link_unit_stack_evidence(16, 0, &detached_epilogue, 20, 4).is_err());
    }
}
