//! Native branch topology and its source and stack-replay evidence.

use abstract_operations::RankedU32CountdownCustody;
use calling_conventions::CallPlan;
use semantic_vocabulary::{EdgeId, OperationId, PlaceId, StructuralFieldId};
use target_operations::TargetStructuralParameter;
use terminal_psi::{StructuralTypeDeclaration, TerminalAffineCleanupAction};

/// Complete machine-code custody for the one admitted structural Unit / `u32`
/// countdown. Target layout is deliberately not copied here: object replay
/// must derive it independently from the target's canonical encoding and bind
/// the generic fuel rows to that result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedU32CountdownMachineCodeRecord {
    pub custody: RankedU32CountdownCustody,
    pub call_plan: CallPlan,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub structural_parameters: Vec<TargetStructuralParameter>,
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarControlFlowEvidence {
    Linear,
    /// Complete forward control flow of a no-call scalar function. Blocks
    /// partition the emitted bytes; jumps retain real edges and reconvergence
    /// requires the same stack depth from every predecessor.
    Acyclic {
        blocks: Vec<ScalarControlBlockEvidence>,
    },
    /// A direct scalar return containing one or more compiler-generated x86-64
    /// signed division/remainder diamonds. Each branch selects the `-1`
    /// overflow handling arm or the ordinary DIV/IDIV arm, and both paths
    /// reconverge before expression evaluation continues. The object boundary
    /// validates both branch targets and replays the two stack paths
    /// independently.
    LinearWithDivisionBranches {
        branches: Vec<ScalarDivisionBranchEvidence>,
    },
    /// One acyclic Boolean decision tree whose branches are retained in
    /// increasing physical code order. Its terminal bitmap is in physical
    /// true-before-false DFS order and therefore contains exactly one more
    /// entry than `decisions`. Ordered x86 division diamonds are partitioned
    /// across decision prefixes and returning leaves during object replay.
    ConditionalTree {
        decisions: Vec<ScalarConditionalBranchEvidence>,
        crash_leaves: Vec<bool>,
        branches: Vec<ScalarDivisionBranchEvidence>,
    },
    /// One native Boolean decision tree whose ordered value leaves reconverge
    /// at an exact shared return/cleanup tail. Every non-final leaf ends in one
    /// retained unconditional branch targeting `merge_offset`; the final leaf
    /// falls through to that same offset.
    BooleanSharedConvergence {
        decisions: Vec<ScalarConditionalBranchEvidence>,
        joins: Vec<ScalarJoinBranchEvidence>,
        /// Exact source return edges in physical true-before-false leaf order.
        /// One source convergence block has one row even when several value
        /// leaves reach it; otherwise each uniform source return is retained.
        return_edges: Vec<EdgeId>,
        /// Source return edge of the final physical leaf, which falls through
        /// rather than owning a [`ScalarJoinBranchEvidence`] row.
        fallthrough_return_edge: EdgeId,
        /// Exact emitted condition regions containing structural-field reads.
        /// Object replay checks these bytes independently from the generic
        /// scalar instruction walk before accepting the shared tail.
        structural_conditions: Vec<BooleanStructuralConditionEvidence>,
        merge_offset: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarControlBlockEvidence {
    pub offset: usize,
    pub byte_count: usize,
    pub terminator: ScalarControlTerminatorEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarControlTerminatorEvidence {
    Return {
        offset: usize,
        byte_count: usize,
    },
    Jump {
        offset: usize,
        byte_count: usize,
        target_offset: usize,
    },
    Conditional(ScalarDirectConditionalBranchEvidence),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarDirectConditionalBranchEvidence {
    pub predicate: crate::FunctionFragmentConditionalBranchPredicate,
    pub branch_offset: usize,
    pub branch_byte_count: usize,
    pub taken_offset: usize,
    pub fallthrough_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanStructuralConditionEvidence {
    pub reads: Vec<BooleanStructuralFieldRead>,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BooleanStructuralFieldRead {
    pub psi_operation: OperationId,
    pub source: PlaceId,
    pub field: StructuralFieldId,
    pub field_byte_offset: u32,
    /// Exact native interval which loads this field and normalizes it to a
    /// Boolean result. Object replay reconstructs these bytes independently
    /// from the retained structural home and canonical layout.
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarJoinBranchEvidence {
    /// Exact source return edge whose value leaf owns this native join.
    pub return_edge: EdgeId,
    pub join_offset: usize,
    pub join_byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarDivisionBranchEvidence {
    pub branch_offset: usize,
    pub branch_byte_count: usize,
    pub ordinary_arm_offset: usize,
    pub join_offset: usize,
    pub join_byte_count: usize,
    pub merge_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarConditionalBranchEvidence {
    pub condition: ScalarConditionalCondition,
    pub branch_offset: usize,
    pub branch_byte_count: usize,
    pub false_arm_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarConditionalCondition {
    Parameter,
    Expression,
}
