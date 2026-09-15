//! Symbolic instruction-pair descriptors carried by selected-lowering catalog rows.
//!
//! A row names its producer, consumer, and rewritten instructions by
//! [`MachineSemanticKind`], the fieldless machine vocabulary the effect
//! catalog already uses. Only the producer (`compute/`) reads descriptors; the
//! validator keeps its own inline matching so a descriptor mistake cannot
//! self-certify. `LiteralFoldPolicy` bits stay the identity-bearing selection;
//! descriptors are realization data derived from the catalog row. Four
//! dimensions are declared today: the result disposition carries the
//! physical-register-unit output channel — whether the rewritten instruction
//! delivers its output through a scalar `Def` operand or through implicit
//! unit definitions such as the target condition state — the unit-effect
//! surface carries the remaining implicit-unit traffic the rewrite may touch
//! (implicit uses, clobbers, and operand unit bindings on the rewritten row,
//! on the admitted consumer, and on the eliminated producer), the
//! machine-effect surface carries the non-unit dimensions — memory, trap,
//! stack, control flow, barrier, call, and cleanup — that each form's
//! [`MachineEffectDeclaration`] must satisfy, and the operand shape carries
//! the consumer-grammar dimension: whether the folded literal is a binary
//! consumer's right `Use` operand, a commutative binary consumer's left
//! `Use` operand, or a unary consumer's sole `Use` operand. Beyond the
//! isolated machine-effect surface, [`PairMachineEffects::IndexedPointerReadFold`]
//! declares the first non-isolated relationship: a consumer that reads memory
//! through the folded index and a rewritten form that reads the same bytes
//! through a materialized byte offset. When a rule needs shape data beyond
//! those — a further non-isolated effect relationship or further operand
//! roles — extend this struct rather than re-inlining kind matches in
//! compute.

use register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandConstraint,
    TargetRegisterEnvironmentConstraintKeys,
};
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior, SelectedInstruction, SelectedInstructionKind,
};
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

    /// Whether the eliminated producer's instruction record carries no unit
    /// traffic — implicit uses, definitions, clobbers, or operand bindings —
    /// that removing the instruction would silently drop.
    pub fn admits_producer(self, producer: &SelectedInstruction) -> bool {
        match self {
            Self::Isolated => {
                producer.implicit_uses.is_empty()
                    && producer.implicit_defs.is_empty()
                    && producer.clobbers.is_empty()
                    && producer.operands.iter().all(|operand| {
                        operand.fixed_view.is_none()
                            && operand.tied_to.is_none()
                            && !operand.early_clobber
                    })
            }
        }
    }
}

/// The non-unit machine-effect surface the pair's rewrite may carry, plus the
/// unit-traffic relation the three declarations must satisfy.
///
/// `PairUnitEffects` owns the physical-register-unit traffic carried by the
/// selected instruction records and constraint rows (implicit uses,
/// definitions as the declared result channel, clobbers, and operand unit
/// bindings); this declaration covers the validated
/// [`MachineEffectDeclaration`] surface — memory, trap, stack, control flow,
/// barrier, call, and cleanup — for all three instruction forms the rewrite
/// involves, and the relationship between the consumer's and rewritten
/// form's encoded implicit-unit traffic. The producer admits the eliminated
/// producer's, the admitted consumer's, and the rewritten form's catalog
/// declarations through this dimension; the validator re-derives the same
/// requirements from its own matching so a descriptor mistake cannot
/// self-certify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairMachineEffects {
    /// Every form the rewrite touches is effect-isolated: its declaration
    /// carries no memory access, no trap or fault behavior, no barrier, call,
    /// or cleanup behavior, and every encoded alternative falls through
    /// without memory, stack, or trap traffic.
    ///
    /// The three roles differ only in their encoded implicit-unit traffic.
    /// The eliminated producer must declare none at all — removing the
    /// instruction would silently drop it. The consumer declares no implicit
    /// unit uses — a use the rewritten form does not carry would be unit
    /// state the rewrite silently stops observing — and every unit it
    /// defines must stay defined by every rewritten alternative, or its
    /// readers would observe a stale unit. Its clobbers are unrestricted:
    /// dropping a clobber only narrows what may be destroyed, which is the
    /// intended refinement when the x86-64 `sub` consumer's `rflags` clobber
    /// disappears under the flag-preserving immediate form. The rewritten
    /// form declares no implicit uses or clobbers; its implicit definitions
    /// are the declared result channel under `PairResultDisposition`.
    Isolated,
    /// The consumer reads memory through a pointer plus an index operand the
    /// fold removes; the rewritten form reads the same pointer through a
    /// materialized byte offset. `index_operand` is the consumer operand
    /// position the literal victim occupies — it must equal the operand
    /// shape's declared victim position.
    ///
    /// The eliminated producer stays effect-isolated, as under
    /// [`Isolated`](Self::Isolated). The consumer and rewritten declarations
    /// must carry an identical non-unit surface — pointer-read memory, the
    /// same trap, barrier, call, and cleanup behavior — and every consumer
    /// alternative must be an indexed pointer read whose index is exactly
    /// `index_operand`, matched by a direct pointer read over the same
    /// pointer operand and byte count in every rewritten alternative, with
    /// pairwise-equal stack, trap, and control encodings. The unit-traffic
    /// relation is unchanged: no implicit uses on either side, every unit the
    /// consumer defines still defined by every rewritten alternative, and no
    /// implicit uses or clobbers on the rewritten form.
    IndexedPointerReadFold {
        /// The consumer operand position carrying the folded index register.
        index_operand: u16,
    },
}

impl PairMachineEffects {
    /// Whether the eliminated producer's catalog declaration is
    /// effect-isolated including every implicit unit it could have written.
    /// Every landed pair requires this surface: removing the producer must
    /// drop nothing machine-visible.
    pub fn admits_producer(self, declaration: &MachineEffectDeclaration) -> bool {
        match self {
            Self::Isolated | Self::IndexedPointerReadFold { .. } => {
                isolated_declaration(declaration)
                    && declaration.alternatives.iter().all(|alternative| {
                        isolated_alternative(alternative)
                            && alternative.encoded.implicit_unit_uses.is_empty()
                            && alternative.encoded.implicit_unit_defs.is_empty()
                            && alternative.encoded.implicit_unit_clobbers.is_empty()
                    })
            }
        }
    }

    /// Whether the admitted consumer's catalog declaration satisfies the
    /// pair's declared relationship to `rewritten`. For [`Isolated`](Self::Isolated)
    /// the consumer is effect-isolated outside its unit surface and the
    /// rewrite may replace that surface wholesale: no implicit uses at all,
    /// and every implicit definition covered by every alternative the
    /// rewritten form could select. For
    /// [`IndexedPointerReadFold`](Self::IndexedPointerReadFold) the consumer
    /// is the indexed pointer read: the two declarations share the same
    /// non-unit surface, and every consumer alternative's encoded indexed
    /// read at the folded operand position is matched by every rewritten
    /// alternative's direct read over the same pointer and byte count.
    pub fn admits_consumer(
        self,
        declaration: &MachineEffectDeclaration,
        rewritten: &MachineEffectDeclaration,
    ) -> bool {
        match self {
            Self::Isolated => {
                isolated_declaration(declaration)
                    && declaration.alternatives.iter().all(|alternative| {
                        isolated_alternative(alternative)
                            && alternative.encoded.implicit_unit_uses.is_empty()
                            && implicit_defs_covered(alternative, rewritten)
                    })
            }
            Self::IndexedPointerReadFold { index_operand } => {
                declaration.memory == MachineMemoryEffect::ReadPointerV1
                    && declaration.memory == rewritten.memory
                    && declaration.trap == rewritten.trap
                    && declaration.barrier == rewritten.barrier
                    && declaration.call == rewritten.call
                    && declaration.cleanup == rewritten.cleanup
                    && declaration.alternatives.iter().all(|alternative| {
                        rewritten.alternatives.iter().all(|rewritten_alternative| {
                            indexed_pointer_read_matches(
                                alternative,
                                rewritten_alternative,
                                index_operand,
                            )
                        })
                    })
            }
        }
    }

    /// Whether the rewritten form's catalog declaration satisfies the pair's
    /// declared shape. [`Isolated`](Self::Isolated) requires an
    /// effect-isolated form with no implicit unit uses or clobbers beyond
    /// its result channel; [`IndexedPointerReadFold`](Self::IndexedPointerReadFold)
    /// requires the plain pointer-read form whose alternatives all read
    /// memory through a pointer operand and byte offset, fall through, leave
    /// the stack unchanged, and declare no implicit uses or clobbers.
    pub fn admits_rewritten(self, declaration: &MachineEffectDeclaration) -> bool {
        match self {
            Self::Isolated => {
                isolated_declaration(declaration)
                    && declaration.alternatives.iter().all(|alternative| {
                        isolated_alternative(alternative)
                            && alternative.encoded.implicit_unit_uses.is_empty()
                            && alternative.encoded.implicit_unit_clobbers.is_empty()
                    })
            }
            Self::IndexedPointerReadFold { .. } => {
                declaration.memory == MachineMemoryEffect::ReadPointerV1
                    && declaration.alternatives.iter().all(|alternative| {
                        matches!(
                            alternative.encoded.memory,
                            MachineEncodedMemoryEffect::ReadPointerV1 { .. }
                        ) && alternative.encoded.stack == MachineEncodedStackEffect::UnchangedV1
                            && alternative.encoded.control
                                == MachineEncodedControlEffect::FallThroughV1
                            && alternative.encoded.implicit_unit_uses.is_empty()
                            && alternative.encoded.implicit_unit_clobbers.is_empty()
                    })
            }
        }
    }
}

/// The indexed pointer read a `consumer` alternative encodes is the read
/// `rewritten`'s alternative performs once the fold replaces the index
/// register with a byte offset: same pointer operand, same byte count, the
/// folded operand position as the index, and pairwise-identical stack,
/// trap, control, and implicit-unit surface — no implicit uses on either
/// side, every unit the consumer defines still defined, and no clobbers on
/// the rewritten form.
fn indexed_pointer_read_matches(
    consumer: &MachineAlternative,
    rewritten: &MachineAlternative,
    index_operand: u16,
) -> bool {
    let (
        MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
            pointer_operand,
            index_operand: index,
            byte_count,
        },
        MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand: rewritten_pointer,
            byte_count: rewritten_bytes,
        },
    ) = (consumer.encoded.memory, rewritten.encoded.memory)
    else {
        return false;
    };
    index == index_operand
        && pointer_operand == rewritten_pointer
        && byte_count == rewritten_bytes
        && consumer.encoded.stack == rewritten.encoded.stack
        && consumer.encoded.trap == rewritten.encoded.trap
        && consumer.encoded.control == rewritten.encoded.control
        && consumer.encoded.implicit_unit_uses.is_empty()
        && consumer
            .encoded
            .implicit_unit_defs
            .iter()
            .all(|unit| rewritten.encoded.implicit_unit_defs.contains(unit))
        && rewritten.encoded.implicit_unit_uses.is_empty()
        && rewritten.encoded.implicit_unit_clobbers.is_empty()
}

/// Every implicit unit `consumer`'s alternative defines remains defined no
/// matter which alternative the rewritten form selects: the encoding choice
/// is not fixed at fold time, so coverage must hold unconditionally.
fn implicit_defs_covered(
    consumer: &MachineAlternative,
    rewritten: &MachineEffectDeclaration,
) -> bool {
    rewritten.alternatives.iter().all(|alternative| {
        consumer
            .encoded
            .implicit_unit_defs
            .iter()
            .all(|unit| alternative.encoded.implicit_unit_defs.contains(unit))
    })
}

/// The non-unit declaration surface an isolated pair form must carry: no
/// memory access, no trap, no barrier, no call, no cleanup.
fn isolated_declaration(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::None
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
}

/// The non-unit encoded surface an isolated pair form's alternatives must
/// carry: no memory access, unchanged stack, never traps, falls through.
fn isolated_alternative(alternative: &MachineAlternative) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::NeverV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
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
    /// Commutative binary consumer: the literal victim is the left `Use`
    /// operand (operand index 0) and the right `Use` operand survives into
    /// the rewritten instruction's `Use` position. Declaring this shape
    /// attests that the fold computes the same value under operand exchange:
    /// the immediate form encodes `surviving <op> literal`, so only an
    /// operation exact under commutation may declare it. Exact addition
    /// commutes; subtraction and comparison fix the literal's role, so only
    /// the add family declares a left-literal pair.
    BinaryLeftLiteral,
    /// Unary consumer: the literal victim is the sole `Use` operand (operand
    /// index 0). The rewritten instruction consumes no register input — the
    /// fold recomputes the consumer's constant output directly, as in the
    /// extension-elimination and copy-materialization rules whose rewritten
    /// form is a `MaterializeI64`.
    UnaryLiteral,
}

/// The symbolic instruction triple, immediate bound, result channel,
/// unit-effect surface, machine-effect surface, and consumer operand grammar
/// of one lowering rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedInstructionPairRule {
    producer: MachineSemanticKind,
    consumer: MachineSemanticKind,
    rewritten: MachineSemanticKind,
    operand_shape: PairOperandShape,
    immediate_limit: u64,
    result: PairResultDisposition,
    unit_effects: PairUnitEffects,
    machine_effects: PairMachineEffects,
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
        machine_effects: PairMachineEffects::Isolated,
    };
    /// Eliminate `MaterializeI64` feeding the left operand of `ExactAddI64`:
    /// exact addition commutes, so `literal + x` rewrites to the same
    /// `ExactAddI64Immediate` form `x + literal` uses. The same catalog
    /// selection admits both grammars; the pair disambiguates by which `Use`
    /// position the folded literal occupies.
    pub const EXACT_ADD_LEFT_IMMEDIATE_U12: Self = Self {
        operand_shape: PairOperandShape::BinaryLeftLiteral,
        ..Self::EXACT_ADD_IMMEDIATE_U12
    };
    pub const EXACT_SUBTRACT_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::ExactSubtractI64,
        rewritten: MachineSemanticKind::ExactSubtractI64Immediate,
        operand_shape: PairOperandShape::BinaryRightLiteral,
        immediate_limit: 4095,
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::Isolated,
        machine_effects: PairMachineEffects::Isolated,
    };
    pub const COMPARE_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::CompareI64,
        rewritten: MachineSemanticKind::CompareI64Immediate,
        operand_shape: PairOperandShape::BinaryRightLiteral,
        immediate_limit: 4095,
        result: PairResultDisposition::ImplicitUnits,
        unit_effects: PairUnitEffects::Isolated,
        machine_effects: PairMachineEffects::Isolated,
    };

    /// Shared base of the unary materialization folds: `MaterializeI64`
    /// feeding a sole-`Use` consumer rewrites into a direct
    /// `MaterializeI64` of the constant the consumer computes — the
    /// extension's folded output bits, or the literal itself under `CopyI64`.
    /// `consumer` is a placeholder each concrete rule overrides.
    const UNARY_MATERIALIZE_FOLD: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::MaterializeI64,
        rewritten: MachineSemanticKind::MaterializeI64,
        operand_shape: PairOperandShape::UnaryLiteral,
        // The folded value always encodes as a `MaterializeI64` constant; no
        // target immediate bound applies to the source literal itself.
        immediate_limit: u64::MAX,
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::Isolated,
        machine_effects: PairMachineEffects::Isolated,
    };
    /// Eliminate `MaterializeI64` feeding `ZeroExtendU8`: the result is the
    /// literal's low eight bits materialized directly.
    pub const ZERO_EXTEND_U8_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::ZeroExtendU8,
        ..Self::UNARY_MATERIALIZE_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `ZeroExtendU16`: the result is the
    /// literal's low sixteen bits materialized directly.
    pub const ZERO_EXTEND_U16_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::ZeroExtendU16,
        ..Self::UNARY_MATERIALIZE_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `ZeroExtendU32`: the result is the
    /// literal's low thirty-two bits materialized directly.
    pub const ZERO_EXTEND_U32_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::ZeroExtendU32,
        ..Self::UNARY_MATERIALIZE_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `SignExtendI8`: the result is the
    /// sign extension of the literal's low eight bits materialized directly.
    pub const SIGN_EXTEND_I8_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::SignExtendI8,
        ..Self::UNARY_MATERIALIZE_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `SignExtendI16`: the result is the
    /// sign extension of the literal's low sixteen bits materialized directly.
    pub const SIGN_EXTEND_I16_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::SignExtendI16,
        ..Self::UNARY_MATERIALIZE_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `SignExtendI32`: the result is the
    /// sign extension of the literal's low thirty-two bits materialized
    /// directly.
    pub const SIGN_EXTEND_I32_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::SignExtendI32,
        ..Self::UNARY_MATERIALIZE_FOLD
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

    /// Eliminate `MaterializeI64` feeding `CopyI64`: the copy's output is
    /// the literal itself, so the fold materializes it directly at the
    /// copy's destination register — a copy of a constant is the constant.
    pub const COPY_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::CopyI64,
        ..Self::UNARY_MATERIALIZE_FOLD
    };

    /// Eliminate `MaterializeI64` feeding the index operand of
    /// `Load8Indexed`: the indexed byte read rewrites to the direct-offset
    /// `Load8` whose `byte_offset` is the folded literal. The rewrite
    /// preserves the read — the consumer's `ReadIndexedPointerV1`
    /// alternatives and the rewritten form's `ReadPointerV1` alternatives
    /// name the same pointer and byte count — while the may-fault trap
    /// surface is identical on both forms, so no trap behavior the fold
    /// removes is left unobserved.
    ///
    /// The immediate bound is the narrowest byte-offset field any target's
    /// `Load8` encoder admits: aarch64 `ldrb` carries a 12-bit unsigned
    /// scaled offset, so a target-independent rule declares 4095 even though
    /// x86-64's disp32 form would admit more.
    pub const LOAD8_INDEXED_U12: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::Load8Indexed,
            rewritten: MachineSemanticKind::Load8,
            operand_shape: PairOperandShape::BinaryRightLiteral,
            immediate_limit: 4095,
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::Isolated,
            machine_effects: PairMachineEffects::IndexedPointerReadFold { index_operand: 1 },
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::IndexedPointerReadFold { index_operand }
                    if index_operand == rule.victim_operand()
            ),
            "the folded operand is the indexed read's index operand"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the offset operand of
    /// `ByteViewAddress`: the base-plus-index address computation rewrites to
    /// the constant-offset `AddressOffset` form whose `byte_offset` is the
    /// folded literal. Both forms are effect-isolated — neither touches
    /// memory, traps, or implicit units — so the rewrite replaces the
    /// consumer's surface wholesale under the ordinary
    /// [`Isolated`](PairMachineEffects::Isolated) relationship.
    ///
    /// The immediate bound is the narrowest byte-offset field any target's
    /// `AddressOffset` encoder admits: aarch64 `add xD, xN, #imm12` carries a
    /// 12-bit unsigned displacement, so a target-independent rule declares
    /// 4095 even though x86-64's `lea` disp32 form would admit more.
    pub const BYTE_VIEW_ADDRESS_OFFSET_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::ByteViewAddress,
        rewritten: MachineSemanticKind::AddressOffset,
        operand_shape: PairOperandShape::BinaryRightLiteral,
        immediate_limit: 4095,
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::Isolated,
        machine_effects: PairMachineEffects::Isolated,
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

    /// The pair's declared machine-effect surface: which memory, trap, stack,
    /// and control-flow traffic the producer, consumer, and rewritten forms
    /// may carry in the bound effect catalog.
    pub const fn machine_effects(self) -> PairMachineEffects {
        self.machine_effects
    }

    /// The consumer operand index the literal victim must occupy.
    pub const fn victim_operand(self) -> u16 {
        match self.operand_shape {
            PairOperandShape::BinaryRightLiteral => 1,
            PairOperandShape::BinaryLeftLiteral | PairOperandShape::UnaryLiteral => 0,
        }
    }

    pub const fn immediate_limit(self) -> u64 {
        self.immediate_limit
    }

    pub const fn admits_immediate(self, value: u64) -> bool {
        value <= self.immediate_limit
    }

    /// The constant payload the rewritten instruction embeds for `literal`:
    /// the literal itself for the immediate forms and the copy fold, or the
    /// extension's exact output bits for the unary extension folds. The
    /// result is the recorded `immediate` in [`crate::LiteralFoldAction`].
    pub fn fold_immediate(self, literal: u64) -> Option<u64> {
        match self.operand_shape {
            PairOperandShape::BinaryRightLiteral | PairOperandShape::BinaryLeftLiteral => {
                Some(literal)
            }
            PairOperandShape::UnaryLiteral => match self.consumer {
                MachineSemanticKind::CopyI64 => Some(literal),
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
            MachineSemanticKind::Load8 => keys.load8,
            MachineSemanticKind::AddressOffset => keys.address_offset,
            _ => None,
        }
    }

    /// Rewrite `kind` into this rule's folded form, retaining proof custody.
    ///
    /// `immediate` is the payload [`fold_immediate`](Self::fold_immediate)
    /// computed for the folded literal. `result_scalar` is the scalar type of
    /// the consumer's `Def` operand when it has one; the materialization
    /// folds require it so the materialized constant sign-matches and is
    /// admitted by the result register's declared integer type.
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
                | SelectedInstructionKind::SignExtendI32
                | SelectedInstructionKind::CopyI64),
            ) if machine_semantic_kind(kind) == self.consumer => {
                scalar_materialize_value(immediate, result_scalar?)
                    .map(|value| SelectedInstructionKind::MaterializeI64 { value })
            }
            (MachineSemanticKind::Load8, SelectedInstructionKind::Load8Indexed) => {
                u32::try_from(immediate)
                    .ok()
                    .map(|byte_offset| SelectedInstructionKind::Load8 { byte_offset })
            }
            (MachineSemanticKind::AddressOffset, SelectedInstructionKind::ByteViewAddress) => {
                u32::try_from(immediate)
                    .ok()
                    .map(|byte_offset| SelectedInstructionKind::AddressOffset { byte_offset })
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
