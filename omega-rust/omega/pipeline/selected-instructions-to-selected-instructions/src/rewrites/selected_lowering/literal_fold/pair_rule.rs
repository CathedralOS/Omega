//! Symbolic instruction-pair descriptors carried by selected-lowering catalog rows.
//!
//! A row names its producer, consumer, and rewritten instructions by
//! [`MachineSemanticKind`], the fieldless machine vocabulary the effect
//! catalog already uses. Only the producer (`compute/`) reads descriptors; the
//! validator keeps its own inline matching so a descriptor mistake cannot
//! self-certify. `LiteralFoldPolicy` bits stay the identity-bearing selection;
//! descriptors are realization data derived from the catalog row. Three
//! dimensions are declared today: the result disposition carries the
//! physical-register-unit output channel — whether the rewritten instruction
//! delivers its output through a scalar `Def` operand or through implicit
//! unit definitions such as the target condition state — the unit-effect
//! surface carries the remaining implicit-unit traffic the rewrite may touch
//! (implicit uses, clobbers, and operand unit bindings on the rewritten row
//! and on the admitted consumer), and the operand shape carries the
//! consumer-grammar dimension: whether the folded literal is a binary
//! consumer's right `Use` operand or a unary consumer's sole `Use` operand.
//! When a rule needs shape data beyond those — traps, memory, stack, control
//! flow, or further operand roles — extend this struct rather than
//! re-inlining kind matches in compute.

use register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandConstraint,
    TargetRegisterEnvironmentConstraintKeys,
};
use selected_instructions::{MachineSemanticKind, SelectedInstruction, SelectedInstructionKind};
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

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

/// Where the folded literal sits in the consumer's operand list, and therefore
/// what shape the rewritten constraint row carries.
///
/// The producer admits the operand arrangement matching this declared grammar
/// instead of inferring it from operand counts; the independent validator
/// re-derives the same distinction from the consumer kind alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairOperandShape {
    /// Binary consumer: the literal victim is the right `Use` operand
    /// (operand index 1) and the left `Use` operand survives into the
    /// rewritten instruction — the immediate-form binary arithmetic and
    /// compare rules.
    BinaryRightLiteral,
    /// Unary consumer: the literal victim is the sole `Use` operand (operand
    /// index 0). The rewritten instruction consumes no register input — the
    /// fold recomputes the consumer's constant output directly, as in the
    /// extension-elimination rules whose rewritten form is a `MaterializeI64`.
    UnaryLiteral,
}

/// The symbolic instruction triple, immediate bound, result channel,
/// unit-effect surface, and consumer operand grammar of one lowering rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedInstructionPairRule {
    producer: MachineSemanticKind,
    consumer: MachineSemanticKind,
    rewritten: MachineSemanticKind,
    operand_shape: PairOperandShape,
    immediate_limit: u64,
    result: PairResultDisposition,
    unit_effects: PairUnitEffects,
}

impl SelectedInstructionPairRule {
    pub const EXACT_ADD_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::ExactAddI64,
        rewritten: MachineSemanticKind::ExactAddI64Immediate,
        operand_shape: PairOperandShape::BinaryRightLiteral,
        immediate_limit: 4095,
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::Isolated,
    };
    pub const EXACT_SUBTRACT_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::ExactSubtractI64,
        rewritten: MachineSemanticKind::ExactSubtractI64Immediate,
        operand_shape: PairOperandShape::BinaryRightLiteral,
        immediate_limit: 4095,
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::Isolated,
    };
    pub const COMPARE_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::CompareI64,
        rewritten: MachineSemanticKind::CompareI64Immediate,
        operand_shape: PairOperandShape::BinaryRightLiteral,
        immediate_limit: 4095,
        result: PairResultDisposition::ImplicitUnits,
        unit_effects: PairUnitEffects::Isolated,
    };

    const EXTENSION_FOLD: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::MaterializeI64,
        rewritten: MachineSemanticKind::MaterializeI64,
        operand_shape: PairOperandShape::UnaryLiteral,
        // The folded value always encodes as a `MaterializeI64` constant; no
        // target immediate bound applies to the source literal itself.
        immediate_limit: u64::MAX,
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::Isolated,
    };
    /// Eliminate `MaterializeI64` feeding `ZeroExtendU8`: the result is the
    /// literal's low eight bits materialized directly.
    pub const ZERO_EXTEND_U8_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::ZeroExtendU8,
        ..Self::EXTENSION_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `ZeroExtendU16`: the result is the
    /// literal's low sixteen bits materialized directly.
    pub const ZERO_EXTEND_U16_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::ZeroExtendU16,
        ..Self::EXTENSION_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `ZeroExtendU32`: the result is the
    /// literal's low thirty-two bits materialized directly.
    pub const ZERO_EXTEND_U32_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::ZeroExtendU32,
        ..Self::EXTENSION_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `SignExtendI8`: the result is the
    /// sign extension of the literal's low eight bits materialized directly.
    pub const SIGN_EXTEND_I8_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::SignExtendI8,
        ..Self::EXTENSION_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `SignExtendI16`: the result is the
    /// sign extension of the literal's low sixteen bits materialized directly.
    pub const SIGN_EXTEND_I16_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::SignExtendI16,
        ..Self::EXTENSION_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `SignExtendI32`: the result is the
    /// sign extension of the literal's low thirty-two bits materialized
    /// directly.
    pub const SIGN_EXTEND_I32_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::SignExtendI32,
        ..Self::EXTENSION_FOLD
    };
    /// The six unary extension-elimination rules, in extension-kind order.
    pub const EXTENSION_LITERAL_FOLDS: [Self; 6] = [
        Self::ZERO_EXTEND_U8_LITERAL_FOLD,
        Self::ZERO_EXTEND_U16_LITERAL_FOLD,
        Self::ZERO_EXTEND_U32_LITERAL_FOLD,
        Self::SIGN_EXTEND_I8_LITERAL_FOLD,
        Self::SIGN_EXTEND_I16_LITERAL_FOLD,
        Self::SIGN_EXTEND_I32_LITERAL_FOLD,
    ];

    pub const fn producer(self) -> MachineSemanticKind {
        self.producer
    }

    pub const fn consumer(self) -> MachineSemanticKind {
        self.consumer
    }

    pub const fn rewritten(self) -> MachineSemanticKind {
        self.rewritten
    }

    /// The consumer operand grammar: where the literal victim sits and what
    /// shape the rewritten constraint row therefore carries.
    pub const fn operand_shape(self) -> PairOperandShape {
        self.operand_shape
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

    /// The consumer operand index the literal victim must occupy.
    pub const fn victim_operand(self) -> u16 {
        match self.operand_shape {
            PairOperandShape::BinaryRightLiteral => 1,
            PairOperandShape::UnaryLiteral => 0,
        }
    }

    pub const fn immediate_limit(self) -> u64 {
        self.immediate_limit
    }

    pub const fn admits_immediate(self, value: u64) -> bool {
        value <= self.immediate_limit
    }

    /// The constant payload the rewritten instruction embeds for `literal`:
    /// the literal itself for the immediate forms, or the extension's exact
    /// output bits for the unary extension folds. The result is the recorded
    /// `immediate` in [`crate::LiteralFoldAction`].
    pub fn fold_immediate(self, literal: u64) -> Option<u64> {
        match self.operand_shape {
            PairOperandShape::BinaryRightLiteral => Some(literal),
            PairOperandShape::UnaryLiteral => match self.consumer {
                MachineSemanticKind::ZeroExtendU8 => Some(literal & 0xFF),
                MachineSemanticKind::ZeroExtendU16 => Some(literal & 0xFFFF),
                MachineSemanticKind::ZeroExtendU32 => Some(literal & 0xFFFF_FFFF),
                MachineSemanticKind::SignExtendI8 => {
                    Some((literal & 0xFF) as u8 as i8 as i64 as u64)
                }
                MachineSemanticKind::SignExtendI16 => {
                    Some((literal & 0xFFFF) as u16 as i16 as i64 as u64)
                }
                MachineSemanticKind::SignExtendI32 => {
                    Some((literal & 0xFFFF_FFFF) as u32 as i32 as i64 as u64)
                }
                _ => None,
            },
        }
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
            MachineSemanticKind::MaterializeI64 => Some(keys.materialize_i64),
            _ => None,
        }
    }

    /// Rewrite `kind` into this rule's folded form, retaining proof custody.
    ///
    /// `immediate` is the payload [`fold_immediate`](Self::fold_immediate)
    /// computed for the folded literal. `result_scalar` is the scalar type of
    /// the consumer's `Def` operand when it has one; the extension folds
    /// require it so the materialized constant sign-matches and is admitted by
    /// the result register's declared integer type.
    pub fn rewrite_consumer(
        self,
        kind: SelectedInstructionKind,
        immediate: u64,
        result_scalar: Option<ScalarType>,
    ) -> Option<SelectedInstructionKind> {
        let literal = IntegerValue::Unsigned(u128::from(immediate));
        match (self.rewritten, kind) {
            (
                MachineSemanticKind::ExactAddI64Immediate,
                SelectedInstructionKind::ExactAddI64 {
                    obligation,
                    accepted_fact,
                },
            ) => Some(SelectedInstructionKind::ExactAddI64Immediate {
                immediate: literal,
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
                immediate: literal,
                obligation,
                accepted_fact,
            }),
            (MachineSemanticKind::CompareI64Immediate, SelectedInstructionKind::CompareI64) => {
                Some(SelectedInstructionKind::CompareI64Immediate { immediate: literal })
            }
            (
                MachineSemanticKind::MaterializeI64,
                kind @ (SelectedInstructionKind::ZeroExtendU8
                | SelectedInstructionKind::ZeroExtendU16
                | SelectedInstructionKind::ZeroExtendU32
                | SelectedInstructionKind::SignExtendI8
                | SelectedInstructionKind::SignExtendI16
                | SelectedInstructionKind::SignExtendI32),
            ) if machine_semantic_kind(kind) == self.consumer => {
                scalar_materialize_value(immediate, result_scalar?)
                    .map(|value| SelectedInstructionKind::MaterializeI64 { value })
            }
            _ => None,
        }
    }
}

/// The `IntegerValue` a folded `MaterializeI64` must declare for `bits` so the
/// result register's scalar type keeps the exact constant: `Unsigned` under an
/// unsigned result type, the two's-complement signed interpretation under a
/// signed one. Non-integer, address-carrier, or wider-than-64 result types —
/// and any result type that cannot admit the full folded value — reject.
fn scalar_materialize_value(bits: u64, scalar: ScalarType) -> Option<IntegerValue> {
    let ScalarType::Integer(integer) = scalar else {
        return None;
    };
    if integer.is_address() || integer.bits() > 64 {
        return None;
    }
    let value = match integer.sign() {
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(bits)),
        IntegerSign::Signed => IntegerValue::Signed(i128::from(bits as i64)),
    };
    integer.admits(value).then_some(value)
}
