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
//! `Use` operand, a unary consumer's sole `Use` operand, or a binary
//! consumer whose folded result is a constant of the literal alone — no
//! `Use` operand survives, and every operand past the result is a
//! dropped `Def` scratch or, under the divide's auxiliary grammar, a
//! dropped `Use` proven to read only a zero materialization — at the
//! right `Use` position or, under the annihilator and zero-dividend
//! grammars, the left one. The surviving-`Use` grammars also carry a
//! scratch-tail form — [`PairOperandShape::BinaryRightLiteralScratchDefs`]
//! and [`PairOperandShape::BinaryLeftLiteralScratchDefs`] — for a consumer
//! whose operand list continues past its `Def` result with scratch outputs
//! the fold drops under occurrence-free custody: the clamped saturating-add
//! rows carry such a bound scratch at operand 3. Beyond the
//! isolated machine-effect surface, [`PairMachineEffects::IndexedPointerReadFold`]
//! declares the first non-isolated relationship — a consumer that reads
//! memory through the folded index and a rewritten form that reads the same
//! bytes through a materialized byte offset — and the trap-carrying
//! relationships come in two discharge forms:
//! [`PairMachineEffects::FaultDischargedByLiteral`] declares a consumer
//! whose encoded alternatives may architecturally fault, where the folded
//! literal is exactly the value that makes the fault unreachable, and
//! [`PairMachineEffects::FaultDischargedByObligation`] declares the same
//! may-fault consumer surface where the fault is already unreachable under
//! the obligation the consumer kind carries — the literal fixes the result,
//! not the divisor's definedness.
//! [`PairMachineEffects::DeadConsumerUnitDefs`] declares the dead-unit-def
//! relationship: a consumer whose implicit unit *definitions* the rewrite
//! retires because the rewritten form does not define them — the saturating
//! add's condition-state definition an isolated copy does not carry — where
//! retiring them is admitted only while no instruction or terminator in the
//! function implicitly uses a unit the consumer record defines. The
//! immediate bound carries whether
//! any literal up to an encoding limit is admitted or the fold's
//! correctness requires one exact literal value — and, for families that
//! share one consumer kind and operand position, keeps the grammars
//! disjoint on the literal's value: the bitwise-and annihilator and
//! identity selections both fold `BitwiseAndI64` at either `Use`
//! position, so pair selection matches the bound as well as the kind and
//! position. When a rule needs shape
//! data beyond those — a further non-isolated effect relationship or
//! further operand roles — extend this struct rather than re-inlining kind
//! matches in compute.

use register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandConstraint,
    RegisterUnitId, TargetRegisterEnvironmentConstraintKeys,
};
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior, SaturatingCarrier, SelectedFunction,
    SelectedInstruction, SelectedInstructionKind, SelectedTerminator,
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
    /// The rewritten row stays as under [`Isolated`](Self::Isolated), but
    /// the admitted consumer's operands may carry `fixed_view` bindings —
    /// the register pins a pinned-operand form such as the x86-64 `div`
    /// realization requires. The rewrite rebuilds the operand list from
    /// the unpinned rewritten row, so every pin the folded form needed is
    /// deliberately discarded with it: a surviving operand's register
    /// keeps its other uses' own constraints and gains strictly more
    /// allocation freedom, and a dropped operand's pin dies with the
    /// operand. `tied_to` and `early_clobber` still reject — neither has a
    /// carried meaning once the operand list is rebuilt.
    BoundConsumerOperands,
    /// The rewritten row stays as under [`Isolated`](Self::Isolated), but
    /// the admitted consumer's operands may carry `early_clobber` marks as
    /// well as `fixed_view` pins — the write-before-read hazard a
    /// pinned-scratch realization such as the x86-64 `idiv` remainder form
    /// declares between its result and scratch outputs. Both bindings
    /// constrain only the folded operand list: the rewrite rebuilds the
    /// operands from the undecorated rewritten row, so every pin and
    /// early-clobber mark the folded form needed dies with its operand. A
    /// surviving operand's register keeps its other uses' own constraints
    /// and gains strictly more allocation freedom. `tied_to` still rejects
    /// — a tie would silently lose the shared-home requirement a surviving
    /// operand might have observed.
    BoundEarlyClobberConsumerOperands,
}

impl PairUnitEffects {
    /// Whether the constraint row's instruction-level unit traffic
    /// satisfies the declaration. Implicit *definitions* are the result
    /// channel and stay under `PairResultDisposition`.
    pub fn admits_row_units(self, row: &RegisterInstructionConstraint) -> bool {
        match self {
            Self::Isolated
            | Self::BoundConsumerOperands
            | Self::BoundEarlyClobberConsumerOperands => {
                row.implicit_uses.is_empty() && row.clobbers.is_empty()
            }
        }
    }

    /// Whether one constraint-row operand carries no unit binding.
    pub fn admits_operand(self, operand: &RegisterOperandConstraint) -> bool {
        match self {
            Self::Isolated
            | Self::BoundConsumerOperands
            | Self::BoundEarlyClobberConsumerOperands => {
                operand.fixed_view.is_none() && operand.tied_to.is_none() && !operand.early_clobber
            }
        }
    }

    /// Whether the admitted consumer's operand unit bindings survive the
    /// wholesale rebuild from the constraint row. Under
    /// [`Isolated`](Self::Isolated) no operand may carry a binding; under
    /// [`BoundConsumerOperands`](Self::BoundConsumerOperands) a `fixed_view`
    /// pin is admitted because the rewrite deliberately drops it with the
    /// pinned form; under
    /// [`BoundEarlyClobberConsumerOperands`](Self::BoundEarlyClobberConsumerOperands)
    /// an `early_clobber` mark is admitted for the same reason — the
    /// hazard it names exists only inside the dropped operand list.
    pub fn admits_consumer(self, consumer: &SelectedInstruction) -> bool {
        match self {
            Self::Isolated => consumer.operands.iter().all(|operand| {
                operand.fixed_view.is_none() && operand.tied_to.is_none() && !operand.early_clobber
            }),
            Self::BoundConsumerOperands => consumer
                .operands
                .iter()
                .all(|operand| operand.tied_to.is_none() && !operand.early_clobber),
            Self::BoundEarlyClobberConsumerOperands => consumer
                .operands
                .iter()
                .all(|operand| operand.tied_to.is_none()),
        }
    }

    /// Whether the eliminated producer's instruction record carries no unit
    /// traffic — implicit uses, definitions, clobbers, or operand bindings —
    /// that removing the instruction would silently drop. The requirement is
    /// the same under every variant: the eliminated instruction never
    /// survives in any form.
    pub fn admits_producer(self, producer: &SelectedInstruction) -> bool {
        match self {
            Self::Isolated
            | Self::BoundConsumerOperands
            | Self::BoundEarlyClobberConsumerOperands => {
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
    /// The consumer may architecturally fault — its encoded alternatives
    /// carry `MayArchitecturalFaultV1` — and the folded literal is exactly
    /// the value that makes every such fault unreachable. Declaring this
    /// surface attests that the rewrite replaces the consumer's trap
    /// surface wholesale because the admitted immediate discharges it:
    /// `EXACT_DIVIDE_ONE_COPY` folds a divisor of one, under which an
    /// unsigned divide can neither divide by zero nor overflow — provided
    /// the auxiliary `Use` operands the shape drops are provably zero, the
    /// operand-shape contract's own requirement — and
    /// `WRAPPING_REMAINDER_ONE_MATERIALIZE` folds a divisor of one, under
    /// which a signed or unsigned remainder can neither divide by zero nor
    /// overflow. The eliminated producer
    /// stays effect-isolated, as under [`Isolated`](Self::Isolated). The
    /// consumer declaration must be non-unit isolated — no memory, hosted
    /// trap, barrier, call, or cleanup surface — with alternatives that
    /// touch no memory, leave the stack unchanged, fall through, carry no
    /// implicit uses, define no unit the rewritten form does not also
    /// define, and encode only `NeverV1` or `MayArchitecturalFaultV1` trap
    /// behavior; clobbers are unrestricted since dropping them only
    /// narrows what may be destroyed. The rewritten form is fully
    /// effect-isolated.
    FaultDischargedByLiteral,
    /// The consumer may architecturally fault — its encoded alternatives
    /// carry `MayArchitecturalFaultV1` — and the fault is unreachable under
    /// the consumer's own carried obligation rather than under the folded
    /// literal alone: `WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE` folds
    /// a dividend literal of zero, under which the quotient is zero and
    /// cannot overflow, and `EXACT_DIVIDE_ZERO_DIVIDEND_MATERIALIZE` folds
    /// the same dividend literal of an unsigned exact divide, under which
    /// the quotient is exactly zero, while the nonzero-divisor obligation
    /// each kind carries as its accepted fact already excludes the only
    /// reachable fault — division by zero. Declaring
    /// this surface attests that the rewrite replaces the consumer's trap
    /// surface wholesale because every fault case was already unreachable:
    /// the literal fixes the quotient and the carried obligation fixes the
    /// divisor. Unlike
    /// [`FaultDischargedByLiteral`](Self::FaultDischargedByLiteral), the
    /// folded literal is not by itself the discharging value — a dividend
    /// of zero with an unproven divisor would still fault.
    ///
    /// The eliminated producer stays effect-isolated, as under
    /// [`Isolated`](Self::Isolated). The consumer declaration must be
    /// non-unit isolated — no memory, hosted trap, barrier, call, or
    /// cleanup surface — with alternatives that touch no memory, leave the
    /// stack unchanged, fall through, carry no implicit uses, define no
    /// unit the rewritten form does not also define, and encode only
    /// `NeverV1` or `MayArchitecturalFaultV1` trap behavior; clobbers are
    /// unrestricted since dropping them only narrows what may be
    /// destroyed. The rewritten form is fully effect-isolated.
    FaultDischargedByObligation,
    /// The consumer implicitly *defines* physical units the rewritten form
    /// does not define — the target condition state an aarch64
    /// flag-setting saturating add writes into `nzcv`, which the isolated
    /// `CopyI64` rewrite does not carry — and removing the consumer retires
    /// those definitions. Declaring this surface attests that the rewrite
    /// narrows the defined-unit surface deliberately: the fold is admitted
    /// only while every unit the consumer record defines is dead in the
    /// function — no instruction or terminator implicitly uses it — so no
    /// reader observes a stale unit once the defining instruction
    /// disappears. Unlike [`Isolated`](Self::Isolated), coverage of the
    /// consumer's implicit definitions by the rewritten form is not
    /// required; it is exactly what the relationship retires.
    ///
    /// The eliminated producer stays effect-isolated including every
    /// implicit unit it could have written, as under
    /// [`Isolated`](Self::Isolated). The consumer declaration must be
    /// non-unit isolated — no memory, hosted trap, barrier, call, or
    /// cleanup surface — with alternatives that touch no memory, leave the
    /// stack unchanged, fall through, and carry no implicit uses: a use
    /// the rewritten form does not carry would be unit state the rewrite
    /// silently stops observing. Implicit unit *definitions* are
    /// unrestricted at the declaration level — the record-level deadness
    /// gate decides whether retiring them is observable — and clobbers are
    /// unrestricted since dropping them only narrows what may be
    /// destroyed: the x86-64 saturating add's `rflags` clobber retires
    /// under this surface even while a flag-reading branch keeps `rflags`
    /// live. The rewritten form is fully effect-isolated.
    DeadConsumerUnitDefs,
}

impl PairMachineEffects {
    /// Whether the eliminated producer's catalog declaration is
    /// effect-isolated including every implicit unit it could have written.
    /// Every landed pair requires this surface: removing the producer must
    /// drop nothing machine-visible.
    pub fn admits_producer(self, declaration: &MachineEffectDeclaration) -> bool {
        match self {
            Self::Isolated
            | Self::IndexedPointerReadFold { .. }
            | Self::FaultDischargedByLiteral
            | Self::FaultDischargedByObligation
            | Self::DeadConsumerUnitDefs => {
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
    /// alternative's direct read over the same pointer and byte count. For
    /// [`FaultDischargedByLiteral`](Self::FaultDischargedByLiteral) the
    /// consumer keeps the isolated non-unit surface but may encode an
    /// architectural fault: the admitted literal is the value that
    /// discharges it. For
    /// [`FaultDischargedByObligation`](Self::FaultDischargedByObligation)
    /// the same may-fault surface is admitted because the consumer's own
    /// carried obligation — the nonzero divisor a remainder or an exact
    /// divide requires — already makes the fault unreachable; the literal
    /// fixes the folded value, not the divisor's definedness. For
    /// [`DeadConsumerUnitDefs`](Self::DeadConsumerUnitDefs) the consumer
    /// keeps the isolated non-unit surface with no implicit uses, while
    /// its implicit definitions need no rewritten coverage — retiring them
    /// is the point of the relationship, gated separately on the record's
    /// defined units being dead in the function.
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
            // The consumer keeps the isolated non-unit declaration surface
            // but may encode an architectural fault the literal discharges;
            // every other encoded dimension is the isolated contract.
            Self::FaultDischargedByLiteral => {
                isolated_declaration(declaration)
                    && declaration.alternatives.iter().all(|alternative| {
                        fault_discharged_alternative(alternative)
                            && alternative.encoded.implicit_unit_uses.is_empty()
                            && implicit_defs_covered(alternative, rewritten)
                    })
            }
            // The same may-fault surface admitted where the consumer's
            // carried obligation — not the folded literal — is what makes
            // the encoded fault unreachable.
            Self::FaultDischargedByObligation => {
                isolated_declaration(declaration)
                    && declaration.alternatives.iter().all(|alternative| {
                        fault_discharged_alternative(alternative)
                            && alternative.encoded.implicit_unit_uses.is_empty()
                            && implicit_defs_covered(alternative, rewritten)
                    })
            }
            // The consumer keeps the isolated non-unit declaration surface
            // and may define implicit units the rewritten form does not
            // carry — retiring them is the relationship's own point, so no
            // coverage requirement applies at the declaration level. The
            // deadness of each defined unit is the record-level gate
            // `admits_dead_consumer_defs` enforces. Implicit uses stay
            // forbidden: one the rewritten form does not carry would be
            // unit state the rewrite silently stops observing.
            Self::DeadConsumerUnitDefs => {
                isolated_declaration(declaration)
                    && declaration.alternatives.iter().all(|alternative| {
                        isolated_alternative(alternative)
                            && alternative.encoded.implicit_unit_uses.is_empty()
                    })
            }
        }
    }

    /// Whether the admitted consumer's instruction record retains the
    /// obligation this discharge relationship depends on.
    /// [`FaultDischargedByObligation`](Self::FaultDischargedByObligation)
    /// admits a consumer whose encoded fault is unreachable only because
    /// the obligation its kind names — the proven nonzero divisor a
    /// `WrappingRemainderI64` or an `ExactDivideU64` carries — is in the
    /// instruction's recorded proof custody: the folded literal alone does
    /// not discharge the fault, so an instruction record not retaining the
    /// obligation its kind declares cannot fold under this surface. Every
    /// other relationship needs no carried obligation.
    pub fn admits_consumer_obligation(self, consumer: &SelectedInstruction) -> bool {
        match self {
            Self::FaultDischargedByObligation => {
                let obligation = match consumer.kind {
                    SelectedInstructionKind::WrappingRemainderI64 { obligation, .. }
                    | SelectedInstructionKind::ExactDivideU64 { obligation, .. } => obligation,
                    _ => return false,
                };
                consumer.provenance.obligations.contains(&obligation)
            }
            Self::Isolated
            | Self::IndexedPointerReadFold { .. }
            | Self::FaultDischargedByLiteral
            | Self::DeadConsumerUnitDefs => true,
        }
    }

    /// Whether the admitted consumer's instruction record retires only
    /// unobserved unit state. Under
    /// [`DeadConsumerUnitDefs`](Self::DeadConsumerUnitDefs) the record must
    /// declare no implicit unit *uses* — one the rewritten form does not
    /// carry would silently stop being observed — and every unit it
    /// *defines* must be dead in `function`: no instruction or terminator
    /// may implicitly use it, or its readers would observe a stale unit
    /// once the defining instruction disappears. A use textually before
    /// the definition still counts — it reads the unit on a later loop
    /// iteration — so the whole-function scan is the only sound order.
    /// Every other relationship keeps its definitions by coverage and
    /// needs no record-level deadness gate.
    pub fn admits_dead_consumer_defs(
        self,
        consumer: &SelectedInstruction,
        function: &SelectedFunction,
    ) -> bool {
        match self {
            Self::DeadConsumerUnitDefs => {
                consumer.implicit_uses.is_empty()
                    && consumer
                        .implicit_defs
                        .iter()
                        .all(|unit| !implicit_unit_used(function, *unit))
            }
            Self::Isolated
            | Self::IndexedPointerReadFold { .. }
            | Self::FaultDischargedByLiteral
            | Self::FaultDischargedByObligation => true,
        }
    }

    /// Whether the rewritten form's catalog declaration satisfies the pair's
    /// declared shape. [`Isolated`](Self::Isolated) requires an
    /// effect-isolated form with no implicit unit uses or clobbers beyond
    /// its result channel; [`IndexedPointerReadFold`](Self::IndexedPointerReadFold)
    /// requires the plain pointer-read form whose alternatives all read
    /// memory through a pointer operand and byte offset, fall through, leave
    /// the stack unchanged, and declare no implicit uses or clobbers;
    /// [`FaultDischargedByLiteral`](Self::FaultDischargedByLiteral) and
    /// [`FaultDischargedByObligation`](Self::FaultDischargedByObligation)
    /// require the same fully isolated surface as
    /// [`Isolated`](Self::Isolated) because the consumer's fault does not
    /// survive the fold; [`DeadConsumerUnitDefs`](Self::DeadConsumerUnitDefs)
    /// requires it because the consumer's dead definitions and clobbers
    /// must not reappear on the rewritten form.
    pub fn admits_rewritten(self, declaration: &MachineEffectDeclaration) -> bool {
        match self {
            Self::Isolated | Self::DeadConsumerUnitDefs => {
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
            // The folded form is fully isolated: the discharged fault does
            // not reappear anywhere in the rewrite.
            Self::FaultDischargedByLiteral | Self::FaultDischargedByObligation => {
                isolated_declaration(declaration)
                    && declaration.alternatives.iter().all(|alternative| {
                        isolated_alternative(alternative)
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

/// The encoded surface a fault-discharging fold's consumer may carry: the
/// isolated contract except that `MayArchitecturalFaultV1` is admitted —
/// under the divisor-one grammars the folded literal is the value that
/// makes the fault unreachable, and under the zero-dividend grammars the
/// consumer's carried nonzero-divisor obligation makes it unreachable, so
/// either way the rewrite may retire the surface wholesale.
fn fault_discharged_alternative(alternative: &MachineAlternative) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && matches!(
            encoded.trap,
            MachineEncodedTrapBehavior::NeverV1
                | MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        )
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
}

/// Whether any instruction or terminator in `function` implicitly uses
/// `unit` — the observation channel a removed definition would leave stale.
/// A use anywhere in the function can observe the unit: a use textually
/// before the definition still reads it on a later loop iteration, and a
/// terminator's uses include the function's live-out unit state.
fn implicit_unit_used(function: &SelectedFunction, unit: RegisterUnitId) -> bool {
    function.blocks.iter().any(|block| {
        block
            .instructions
            .iter()
            .chain(match &block.terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. }
                | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
                | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
                | SelectedTerminator::Jump { instruction, .. }
                | SelectedTerminator::Return { instruction, .. }
                | SelectedTerminator::HostedExitProcess { instruction, .. } => {
                    std::iter::once(instruction)
                }
            })
            .any(|instruction| instruction.implicit_uses.contains(&unit))
    })
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
    /// the immediate form encodes `surviving <op> literal` — and the
    /// xor-zero and wrapping-add-zero identity copies bind `surviving`
    /// unchanged, which `0 ^ x` and `x ^ 0` — likewise `0 + x` and
    /// `x + 0` — share — so only an operation exact under commutation
    /// may declare it. Exact addition, bitwise xor, and wrapping addition
    /// commute; subtraction and comparison fix the literal's role, so
    /// only those families declare a left-literal pair.
    BinaryLeftLiteral,
    /// Unary consumer: the literal victim is the sole `Use` operand (operand
    /// index 0). The rewritten instruction consumes no register input — the
    /// fold recomputes the consumer's constant output directly, as in the
    /// extension-elimination and copy-materialization rules whose rewritten
    /// form is a `MaterializeI64`.
    UnaryLiteral,
    /// Binary right-literal consumer whose operand list continues past its
    /// scalar `Def` result: the literal victim is the operand-1 `Use`,
    /// operand 0 is the surviving `Use`, operand 2 is the `Def` result, and
    /// every operand past the result must be a `Use` the fold drops.
    /// Declaring this shape attests each dropped operand is semantically
    /// inert once the literal folds: its register must be defined in the
    /// same function only by `MaterializeI64` instructions producing
    /// `Unsigned(0)` — the zeroed high-half input an x86-64 `div`
    /// realization requires, which a divide by one leaves dead. An operand
    /// that is not a dropped `Use` — a `Def`, or any operand at position 0
    /// through 2 outside this grammar — rejects.
    BinaryRightLiteralAuxiliaryUses,
    /// Binary right-literal consumer whose folded result is a constant of
    /// the literal alone: the literal victim is the operand-1 `Use`,
    /// operand 0 is a `Use` the fold drops because the constant result
    /// never reads it, operand 2 is the `Def` result, and every operand
    /// past the result is a `Def` scratch output the fold drops. Declaring
    /// this shape attests the rewritten constant is exact regardless of
    /// the dropped operand-0 value — `x % 1` is zero for every `x` — and
    /// that each dropped `Def` register occurs nowhere else in the
    /// function: a scratch output another instruction read or defined
    /// would leave a use of a register the rewrite stopped defining. An
    /// operand that is not in its declared position and access — a `Use`
    /// past the result, or any operand at positions 0 through 2 outside
    /// this grammar — rejects.
    BinaryRightLiteralConstantResult,
    /// Binary left-literal consumer whose folded result is a constant of
    /// the literal alone: the literal victim is the operand-0 `Use`,
    /// operand 1 is a `Use` the fold drops because the constant result
    /// never reads it, operand 2 is the `Def` result, and every operand
    /// past the result is a `Def` scratch output the fold drops under the
    /// same occurrence-free custody
    /// [`BinaryRightLiteralConstantResult`](Self::BinaryRightLiteralConstantResult)
    /// declares. Unlike [`BinaryLeftLiteral`](Self::BinaryLeftLiteral),
    /// declaring this shape attests nothing about commutation: the
    /// rewritten constant must be exact regardless of the dropped
    /// operand-1 value — `0 & x` is zero for every `x`, because zero is
    /// the bitwise-and annihilator, and `0 % x` is zero for every `x`
    /// the remainder's proven nonzero divisor admits — so only an
    /// operation whose operand-0 literal alone fixes the result may
    /// declare it. An operand that is
    /// not in its declared position and access — a `Use` past the result,
    /// or any operand at positions 0 through 2 outside this grammar —
    /// rejects.
    BinaryLeftLiteralConstantResult,
    /// Binary left-literal consumer whose folded result is a constant of
    /// the literal alone and whose operand list continues past its scalar
    /// `Def` result: the literal victim is the operand-0 `Use`, operand 1
    /// is a `Use` the fold drops because the constant result never reads
    /// it, operand 2 is the `Def` result, and every operand past the
    /// result is a `Use` the fold drops under the same provenance custody
    /// [`BinaryRightLiteralAuxiliaryUses`](Self::BinaryRightLiteralAuxiliaryUses)
    /// declares — each dropped register must be defined in the same
    /// function only by `MaterializeI64` instructions producing
    /// `Unsigned(0)`. Declaring this shape attests the operand-0 literal
    /// alone fixes the result *and* that every auxiliary `Use` is an
    /// operand the realization reads but the constant result leaves
    /// inert: under `0 / x` the operand-0 literal is only the low half of
    /// the dividend an x86-64 `div` reads — the auxiliary high-half
    /// `Use` must be provably zero or the dropped operand would carry a
    /// value the folded form silently stopped observing. An operand that
    /// is not in its declared position and access — a `Def` past the
    /// result, or any operand at positions 0 through 2 outside this
    /// grammar — rejects.
    BinaryLeftLiteralConstantResultAuxiliaryUses,
    /// Binary right-literal consumer whose operand list continues past its
    /// scalar `Def` result with scratch outputs: the literal victim is the
    /// operand-1 `Use`, operand 0 is the surviving `Use` the rewritten row
    /// binds, operand 2 is the `Def` result, and every operand past the
    /// result is a `Def` scratch output the fold drops — the bound scratch
    /// a clamped saturating realization computes its saturation bound
    /// through. Declaring this shape attests the surviving operand alone
    /// produces the folded result — `x +| 0` is `x` inside the carrier's
    /// bounds — and that each dropped `Def` register occurs nowhere else
    /// in the function under the same occurrence-free custody
    /// [`BinaryRightLiteralConstantResult`](Self::BinaryRightLiteralConstantResult)
    /// declares: a scratch output another instruction read or defined
    /// would leave a use of a register the rewrite stopped defining. An
    /// operand that is not in its declared position and access — a `Use`
    /// past the result, or any operand at positions 0 through 2 outside
    /// this grammar — rejects.
    BinaryRightLiteralScratchDefs,
    /// Binary left-literal consumer whose operand list continues past its
    /// scalar `Def` result with scratch outputs: the literal victim is the
    /// operand-0 `Use`, operand 1 is the surviving `Use` the rewritten row
    /// binds, operand 2 is the `Def` result, and every operand past the
    /// result is a `Def` scratch output the fold drops under the same
    /// occurrence-free custody
    /// [`BinaryRightLiteralScratchDefs`](Self::BinaryRightLiteralScratchDefs)
    /// declares. Like
    /// [`BinaryLeftLiteral`](Self::BinaryLeftLiteral), declaring this
    /// shape attests that the fold computes the same value under operand
    /// exchange — `0 +| x` and `x +| 0` are both `x` inside the carrier's
    /// bounds — so only an operation exact under commutation may declare
    /// it. An operand that is not in its declared position and access — a
    /// `Use` past the result, or any operand at positions 0 through 2
    /// outside this grammar — rejects.
    BinaryLeftLiteralScratchDefs,
}

/// The literal values a pair's fold admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairImmediateBound {
    /// Any unsigned literal up to the encoding limit the rewritten form's
    /// immediate field admits — the folded value only needs to fit.
    Encoding(u64),
    /// Exactly one literal value: the fold is an algebraic identity whose
    /// correctness depends on the value itself — the divide-by-one divisor
    /// — not merely on fitting an immediate field.
    Exactly(u64),
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
    immediate_bound: PairImmediateBound,
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
        immediate_bound: PairImmediateBound::Encoding(4095),
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
        immediate_bound: PairImmediateBound::Encoding(4095),
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::Isolated,
        machine_effects: PairMachineEffects::Isolated,
    };
    pub const COMPARE_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::CompareI64,
        rewritten: MachineSemanticKind::CompareI64Immediate,
        operand_shape: PairOperandShape::BinaryRightLiteral,
        immediate_bound: PairImmediateBound::Encoding(4095),
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
        immediate_bound: PairImmediateBound::Encoding(u64::MAX),
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
            immediate_bound: PairImmediateBound::Encoding(4095),
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
        immediate_bound: PairImmediateBound::Encoding(4095),
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::Isolated,
        machine_effects: PairMachineEffects::Isolated,
    };

    /// Eliminate `MaterializeI64` feeding the divisor operand of
    /// `ExactDivideU64` when the literal is exactly one: an unsigned divide
    /// by one returns the dividend, so the rewrite is a `CopyI64` of the
    /// surviving operand-0 register. The declared surface carries the
    /// dimensions a `div` realization brings: the consumer may encode an
    /// architectural fault — divide by zero or quotient overflow — which
    /// the folded divisor of one discharges under
    /// [`FaultDischargedByLiteral`](PairMachineEffects::FaultDischargedByLiteral),
    /// its operands may carry the register pins the pinned-operand form
    /// requires under
    /// [`BoundConsumerOperands`](PairUnitEffects::BoundConsumerOperands),
    /// and every `Use` operand past the operand-2 `Def` result — the
    /// zeroed high-half scratch a realization like x86-64 `div` reads — is
    /// dropped under
    /// [`BinaryRightLiteralAuxiliaryUses`](PairOperandShape::BinaryRightLiteralAuxiliaryUses),
    /// which requires each such register to be defined only by zero
    /// materializations. Targets whose divide row carries no auxiliary
    /// `Use` — aarch64's `udiv` — admit the same rule with an empty
    /// auxiliary tail.
    pub const EXACT_DIVIDE_ONE_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::ExactDivideU64,
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BinaryRightLiteralAuxiliaryUses,
            immediate_bound: PairImmediateBound::Exactly(1),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BoundConsumerOperands,
            machine_effects: PairMachineEffects::FaultDischargedByLiteral,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FaultDischargedByLiteral
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(1)),
            "the fault discharge holds only for the divisor literal one"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the dividend operand of
    /// `ExactDivideU64` when the literal is exactly zero: an unsigned
    /// divide of a zero dividend is always zero — `0 / x` is `0` for
    /// every `x` — so the rewrite is a `MaterializeI64` of the constant
    /// zero at the consumer's result register. The declared surface
    /// carries the dimensions a `div` realization brings, with one
    /// distinction from the divisor-one fold: the folded dividend is not
    /// the value that discharges the consumer's encoded architectural
    /// fault. A quotient of zero can never overflow, but the
    /// divide-by-zero case the encoding could still name is unreachable
    /// only because the `ExactDivideU64` kind carries its proven nonzero
    /// divisor as an accepted obligation — the fold declares
    /// [`FaultDischargedByObligation`](PairMachineEffects::FaultDischargedByObligation)
    /// so the descriptor never claims the literal did the obligation's
    /// work. The operands may carry the register pins the pinned-operand
    /// realization requires under
    /// [`BoundConsumerOperands`](PairUnitEffects::BoundConsumerOperands),
    /// the operand-1 divisor `Use` is dropped with the form because the
    /// constant result never reads it, and every `Use` operand past the
    /// operand-2 `Def` result — the zeroed high-half input an x86-64
    /// `div` realization reads as the dividend's upper half — is dropped
    /// under
    /// [`BinaryLeftLiteralConstantResultAuxiliaryUses`](PairOperandShape::BinaryLeftLiteralConstantResultAuxiliaryUses),
    /// which requires each such register to be defined only by zero
    /// materializations: a literal of zero fixes the low dividend half,
    /// so the fold is exact only when every auxiliary half is provably
    /// zero as well. Targets whose divide row carries no auxiliary `Use`
    /// — aarch64's `udiv` — admit the same rule with an empty auxiliary
    /// tail. The family shares its consumer kind with the divisor-one
    /// fold; the grammars stay disjoint on the folded literal's operand
    /// position.
    pub const EXACT_DIVIDE_ZERO_DIVIDEND_MATERIALIZE: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::ExactDivideU64,
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BinaryLeftLiteralConstantResultAuxiliaryUses,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BoundConsumerOperands,
            machine_effects: PairMachineEffects::FaultDischargedByObligation,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FaultDischargedByObligation
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the obligation discharge holds only for the dividend literal zero"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the divisor operand of
    /// `WrappingRemainderI64` when the literal is exactly one: a wrapping
    /// remainder by one is always zero — signed or unsigned, `x % 1` is
    /// `0` and `i64::MIN % 1` cannot overflow — so the rewrite is a
    /// `MaterializeI64` of the constant zero at the consumer's result
    /// register. The declared surface carries the dimensions an
    /// `idiv`-class realization brings: the consumer may encode an
    /// architectural fault — divide by zero or quotient overflow — which
    /// the folded divisor of one discharges under
    /// [`FaultDischargedByLiteral`](PairMachineEffects::FaultDischargedByLiteral),
    /// its operands may carry the register pins and early-clobber marks a
    /// pinned-scratch realization requires under
    /// [`BoundEarlyClobberConsumerOperands`](PairUnitEffects::BoundEarlyClobberConsumerOperands),
    /// and every operand past the operand-2 `Def` result — the dead
    /// quotient scratch an x86-64 `idiv` realization writes — is a `Def`
    /// the fold drops under
    /// [`BinaryRightLiteralConstantResult`](PairOperandShape::BinaryRightLiteralConstantResult),
    /// which requires each such register to occur nowhere else in the
    /// function. The operand-0 dividend `Use` is dropped with the form:
    /// the constant result never reads it. Targets whose remainder row
    /// carries no scratch `Def` — aarch64's `udiv`/`msub` realization —
    /// admit the same rule with an empty scratch tail.
    pub const WRAPPING_REMAINDER_ONE_MATERIALIZE: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::WrappingRemainderI64,
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BinaryRightLiteralConstantResult,
            immediate_bound: PairImmediateBound::Exactly(1),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BoundEarlyClobberConsumerOperands,
            machine_effects: PairMachineEffects::FaultDischargedByLiteral,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FaultDischargedByLiteral
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(1)),
            "the fault discharge holds only for the divisor literal one"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the dividend operand of
    /// `WrappingRemainderI64` when the literal is exactly zero: a
    /// remainder of a zero dividend is always zero — `0 % x` is `0` for
    /// every `x` — so the rewrite is a `MaterializeI64` of the constant
    /// zero at the consumer's result register. The declared surface
    /// carries the dimensions an `idiv`-class realization brings, with
    /// one distinction from the divisor-one fold: the folded dividend is
    /// not the value that discharges the consumer's encoded
    /// architectural fault. A quotient of zero can never overflow, but
    /// the divide-by-zero case the encoding could still name is
    /// unreachable only because the `WrappingRemainderI64` kind carries
    /// its proven nonzero divisor as an accepted obligation — the fold
    /// declares
    /// [`FaultDischargedByObligation`](PairMachineEffects::FaultDischargedByObligation)
    /// so the descriptor never claims the literal did the obligation's
    /// work. The operands may carry the register pins and early-clobber
    /// marks a pinned-scratch realization requires under
    /// [`BoundEarlyClobberConsumerOperands`](PairUnitEffects::BoundEarlyClobberConsumerOperands),
    /// the operand-1 divisor `Use` is dropped with the form because the
    /// constant result never reads it, and every operand past the
    /// operand-2 `Def` result — the dead quotient scratch an x86-64
    /// `idiv` realization writes — is a `Def` the fold drops under
    /// [`BinaryLeftLiteralConstantResult`](PairOperandShape::BinaryLeftLiteralConstantResult),
    /// which requires each such register to occur nowhere else in the
    /// function. Targets whose remainder row carries no scratch `Def` —
    /// aarch64's `udiv`/`msub` realization — admit the same rule with an
    /// empty scratch tail. The family shares its consumer kind with the
    /// divisor-one fold; the grammars stay disjoint on the folded
    /// literal's operand position.
    pub const WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::WrappingRemainderI64,
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BinaryLeftLiteralConstantResult,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BoundEarlyClobberConsumerOperands,
            machine_effects: PairMachineEffects::FaultDischargedByObligation,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FaultDischargedByObligation
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the obligation discharge holds only for the dividend literal zero"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `BitwiseAndI64` when the literal is exactly zero: zero is the
    /// bitwise-and annihilator — `x & 0` is `0` for every `x` — so the
    /// rewrite is a `MaterializeI64` of the constant zero at the
    /// consumer's result register. Both forms are effect-isolated on every
    /// target — the consumer's flag clobber, where one is declared, dies
    /// with the folded form — and neither side pins or binds an operand,
    /// so the ordinary
    /// [`Isolated`](PairMachineEffects::Isolated) and
    /// [`Isolated`](PairUnitEffects::Isolated) surfaces apply. The
    /// operand-0 `Use` is dropped because the constant result never reads
    /// it, under the
    /// [`BinaryRightLiteralConstantResult`](PairOperandShape::BinaryRightLiteralConstantResult)
    /// grammar's custody: every `Def` operand past the result must occur
    /// nowhere else in the function.
    pub const BITWISE_AND_ZERO_MATERIALIZE: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::BitwiseAndI64,
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BinaryRightLiteralConstantResult,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::Isolated,
            machine_effects: PairMachineEffects::Isolated,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the annihilator fold holds only for the literal zero"
        );
        rule
    };

    /// The left-operand annihilator fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of `BitwiseAndI64` when the literal is exactly
    /// zero — `0 & x` is `0` for every `x`. The constant result is a
    /// function of the literal alone, so no commutativity attestation is
    /// needed: under
    /// [`BinaryLeftLiteralConstantResult`](PairOperandShape::BinaryLeftLiteralConstantResult)
    /// the dropped operand-1 `Use` is never read and every `Def` operand
    /// past the result drops under the same occurrence-free custody the
    /// right grammar declares. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const BITWISE_AND_ZERO_LEFT_MATERIALIZE: Self = Self {
        operand_shape: PairOperandShape::BinaryLeftLiteralConstantResult,
        ..Self::BITWISE_AND_ZERO_MATERIALIZE
    };

    /// The two bitwise-and annihilator rules, one per literal `Use`
    /// position.
    pub const BITWISE_AND_ZERO_FOLDS: [Self; 2] = [
        Self::BITWISE_AND_ZERO_MATERIALIZE,
        Self::BITWISE_AND_ZERO_LEFT_MATERIALIZE,
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `BitwiseXorI64` when the literal is exactly zero: zero is the
    /// bitwise-xor identity element — `x ^ 0` is `x` for every `x` — so
    /// the rewrite is a `CopyI64` of the surviving operand-0 register at
    /// the consumer's result register. Both forms are effect-isolated on
    /// every target — the consumer's flag clobber, where one is declared,
    /// dies with the folded form — and neither side pins or binds an
    /// operand, so the ordinary
    /// [`Isolated`](PairMachineEffects::Isolated) and
    /// [`Isolated`](PairUnitEffects::Isolated) surfaces apply. The
    /// operand-0 `Use` survives under the ordinary
    /// [`BinaryRightLiteral`](PairOperandShape::BinaryRightLiteral)
    /// grammar: the rewritten row binds it as its `Use` operand.
    pub const BITWISE_XOR_ZERO_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::BitwiseXorI64,
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BinaryRightLiteral,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::Isolated,
            machine_effects: PairMachineEffects::Isolated,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the identity fold holds only for the literal zero"
        );
        rule
    };

    /// The left-operand identity fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of `BitwiseXorI64` when the literal is exactly
    /// zero — `0 ^ x` is `x` for every `x`. Bitwise xor commutes, so the
    /// `CopyI64` of the surviving operand-1 register computes the same
    /// value `x ^ 0` does; the
    /// [`BinaryLeftLiteral`](PairOperandShape::BinaryLeftLiteral) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const BITWISE_XOR_ZERO_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BinaryLeftLiteral,
        ..Self::BITWISE_XOR_ZERO_COPY
    };

    /// The two bitwise-xor identity rules, one per literal `Use` position.
    pub const BITWISE_XOR_ZERO_COPIES: [Self; 2] = [
        Self::BITWISE_XOR_ZERO_COPY,
        Self::BITWISE_XOR_ZERO_LEFT_COPY,
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `WrappingAddI64` when the literal is exactly zero: zero is the
    /// additive identity under modulo-2^64 wrap — `x + 0` is `x` for
    /// every `x` — so the rewrite is a `CopyI64` of the surviving
    /// operand-0 register at the consumer's result register. Both forms
    /// are effect-isolated on every target — the wrapping-add consumer
    /// binds the flag-transparent add row, so unlike the bitwise forms
    /// there is no flag clobber to retire — and neither side pins or
    /// binds an operand, so the ordinary
    /// [`Isolated`](PairMachineEffects::Isolated) and
    /// [`Isolated`](PairUnitEffects::Isolated) surfaces apply. The
    /// operand-0 `Use` survives under the ordinary
    /// [`BinaryRightLiteral`](PairOperandShape::BinaryRightLiteral)
    /// grammar: the rewritten row binds it as its `Use` operand.
    pub const WRAPPING_ADD_ZERO_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::WrappingAddI64,
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BinaryRightLiteral,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::Isolated,
            machine_effects: PairMachineEffects::Isolated,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the identity fold holds only for the literal zero"
        );
        rule
    };

    /// The left-operand identity fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of `WrappingAddI64` when the literal is exactly
    /// zero — `0 + x` is `x` for every `x`. Wrapping addition commutes,
    /// so the `CopyI64` of the surviving operand-1 register computes the
    /// same value `x + 0` does; the
    /// [`BinaryLeftLiteral`](PairOperandShape::BinaryLeftLiteral) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const WRAPPING_ADD_ZERO_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BinaryLeftLiteral,
        ..Self::WRAPPING_ADD_ZERO_COPY
    };

    /// The two wrapping-add identity rules, one per literal `Use` position.
    pub const WRAPPING_ADD_ZERO_COPIES: [Self; 2] = [
        Self::WRAPPING_ADD_ZERO_COPY,
        Self::WRAPPING_ADD_ZERO_LEFT_COPY,
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `BitwiseAndI64` when the literal is all ones (`u64::MAX`): all-ones
    /// is the bitwise-and identity element — `x & MAX` is `x` for every
    /// `x` — so the rewrite is a `CopyI64` of the surviving operand-0
    /// register at the consumer's result register. Both forms are
    /// effect-isolated on every target — the consumer's flag clobber,
    /// where one is declared, dies with the folded form — and neither
    /// side pins or binds an operand, so the ordinary
    /// [`Isolated`](PairMachineEffects::Isolated) and
    /// [`Isolated`](PairUnitEffects::Isolated) surfaces apply. The
    /// operand-0 `Use` survives under the ordinary
    /// [`BinaryRightLiteral`](PairOperandShape::BinaryRightLiteral)
    /// grammar: the rewritten row binds it as its `Use` operand. The
    /// family shares its consumer kind and operand grammar with the
    /// and-zero annihilator rules; the two families stay disjoint on the
    /// literal's value, which the producer's admission now carries as
    /// part of pair selection.
    pub const BITWISE_AND_ONES_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::BitwiseAndI64,
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BinaryRightLiteral,
            immediate_bound: PairImmediateBound::Exactly(u64::MAX),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::Isolated,
            machine_effects: PairMachineEffects::Isolated,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(u64::MAX)),
            "the identity fold holds only for the all-ones literal"
        );
        rule
    };

    /// The left-operand identity fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of `BitwiseAndI64` when the literal is all ones —
    /// `MAX & x` is `x` for every `x`. Bitwise and commutes, so the
    /// `CopyI64` of the surviving operand-1 register computes the same
    /// value `x & MAX` does; the
    /// [`BinaryLeftLiteral`](PairOperandShape::BinaryLeftLiteral) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const BITWISE_AND_ONES_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BinaryLeftLiteral,
        ..Self::BITWISE_AND_ONES_COPY
    };

    /// The two bitwise-and identity rules, one per literal `Use` position.
    pub const BITWISE_AND_ONES_COPIES: [Self; 2] = [
        Self::BITWISE_AND_ONES_COPY,
        Self::BITWISE_AND_ONES_LEFT_COPY,
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `SaturatingAdd` on the u64 carrier when the literal is exactly
    /// zero: zero is the additive identity under unsigned saturating
    /// addition — `x +| 0` is `x` for every `x`, already inside the
    /// carrier's bounds — so the rewrite is a `CopyI64` of the surviving
    /// operand-0 register at the consumer's result register. This is the
    /// first family whose consumer carries an implicit unit *definition*
    /// the rewrite retires: the three-operand u64 saturating-add row
    /// defines `nzcv` on aarch64 — its flag-setting `adds` realization —
    /// while the isolated `CopyI64` defines nothing. Under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DeadConsumerUnitDefs)
    /// that definition may retire only while it is dead in the function —
    /// a conditional branch reading `nzcv` would go stale — and the
    /// consumer's clobbers retire wholesale, as the x86-64 row's `rflags`
    /// clobber does. The consumer's operands may carry the `early_clobber`
    /// mark the x86-64 saturating realization declares on its result —
    /// the hazard it names exists only inside the dropped operand list —
    /// under
    /// [`BoundEarlyClobberConsumerOperands`](PairUnitEffects::BoundEarlyClobberConsumerOperands).
    /// The operand-0 `Use` survives under the ordinary
    /// [`BinaryRightLiteral`](PairOperandShape::BinaryRightLiteral)
    /// grammar: the rewritten row binds it as its `Use` operand. The u64
    /// row carries no operand past its `Def` result; the narrower and
    /// signed carriers bind the clamped row whose bound scratch the
    /// scratch-`Def` grammars below drop.
    pub const SATURATING_ADD_ZERO_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingAdd(SaturatingCarrier::U64),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BinaryRightLiteral,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BoundEarlyClobberConsumerOperands,
            machine_effects: PairMachineEffects::DeadConsumerUnitDefs,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the identity fold holds only for the literal zero"
        );
        rule
    };

    /// The left-operand identity fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of the u64 saturating add when the literal is
    /// exactly zero — `0 +| x` is `x` for every `x`. Unsigned saturating
    /// addition commutes, so the `CopyI64` of the surviving operand-1
    /// register computes the same value `x +| 0` does; the
    /// [`BinaryLeftLiteral`](PairOperandShape::BinaryLeftLiteral) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const SATURATING_ADD_ZERO_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BinaryLeftLiteral,
        ..Self::SATURATING_ADD_ZERO_COPY
    };

    /// The right-operand identity fold for a clamped-carrier saturating
    /// add: `MaterializeI64` feeding the operand-1 `Use` of
    /// `SaturatingAdd` on `carrier` — every carrier but u64 — when the
    /// literal is exactly zero. `x +| 0` is `x` inside the carrier's
    /// bounds under signed or unsigned saturation, so the rewrite is a
    /// `CopyI64` of the surviving operand-0 register. Unlike the u64
    /// row, the clamped row's operand list continues past its `Def`
    /// result with an early-clobber bound scratch — a `Def` output the
    /// realization writes and nothing else may observe — so the rule
    /// declares
    /// [`BinaryRightLiteralScratchDefs`](PairOperandShape::BinaryRightLiteralScratchDefs):
    /// each dropped `Def` register must occur nowhere else in the
    /// function. The unit surface is the family's own: the aarch64
    /// clamped row still defines `nzcv` — retired under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DeadConsumerUnitDefs)
    /// only while dead in the function — both targets mark the dropped
    /// result and scratch `early_clobber`, admitted under
    /// [`BoundEarlyClobberConsumerOperands`](PairUnitEffects::BoundEarlyClobberConsumerOperands),
    /// and the x86-64 row's `rflags` clobber retires unconditionally.
    const fn saturating_add_zero_clamped(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingAdd(carrier),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BinaryRightLiteralScratchDefs,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BoundEarlyClobberConsumerOperands,
            machine_effects: PairMachineEffects::DeadConsumerUnitDefs,
        };
        assert!(
            !matches!(carrier, SaturatingCarrier::U64)
                && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the clamped identity fold holds only for a non-u64 carrier's zero literal"
        );
        rule
    }

    /// The left-operand identity fold for a clamped-carrier saturating
    /// add: `0 +| x` is `x` for every `x`. Saturating addition commutes
    /// under every carrier — its saturation bounds are symmetric around
    /// the operation — so the `CopyI64` of the surviving operand-1
    /// register computes the same value `x +| 0` does; the
    /// [`BinaryLeftLiteralScratchDefs`](PairOperandShape::BinaryLeftLiteralScratchDefs)
    /// grammar attests that commutation and drops the same bound-scratch
    /// `Def` tail under occurrence-free custody.
    const fn saturating_add_zero_clamped_left(carrier: SaturatingCarrier) -> Self {
        Self {
            operand_shape: PairOperandShape::BinaryLeftLiteralScratchDefs,
            ..Self::saturating_add_zero_clamped(carrier)
        }
    }

    /// The saturating-add identity rules: one pair per literal `Use`
    /// position for the u64 carrier's three-operand row, and one pair
    /// per position for each carrier binding the clamped row — every
    /// other carrier — whose bound scratch `Def` drops under the
    /// scratch-defs grammar.
    pub const SATURATING_ADD_ZERO_COPIES: [Self; 16] = [
        Self::SATURATING_ADD_ZERO_COPY,
        Self::SATURATING_ADD_ZERO_LEFT_COPY,
        Self::saturating_add_zero_clamped(SaturatingCarrier::I8),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I8),
        Self::saturating_add_zero_clamped(SaturatingCarrier::I16),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I16),
        Self::saturating_add_zero_clamped(SaturatingCarrier::I32),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I32),
        Self::saturating_add_zero_clamped(SaturatingCarrier::I64),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I64),
        Self::saturating_add_zero_clamped(SaturatingCarrier::U8),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::U8),
        Self::saturating_add_zero_clamped(SaturatingCarrier::U16),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::U16),
        Self::saturating_add_zero_clamped(SaturatingCarrier::U32),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::U32),
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `SaturatingSubtract` on an unsigned carrier when the literal is
    /// exactly zero: zero is the right identity under saturating
    /// subtraction — `x -| 0` is `x` for every `x`, already inside the
    /// carrier's bounds — so the rewrite is a `CopyI64` of the surviving
    /// operand-0 register at the consumer's result register. The grammar
    /// is deliberately asymmetric: `0 -| x` is `-x` clamped to the
    /// carrier's bounds, not `x`, so this family declares no left-literal
    /// pair and a literal recorded at operand 0 names no admitted
    /// grammar. Like the saturating add, the consumer carries an implicit
    /// unit *definition* the rewrite retires: the three-operand unsigned
    /// saturating-subtract row defines `nzcv` on aarch64 — its
    /// flag-setting `subs` realization — while the isolated `CopyI64`
    /// defines nothing. Under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DeadConsumerUnitDefs)
    /// that definition may retire only while it is dead in the function —
    /// a conditional branch reading `nzcv` would go stale — and the
    /// consumer's clobbers retire wholesale, as the x86-64 row's `rflags`
    /// clobber does. The consumer's operands may carry the `early_clobber`
    /// mark the x86-64 saturating realization declares on its result —
    /// the hazard it names exists only inside the dropped operand list —
    /// under
    /// [`BoundEarlyClobberConsumerOperands`](PairUnitEffects::BoundEarlyClobberConsumerOperands).
    /// The operand-0 `Use` survives under the ordinary
    /// [`BinaryRightLiteral`](PairOperandShape::BinaryRightLiteral)
    /// grammar: the rewritten row binds it as its `Use` operand.
    const fn saturating_subtract_zero_unsigned(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BinaryRightLiteral,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BoundEarlyClobberConsumerOperands,
            machine_effects: PairMachineEffects::DeadConsumerUnitDefs,
        };
        assert!(
            !carrier.is_signed() && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the unsigned identity fold holds only for an unsigned carrier's zero literal"
        );
        rule
    }

    /// The right-operand identity fold for a clamped-carrier saturating
    /// subtract: `MaterializeI64` feeding the operand-1 `Use` of
    /// `SaturatingSubtract` on `carrier` — every signed carrier — when
    /// the literal is exactly zero. `x -| 0` is `x` inside the carrier's
    /// bounds under signed saturation, so the rewrite is a `CopyI64` of
    /// the surviving operand-0 register; the asymmetric grammar of the
    /// unsigned rules applies here too. Unlike the unsigned row, the
    /// clamped row's operand list continues past its `Def` result with
    /// an early-clobber bound scratch — a `Def` output the realization
    /// writes and nothing else may observe — so the rule declares
    /// [`BinaryRightLiteralScratchDefs`](PairOperandShape::BinaryRightLiteralScratchDefs):
    /// each dropped `Def` register must occur nowhere else in the
    /// function. The unit surface is the family's own: the aarch64
    /// clamped row still defines `nzcv` — retired under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DeadConsumerUnitDefs)
    /// only while dead in the function — both targets mark the dropped
    /// result and scratch `early_clobber`, admitted under
    /// [`BoundEarlyClobberConsumerOperands`](PairUnitEffects::BoundEarlyClobberConsumerOperands),
    /// and the x86-64 row's `rflags` clobber retires unconditionally.
    const fn saturating_subtract_zero_clamped(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BinaryRightLiteralScratchDefs,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BoundEarlyClobberConsumerOperands,
            machine_effects: PairMachineEffects::DeadConsumerUnitDefs,
        };
        assert!(
            carrier.is_signed() && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the clamped identity fold holds only for a signed carrier's zero literal"
        );
        rule
    }

    /// The saturating-subtract identity rules: one right-literal pair for
    /// each unsigned carrier's three-operand row, and one for each signed
    /// carrier's clamped row whose bound scratch `Def` drops under the
    /// scratch-defs grammar. Saturating subtraction does not commute —
    /// `0 -| x` is `-x` clamped to the carrier's bounds, not `x` — so the
    /// family declares no left-literal pair at either operand grammar.
    pub const SATURATING_SUBTRACT_ZERO_COPIES: [Self; 8] = [
        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U8),
        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U16),
        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U32),
        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U64),
        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I8),
        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I16),
        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I32),
        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I64),
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

    /// The pair's declared machine-effect surface: which memory, trap, stack,
    /// and control-flow traffic the producer, consumer, and rewritten forms
    /// may carry in the bound effect catalog.
    pub const fn machine_effects(self) -> PairMachineEffects {
        self.machine_effects
    }

    /// The consumer operand index the literal victim must occupy.
    pub const fn victim_operand(self) -> u16 {
        match self.operand_shape {
            PairOperandShape::BinaryRightLiteral
            | PairOperandShape::BinaryRightLiteralAuxiliaryUses
            | PairOperandShape::BinaryRightLiteralConstantResult
            | PairOperandShape::BinaryRightLiteralScratchDefs => 1,
            PairOperandShape::BinaryLeftLiteral
            | PairOperandShape::BinaryLeftLiteralConstantResult
            | PairOperandShape::BinaryLeftLiteralConstantResultAuxiliaryUses
            | PairOperandShape::BinaryLeftLiteralScratchDefs
            | PairOperandShape::UnaryLiteral => 0,
        }
    }

    /// The declared immediate admission: an encoding limit any literal up
    /// to, or one exact literal value the fold's semantics require.
    pub const fn immediate_bound(self) -> PairImmediateBound {
        self.immediate_bound
    }

    pub const fn admits_immediate(self, value: u64) -> bool {
        match self.immediate_bound {
            PairImmediateBound::Encoding(limit) => value <= limit,
            PairImmediateBound::Exactly(exact) => value == exact,
        }
    }

    /// The constant payload the rewritten instruction embeds for `literal`:
    /// the literal itself for the immediate forms and the copy fold, or the
    /// extension's exact output bits for the unary extension folds. The
    /// result is the recorded `immediate` in [`crate::LiteralFoldAction`].
    pub fn fold_immediate(self, literal: u64) -> Option<u64> {
        match self.operand_shape {
            PairOperandShape::BinaryRightLiteral
            | PairOperandShape::BinaryLeftLiteral
            | PairOperandShape::BinaryRightLiteralAuxiliaryUses
            | PairOperandShape::BinaryRightLiteralScratchDefs
            | PairOperandShape::BinaryLeftLiteralScratchDefs => Some(literal),
            // The constant-result grammars record the constant the
            // rewritten `MaterializeI64` embeds: a remainder by one or of
            // a zero dividend is always zero, an unsigned divide of a
            // zero dividend is always zero, whatever the folded literal
            // was, and a bitwise-and with a zero literal is always zero
            // at either `Use` position.
            PairOperandShape::BinaryRightLiteralConstantResult
            | PairOperandShape::BinaryLeftLiteralConstantResult
            | PairOperandShape::BinaryLeftLiteralConstantResultAuxiliaryUses => Some(0),
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
            MachineSemanticKind::CopyI64 => Some(keys.copy_i64),
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
            // A divide by one is the dividend: the `CopyI64` rewrite drops
            // the proof-custody fields from the kind — the obligation list
            // in the rebuilt provenance retains them.
            (MachineSemanticKind::CopyI64, SelectedInstructionKind::ExactDivideU64 { .. }) => {
                Some(SelectedInstructionKind::CopyI64)
            }
            // An exclusive-or or a wrapping add with a zero literal, a
            // bitwise-and with an all-ones literal, a saturating add
            // with a zero literal, or a saturating subtract with a zero
            // right literal is the other operand — `x ^ 0` and `0 ^ x`
            // are both `x`, `x + 0` and `0 + x` are both `x` modulo 2^64,
            // `x & MAX` and `MAX & x` are both `x`, `x +| 0` and `0 +| x`
            // are both `x` inside the carrier's bounds, and `x -| 0` is
            // `x` inside the carrier's bounds: the `CopyI64` rewrite
            // binds the surviving register the recorded action names. The
            // consumer guard keeps each rule bound to its own consumer
            // kind — the xor rule never rewrites an add, the add rule
            // never rewrites an and, the and-ones rule never rewrites
            // either, each saturating-add rule rewrites only the carrier
            // kind its pair admits, and each saturating-subtract rule
            // likewise — while the subtraction family stays bound to the
            // right-literal grammar alone: `0 -| x` is not `x`.
            (
                MachineSemanticKind::CopyI64,
                kind @ (SelectedInstructionKind::BitwiseXorI64
                | SelectedInstructionKind::WrappingAddI64
                | SelectedInstructionKind::BitwiseAndI64
                | SelectedInstructionKind::SaturatingAdd { .. }
                | SelectedInstructionKind::SaturatingSubtract { .. }),
            ) if machine_semantic_kind(kind) == self.consumer => {
                Some(SelectedInstructionKind::CopyI64)
            }
            // A remainder by one or of a zero dividend is always zero, an
            // unsigned divide of a zero dividend is always zero, and a
            // bitwise-and with a zero literal is always zero at either
            // `Use` position: the `MaterializeI64` rewrite materializes
            // the folded constant at the result register, sign-matched
            // and admitted by its scalar type. The consumer guard keeps
            // each rule bound to its own consumer kind — the remainder
            // rules never rewrite an and or a divide, the and-zero rule
            // never rewrites a remainder or a divide, and the divide
            // zero-dividend rule never rewrites either — while each pair
            // sharing a kind legitimately coexists: admission already
            // fixed which grammar applies by the folded literal's operand
            // position, and both produce the same materialized zero.
            (
                MachineSemanticKind::MaterializeI64,
                kind @ (SelectedInstructionKind::WrappingRemainderI64 { .. }
                | SelectedInstructionKind::ExactDivideU64 { .. }
                | SelectedInstructionKind::BitwiseAndI64),
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
