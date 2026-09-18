//! Declarative condition-materialization descriptors: producer, consumer,
//! and the rewritten form plus the admissibility axes each must satisfy.
//!
//! The [`terminator_pair`](super::terminator_pair) grammar describes
//! consumers whose flag reads sit at a block's terminator; the
//! `MaterializeBoolean*` instruction reads the same condition state at a
//! body position and publishes it as a GPR. This module extends the same
//! declaration discipline to that consumer position. A
//! [`ConditionMaterializationRule`] declares one (producer kind, consumer
//! kind) pair — the rewritten form is always the target's `MaterializeI64`
//! row — and the axes the relationship must satisfy; the producer consults
//! its own descriptor when admitting a candidate, while the independent
//! replay re-derives the whole grammar — consumer kind, flag-unit flow,
//! operand resolution, and decided value — from the instruction records
//! alone and never reads a rule.
//!
//! The landed family is the decided condition materialization: a
//! `MaterializeBoolean*` consumer whose flag reads all reach one
//! condition-state producer whose operands are compile-time constant
//! becomes the `MaterializeI64` carrying the decided zero or one. The
//! rewrite exercises the unit role the instruction-pair and terminator-pair
//! vocabularies could not name: the flag-consuming *instruction* consumer
//! — its implicit flag uses retire (the compile-time decision replaces the
//! observation) while its `Def` operand carries verbatim onto the rewritten
//! record. The producer stays: its flag definitions remain published for
//! every other reader the function still holds.

use std::collections::BTreeSet;

use register_model::{RegisterInstructionConstraint, RegisterUnitId};
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior, SelectedInstruction, SelectedInstructionKind,
};
use semantic_vocabulary::IntegerValue;

use crate::machine_semantic_kind;
use crate::peepholes::condition_flow::ConditionOperandResolution;

/// The implicit physical-unit relationship the rewrite asserts between the
/// producer's definitions and the consumer's uses — the flag-consuming
/// *instruction* role: the consumer's whole input arrives through implicit
/// uses, not through an operand the instruction-pair grammar could bind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConditionMaterializationUnitFlow {
    /// The producer's implicit definitions feed the consumer's
    /// condition-state uses. Every flag-universe unit — the units any of the
    /// environment's condition-state producer rows defines — that the
    /// consumer implicitly uses must resolve to the producer as its single
    /// last event on every path reaching the consumer's body position, and
    /// the producer's record must define it. Those uses retire: the
    /// compile-time decision replaces the observation, so the rewritten
    /// `MaterializeI64` row reads no flag unit.
    ///
    /// The rewritten row reads nothing at all: every unit the consumer
    /// used outside the flag universe would have to stay read on the
    /// rewritten row, and `MaterializeI64` carries none — so a consumer
    /// with a non-flag use cannot fold. The consumer's implicit
    /// definitions and clobbers republish verbatim: both lists are empty on
    /// the emitted materialization forms, so the empty rewritten lists
    /// satisfy the republish contract exactly.
    ConditionStateResolved,
}

/// The machine-effect relationship the rewrite carries between the
/// consumer's and the rewritten form's effect declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConditionMaterializationEffects {
    /// Both declarations are effect-isolated fall-through rows: no declared
    /// or encoded memory, stack, trap, barrier, call, or cleanup surface,
    /// and every encoded alternative falls through. The consumer's encoded
    /// alternatives may carry implicit unit *uses* — the flag read is the
    /// form's whole input — but only inside the flag universe, and no
    /// implicit definitions or clobbers; the rewritten form's alternatives
    /// carry no implicit unit traffic at all. A target whose
    /// materialization or boolean-materialization rows encode anything else
    /// cannot admit the pair.
    Isolated,
}

/// One declarative condition-materialization rule: the condition-state
/// producer kind the consumer's flag uses resolve to, the flag-consuming
/// consumer kind, the `MaterializeI64` rewrite, and the axes the
/// relationship must satisfy.
///
/// The descriptor declares admissibility; it never certifies it. The
/// producer consults its own rule through
/// [`condition_materialization_for`] and the axes' `admits_*` predicates;
/// the replay in `replay` re-derives the grammar from the instruction
/// records without reading this table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ConditionMaterializationRule {
    producer: MachineSemanticKind,
    consumer: MachineSemanticKind,
    operand_resolution: ConditionOperandResolution,
    unit_flow: ConditionMaterializationUnitFlow,
    effects: ConditionMaterializationEffects,
}

impl ConditionMaterializationRule {
    /// Shared declaration of every decided-materialization pair: the rewrite
    /// is the `MaterializeI64` row, the flag uses retire under the resolved
    /// unit-surface contract, and both declarations stay effect-isolated.
    const DECIDED_MATERIALIZATION: Self = Self {
        producer: MachineSemanticKind::CompareI64,
        consumer: MachineSemanticKind::MaterializeBooleanEqual,
        operand_resolution: ConditionOperandResolution::TwoRegisterOperands,
        unit_flow: ConditionMaterializationUnitFlow::ConditionStateResolved,
        effects: ConditionMaterializationEffects::Isolated,
    };

    /// `compare left, right` deciding the equality materialization.
    pub(super) const COMPARE_EQUAL: Self = Self::DECIDED_MATERIALIZATION;
    /// `compare left, right` deciding the unsigned-less-than materialization.
    pub(super) const COMPARE_U64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanU64LessThan,
        ..Self::DECIDED_MATERIALIZATION
    };
    /// `compare left, right` deciding the signed-less-than materialization.
    pub(super) const COMPARE_I64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanI64LessThan,
        ..Self::DECIDED_MATERIALIZATION
    };
    /// `compare left, right` deciding the unsigned-less-or-equal
    /// materialization.
    pub(super) const COMPARE_U64_LESS_OR_EQUAL: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanU64LessOrEqual,
        ..Self::DECIDED_MATERIALIZATION
    };
    /// `compare left, right` deciding the signed-less-or-equal
    /// materialization.
    pub(super) const COMPARE_I64_LESS_OR_EQUAL: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanI64LessOrEqual,
        ..Self::DECIDED_MATERIALIZATION
    };
    /// `compare left, immediate` deciding the equality materialization.
    pub(super) const COMPARE_IMMEDIATE_EQUAL: Self = Self {
        producer: MachineSemanticKind::CompareI64Immediate,
        operand_resolution: ConditionOperandResolution::RegisterAndKindImmediate,
        ..Self::DECIDED_MATERIALIZATION
    };
    /// `compare left, immediate` deciding the unsigned-less-than
    /// materialization.
    pub(super) const COMPARE_IMMEDIATE_U64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanU64LessThan,
        ..Self::COMPARE_IMMEDIATE_EQUAL
    };
    /// `compare left, immediate` deciding the signed-less-than
    /// materialization.
    pub(super) const COMPARE_IMMEDIATE_I64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanI64LessThan,
        ..Self::COMPARE_IMMEDIATE_EQUAL
    };
    /// `compare left, immediate` deciding the unsigned-less-or-equal
    /// materialization.
    pub(super) const COMPARE_IMMEDIATE_U64_LESS_OR_EQUAL: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanU64LessOrEqual,
        ..Self::COMPARE_IMMEDIATE_EQUAL
    };
    /// `compare left, immediate` deciding the signed-less-or-equal
    /// materialization.
    pub(super) const COMPARE_IMMEDIATE_I64_LESS_OR_EQUAL: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanI64LessOrEqual,
        ..Self::COMPARE_IMMEDIATE_EQUAL
    };
    /// `compare left, 0` deciding the equality materialization.
    pub(super) const COMPARE_ZERO_EQUAL: Self = Self {
        producer: MachineSemanticKind::CompareI64Zero,
        operand_resolution: ConditionOperandResolution::RegisterAndZeroBound,
        ..Self::DECIDED_MATERIALIZATION
    };
    /// `compare left, 0` deciding the unsigned-less-than materialization.
    pub(super) const COMPARE_ZERO_U64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanU64LessThan,
        ..Self::COMPARE_ZERO_EQUAL
    };
    /// `compare left, 0` deciding the signed-less-than materialization.
    pub(super) const COMPARE_ZERO_I64_LESS_THAN: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanI64LessThan,
        ..Self::COMPARE_ZERO_EQUAL
    };
    /// `compare left, 0` deciding the unsigned-less-or-equal materialization.
    pub(super) const COMPARE_ZERO_U64_LESS_OR_EQUAL: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanU64LessOrEqual,
        ..Self::COMPARE_ZERO_EQUAL
    };
    /// `compare left, 0` deciding the signed-less-or-equal materialization.
    pub(super) const COMPARE_ZERO_I64_LESS_OR_EQUAL: Self = Self {
        consumer: MachineSemanticKind::MaterializeBooleanI64LessOrEqual,
        ..Self::COMPARE_ZERO_EQUAL
    };

    pub(super) const fn producer(self) -> MachineSemanticKind {
        self.producer
    }
    pub(super) const fn consumer(self) -> MachineSemanticKind {
        self.consumer
    }
    /// The rewritten form every declared pair publishes: the target's own
    /// `MaterializeI64` row.
    pub(super) const fn rewritten(self) -> MachineSemanticKind {
        MachineSemanticKind::MaterializeI64
    }
    pub(super) const fn operand_resolution(self) -> ConditionOperandResolution {
        self.operand_resolution
    }

    /// Whether `kind` is the consumer this rule declares.
    pub(super) fn matches_consumer(&self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.consumer
    }

    /// Whether `kind` is the producer this rule declares.
    pub(super) fn matches_producer(&self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.producer
    }

    /// The zero or one the consumer publishes for the decided `(left,
    /// right)` state — the predicate the declared consumer kind names,
    /// read in the compare's own direction.
    pub(super) fn decided_value(&self, left: u64, right: u64) -> IntegerValue {
        let holds = match self.consumer {
            MachineSemanticKind::MaterializeBooleanEqual => left == right,
            MachineSemanticKind::MaterializeBooleanU64LessThan => left < right,
            MachineSemanticKind::MaterializeBooleanI64LessThan => (left as i64) < (right as i64),
            MachineSemanticKind::MaterializeBooleanU64LessOrEqual => left <= right,
            MachineSemanticKind::MaterializeBooleanI64LessOrEqual => {
                (left as i64) <= (right as i64)
            }
            _ => unreachable!("declared consumers are the materialization kinds"),
        };
        IntegerValue::Unsigned(u128::from(holds))
    }

    /// The implicit-unit declaration surface the rule admits between the
    /// consumer's record and the rewritten row: under
    /// `ConditionStateResolved` the flag uses retire — every non-flag use
    /// must stay read on the rewritten row, which reads nothing — and the
    /// rewritten row must republish the consumer's implicit definitions and
    /// clobbers verbatim.
    pub(super) fn admits_unit_flow(
        &self,
        flag_uses: &[RegisterUnitId],
        plain_uses: &[RegisterUnitId],
        consumer: &SelectedInstruction,
        rewritten: &RegisterInstructionConstraint,
    ) -> bool {
        match self.unit_flow {
            ConditionMaterializationUnitFlow::ConditionStateResolved => {
                crate::peepholes::condition_flow::resolved_unit_surface(
                    flag_uses, plain_uses, consumer, rewritten,
                )
            }
        }
    }

    /// The effect-declaration surface the rule admits between the
    /// consumer's and the rewritten form's catalog declarations: under
    /// `Isolated` both declarations are isolated fall-through rows, the
    /// consumer's encoded implicit uses stay inside `flag_universe` — the
    /// condition-state read is the whole input — and the rewritten form's
    /// alternatives carry no implicit unit traffic at all.
    pub(super) fn admits_declarations(
        &self,
        flag_universe: &BTreeSet<RegisterUnitId>,
        consumer: &MachineEffectDeclaration,
        rewritten: &MachineEffectDeclaration,
    ) -> bool {
        match self.effects {
            ConditionMaterializationEffects::Isolated => {
                isolated_declaration(consumer)
                    && isolated_declaration(rewritten)
                    && consumer
                        .alternatives
                        .iter()
                        .all(|alternative| materialization_alternative(alternative, flag_universe))
                    && rewritten.alternatives.iter().all(materialize_alternative)
            }
        }
    }
}

/// Every condition-materialization pair the family declares: each
/// condition-state producer kind against each flag-consuming
/// materialization kind.
pub(super) const CONDITION_MATERIALIZATION_RULES: &[ConditionMaterializationRule] = &[
    ConditionMaterializationRule::COMPARE_EQUAL,
    ConditionMaterializationRule::COMPARE_U64_LESS_THAN,
    ConditionMaterializationRule::COMPARE_I64_LESS_THAN,
    ConditionMaterializationRule::COMPARE_U64_LESS_OR_EQUAL,
    ConditionMaterializationRule::COMPARE_I64_LESS_OR_EQUAL,
    ConditionMaterializationRule::COMPARE_IMMEDIATE_EQUAL,
    ConditionMaterializationRule::COMPARE_IMMEDIATE_U64_LESS_THAN,
    ConditionMaterializationRule::COMPARE_IMMEDIATE_I64_LESS_THAN,
    ConditionMaterializationRule::COMPARE_IMMEDIATE_U64_LESS_OR_EQUAL,
    ConditionMaterializationRule::COMPARE_IMMEDIATE_I64_LESS_OR_EQUAL,
    ConditionMaterializationRule::COMPARE_ZERO_EQUAL,
    ConditionMaterializationRule::COMPARE_ZERO_U64_LESS_THAN,
    ConditionMaterializationRule::COMPARE_ZERO_I64_LESS_THAN,
    ConditionMaterializationRule::COMPARE_ZERO_U64_LESS_OR_EQUAL,
    ConditionMaterializationRule::COMPARE_ZERO_I64_LESS_OR_EQUAL,
];

/// The declared pair for one (producer, consumer) kind combination, or none
/// when the family declares no such relationship — several rules matching
/// one combination is a catalog defect and refuses rather than preferring
/// table order.
pub(super) fn condition_materialization_for(
    producer: MachineSemanticKind,
    consumer: MachineSemanticKind,
) -> Option<&'static ConditionMaterializationRule> {
    let mut matches = CONDITION_MATERIALIZATION_RULES
        .iter()
        .filter(|rule| rule.producer() == producer && rule.consumer() == consumer);
    let rule = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(rule)
}

/// The consumer kinds the family declares at all — for admission error
/// reporting that distinguishes an unadmitted consumer kind from an
/// admitted one whose flag flow or operands refuse.
pub(super) fn declared_consumer(kind: SelectedInstructionKind) -> bool {
    CONDITION_MATERIALIZATION_RULES
        .iter()
        .any(|rule| rule.matches_consumer(kind))
}

/// The declared producer kinds — the condition-state producers whose
/// implicit definitions the flag universe is computed from.
pub(super) fn declared_producers() -> impl Iterator<Item = MachineSemanticKind> {
    [
        MachineSemanticKind::CompareI64,
        MachineSemanticKind::CompareI64Immediate,
        MachineSemanticKind::CompareI64Zero,
    ]
    .into_iter()
}

/// The declared surface a decided-materialization form must carry: no
/// memory access, the operation itself never traps, no control-flow
/// barrier, and no call or cleanup — the plain fall-through isolation both
/// the flag read and the literal materialization publish.
fn isolated_declaration(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::None
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
}

/// The encoded surface a condition-reading alternative must carry: no
/// memory access, unchanged stack, no trap surface, a plain fall-through —
/// and implicit uses only inside the flag universe, with no implicit
/// definitions or clobbers: the condition-state read is the whole input.
fn materialization_alternative(
    alternative: &MachineAlternative,
    flag_universe: &BTreeSet<RegisterUnitId>,
) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::NeverV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
        && !encoded.implicit_unit_uses.is_empty()
        && encoded
            .implicit_unit_uses
            .iter()
            .all(|unit| flag_universe.contains(unit))
        && encoded.implicit_unit_defs.is_empty()
        && encoded.implicit_unit_clobbers.is_empty()
}

/// The encoded surface a materialization alternative must carry: no memory
/// access, unchanged stack, no trap surface, a plain fall-through — and no
/// implicit unit traffic at all.
fn materialize_alternative(alternative: &MachineAlternative) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::NeverV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
        && encoded.implicit_unit_uses.is_empty()
        && encoded.implicit_unit_defs.is_empty()
        && encoded.implicit_unit_clobbers.is_empty()
}
