/*
Resolved function-relative machine layout, branch displacement evidence, layout
ordering policy, and post-layout x86 branch relaxation evidence.

We represent the resolved machine layout (`ResolvedMachineLayout`) and program
bundle (`ResolvedMachineProgram`) independently of the producing stage history.
Each function layout (`ResolvedSelectedFunctionLayout`) owns ordered block spans
(`ResolvedSelectedBlockLayout`) and encoded instruction rows
(`ResolvedSelectedFormRow`) with exact byte offsets, resolved branch evidence
(`ResolvedBranchEvidence`), and unresolved call or image-address fixup records.
We also define the post-layout x86 branch relaxation evidence and identity
contracts (`evidence`) used to replay six-byte-near to two-byte-short branch
rewrites without embedding optimizer heuristics into the baseline layout.
Layouts carry no content hash: checks compare the retained rows and roots.
*/

mod relaxation_record;
pub mod text_section;
pub use relaxation_record::X86BranchRelaxationRecord;

use std::sync::Arc;

use post_allocation_machine::{
    PostAllocationMachineIdentity, PostAllocationMachineOptimizationCustody,
};
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::{EdgeId, MachineId};
use target::NativeTarget;

use crate::{
    SelectedFormEncoding, SelectedFormEncodingIdentity, SelectedFormImageAddressFixup,
    SelectedFormInternalMachineFixup, SelectedFormNormalizedForeignCallFixup,
    TargetFrameLayoutPlan, TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
};

/// The unresolved bridge call in its encoded instruction stream. x86 identifies
/// the displacement field and AArch64 identifies the instruction containing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterCallField {
    X86Relative32 { field_offset: usize },
    Aarch64Branch26 { instruction_offset: usize },
}

pub use evidence::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
    relaxation, relaxation_identity,
};
pub use text_section::{
    InternalMachineCallResolutionKind, InternalMachineCallResolutionState,
    NormalizedForeignCallResolutionKind, NormalizedForeignCallResolutionState, PlacedBlockSpan,
    PlacedFunctionFragment, PlacedInstructionSpan, PlacedInternalMachineCallResolution,
    PlacedNormalizedForeignCallResolution, RelocationFreeTextSectionPlacement,
    TextSectionPlacementPolicy, TextSectionRelocationRequirements,
};

/// Retains the exact input joins for a layout without retaining its earlier rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedLayoutRoots {
    pub selected: SelectedInstructionPlanIdentity,
    pub machine: PostAllocationMachineIdentity,
    pub pre_layout: SelectedFormEncodingIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedFunctionLayoutPolicy {
    EntryThenZeroFallthroughThenNonzeroV1,
    EntryThenNotLessFallthroughThenLessV1,
    SingleEntryBlockV1,
    PerFunctionCanonicalShapeV1, // Entry-first conditional-fallthrough chains; remaining chain roots follow selected block roster
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedBranchEvidence {
    Conditional(ResolvedConditionalBranchEvidence),
    Jump(ResolvedJumpEvidence),
}

impl ResolvedBranchEvidence {
    pub const fn as_conditional(&self) -> Option<&ResolvedConditionalBranchEvidence> {
        match self {
            Self::Conditional(value) => Some(value),
            Self::Jump(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedJumpEvidence {
    pub source_block: SelectedBlockId,
    pub target_edge: Option<EdgeId>,
    pub target_block: SelectedBlockId,
    pub target_offset: u64,
    pub byte_displacement: i64,
    pub decoded_effects: MachineEncodedEffects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedConditionalBranchPredicate {
    NonZeroV1,
    U64LessThanV1,
    I64LessThanV1,
}

impl ResolvedConditionalBranchPredicate {
    pub const fn rank(self) -> u8 {
        match self {
            Self::NonZeroV1 => 0,
            Self::U64LessThanV1 => 1,
            Self::I64LessThanV1 => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConditionalBranchEvidence {
    pub predicate: ResolvedConditionalBranchPredicate,
    pub source_block: SelectedBlockId,
    pub when_taken_edge: Option<EdgeId>,
    pub when_taken_block: SelectedBlockId,
    pub when_taken_offset: u64,
    pub when_fallthrough_edge: Option<EdgeId>,
    pub when_fallthrough_block: SelectedBlockId,
    pub when_fallthrough_offset: u64,
    pub byte_displacement: i64, // x86-64 measures from instruction end; AArch64 measures from the branch word address
    pub decoded_register_reads: Vec<register_model::RegisterViewId>,
    pub decoded_effects: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedFormRow {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub branch: Option<Box<ResolvedBranchEvidence>>,
    pub internal_machine_fixup: Option<Box<SelectedFormInternalMachineFixup>>, // Owned only by relocation-bearing rows
    pub normalized_foreign_call_fixup: Option<Box<SelectedFormNormalizedForeignCallFixup>>, // Bound to declared import symbol during object construction
    pub image_address_fixup: Option<Box<SelectedFormImageAddressFixup>>, // Row-relative byte-literal address bound during object construction
}

impl ResolvedSelectedFormRow {
    pub fn internal_machine_fixup(&self) -> Option<SelectedFormInternalMachineFixup> {
        self.internal_machine_fixup.as_deref().copied()
    }

    pub fn normalized_foreign_call_fixup(&self) -> Option<&SelectedFormNormalizedForeignCallFixup> {
        self.normalized_foreign_call_fixup.as_deref()
    }

    pub fn image_address_fixup(&self) -> Option<SelectedFormImageAddressFixup> {
        self.image_address_fixup.as_deref().copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedBlockLayout {
    pub block: SelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub instructions: Vec<ResolvedSelectedFormRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedFunctionLayout {
    pub machine: MachineId,
    pub byte_count: u64,
    pub blocks: Vec<ResolvedSelectedBlockLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMachineLayout {
    pub selected: SelectedInstructionPlanIdentity,
    pub machine: PostAllocationMachineIdentity,
    pub pre_layout: SelectedFormEncodingIdentity,
    pub post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    pub target: NativeTarget,
    pub policy: SelectedFunctionLayoutPolicy,
    pub functions: Vec<ResolvedSelectedFunctionLayout>,
}

impl ResolvedMachineLayout {
    pub fn roots(&self) -> ResolvedLayoutRoots {
        ResolvedLayoutRoots {
            selected: self.selected,
            machine: self.machine,
            pre_layout: self.pre_layout,
        }
    }

    pub fn selected(&self) -> SelectedInstructionPlanIdentity {
        self.selected
    }

    pub fn machine(&self) -> PostAllocationMachineIdentity {
        self.machine
    }

    pub fn pre_layout(&self) -> SelectedFormEncodingIdentity {
        self.pre_layout
    }

    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.post_allocation_machine_optimization
    }

    pub fn target(&self) -> NativeTarget {
        self.target
    }

    pub fn policy(&self) -> SelectedFunctionLayoutPolicy {
        self.policy
    }

    pub fn functions(&self) -> &[ResolvedSelectedFunctionLayout] {
        &self.functions
    }

    pub fn function(&self, machine: MachineId) -> Option<&ResolvedSelectedFunctionLayout> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

    pub fn blocks(&self) -> impl Iterator<Item = &ResolvedSelectedBlockLayout> + '_ {
        self.functions()
            .iter()
            .flat_map(|function| &function.blocks)
    }

    pub fn instructions(&self) -> impl Iterator<Item = &ResolvedSelectedFormRow> + '_ {
        self.blocks().flat_map(|block| &block.instructions)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FramedMachineProgram {
    pub bindings: crate::import_code::EncodedBodyBindings,
    pub fragment_origins: Arc<crate::fragment_origins::FragmentOrigins>,
    pub frame_demands: Arc<crate::frame_demands::MachineFrameDemands>,
    pub protocol_identity: TargetFrameProtocolEncodingIdentity,
    pub instructions: post_allocation_machine::PostAllocationMachineProgram,
    pub storage: Arc<crate::NonAuthoritativeCalleeSaveStoragePlan>,
    pub frame: Arc<TargetFrameLayoutPlan>,
    pub protocol: Arc<TargetFrameProtocolEncodingPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedMachineProgram {
    pub bindings: crate::import_code::EncodedBodyBindings,
    pub exit_requirements: Arc<crate::exit_requirements::EncodedExitRequirements>,
    pub fragment_origins: Arc<crate::fragment_origins::FragmentOrigins>,
    pub layout_requirements: Arc<crate::layout_requirements::EncodedLayoutRequirements>,
    pub frame_demands: Arc<crate::frame_demands::MachineFrameDemands>,
    pub protocol_identity: TargetFrameProtocolEncodingIdentity,
    pub context: Arc<crate::program_context::EncodedProgramContext>,
    pub storage: Arc<crate::NonAuthoritativeCalleeSaveStoragePlan>,
    pub frame: Arc<TargetFrameLayoutPlan>,
    pub protocol: Arc<TargetFrameProtocolEncodingPlan>,
    pub encoding: Arc<SelectedFormEncoding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMachineProgram {
    /// The encoder’s branch alternatives in the same function order as the resolved layout.
    pub control_alternatives: Arc<[crate::EncodedControlIndex]>,
    pub bindings: crate::import_code::EncodedBodyBindings,
    pub exit_requirements: Arc<crate::exit_requirements::EncodedExitRequirements>,
    pub fragment_origins: Arc<crate::fragment_origins::FragmentOrigins>,
    pub layout_requirements: Arc<crate::layout_requirements::EncodedLayoutRequirements>,
    pub frame_demands: Arc<crate::frame_demands::MachineFrameDemands>,
    pub protocol_identity: TargetFrameProtocolEncodingIdentity,
    pub context: Arc<crate::program_context::EncodedProgramContext>,
    pub storage: Arc<crate::NonAuthoritativeCalleeSaveStoragePlan>,
    pub frame: Arc<TargetFrameLayoutPlan>,
    pub protocol: Arc<TargetFrameProtocolEncodingPlan>,
    pub encoding: Arc<SelectedFormEncoding>,
    pub layout: Arc<ResolvedMachineLayout>,
}

/// The resolved-layout stage's output: the program with its active
/// layout, the baseline layout it came from, the layout-phase selections and
/// budget that governed the run, and the identity of any x86 branch relaxation.
/// Native stage 12 retains the relaxation trace for replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedMachineLayout {
    pub baseline: ResolvedLayoutRoots,
    pub program: ResolvedMachineProgram,
    pub selections: optimization_core::OptimizationPhaseSelections,
    pub budget: optimization_core::OptimizationWorkBudget,
    pub relaxation: Option<X86BranchRelaxationIdentity>,
}

impl OptimizedMachineLayout {
    /// The active layout: the baseline, or the relaxed layout when relaxation ran.
    pub fn layout(&self) -> &ResolvedMachineLayout {
        &self.program.layout
    }
}

pub mod evidence {
    register_homes::identity_newtype! { pub struct X86BranchRelaxationIdentity([u8; 32]); }

    pub mod relaxation {
        use selected_instructions::SelectedInstructionId;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum X86BranchRelaxationPolicy {
            X86RelaxConditionalBranchesToRel8V1,
        }

        register_homes::identity_newtype! { pub struct X86BranchRelaxationRevisionIdentity([u8; 32]); }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum X86BranchRelaxationAttemptOutcome {
            AlreadyShort,
            NearDisplacementOutsideI8,
            SelectedForRelaxation,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct X86BranchRelaxationAttempt {
            pub iteration: u64,
            pub input: X86BranchRelaxationRevisionIdentity,
            pub instruction: SelectedInstructionId,
            pub offset: u64,
            pub byte_displacement: i64,
            pub encoded_bytes: u8,
            pub outcome: X86BranchRelaxationAttemptOutcome,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct X86BranchRelaxationAction {
            pub iteration: u64,
            pub input: X86BranchRelaxationRevisionIdentity,
            pub output: X86BranchRelaxationRevisionIdentity,
            pub instruction: SelectedInstructionId,
            pub old_offset: u64,
            pub new_offset: u64,
            pub old_displacement: i64,
            pub new_displacement: i64,
            pub old_bytes: Vec<u8>,
            pub new_bytes: Vec<u8>,
        }
    }

    pub mod relaxation_identity {
        use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
        use target::NativeTarget;

        use crate::ResolvedSelectedFunctionLayout;
        use crate::identity_encoding::{encode_conditional_branch_arm, encode_jump_arm};

        use super::{
            X86BranchRelaxationAction, X86BranchRelaxationAttempt,
            X86BranchRelaxationAttemptOutcome, X86BranchRelaxationIdentity,
            X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
        };

        const RELAXATION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation.v11";
        const REVISION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation-revision.v4";

        #[derive(Clone, Copy)]
        pub struct RevisionRoots {
            pub selected: selected_instructions::SelectedInstructionPlanIdentity,
            pub machine: post_allocation_machine::PostAllocationMachineIdentity,
            pub pre_layout: crate::SelectedFormEncodingIdentity,
            pub target: NativeTarget,
        }

        pub fn revision_identity(
            roots: RevisionRoots,
            functions: &[ResolvedSelectedFunctionLayout],
        ) -> X86BranchRelaxationRevisionIdentity {
            let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(REVISION_SCHEMA);
            encode_roots(&mut hasher, roots);
            encode_functions(&mut hasher, functions);
            X86BranchRelaxationRevisionIdentity::from_bytes(hasher.finish())
        }

        #[allow(clippy::too_many_arguments)]
        pub fn artifact_identity(
            roots: RevisionRoots,
            policy: X86BranchRelaxationPolicy,
            budget: OptimizationWorkBudget,
            usage: OptimizationWorkUsage,
            output_revision: X86BranchRelaxationRevisionIdentity,
            attempts: &[X86BranchRelaxationAttempt],
            actions: &[X86BranchRelaxationAction],
            functions: &[ResolvedSelectedFunctionLayout],
        ) -> X86BranchRelaxationIdentity {
            let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(RELAXATION_SCHEMA);
            encode_roots(&mut hasher, roots);
            hasher.byte(match policy {
                X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1 => 0,
            });
            hasher.raw(&budget.encode());
            hasher.raw(&usage.encode());
            hasher.raw(&output_revision.bytes());
            hasher.u64(attempts.len() as u64);
            for attempt in attempts {
                hasher.raw(&attempt.iteration.to_le_bytes());
                hasher.raw(&attempt.input.bytes());
                hasher.raw(&attempt.instruction.0.to_le_bytes());
                hasher.raw(&attempt.offset.to_le_bytes());
                hasher.raw(&attempt.byte_displacement.to_le_bytes());
                hasher.byte(attempt.encoded_bytes);
                hasher.byte(match attempt.outcome {
                    X86BranchRelaxationAttemptOutcome::AlreadyShort => 0,
                    X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8 => 1,
                    X86BranchRelaxationAttemptOutcome::SelectedForRelaxation => 2,
                });
            }
            hasher.u64(actions.len() as u64);
            for action in actions {
                hasher.raw(&action.iteration.to_le_bytes());
                hasher.raw(&action.input.bytes());
                hasher.raw(&action.output.bytes());
                hasher.raw(&action.instruction.0.to_le_bytes());
                hasher.raw(&action.old_offset.to_le_bytes());
                hasher.raw(&action.new_offset.to_le_bytes());
                hasher.raw(&action.old_displacement.to_le_bytes());
                hasher.raw(&action.new_displacement.to_le_bytes());
                hasher.bytes(&action.old_bytes);
                hasher.bytes(&action.new_bytes);
            }
            encode_functions(&mut hasher, functions);
            X86BranchRelaxationIdentity::from_bytes(hasher.finish())
        }

        fn encode_roots(
            hasher: &mut crate::digest_encoder::DomainDigestEncoder,
            roots: RevisionRoots,
        ) {
            hasher.raw(&roots.selected.bytes());
            hasher.raw(&roots.machine.bytes());
            hasher.raw(&roots.pre_layout.bytes());
            hasher.raw(&roots.target.identity_bytes());
        }

        fn encode_functions(
            hasher: &mut crate::digest_encoder::DomainDigestEncoder,
            functions: &[ResolvedSelectedFunctionLayout],
        ) {
            hasher.u64(functions.len() as u64);
            for function in functions {
                hasher.raw(&function.machine.get().to_le_bytes());
                hasher.raw(&function.byte_count.to_le_bytes());
                hasher.u64(function.blocks.len() as u64);
                for block in &function.blocks {
                    hasher.raw(&block.block.0.to_le_bytes());
                    hasher.raw(&block.offset.to_le_bytes());
                    hasher.raw(&block.byte_count.to_le_bytes());
                    hasher.u64(block.instructions.len() as u64);
                    for row in &block.instructions {
                        hasher.raw(&row.instruction.0.to_le_bytes());
                        selected_instructions::encode_machine_alternative_key_identity(
                            hasher,
                            row.alternative,
                        );
                        hasher.raw(&row.offset.to_le_bytes());
                        hasher.bytes(&row.bytes);
                        match row.branch.as_deref() {
                            None => hasher.byte(0),
                            Some(crate::ResolvedBranchEvidence::Conditional(branch)) => {
                                encode_conditional_branch_arm(hasher, branch);
                                selected_instructions::encode_machine_encoded_effects_identity(
                                    hasher,
                                    &branch.decoded_effects,
                                );
                            }
                            Some(crate::ResolvedBranchEvidence::Jump(jump)) => {
                                encode_jump_arm(hasher, jump);
                                selected_instructions::encode_machine_encoded_effects_identity(
                                    hasher,
                                    &jump.decoded_effects,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    pub use relaxation::{
        X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
        X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
    };
}

/// Selection policy and rejection reasons shared by layout and independent replay.
pub mod relaxation_policy {
    use crate::ResolvedMachineLayout;
    use optimization_core::{
        Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
        OptimizationPhaseMismatch, OptimizationPhaseSelections,
    };
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::{SelectedBlockId, SelectedInstructionId};
    use target::{Architecture, NativeTarget};
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum X86BranchRelaxationWorkAxis {
        RuleEvaluations,
        Candidates,
        ValidationSteps,
        Commits,
        Iterations,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedX86BranchRelaxationError {
        UnsupportedTarget(NativeTarget),
        BudgetExceeded(X86BranchRelaxationWorkAxis),
        DuplicateInstruction(SelectedInstructionId),
        MissingTargetBlock(SelectedBlockId),
        OffsetOverflow,
        NonContiguousBlock(SelectedBlockId),
        BranchFallthroughMismatch(SelectedInstructionId),
        MalformedBranch(SelectedInstructionId),
        BranchEffectsMismatch(SelectedInstructionId),
        NonDecreasingByteMeasure,
        X86_64(X86BranchFormError),
        ArtifactMismatch,
    }

    impl std::fmt::Display for OptimizedX86BranchRelaxationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "optimized x86 branch relaxation failed: {self:?}"
            )
        }
    }

    impl std::error::Error for OptimizedX86BranchRelaxationError {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum X86BranchFormError {
        AlternativeMismatch,
        MalformedEncoding,
        EncodedFormMismatch,
        BranchDisplacementOutsideI8,
        BranchDisplacementOutsideI32,
    }

    pub fn validate_roots(
        source: &ResolvedMachineLayout,
        physical: &ValidatedPhysicalRegisterModel,
    ) -> Result<(), OptimizedX86BranchRelaxationError> {
        ensure_x86_target(source.target(), physical)
    }

    pub fn ensure_x86_target(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
    ) -> Result<(), OptimizedX86BranchRelaxationError> {
        if target.architecture != Architecture::X86_64
            || physical.model().architecture != Architecture::X86_64
        {
            return Err(OptimizedX86BranchRelaxationError::UnsupportedTarget(target));
        }
        Ok(())
    }

    pub type FunctionRelativeLayoutRuleCatalogEntry = OptimizationCatalogDescriptor<Architecture>;

    pub const FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG: [FunctionRelativeLayoutRuleCatalogEntry; 1] =
        [FunctionRelativeLayoutRuleCatalogEntry::new(
            Optimization::X86RelaxConditionalBranchesToRel8V1,
            Architecture::X86_64,
        )];

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FunctionRelativeLayoutCatalogError {
        WrongPhase(OptimizationPhaseMismatch),
        UnsupportedSelection(Optimization),
        UnsupportedComposition,
        UnsupportedTarget {
            optimization: Optimization, // Optimization rule rejected for the target architecture.
            required: Architecture,     // Architecture required by the catalog entry.
            actual: Architecture,       // Architecture supplied by the compilation target.
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ResolvedLayoutOptimizationError {
        Catalog(FunctionRelativeLayoutCatalogError),
        Relaxation(OptimizedX86BranchRelaxationError),
        CurrentProgramMismatch,
        SelectionMismatch,
    }

    impl std::fmt::Display for ResolvedLayoutOptimizationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "resolved-layout optimization failed: {self:?}")
        }
    }

    impl std::error::Error for ResolvedLayoutOptimizationError {}

    /// Replay the run from the original baseline layout: the current form and its
    /// relaxation trace must be what the selections and budget produce from it.
    pub fn x86_rel8_selected(
        selections: &OptimizationPhaseSelections,
        architecture: Architecture,
    ) -> Result<bool, FunctionRelativeLayoutCatalogError> {
        let phase = selections
            .require_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
            .map_err(FunctionRelativeLayoutCatalogError::WrongPhase)?;
        match phase.as_slice() {
            [] => Ok(false),
            [Optimization::X86RelaxConditionalBranchesToRel8V1] => {
                let descriptor = FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG[0];
                let required = *descriptor.payload();
                if architecture != required {
                    return Err(FunctionRelativeLayoutCatalogError::UnsupportedTarget {
                        optimization: descriptor.optimization(),
                        required,
                        actual: architecture,
                    });
                }
                Ok(true)
            }
            [unsupported] => Err(FunctionRelativeLayoutCatalogError::UnsupportedSelection(
                *unsupported,
            )),
            _ => Err(FunctionRelativeLayoutCatalogError::UnsupportedComposition),
        }
    }
}

/// Encoding uses the bound program identity and the two requested readbacks.
pub struct BoundEncodingRequirements<Publication> {
    subject: crate::program_context::BoundEncodingSubject,
    publication: Publication,
    retain_original: bool,
    check_installed_branches: bool,
}

impl<Publication> BoundEncodingRequirements<Publication> {
    pub fn new(
        subject: crate::program_context::BoundEncodingSubject,
        publication: Publication,
        retain_original: bool,
        check_installed_branches: bool,
    ) -> Self {
        Self {
            subject,
            publication,
            retain_original,
            check_installed_branches,
        }
    }

    pub fn capture_original(
        &self,
        instructions: &post_allocation_machine::PostAllocationMachineProgram,
    ) -> Option<std::sync::Arc<post_allocation_machine::PostAllocationMachineProgram>> {
        self.retain_original
            .then(|| std::sync::Arc::new(instructions.clone()))
    }

    pub fn finish(
        self,
        program: &EncodedMachineProgram,
        original: Option<std::sync::Arc<post_allocation_machine::PostAllocationMachineProgram>>,
    ) -> Result<EncodedBodyContracts<EncodedBodyPublication<Publication>>, &'static str> {
        let encoded = crate::program_context::BoundEncodingSubject::from_encoded(
            &program.context,
            Arc::clone(&program.frame),
        )?;
        if self.subject != encoded {
            return Err("bound body contracts differ from the encoded program or its frame");
        }
        if self.retain_original != original.is_some() {
            return Err("instruction readback differs from the bound body's audit request");
        }
        let publication =
            EncodedBodyPublication::from_encoding(self.publication, program, original)?;
        Ok(EncodedBodyContracts::new(
            program,
            FramePlacementContracts {
                encoded,
                retain_original: self.retain_original,
                publication: std::sync::Arc::new(publication),
                check_installed_branches: self.check_installed_branches,
            },
        ))
    }
}

/// Layout can move the contracts with the code. Only the matching encoded
/// context and frame release them for object construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedBodyContracts<Contracts> {
    context: std::sync::Arc<crate::program_context::EncodedProgramContext>,
    frame: Arc<TargetFrameLayoutPlan>,
    placement: FramePlacementContracts<Contracts>,
}

impl<Contracts> EncodedBodyContracts<Contracts> {
    fn new(program: &EncodedMachineProgram, placement: FramePlacementContracts<Contracts>) -> Self {
        Self {
            context: std::sync::Arc::clone(&program.context),
            frame: Arc::clone(&program.frame),
            placement,
        }
    }
    pub fn audit_requested(&self) -> bool {
        self.placement.retain_original
    }

    pub fn into_placement(
        self,
        program: &ResolvedMachineProgram,
    ) -> Option<FramePlacementContracts<Contracts>> {
        (self.frame == program.frame && self.context == program.context).then_some(self.placement)
    }
}

/// Frame placement reads its branch obligation while carrying the sealed publication facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FramePlacementContracts<Contracts> {
    pub encoded: crate::program_context::BoundEncodingSubject,
    pub retain_original: bool,
    pub publication: std::sync::Arc<Contracts>,
    pub check_installed_branches: bool,
}

/// The physical instructions and encoding used by object readback on explicit request.
///
/// ```compile_fail,E0609
/// fn recover_allocator(input: &machine_code::layout::PhysicalEncodingReadback) {
///     let _ = &input.allocation;
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEncodingReadback {
    pub physical: std::sync::Arc<post_allocation_machine::PostAllocationMachineProgram>,
    pub encoding: std::sync::Arc<crate::SelectedFormEncoding>,
}

/// The encoder pairs the publication facts and optional readback with the same
/// instruction stream. Layout carries this value as its opaque publication.
#[derive(Debug, PartialEq, Eq)]
pub struct EncodedBodyPublication<Contracts> {
    pub body: Contracts,
    original: Option<PhysicalEncodingReadback>,
}

impl<Contracts> EncodedBodyPublication<Contracts> {
    fn from_encoding(
        body: Contracts,
        program: &EncodedMachineProgram,
        physical: Option<std::sync::Arc<post_allocation_machine::PostAllocationMachineProgram>>,
    ) -> Result<Self, &'static str> {
        if physical
            .as_ref()
            .is_some_and(|original| original.identity != program.context.identity)
        {
            return Err("original physical instructions differ from the encoded program");
        }
        Ok(Self {
            body,
            original: physical.map(|physical| PhysicalEncodingReadback {
                physical,
                encoding: program.encoding.clone(),
            }),
        })
    }
    pub fn original_encoding(&self) -> Option<&PhysicalEncodingReadback> {
        self.original.as_ref()
    }
}
