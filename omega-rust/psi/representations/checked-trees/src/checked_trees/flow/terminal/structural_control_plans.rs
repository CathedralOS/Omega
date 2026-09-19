//! Structural control plans: cleanup edges, unit control machines, ranked
//! strongly connected components and control transfers.

use crate::checked_trees::flow::terminal::{
    CheckedStructuralScalarParameterPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan,
};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

/// Source-handle-free no-code cleanup evidence for ordinary structural
/// control edges. This is intentionally narrower than the language's complete
/// `EdgeCleanupPlan`: executable rows name only whole, claim-free affine
/// parameters whose checked state-exit events can be realized as terminal-Psi
/// trivial discards. The separate projected row remains checked-only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralControlCleanupPlans {
    pub states: Vec<CheckedStructuralControlStateCleanupPlan>,
    /// Checked-only cleanup rows for the first direct-record projected jump
    /// cohort. These are deliberately kept out of `states`: every existing
    /// Terminal consumer uses `for_edge`, so a path-sensitive row cannot be
    /// mistaken for the older whole-root executable vocabulary.
    pub projected_edges: Vec<CheckedStructuralControlProjectedEdgeCleanupPlan>,
}

impl CheckedStructuralControlCleanupPlans {
    pub fn for_state(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<&CheckedStructuralControlStateCleanupPlan> {
        self.states
            .iter()
            .find(|plan| plan.machine == machine && plan.state == state)
    }

    pub fn for_edge(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<&CheckedStructuralControlEdgeCleanupPlan> {
        if self
            .for_projected_edge(machine, state, statement_ordinal)
            .is_some()
        {
            return None;
        }
        self.for_state(machine, state)?
            .edges
            .iter()
            .find(|edge| edge.statement_ordinal == statement_ordinal)
    }

    pub fn for_projected_edge(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<&CheckedStructuralControlProjectedEdgeCleanupPlan> {
        self.projected_edges.iter().find(|edge| {
            edge.machine == machine
                && edge.state == state
                && edge.statement_ordinal == statement_ordinal
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralControlStateCleanupPlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    /// One row per supported ordinary named transition, in source statement
    /// order. Conditional arms therefore retain their exact arm coordinate.
    pub edges: Vec<CheckedStructuralControlEdgeCleanupPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralControlEdgeCleanupPlan {
    pub statement_ordinal: u32,
    pub target_state: SymbolHandle,
    /// Source-state parameter positions in reverse declaration order. A later
    /// terminal producer resolves these positions against its independently
    /// checked structural signature before assigning terminal `PlaceId`s.
    pub trivial_affine_discard_parameter_positions: Vec<u32>,
}

/// One checked-only path-sensitive cleanup row for an ordinary state jump.
/// The first cohort has exactly one source root, one whole direct-field move,
/// and one maximal sibling residual; Terminal control has no corresponding
/// path vocabulary yet and must continue to reject it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralControlProjectedEdgeCleanupPlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub target_state: SymbolHandle,
    pub transfer: CheckedStructuralControlProjectedTransferPlan,
    pub residual_affine_discards: Vec<CheckedUnitPartialAffineDiscardPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralControlProjectedTransferPlan {
    /// Authored source-state parameter position. The bounded first cohort
    /// admits exactly position zero, but retains the coordinate explicitly.
    pub source_parameter_position: u32,
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: String,
    pub target_parameter_position: u32,
}

/// Complete checked input for the first terminal structural-control producer.
/// This deliberately supports only claim-free affine, Unit-returning attached
/// graphs whose states return naturally, unconditionally transfer whole
/// parameters, or have at most two states select independent whole-parameter
/// successors from one retained Boolean scalar input. One two-predecessor join
/// may reconverge identical structural frontiers. Ordinary successor edges may
/// also forward direct primitive scalar inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralUnitControlPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub machines: Vec<CheckedStructuralUnitControlMachinePlan>,
}

impl CheckedStructuralUnitControlPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedStructuralUnitControlMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralUnitControlMachinePlan {
    pub machine: SymbolHandle,
    pub attachment_type_identity: String,
    pub states: Vec<CheckedStructuralUnitControlStatePlan>,
    /// The first retained cyclic-control proof. `None` preserves the acyclic
    /// structural-Unit slice; a cyclic plan is published only when the
    /// termination checker supplied this exact source-handle-free component.
    pub ranked_scc: Option<CheckedStructuralRankedSccPlan>,
}

/// One canonical Nat-descending component admitted by the first cyclic
/// structural-Unit slice. Bounds are the exact unsigned carrier bounds, not
/// authored text, and every retained edge names the checked transition
/// coordinate whose positive guard and decrement the ranking checker proved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralRankedSccPlan {
    pub header_state: SymbolHandle,
    pub rank_scalar_parameter_index: u32,
    pub rank_primitive_type: PrimitiveType,
    pub rank_lower_bound: u128,
    pub rank_upper_bound: u128,
    pub covered_cyclic_edges: Vec<CheckedStructuralRankedSccEdgePlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedStructuralRankedSccEdgePlan {
    pub source_state: SymbolHandle,
    pub target_state: SymbolHandle,
    pub statement_ordinal: u32,
    pub guard: CheckedStructuralRankedGuardPlan,
    pub successor_argument: CheckedStructuralRankedArgumentPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralRankedGuardPlan {
    UnsignedParameterPositive {
        scalar_parameter_index: u32,
        primitive_type: PrimitiveType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralRankedArgumentPlan {
    UnsignedParameterMinusOne {
        argument_ordinal: u32,
        source_scalar_parameter_index: u32,
        target_scalar_parameter_index: u32,
        primitive_type: PrimitiveType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralUnitControlStatePlan {
    pub state: SymbolHandle,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub terminator: CheckedStructuralUnitControlTerminatorPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralUnitControlTerminatorPlan {
    ReturnUnit {
        trivial_affine_discard_parameter_positions: Vec<u32>,
    },
    Jump {
        statement_ordinal: u32,
        target_state: SymbolHandle,
        transfers: Vec<CheckedStructuralControlTransferPlan>,
        scalar_arguments: Vec<CheckedStructuralScalarArgumentPlan>,
        trivial_affine_discard_parameter_positions: Vec<u32>,
    },
    Conditional {
        guard_scalar_parameter_index: u32,
        when_true: CheckedStructuralControlSuccessorPlan,
        when_false: CheckedStructuralControlSuccessorPlan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralControlSuccessorPlan {
    pub statement_ordinal: u32,
    pub target_state: SymbolHandle,
    pub transfers: Vec<CheckedStructuralControlTransferPlan>,
    pub scalar_arguments: Vec<CheckedStructuralScalarArgumentPlan>,
    /// Proof-only erased actuals pairing the target's erased formals with the
    /// retained scalar expressions recorded at the transition.
    pub erased_arguments: Vec<CheckedStructuralScalarArgumentPlan>,
    pub trivial_affine_discard_parameter_positions: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedStructuralScalarArgumentPlan {
    /// Authored target-argument position retained as the checked expression
    /// coordinate.
    pub argument_ordinal: u32,
    pub source: CheckedStructuralScalarArgumentSourcePlan,
    pub target_scalar_parameter_index: u32,
    pub primitive_type: PrimitiveType,
}

impl Default for CheckedStructuralScalarArgumentPlan {
    fn default() -> Self {
        Self {
            argument_ordinal: 0,
            source: CheckedStructuralScalarArgumentSourcePlan::Expression,
            target_scalar_parameter_index: 0,
            primitive_type: PrimitiveType::Bool,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralScalarArgumentSourcePlan {
    /// Exact source scalar parameter, including its source-name binding checks.
    Parameter { index: u32 },
    /// Existing checked TransitionArgument expression at the enclosing edge's
    /// statement_ordinal and this argument's argument_ordinal.
    Expression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedStructuralControlTransferPlan {
    pub source: CheckedStructuralControlTransferSourcePlan,
    pub target_parameter_index: u32,
}

impl Default for CheckedStructuralControlTransferPlan {
    fn default() -> Self {
        Self {
            source: CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 },
            target_parameter_index: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralControlTransferSourcePlan {
    /// Exact earlier state-local call result; rejoined to its producing operation.
    StructuralResult {
        binding_ordinal: u32,
    },
    Parameter {
        index: u32,
    },
    /// Exclusive borrowed-byte range at the enclosing transition. Endpoint
    /// facts use its authored target argument position in the scalar plans.
    ByteSequenceSubslice {
        parameter_index: u32,
        expression: typed_trees::expression::ExpressionHandle,
    },
}
