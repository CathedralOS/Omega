//! Test-only legacy fragment-to-machine-plan reference.
//!
//! Differential controls compare return/conditional bytes and stack evidence
//! against this former publication route. Production uses the current shared
//! object and image-emission; this projection is not an executable fallback.

mod scalar;
mod unit_stack;

use crate::{
    StagedFunctionFragmentFrameApplication, StagedOptimizedFunctionFragmentEmission,
    stage_function_fragment_frame_application, validate_optimized_function_fragment_emission,
};
use machine_code::{
    FunctionAppliedFrameProtocol, FunctionFragmentEmissionPlan, MachineCodeFunction,
    MachineCodePlan, SemanticCodeAttribution, SemanticCodeSite,
};
use selected_instructions::MachineAlternativeFamily;
use unit_stack::unit_stack_evidence;

/// A current native plan with the exact fragment/frame input retained for
/// product evidence. Object construction still independently checks its bytes
/// and native records. Callers cannot construct or mutate this stage result.
#[derive(Debug)]
pub struct StagedFragmentNativePublication {
    plan: MachineCodePlan,
    fragments: ProjectedFragments,
}

impl StagedFragmentNativePublication {
    pub const fn plan(&self) -> &MachineCodePlan {
        &self.plan
    }

    /// Explicit replay evidence, not the route to reading the current program.
    pub const fn source(&self) -> &StagedOptimizedFunctionFragmentEmission {
        self.fragments.emission()
    }

    pub fn into_plan(self) -> MachineCodePlan {
        self.plan
    }
}

#[derive(Debug)]
pub enum FragmentNativePublicationError {
    FrameApplication(crate::FunctionFragmentFrameApplicationError),
    UnsupportedProjection(&'static str),
}

impl std::fmt::Display for FragmentNativePublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FrameApplication(error) => write!(formatter, "frame application: {error}"),
            Self::UnsupportedProjection(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for FragmentNativePublicationError {}

/// Apply the selected target's frame protocol and publish its complete admitted
/// native function records. This never invokes the assigned-operation emitter.
pub fn publish_function_fragments(
    emitted: StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedFragmentNativePublication, FragmentNativePublicationError> {
    let fragments = ProjectedFragments::from_emission(emitted)
        .map_err(FragmentNativePublicationError::FrameApplication)?;
    let plan = project_leaf_fragments(&fragments)
        .map_err(FragmentNativePublicationError::UnsupportedProjection)?;
    Ok(StagedFragmentNativePublication { plan, fragments })
}

/// The emitted fragments plus, where the realization owns a target frame
/// protocol, its applied prologue and epilogue bytes. A route without a frame
/// protocol publishes the emitted fragments unchanged.
#[derive(Debug)]
enum ProjectedFragments {
    Frameless(StagedOptimizedFunctionFragmentEmission),
    Framed(StagedFunctionFragmentFrameApplication),
}

impl ProjectedFragments {
    fn from_emission(
        emitted: StagedOptimizedFunctionFragmentEmission,
    ) -> Result<Self, crate::FunctionFragmentFrameApplicationError> {
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

fn project_leaf_fragments(projected: &ProjectedFragments) -> Result<MachineCodePlan, &'static str> {
    let staged = projected.emission();
    validate_optimized_function_fragment_emission(staged)
        .map_err(|_| "optimized fragment custody failed replay")?;
    let fragments = projected.fragments();
    let return_bytes: &[u8] = match (
        fragments.target.architecture,
        fragments.target.pointer_size,
        fragments.target.pointer_alignment,
    ) {
        (target::Architecture::X86_64, 8, 8) => &[0xc3],
        (target::Architecture::Aarch64, 8, 8) => &[0xc0, 0x03, 0x5f, 0xd6],
        _ => {
            return Err("fragment publication requires an eight-byte pointer target");
        }
    };
    if !fragments.structural_unit_functions.is_empty() {
        return Err("optimized native publication does not yet admit structural Unit fragments");
    }

    let functions = fragments
        .functions
        .iter()
        .map(|fragment| {
            let target_function = staged
                .source()
                .optimized_target()
                .target_operations()
                .functions
                .iter()
                .find(|function| function.machine == fragment.machine)
                .ok_or("native fragment has no validated target function")?;
            if target_function.attachment != fragment.attachment
                || target_function.provenance != fragment.provenance
            {
                return Err("native fragment is detached from its target function");
            }
            if let Some(abi) = &target_function.fixed_integer_scalar_abi {
                let abstract_function = staged
                    .source()
                    .optimized_target()
                    .optimized()
                    .plan()
                    .functions
                    .iter()
                    .find(|function| function.machine == fragment.machine)
                    .ok_or("native scalar fragment has no current abstract function")?;
                return scalar::project_function(
                    fragment,
                    abi,
                    abstract_function,
                    fragments.target.architecture,
                );
            }
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
    fragment: &machine_code::FunctionFragment,
    return_bytes: &[u8],
    applied_frame: Option<&FunctionAppliedFrameProtocol>,
    frame_layout: Option<&crate::frame_layout::ValidatedTargetFrameLayout>,
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
    let machine_code::FunctionFragmentControlProvenance::Return { psi_return_edge } =
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

    Ok(leaf_function(
        fragment,
        LeafEvidence::Unit {
            stack,
            psi_return_edge,
        },
    ))
}

enum LeafEvidence {
    Unit {
        stack: machine_code::UnitStackEvidence,
        psi_return_edge: semantic_vocabulary::EdgeId,
    },
    Scalar {
        abi: target_operations::FixedIntegerScalarFunctionAbi,
        stack: machine_code::ScalarStackEvidence,
        attribution: Vec<SemanticCodeAttribution>,
    },
}

/// Common native record construction after the role-specific projection has
/// established its complete leaf contract. No Unit cleanup is invented for a
/// scalar return, and no caller can assemble a mixed leaf disposition.
fn leaf_function(
    fragment: &machine_code::FunctionFragment,
    evidence: LeafEvidence,
) -> MachineCodeFunction {
    let (
        fixed_integer_scalar_abi,
        unit_stack,
        scalar_stack,
        unit_affine_cleanup,
        semantic_code_attribution,
    ) = match evidence {
        LeafEvidence::Unit {
            stack,
            psi_return_edge,
        } => (
            None,
            Some(stack),
            None,
            Some(machine_code::UnitAffineCleanupRecord {
                psi_edge: psi_return_edge,
                structural_types: Vec::new(),
                locals: Vec::new(),
                actions: Vec::new(),
                code_offset: 0,
                byte_count: fragment.bytes.len(),
            }),
            vec![SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(psi_return_edge),
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: fragment.bytes.len(),
            }],
        ),
        LeafEvidence::Scalar {
            abi,
            stack,
            attribution,
        } => (Some(abi), None, Some(stack), None, attribution),
    };
    MachineCodeFunction {
        machine: fragment.machine,
        attachment: fragment.attachment,
        fixed_integer_scalar_abi,
        mixed_structural_scalar_abi: None,
        structural_call_scalar_return: None,
        unit_scalar_abi: None,
        provenance: fragment.provenance.clone(),
        bytes: fragment.bytes.clone(),
        x86_scalar_fma: Vec::new(),
        x86_scalar_fma_occurrences: Vec::new(),
        x86_floating_control: None,
        unit_stack,
        unit_parameter_homes: Vec::new(),
        unit_parameters: Vec::new(),
        scalar_stack,
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
        unit_affine_cleanup,
        scalar_affine_cleanup: None,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters: Vec::new(),
        scalar_structural_parameter_homes: Vec::new(),
        ranked_u32_countdown: None,
        semantic_code_attribution,
        port_effects: Vec::new(),
        boundary_settlements: Vec::new(),
        structural_return: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::{
        FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentControlProvenance,
        FunctionFragmentInstructionSpan,
    };
    use selected_instructions::{
        MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionProvenance,
    };
    use semantic_vocabulary::{EdgeId, MachineId};
    use target_operations::TerminalPsiProvenance;

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
}
