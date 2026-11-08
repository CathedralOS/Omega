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
*/

pub mod text_section;

use std::sync::Arc;

use physical_instructions::{
    PostAllocationMachineIdentity, PostAllocationMachineOptimizationCustody,
    PostAllocationMachinePlan,
};
use register_homes::RegisterHomePlan;
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, PreAllocationMachineEffectPlan, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::{EdgeId, MachineId};
use target::NativeTarget;

use crate::identity_encoding::{encode_conditional_branch_arm, encode_jump_arm, encode_views};
use crate::{
    SelectedFormEncoding, SelectedFormEncodingIdentity, SelectedFormImageAddressFixup,
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState, SelectedFormNormalizedForeignCallFixup,
    TargetFrameLayoutPlan, TargetFrameProtocolEncodingPlan,
};

pub use evidence::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
    relaxation, relaxation_identity,
};
pub use text_section::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, InternalMachineCallResolutionKind,
    InternalMachineCallResolutionState, NormalizedForeignCallResolutionKind,
    NormalizedForeignCallResolutionState, PlacedBlockSpan, PlacedFunctionFragment,
    PlacedInstructionSpan, PlacedInternalMachineCallResolution,
    PlacedNormalizedForeignCallResolution, RelocationFreeTextSectionPlacement,
    TextSectionPlacementPolicy, TextSectionRelocationRequirements,
    relocation_free_text_section_identity,
};

register_homes::identity_newtype! { pub struct ResolvedSelectedFormLayoutIdentity(pub(super) [u8; 32]); }

const LAYOUT_SCHEMA: &[u8] = b"omega.terminal.resolved-selected-form-layout.v17";

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
    pub identity: ResolvedSelectedFormLayoutIdentity,
    pub functions: Vec<ResolvedSelectedFunctionLayout>,
}

impl ResolvedMachineLayout {
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

    pub fn identity(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.identity
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

    pub fn recomputed_identity(&self) -> ResolvedSelectedFormLayoutIdentity {
        resolved_machine_layout_identity(
            self.selected,
            self.machine,
            self.pre_layout,
            self.post_allocation_machine_optimization,
            self.target,
            self.policy,
            &self.functions,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMachineProgram {
    pub selected: Arc<SelectedInstructionPlan>,
    pub homes: Arc<RegisterHomePlan>,
    pub effects: Arc<PreAllocationMachineEffectPlan>,
    pub machine: Arc<PostAllocationMachinePlan>,
    pub encoding: Arc<SelectedFormEncoding>,
    pub layout: Arc<ResolvedMachineLayout>,
    pub frame: Arc<TargetFrameLayoutPlan>,
    pub protocol: Arc<TargetFrameProtocolEncodingPlan>,
}

pub fn resolved_machine_layout_identity(
    selected: SelectedInstructionPlanIdentity,
    machine: PostAllocationMachineIdentity,
    pre_layout: SelectedFormEncodingIdentity,
    post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    target: NativeTarget,
    policy: SelectedFunctionLayoutPolicy,
    functions: &[ResolvedSelectedFunctionLayout],
) -> ResolvedSelectedFormLayoutIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(LAYOUT_SCHEMA);
    hasher.raw(&selected.bytes());
    hasher.raw(&machine.bytes());
    hasher.raw(&pre_layout.bytes());
    match post_allocation_machine_optimization {
        None => hasher.byte(0),
        Some(custody) => {
            hasher.byte(1);
            hasher.byte(custody.optimization() as u8);
            hasher.raw(&custody.artifact_identity());
            hasher.raw(&custody.selections().bytes());
            hasher.raw(&custody.post_allocation_machine_selections().bytes());
            hasher.raw(&custody.source().bytes());
            hasher.u64(custody.action_count() as u64);
            hasher.raw(&custody.baseline_bytes().to_le_bytes());
            hasher.raw(&custody.selected_bytes().to_le_bytes());
        }
    }
    hasher.raw(&target.identity_bytes());
    hasher.byte(match policy {
        SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 => 0,
        SelectedFunctionLayoutPolicy::SingleEntryBlockV1 => 1,
        SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1 => 3,
        SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1 => 4,
    });
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
            for instruction in &block.instructions {
                hasher.raw(&instruction.instruction.0.to_le_bytes());
                selected_instructions::encode_machine_alternative_key_identity(
                    &mut hasher,
                    instruction.alternative,
                );
                hasher.raw(&instruction.offset.to_le_bytes());
                hasher.u64(instruction.bytes.len() as u64);
                hasher.raw(&instruction.bytes);
                match instruction.branch.as_deref() {
                    None => hasher.byte(0),
                    Some(ResolvedBranchEvidence::Conditional(branch)) => {
                        encode_conditional_branch_arm(&mut hasher, branch);
                        encode_views(&mut hasher, &branch.decoded_register_reads);
                        selected_instructions::encode_machine_encoded_effects_identity(
                            &mut hasher,
                            &branch.decoded_effects,
                        );
                    }
                    Some(ResolvedBranchEvidence::Jump(jump)) => {
                        encode_jump_arm(&mut hasher, jump);
                        selected_instructions::encode_machine_encoded_effects_identity(
                            &mut hasher,
                            &jump.decoded_effects,
                        );
                    }
                }
                encode_internal_fixup(&mut hasher, instruction.internal_machine_fixup());
                if let Some(fixup) = instruction.image_address_fixup() {
                    let mut identity = b"image-address-v2".to_vec();
                    fixup.reference.encode_identity(&mut identity);
                    fixup.fields.encode_identity(&mut identity);
                    hasher.raw(&identity);
                }
            }
        }
    }
    ResolvedSelectedFormLayoutIdentity(hasher.finish())
}

fn encode_internal_fixup(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    fixup: Option<SelectedFormInternalMachineFixup>,
) {
    let Some(fixup) = fixup else {
        hasher.byte(0);
        return;
    };
    hasher.byte(1);
    hasher.byte(match fixup.kind {
        SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => 0,
        SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => 1,
    });
    hasher.byte(match fixup.state {
        SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1 => 0,
    });
    hasher.raw(&fixup.callee.get().to_le_bytes());
    hasher.raw(&fixup.opcode_row_offset.to_le_bytes());
    hasher.raw(&fixup.patch_row_offset.to_le_bytes());
    hasher.raw(&fixup.reference_row_offset.to_le_bytes());
    hasher.byte(fixup.patch_byte_width);
    hasher.raw(&fixup.addend.to_le_bytes());
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

        use crate::identity_encoding::{encode_conditional_branch_arm, encode_jump_arm};
        use crate::{ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFunctionLayout};

        use super::{
            X86BranchRelaxationAction, X86BranchRelaxationAttempt,
            X86BranchRelaxationAttemptOutcome, X86BranchRelaxationIdentity,
            X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
        };

        const RELAXATION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation.v10";
        const REVISION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation-revision.v3";

        #[derive(Clone, Copy)]
        pub struct RevisionRoots {
            pub source: ResolvedSelectedFormLayoutIdentity,
            pub selected: selected_instructions::SelectedInstructionPlanIdentity,
            pub machine: physical_instructions::PostAllocationMachineIdentity,
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
            output: ResolvedSelectedFormLayoutIdentity,
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
            hasher.raw(&output.bytes());
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
            hasher.raw(&roots.source.bytes());
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
