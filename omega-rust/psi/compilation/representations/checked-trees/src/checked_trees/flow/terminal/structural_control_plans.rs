//! Structural control plans: cleanup edges and control transfers.

use crate::checked_trees::CheckedMemberIdentity;
use crate::checked_trees::flow::terminal::{
    CheckedOperandSequence, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralPathSegment,
};
use crate::checked_trees::values::CheckedProofTerm;
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

/// Source-handle-free no-code cleanup evidence for ordinary structural
/// control edges. This is intentionally narrower than the language's complete
/// `EdgeCleanupPlan`: executable rows name only whole, claim-free affine
/// parameters whose checked state-exit events can be realized as terminal-Psi
/// trivial discards. Composed graphs use exact normal-exit actions instead.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralControlCleanupPlans {
    pub states: Vec<CheckedStructuralControlStateCleanupPlan>,
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
        self.for_state(machine, state)?
            .edges
            .iter()
            .find(|edge| edge.statement_ordinal == statement_ordinal)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralControlSuccessorPlan {
    pub statement_ordinal: u32,
    pub target_state: SymbolHandle,
    /// Whether the transition selects the state's authored continuation
    /// rather than its ordinary target. Scalar terminators and the composed
    /// route share this one plan; a scalar edge carries only this identity
    /// and leaves the rosters empty.
    pub is_continuation: bool,
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
    /// The one construction site for successor plans: the composed route
    /// mints full transfer rosters; scalar terminators mint the same plan
    /// with continuation identity and empty rosters.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        statement_ordinal: u32,
        target_state: SymbolHandle,
        is_continuation: bool,
        transfers: Vec<CheckedStructuralControlTransferPlan>,
        scalar_arguments: Vec<CheckedStructuralScalarArgumentPlan>,
        erased_arguments: Vec<CheckedStructuralScalarArgumentPlan>,
        erased_proof_arguments: Vec<CheckedProofTerm>,
        trivial_affine_discard_parameter_positions: Vec<u32>,
    ) -> Self {
        CheckedStructuralControlSuccessorPlan {
            statement_ordinal,
            target_state,
            is_continuation,
            transfers,
            scalar_arguments,
            erased_arguments,
            erased_proof_arguments,
            trivial_affine_discard_parameter_positions,
        }
    }

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
                    ..
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
        case_identity: CheckedMemberIdentity,
        field_identity: CheckedMemberIdentity,
        path: Vec<CheckedUnitStructuralPathSegment>,
    },
    /// A byte or element view range narrowed on the selected edge: the same
    /// `ByteSequenceSubslice`/`ElementViewSubslice` argument plan a call
    /// argument or view-local binding carries, with its endpoints keyed at
    /// the edge's `TransitionArgument` subslice site.
    ViewSubslice(CheckedUnitStructuralArgumentPlan),
    /// Authored byte-sequence literal supplied directly to a borrowed
    /// `&[u8]`-shaped parameter. The literal has no custody root: the edge
    /// establishes the octets into immutable storage the target parameter
    /// reads, with the parameter's declared domains as its evidence.
    ByteSequenceLiteral {
        bytes: Vec<u8>,
    },
    /// A subtree of an established parameter or result. Unrestricted leaves
    /// copy into fresh storage; affine/linear leaves transfer the exact path
    /// from its owner, with the remaining custody settled by the normal-edge
    /// cleanup plan. The subject retains root, path, type and access together.
    ProjectedValue {
        subject: CheckedUnitStructuralArgumentPlan,
    },
    /// A fresh composite argument with no prior custody root: an inline
    /// record, case, or fixed-array literal, or a call's structural result,
    /// supplied directly to the successor. The edge runs `operands` and
    /// then `establishment` after selection, at the transition's statement
    /// coordinate. A whole call retains its ordinary call operation; an
    /// aggregate retains its structural value evaluator. `presentation` names
    /// the receiving endpoint independently of that producer's home: a stored
    /// reference lends through its Referent crossing, not its descriptor bytes.
    /// Binding ordinals are edge-local, numbered after the source state's results.
    EstablishedValue {
        operands: CheckedOperandSequence,
        establishment: Box<crate::CheckedUnitEffectOperationPlan>,
        presentation: CheckedUnitStructuralArgumentPlan,
    },
    /// A borrowed-place argument loans the named subtree
    /// into the borrowed target parameter: the successor argument keeps the
    /// source's place and walks `subject.path` beneath it, so the loan names
    /// the same storage the borrow does rather than a fresh establishment.
    /// `subject` describes the rooted place end to end — source root, the
    /// projection segments below it, the subtree's normalized type identity,
    /// and the loan's own access. A runtime-indexed storage projection forms an
    /// operation-owned address before later arguments execute; its affine carrier travels
    /// beside the borrowed referent and retains any parent carrier lifetime.
    ProjectedLoan {
        subject: CheckedUnitStructuralArgumentPlan,
        /// The authored borrow and its descriptor type when a storage place
        /// requires a retained address. This prepared type permits formation when
        /// the target chain needs a runtime capture; static geometry can retain
        /// its canonical rooted path. A stored reference already owns
        /// its capture; its zero identity keeps that existing resource.
        formation_expression: typed_trees::expression::ExpressionHandle,
        carrier_type_identity: typed_trees::type_identity::TypeIdentityId,
    },
}
