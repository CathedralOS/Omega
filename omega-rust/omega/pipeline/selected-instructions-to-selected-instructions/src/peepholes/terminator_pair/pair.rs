//! Declarative terminator-pair descriptors: producer, consumer, and the
//! rewritten form plus the admissibility axes each must satisfy.
//!
//! The [`literal_fold`](crate::rewrites::selected_lowering) pair grammar
//! describes instruction-to-instruction pairs — an ordinary consumer operand
//! carries the materialized victim, so its descriptor vocabulary has no way
//! to name the consumer every block actually ends on: the terminator-carried
//! instruction, whose traffic flows through implicit physical units rather
//! than an explicit operand. This module extends the same declaration
//! discipline to that consumer position. A [`TerminatorPairRule`] declares
//! one (producer kind, consumer kind, rewritten kind) triple and the axes
//! the relationship must satisfy; the producer consults its own descriptor
//! when admitting a candidate, while the independent replay re-derives the
//! whole grammar — consumer variant, flag-unit flow, operand resolution,
//! and decided edge — from the instruction records alone and never reads a
//! rule.
//!
//! The landed family is the decided condition-state branch: a conditional
//! branch terminator whose flag reads all reach one condition-state producer
//! whose operands are compile-time constant becomes the `Jump` carrying the
//! decided successor. The rewrite carries control flow the instruction-pair
//! grammar never could — the consumer's alternatives encode
//! `ConditionalRelativeBranchV1` and the rewritten row encodes
//! `UnconditionalRelativeBranchV1` — and it exercises unit roles beyond the
//! retired implicit definitions and operand-swapped preservation the
//! instruction-pair descriptors already declare: the consumer's flag uses
//! are *retired* (the compile-time decision replaces the observation), the
//! non-flag control-unit use is *preserved* onto the rewritten row, and the
//! consumer's implicit definitions and clobbers are *republished* verbatim.
//! The producer stays: its flag definitions remain published for every
//! other reader the function still holds.

use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior, SelectedInstructionKind, SelectedTerminator,
};

use crate::machine_semantic_kind;
use crate::peepholes::condition_flow::ConditionOperandResolution;

/// The implicit physical-unit relationship the rewrite asserts between the
/// producer's definitions and the consumer's uses — the unit roles the
/// instruction-pair `PairUnitEffects` vocabulary cannot name because its
/// consumers carry the victim in an explicit operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminatorPairUnitFlow {
    /// The producer's implicit definitions feed the consumer's
    /// condition-state uses. Every flag-universe unit — the units any of the
    /// environment's condition-state producer rows defines — that the
    /// consumer implicitly uses must resolve to the producer as its single
    /// last event on every path reaching the terminator, and the producer's
    /// record must define it. Those uses retire: the compile-time decision
    /// replaces the observation, so the rewritten row reads no flag unit.
    ///
    /// Every non-flag unit the consumer uses must stay read on the
    /// rewritten row — the control unit carrying the relative displacement
    /// is a use the rewrite preserves, not one it may drop — and the
    /// rewritten row must republish the consumer's implicit definitions and
    /// clobbers verbatim: the program counter the branch defined is still
    /// defined by the jump.
    ConditionStateResolved,
}

/// The control-flow relationship the rewrite carries between the consumer's
/// and the rewritten form's effect declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminatorPairControlFlow {
    /// Every consumer alternative encodes `ConditionalRelativeBranchV1` and
    /// every rewritten alternative encodes `UnconditionalRelativeBranchV1`;
    /// both declarations carry the control-flow barrier and are otherwise
    /// effect-isolated — no declared or encoded memory, stack, or trap
    /// traffic beyond the architectural fault a relative-branch encoding
    /// can raise, and no call or cleanup surface. A target whose branch or
    /// jump row encodes anything else cannot admit the pair.
    ConditionalResolvedToUnconditional,
}

/// One declarative terminator-pair rule: the condition-state producer kind
/// the consumer's flag uses resolve to, the conditional-branch consumer
/// kind, the rewritten form, and the axes the relationship must satisfy.
///
/// The descriptor declares admissibility; it never certifies it. The
/// producer consults its own rule through [`TerminatorPairRule::for_pair`]
/// and the axes' `admits_*` predicates; the replay in `validate` re-derives
/// the grammar from the instruction records without reading this table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminatorPairRule {
    producer: MachineSemanticKind,
    consumer: MachineSemanticKind,
    rewritten: MachineSemanticKind,
    operand_resolution: ConditionOperandResolution,
    unit_flow: TerminatorPairUnitFlow,
    control: TerminatorPairControlFlow,
}

impl TerminatorPairRule {
    /// Shared declaration of every decided-branch pair: the rewrite is the
    /// `Jump` row, the flag uses retire while the control unit and the
    /// definitions are preserved, and the encoded control effect resolves
    /// conditional to unconditional.
    const DECIDED_BRANCH: Self = Self {
        producer: MachineSemanticKind::CompareI64,
        consumer: MachineSemanticKind::ConditionalBranchNonZero,
        rewritten: MachineSemanticKind::Jump,
        operand_resolution: ConditionOperandResolution::TwoRegisterOperands,
        unit_flow: TerminatorPairUnitFlow::ConditionStateResolved,
        control: TerminatorPairControlFlow::ConditionalResolvedToUnconditional,
    };

    /// `compare left, right` deciding a nonzero/zero branch.
    pub const COMPARE_BRANCH_NONZERO: Self = Self::DECIDED_BRANCH;
    /// `compare left, right` deciding an unsigned-less-than branch.
    pub const COMPARE_BRANCH_U64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::ConditionalBranchU64LessThan,
        ..Self::DECIDED_BRANCH
    };
    /// `compare left, right` deciding a signed-less-than branch.
    pub const COMPARE_BRANCH_I64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::ConditionalBranchI64LessThan,
        ..Self::DECIDED_BRANCH
    };
    /// `compare left, immediate` deciding a nonzero/zero branch.
    pub const COMPARE_IMMEDIATE_BRANCH_NONZERO: Self = Self {
        producer: MachineSemanticKind::CompareI64Immediate,
        operand_resolution: ConditionOperandResolution::RegisterAndKindImmediate,
        ..Self::DECIDED_BRANCH
    };
    /// `compare left, immediate` deciding an unsigned-less-than branch.
    pub const COMPARE_IMMEDIATE_BRANCH_U64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::ConditionalBranchU64LessThan,
        ..Self::COMPARE_IMMEDIATE_BRANCH_NONZERO
    };
    /// `compare left, immediate` deciding a signed-less-than branch.
    pub const COMPARE_IMMEDIATE_BRANCH_I64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::ConditionalBranchI64LessThan,
        ..Self::COMPARE_IMMEDIATE_BRANCH_NONZERO
    };
    /// `compare left, 0` deciding a nonzero/zero branch.
    pub const COMPARE_ZERO_BRANCH_NONZERO: Self = Self {
        producer: MachineSemanticKind::CompareI64Zero,
        operand_resolution: ConditionOperandResolution::RegisterAndZeroBound,
        ..Self::DECIDED_BRANCH
    };
    /// `compare left, 0` deciding an unsigned-less-than branch.
    pub const COMPARE_ZERO_BRANCH_U64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::ConditionalBranchU64LessThan,
        ..Self::COMPARE_ZERO_BRANCH_NONZERO
    };
    /// `compare left, 0` deciding a signed-less-than branch.
    pub const COMPARE_ZERO_BRANCH_I64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::ConditionalBranchI64LessThan,
        ..Self::COMPARE_ZERO_BRANCH_NONZERO
    };

    pub const fn producer(self) -> MachineSemanticKind {
        self.producer
    }
    pub const fn consumer(self) -> MachineSemanticKind {
        self.consumer
    }
    pub const fn rewritten(self) -> MachineSemanticKind {
        self.rewritten
    }
    pub const fn operand_resolution(self) -> ConditionOperandResolution {
        self.operand_resolution
    }
    pub const fn unit_flow(self) -> TerminatorPairUnitFlow {
        self.unit_flow
    }
    pub const fn control(self) -> TerminatorPairControlFlow {
        self.control
    }

    /// Whether `kind` is the consumer this rule declares.
    pub(crate) fn matches_consumer(&self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.consumer
    }

    /// Whether `kind` is the producer this rule declares.
    pub(crate) fn matches_producer(&self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.producer
    }

    /// The declared consumer kind pairs with exactly one terminator
    /// variant; a record pairing the kind with a different variant is not
    /// the form the rule describes and cannot decide an arm.
    pub(crate) fn matches_terminator(&self, terminator: &SelectedTerminator) -> bool {
        matches!(
            (self.consumer, terminator),
            (
                MachineSemanticKind::ConditionalBranchNonZero,
                SelectedTerminator::ConditionalBranch { .. }
            ) | (
                MachineSemanticKind::ConditionalBranchU64LessThan,
                SelectedTerminator::ConditionalBranchU64LessThan { .. }
            ) | (
                MachineSemanticKind::ConditionalBranchI64LessThan,
                SelectedTerminator::ConditionalBranchI64LessThan { .. }
            )
        )
    }

    /// The successor the consumer selects for the decided `(left, right)`
    /// state, or `None` when the terminator's variant is not the one the
    /// declared consumer kind pairs with.
    pub(crate) fn decided_successor<'terminator>(
        &self,
        terminator: &'terminator SelectedTerminator,
        left: u64,
        right: u64,
    ) -> Option<&'terminator selected_instructions::SelectedSuccessor> {
        match (self.consumer, terminator) {
            (
                MachineSemanticKind::ConditionalBranchNonZero,
                SelectedTerminator::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                    ..
                },
            ) => Some(if left != right {
                when_nonzero
            } else {
                when_zero
            }),
            (
                MachineSemanticKind::ConditionalBranchU64LessThan,
                SelectedTerminator::ConditionalBranchU64LessThan {
                    when_less,
                    when_not_less,
                    ..
                },
            ) => Some(if left < right {
                when_less
            } else {
                when_not_less
            }),
            (
                MachineSemanticKind::ConditionalBranchI64LessThan,
                SelectedTerminator::ConditionalBranchI64LessThan {
                    when_less,
                    when_not_less,
                    ..
                },
            ) => Some(if (left as i64) < (right as i64) {
                when_less
            } else {
                when_not_less
            }),
            _ => None,
        }
    }

    /// The implicit-unit declaration surface the rule admits between the
    /// consumer's record and the rewritten row: under
    /// `ConditionStateResolved` every consumer implicit use outside the flag
    /// universe must stay read on the rewritten row, and the rewritten row
    /// must republish the consumer's implicit definitions and clobbers
    /// verbatim — the flag uses retire while the control-unit use and the
    /// definitions are preserved.
    pub(crate) fn admits_unit_flow(
        &self,
        flag_uses: &[register_model::RegisterUnitId],
        plain_uses: &[register_model::RegisterUnitId],
        consumer: &selected_instructions::SelectedInstruction,
        rewritten: &RegisterInstructionConstraint,
    ) -> bool {
        match self.unit_flow() {
            TerminatorPairUnitFlow::ConditionStateResolved => {
                crate::peepholes::condition_flow::resolved_unit_surface(
                    flag_uses, plain_uses, consumer, rewritten,
                )
            }
        }
    }

    /// The effect-declaration surface the rule admits between the
    /// consumer's and the rewritten form's catalog declarations: under
    /// `ConditionalResolvedToUnconditional` both declarations are isolated
    /// outside the encoded control effect, the consumer encodes a
    /// conditional relative branch on every alternative, and the rewritten
    /// form encodes an unconditional one.
    pub(crate) fn admits_declarations(
        &self,
        consumer: &MachineEffectDeclaration,
        rewritten: &MachineEffectDeclaration,
    ) -> bool {
        match self.control() {
            TerminatorPairControlFlow::ConditionalResolvedToUnconditional => {
                isolated_declaration(consumer)
                    && isolated_declaration(rewritten)
                    && consumer
                        .alternatives
                        .iter()
                        .all(|alternative| branch_alternative(alternative, true))
                    && rewritten
                        .alternatives
                        .iter()
                        .all(|alternative| branch_alternative(alternative, false))
            }
        }
    }
}

/// Every terminator pair the family declares: each condition-state producer
/// kind against each conditional-branch consumer kind.
pub(crate) const TERMINATOR_PAIR_RULES: &[TerminatorPairRule] = &[
    TerminatorPairRule::COMPARE_BRANCH_NONZERO,
    TerminatorPairRule::COMPARE_BRANCH_U64_LESS_THAN,
    TerminatorPairRule::COMPARE_BRANCH_I64_LESS_THAN,
    TerminatorPairRule::COMPARE_IMMEDIATE_BRANCH_NONZERO,
    TerminatorPairRule::COMPARE_IMMEDIATE_BRANCH_U64_LESS_THAN,
    TerminatorPairRule::COMPARE_IMMEDIATE_BRANCH_I64_LESS_THAN,
    TerminatorPairRule::COMPARE_ZERO_BRANCH_NONZERO,
    TerminatorPairRule::COMPARE_ZERO_BRANCH_U64_LESS_THAN,
    TerminatorPairRule::COMPARE_ZERO_BRANCH_I64_LESS_THAN,
];

/// The declared pair for one (producer, consumer) kind combination, or none
/// when the family declares no such relationship — several rules matching
/// one combination is a catalog defect and refuses rather than preferring
/// table order.
pub(crate) fn terminator_pair_for(
    producer: MachineSemanticKind,
    consumer: MachineSemanticKind,
) -> Option<&'static TerminatorPairRule> {
    let mut matches = TERMINATOR_PAIR_RULES
        .iter()
        .filter(|rule| rule.producer() == producer && rule.consumer() == consumer);
    let rule = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(rule)
}

/// The consumer kinds the family declares at all — for admission error
/// reporting that distinguishes an unadmitted branch kind from an admitted
/// one whose flag flow or operands refuse.
pub(crate) fn declared_consumer(kind: SelectedInstructionKind) -> bool {
    TERMINATOR_PAIR_RULES
        .iter()
        .any(|rule| rule.matches_consumer(kind))
}

/// The declared producer kinds — the condition-state producers whose
/// implicit definitions the flag universe is computed from.
pub(crate) fn declared_producers() -> impl Iterator<Item = MachineSemanticKind> {
    [
        MachineSemanticKind::CompareI64,
        MachineSemanticKind::CompareI64Immediate,
        MachineSemanticKind::CompareI64Zero,
    ]
    .into_iter()
}

/// The declared surface a decided-branch form must carry: no memory access,
/// the operation itself never traps, no call or cleanup — and the
/// control-flow barrier both the conditional and the unconditional branch
/// semantics publish.
fn isolated_declaration(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::ControlFlow
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
}

/// The encoded surface a decided-branch form's alternatives must carry: no
/// memory access, unchanged stack, the architectural-fault surface a
/// relative-branch encoding can raise, and the control effect `conditional`
/// selects — `ConditionalRelativeBranchV1` on the consumer,
/// `UnconditionalRelativeBranchV1` on the rewrite.
fn branch_alternative(alternative: &MachineAlternative, conditional: bool) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        && encoded.control
            == if conditional {
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            } else {
                MachineEncodedControlEffect::UnconditionalRelativeBranchV1
            }
}
