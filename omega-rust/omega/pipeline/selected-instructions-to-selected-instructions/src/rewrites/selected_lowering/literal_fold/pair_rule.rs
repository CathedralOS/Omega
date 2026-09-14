//! Symbolic instruction-pair descriptors carried by selected-lowering catalog rows.
//!
//! A row names its producer, consumer, and rewritten instructions by
//! [`MachineSemanticKind`], the fieldless machine vocabulary the effect
//! catalog already uses. Only the producer (`compute/`) reads descriptors; the
//! validator keeps its own inline matching so a descriptor mistake cannot
//! self-certify. `LiteralFoldPolicy` bits stay the identity-bearing selection;
//! descriptors are realization data derived from the catalog row. Two
//! dimensions are declared today: the result disposition carries the
//! physical-register-unit output channel — whether the rewritten instruction
//! delivers its output through a scalar `Def` operand or through implicit
//! unit definitions such as the target condition state — and the unit-effect
//! surface carries the remaining implicit-unit traffic the rewrite may touch:
//! implicit uses, clobbers, and operand unit bindings on the rewritten row
//! and on the admitted consumer. When a rule needs shape data beyond those —
//! traps, memory, stack, control flow, or further operand roles — extend this
//! struct rather than re-inlining kind matches in compute.

use register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandConstraint,
    TargetRegisterEnvironmentConstraintKeys,
};
use selected_instructions::{MachineSemanticKind, SelectedInstruction, SelectedInstructionKind};
use semantic_vocabulary::IntegerValue;

use crate::machine_semantic_kind;

/// How a pair rule's rewritten instruction delivers its output.
///
/// The producer admits the constraint-row shape matching this declared
/// channel instead of inferring it from operand counts; the independent
/// validator re-derives the same distinction from the consumer kind and row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairResultDisposition {
    /// The rewritten instruction writes one scalar `Def` operand; its
    /// constraint row declares no implicit unit definitions.
    ScalarRegister,
    /// The rewritten instruction writes no `Def` operand; its output is the
    /// constraint row's implicit physical-unit definitions — today the target
    /// condition state defined by the compare-immediate forms.
    ImplicitUnits,
}

/// The implicit-unit traffic the pair's rewrite may carry.
///
/// `PairResultDisposition` owns the result channel — which units the
/// rewritten instruction *defines* as its output. This declaration covers
/// the rest of the unit surface: implicit unit uses and clobbers on the
/// rewritten constraint row, and the operand unit bindings (`fixed_view`,
/// `tied_to`, `early_clobber`) on either side of the rewrite. The producer
/// admits rows and consumers through the declaration; the validator
/// re-derives the same requirements from its own matching so a descriptor
/// mistake cannot self-certify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairUnitEffects {
    /// The rewritten row declares no implicit unit uses and no clobbers, and
    /// no operand on either side of the rewrite carries a unit binding. The
    /// admitted consumer's operands must already be undecorated because the
    /// rewrite rebuilds them from the row — a binding there would be
    /// silently dropped. A rule whose rewritten form implicitly reads or
    /// clobbers a unit — a flag-consuming arithmetic form, a
    /// scratch-clobbering realization — declares a new variant instead of
    /// weakening this one.
    Isolated,
}

impl PairUnitEffects {
    /// Whether the constraint row's instruction-level unit traffic
    /// satisfies the declaration. Implicit *definitions* are the result
    /// channel and stay under `PairResultDisposition`.
    pub fn admits_row_units(self, row: &RegisterInstructionConstraint) -> bool {
        match self {
            Self::Isolated => row.implicit_uses.is_empty() && row.clobbers.is_empty(),
        }
    }

    /// Whether one constraint-row operand carries no unit binding.
    pub fn admits_operand(self, operand: &RegisterOperandConstraint) -> bool {
        match self {
            Self::Isolated => {
                operand.fixed_view.is_none() && operand.tied_to.is_none() && !operand.early_clobber
            }
        }
    }

    /// Whether the admitted consumer's operands carry no unit bindings the
    /// wholesale rebuild from the constraint row would silently drop.
    pub fn admits_consumer(self, consumer: &SelectedInstruction) -> bool {
        match self {
            Self::Isolated => consumer.operands.iter().all(|operand| {
                operand.fixed_view.is_none() && operand.tied_to.is_none() && !operand.early_clobber
            }),
        }
    }
}

/// The symbolic instruction triple, immediate bound, result channel, and
/// unit-effect surface of one lowering rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedInstructionPairRule {
    producer: MachineSemanticKind,
    consumer: MachineSemanticKind,
    rewritten: MachineSemanticKind,
    immediate_limit: u64,
    result: PairResultDisposition,
    unit_effects: PairUnitEffects,
}

impl SelectedInstructionPairRule {
    pub const EXACT_ADD_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::ExactAddI64,
        rewritten: MachineSemanticKind::ExactAddI64Immediate,
        immediate_limit: 4095,
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::Isolated,
    };
    pub const EXACT_SUBTRACT_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::ExactSubtractI64,
        rewritten: MachineSemanticKind::ExactSubtractI64Immediate,
        immediate_limit: 4095,
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::Isolated,
    };
    pub const COMPARE_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::CompareI64,
        rewritten: MachineSemanticKind::CompareI64Immediate,
        immediate_limit: 4095,
        result: PairResultDisposition::ImplicitUnits,
        unit_effects: PairUnitEffects::Isolated,
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

    /// The rewritten instruction's declared output channel: a scalar `Def`
    /// operand or implicit physical-unit definitions.
    pub const fn result(self) -> PairResultDisposition {
        self.result
    }

    /// The pair's declared unit-effect surface: which implicit unit uses,
    /// clobbers, and operand bindings the rewrite may carry.
    pub const fn unit_effects(self) -> PairUnitEffects {
        self.unit_effects
    }

    pub const fn immediate_limit(self) -> u64 {
        self.immediate_limit
    }

    pub const fn admits_immediate(self, value: u64) -> bool {
        value <= self.immediate_limit
    }

    pub fn matches_producer(self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.producer
    }

    pub fn matches_consumer(self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.consumer
    }

    /// The constraint-catalog key of this rule's rewritten instruction form.
    pub fn immediate_constraint_key(
        self,
        keys: &TargetRegisterEnvironmentConstraintKeys,
    ) -> Option<RegisterConstraintKey> {
        match self.rewritten {
            MachineSemanticKind::ExactAddI64Immediate => Some(keys.add_i64_immediate),
            MachineSemanticKind::ExactSubtractI64Immediate => Some(keys.subtract_i64_immediate),
            MachineSemanticKind::CompareI64Immediate => Some(keys.compare_i64_immediate),
            _ => None,
        }
    }

    /// Rewrite `kind` into this rule's immediate form, retaining proof custody.
    pub fn rewrite_consumer(
        self,
        kind: SelectedInstructionKind,
        immediate: u64,
    ) -> Option<SelectedInstructionKind> {
        let immediate = IntegerValue::Unsigned(u128::from(immediate));
        match (self.rewritten, kind) {
            (
                MachineSemanticKind::ExactAddI64Immediate,
                SelectedInstructionKind::ExactAddI64 {
                    obligation,
                    accepted_fact,
                },
            ) => Some(SelectedInstructionKind::ExactAddI64Immediate {
                immediate,
                obligation,
                accepted_fact,
            }),
            (
                MachineSemanticKind::ExactSubtractI64Immediate,
                SelectedInstructionKind::ExactSubtractI64 {
                    obligation,
                    accepted_fact,
                },
            ) => Some(SelectedInstructionKind::ExactSubtractI64Immediate {
                immediate,
                obligation,
                accepted_fact,
            }),
            (MachineSemanticKind::CompareI64Immediate, SelectedInstructionKind::CompareI64) => {
                Some(SelectedInstructionKind::CompareI64Immediate { immediate })
            }
            _ => None,
        }
    }
}
