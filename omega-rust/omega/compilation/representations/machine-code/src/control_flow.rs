/*
Native branch topology and stack-replay control-flow evidence (`ScalarControlFlowEvidence`).

Object and artifact replay must verify that every native branch path preserves stack depth,
honors x86-64 signed division overflow diamonds (`LinearWithDivisionBranches`), and converges
at the exact return or cleanup tail (`BooleanSharedConvergence`). We retain the ordered branch
intervals, boolean decision trees, and structural-field condition reads (`BooleanStructuralConditionEvidence`)
needed to replay control flow directly from emitted bytes.
*/

use semantic_vocabulary::{EdgeId, OperationId, PlaceId, StructuralFieldId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarControlFlowEvidence {
    Linear,
    Acyclic {
        blocks: Vec<ScalarControlBlockEvidence>,
    },
    LinearWithDivisionBranches {
        branches: Vec<ScalarDivisionBranchEvidence>,
    },
    ConditionalTree {
        decisions: Vec<ScalarConditionalBranchEvidence>,
        crash_leaves: Vec<bool>,
        branches: Vec<ScalarDivisionBranchEvidence>,
    },
    BooleanSharedConvergence {
        decisions: Vec<ScalarConditionalBranchEvidence>,
        joins: Vec<ScalarJoinBranchEvidence>,
        return_edges: Vec<EdgeId>, // Source return edges in physical true-before-false leaf order.
        fallthrough_return_edge: EdgeId, // Source return edge of the final physical leaf, which falls through.
        structural_conditions: Vec<BooleanStructuralConditionEvidence>, // Emitted condition regions containing structural-field reads.
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
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarJoinBranchEvidence {
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
