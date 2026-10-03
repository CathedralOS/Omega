//! Structural control plans: cleanup edges, unit control machines, ranked
//! strongly connected components and control transfers.

use crate::checked_trees::flow::terminal::{
    CheckedOperandSequence, CheckedStructuralScalarParameterPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan,
};
use crate::checked_trees::values::CheckedProofTerm;
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
    /// The owner an authored receiver retains as attachment storage; `None`
    /// for a receiver-free machine, qualified or not.
    pub attachment_type_identity: Option<String>,
    pub states: Vec<CheckedStructuralUnitControlStatePlan>,
    /// The first retained cyclic-control proof. `None` preserves the acyclic
    /// structural-Unit slice; a cyclic plan is published only when the
    /// termination checker supplied this exact source-handle-free component.
    pub ranked_scc: Option<CheckedStructuralRankedSccPlan>,
}

/// One canonical Nat-descending component. Bounds are the exact unsigned
/// carrier bounds, not authored text. Each retained edge names the checked
/// transition coordinate and either its direct countdown proof or the
/// runtime component judgment's strict/nonincreasing comparison.
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
    /// The runtime component judgment proved this edge under its exact
    /// authored premises, not necessarily a syntactic positive guard.
    SourceRankingJudgment,
    UnsignedParameterPositive {
        scalar_parameter_index: u32,
        primitive_type: PrimitiveType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralRankedArgumentPlan {
    /// Exact natural rank lanes compared by the runtime component judgment.
    /// `strict = false` means nonincrease, not an assertion of equality.
    UnsignedParameterComparison {
        argument_ordinal: u32,
        source_scalar_parameter_index: u32,
        target_scalar_parameter_index: u32,
        primitive_type: PrimitiveType,
        strict: bool,
    },
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
    /// Erased proof-only actuals in the target's erased-proof roster order.
    /// They carry semantic identity, never a runtime operand.
    pub erased_proof_arguments: Vec<CheckedProofTerm>,
    pub trivial_affine_discard_parameter_positions: Vec<u32>,
}

impl CheckedStructuralControlSuccessorPlan {
    /// The calls this edge runs to establish its fresh composite arguments,
    /// in catalog order. They execute after the edge's selection, so their
    /// effects and custody belong to this edge alone.
    pub fn operand_calls(&self) -> impl Iterator<Item = &crate::CheckedUnitEffectOperationPlan> {
        self.transfers
            .iter()
            .filter_map(|transfer| match &transfer.source {
                CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                    operands,
                    establishment,
                } => Some((operands, establishment.as_ref())),
                _ => None,
            })
            .flat_map(|(operands, establishment)| {
                operands.iter().chain(std::iter::once(establishment))
            })
            .flat_map(crate::CheckedUnitEffectOperationPlan::with_value_calls)
            .filter(|operation| operation.call().is_some())
    }

    /// How many of this edge's operand calls take the source state's result
    /// `binding_ordinal` whole and owned.
    pub fn operand_result_moves(&self, binding_ordinal: u32) -> usize {
        self.operand_calls()
            .flat_map(crate::CheckedUnitEffectOperationPlan::call_structural_arguments)
            .filter(|argument| {
                argument.source_structural_result_binding_ordinal() == Some(binding_ordinal)
                    && argument.access == crate::CheckedStructuralAccess::Owned
                    && argument.path.is_empty()
            })
            .count()
    }

    /// How many times this edge moves the source state's `result` out of
    /// its home: a direct `StructuralResult` transfer to the target, or,
    /// for a result carrying ownership debt, a whole owned operand of one of
    /// the edge's operand calls. A copyable result's operand reads copy it
    /// and leave its home intact. Edge cleanup subtracts exactly these moves:
    /// an edge that moves a local must not also dispose of it, and a sibling
    /// edge that does not move it still must.
    pub fn result_moves(&self, result: &crate::CheckedUnitStructuralResultBindingPlan) -> usize {
        let transfers = self
            .transfers
            .iter()
            .filter(|transfer| {
                matches!(transfer.source,
                    CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal }
                        if binding_ordinal == result.binding_ordinal)
            })
            .count();
        if result.multiplicity == language_semantics::Multiplicity::Unrestricted {
            return transfers;
        }
        transfers + self.operand_result_moves(result.binding_ordinal)
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralControlTransferSourcePlan {
    /// Exact earlier state-local call result; rejoined to its producing operation.
    StructuralResult {
        binding_ordinal: u32,
    },
    Parameter {
        index: u32,
    },
    /// One `[copy]` payload subtree out of the case this edge's guard
    /// selected. `subject` is the case-test operand resolved to its
    /// structural argument plan (a retained parameter or earlier result
    /// plus the projection reaching the tested sum); `case_identity` and
    /// `field_identity` name the selected case and its declared payload
    /// field; `path` walks inside the payload field's own subtree (empty
    /// when the whole field transfers). Proven case membership on the
    /// edge authorizes the projection; reading a copyable subtree leaves
    /// the borrowed subject's custody intact.
    CasePayload {
        subject: CheckedUnitStructuralArgumentPlan,
        case_identity: String,
        field_identity: String,
        path: Vec<CheckedUnitStructuralPathSegment>,
    },
    /// Exclusive borrowed-byte range at the enclosing transition, rooted at a
    /// whole byte-view parameter (dense structural index) or an earlier
    /// byte-view local. Endpoint facts use the `TransitionArgument` subslice
    /// site at its authored target argument position.
    ByteSequenceSubslice {
        root: crate::CheckedStorageRoot,
        expression: typed_trees::expression::ExpressionHandle,
        /// Exact Terminal type identity of the collection the range reads —
        /// the same backing identity the argument plan's
        /// `ByteSequenceSubslice` carries for the range at its site.
        storage_type_identity: String,
    },
    /// Exclusive borrowed `&[T]` element range, T != u8, rooted and keyed
    /// exactly as a byte range is; the extent counts elements. `path` is the
    /// record-field projection below `root` to the fixed array the range
    /// views whole before narrowing — empty when `root` itself is an
    /// established element view, mirroring the `path` contract a
    /// `CheckedUnitStructuralArgumentPlan` carries for the same range at a
    /// call site or a view-local binding.
    ElementViewSubslice {
        root: crate::CheckedStorageRoot,
        path: Vec<CheckedUnitStructuralPathSegment>,
        expression: typed_trees::expression::ExpressionHandle,
        /// Exact Terminal type identity of the collection the range reads.
        storage_type_identity: String,
    },
    /// Authored byte-sequence literal supplied directly to a borrowed
    /// `&[u8]`-shaped parameter. The literal has no custody root: the edge
    /// establishes the octets into immutable storage the target parameter
    /// reads, with the parameter's declared domains as its evidence.
    ByteSequenceLiteral {
        bytes: Vec<u8>,
    },
    /// The machine's own receiver, claimed by a target state that declares
    /// `self` from a source state that did not. A self-less source never
    /// surrendered the receiver: it rides the machine ambiently and keeps
    /// its entry place, so the edge forwards the machine's receiver
    /// descriptor rather than a source parameter.
    MachineReceiver,
    /// A copyable leaf projected out of a source parameter — `param.field`.
    /// The target owns its storage, so the edge mints one
    /// `StructuralLeafCopy` operation reading the projected subtree into a
    /// fresh place: copying is an observation the source survives intact,
    /// admitted only for leaves whose declaration carries `Unrestricted`
    /// multiplicity. `subject` describes the rooted place end to end —
    /// source parameter, the field/index projection below it, and the
    /// leaf's normalized type identity; its access is `Owned`.
    ProjectedCopy {
        subject: CheckedUnitStructuralArgumentPlan,
    },
    /// A fresh composite argument with no prior custody root: an inline
    /// record, case, or fixed-array literal, or a call's structural result,
    /// supplied directly to the successor. The edge runs `operands` and
    /// then `establishment` after selection, at the transition's statement
    /// coordinate. A whole call retains its ordinary call operation; an
    /// aggregate retains its structural value evaluator. Both publish the
    /// target parameter's type. Binding ordinals are edge-local, numbered
    /// after the source state's own results.
    EstablishedValue {
        operands: CheckedOperandSequence,
        establishment: Box<crate::CheckedUnitEffectOperationPlan>,
    },
    /// An authored `&place` or `&mut place` argument loans the named subtree
    /// into the borrowed target parameter: the successor argument keeps the
    /// source's place and walks `subject.path` beneath it, so the loan names
    /// the same storage the borrow does rather than a fresh establishment.
    /// `subject` describes the rooted place end to end — source root, the
    /// projection segments below it, the subtree's normalized type identity,
    /// and the loan's own access. `carrier_identity` is the `&'a V`
    /// reference-type identity the `EstablishReference` result declares when
    /// the path descends below the root; a whole-place loan forwards the
    /// rooted place directly and leaves it empty.
    ProjectedLoan {
        subject: CheckedUnitStructuralArgumentPlan,
        carrier_identity: String,
    },
}
