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
//! on the admitted consumer, and on the eliminated producer), declared as a
//! composition of the independent `consumer_fixed_view` and
//! `consumer_early_clobber` admission axes rather than one variant per
//! combination, the
//! machine-effect surface carries the non-unit dimensions — memory, trap,
//! stack, control flow, barrier, call, and cleanup — that each form's
//! [`MachineEffectDeclaration`] must satisfy, declared as a composition of
//! the independent [`PairNonUnitSurface`], [`PairFaultDischarge`], and
//! [`PairUnitDefRelation`] axes rather than one variant per combination,
//! and the operand shape carries
//! the consumer-grammar dimension: whether the folded literal is a binary
//! consumer's right `Use` operand, a commutative binary consumer's left
//! `Use` operand, a left `Use` operand whose rewrite swaps the operand
//! order under a preserved implicit-unit result channel — the compare
//! grammar — a unary consumer's sole `Use` operand, or a binary
//! consumer whose folded result is a constant of the literal alone — no
//! `Use` operand survives, and every operand past the result is a
//! dropped `Def` scratch or, under the divide's auxiliary grammar, a
//! dropped `Use` proven to read only a zero materialization — at the
//! right `Use` position or, under the annihilator, zero-dividend, and
//! zero-minuend grammars, the left one. The surviving-`Use` grammars also carry a
//! scratch-tail form — [`PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS`]
//! and [`PairOperandShape::BINARY_LEFT_LITERAL_SCRATCH_DEFS`] — for a consumer
//! whose operand list continues past its `Def` result with scratch outputs
//! the fold drops under occurrence-free custody: the clamped saturating-add
//! rows carry such a bound scratch at operand 3. Beyond the
//! isolated machine-effect surface, [`PairNonUnitSurface::IndexedPointerRead`]
//! declares the first non-isolated relationship — a consumer that reads
//! memory through the folded index and a rewritten form that reads the same
//! bytes through a materialized byte offset — and the trap-carrying
//! relationships come in two discharge forms:
//! [`PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL`] declares a consumer
//! whose encoded alternatives may architecturally fault, where the folded
//! literal is exactly the value that makes the fault unreachable, and
//! [`PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION`] declares the same
//! may-fault consumer surface where the fault is already unreachable under
//! the obligation the consumer kind carries — the literal fixes the result,
//! not the divisor's definedness.
//! [`PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS`] declares the dead-unit-def
//! relationship: a consumer whose implicit unit *definitions* the rewrite
//! retires because the rewritten form does not define them — the saturating
//! add's condition-state definition an isolated copy does not carry — where
//! retiring them is admitted only while no instruction or terminator in the
//! function implicitly uses a unit the consumer record defines.
//! [`PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL_DEAD_UNIT_DEFS`] composes
//! the two relationships for a may-fault consumer that also retires dead
//! implicit definitions — the saturating divide, whose divisor-one
//! literal discharges the encoded fault while its `nzcv` definition or
//! scratch tail drops under the dead-definitions custody.
//! [`PairMachineEffects::OPERAND_SWAPPED_UNIT_DEFS`] declares the
//! condition-state reader-flow relationship: a consumer whose implicit
//! unit *definitions* the rewrite keeps — the compare's target condition
//! state — under an operand order the rewrite reverses, so the zero
//! condition the units' equality readers observe is identical while
//! every ordering predicate inverts, admitted only while every reader
//! each defined unit can reach through the function's CFG is
//! equality-sensing. The
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

use crate::analyses::machine_effects::machine_semantic_kind;
use crate::rewrites::block_edges::{block_instructions, terminator_successors};

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

/// The implicit-unit traffic the pair's rewrite may carry, composed from
/// independent axes rather than one variant per combination.
///
/// `PairResultDisposition` owns the result channel — which units the
/// rewritten instruction *defines* as its output. This declaration covers
/// the rest of the unit surface: implicit unit uses and clobbers on the
/// rewritten constraint row, and the operand unit bindings (`fixed_view`,
/// `tied_to`, `early_clobber`) on either side of the rewrite. Its two axes
/// are independent: `consumer_fixed_view` names whether the admitted
/// consumer's operands may carry `fixed_view` pins, and
/// `consumer_early_clobber` names whether they may carry `early_clobber`
/// marks — each binding is admitted on its own because the rewrite
/// rebuilds the consumer's operand list from the undecorated rewritten
/// row, so every pin and mark the folded form needed dies with its
/// operand. `tied_to` has no axis: no composition admits it, since a tie
/// would silently lose the shared-home requirement a surviving operand
/// might have observed. A new consumer-binding relationship is a row in
/// this product, not a new variant: admission is the conjunction of the
/// per-axis gates below. The producer admits rows and consumers through
/// the declaration; the validator re-derives the same requirements from
/// its own matching so a descriptor mistake cannot self-certify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairUnitEffects {
    /// Whether `fixed_view` pins may appear on the admitted consumer's
    /// operands.
    pub consumer_fixed_view: PairConsumerBindingAdmission,
    /// Whether `early_clobber` marks may appear on the admitted consumer's
    /// operands.
    pub consumer_early_clobber: PairConsumerBindingAdmission,
}

/// Whether one operand unit binding kind may appear on the admitted
/// consumer's operand list — the axis `PairUnitEffects`'s
/// `consumer_fixed_view` and `consumer_early_clobber` fields each carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairConsumerBindingAdmission {
    /// The binding may not appear on any admitted-consumer operand: the
    /// rewrite rebuilds the operand list from the constraint row, so a
    /// binding there would be silently dropped.
    Rejected,
    /// The binding may appear on the admitted consumer's operands: the
    /// rewrite rebuilds the operand list from the undecorated rewritten
    /// row, so the pin or hazard mark the folded form needed is
    /// deliberately discarded with it — a surviving operand's register
    /// keeps its other uses' own constraints and gains strictly more
    /// allocation freedom, and a dropped operand's binding dies with the
    /// operand.
    DroppedWithOperandList,
}

impl PairUnitEffects {
    /// The rewritten row declares no implicit unit uses and no clobbers,
    /// and no operand on either side of the rewrite carries a unit
    /// binding. The admitted consumer's operands must already be
    /// undecorated because the rewrite rebuilds them from the row — a
    /// binding there would be silently dropped. A rule whose rewritten
    /// form implicitly reads or clobbers a unit — a flag-consuming
    /// arithmetic form, a scratch-clobbering realization — declares a new
    /// axis composition instead of weakening this one.
    pub const ISOLATED: Self = Self {
        consumer_fixed_view: PairConsumerBindingAdmission::Rejected,
        consumer_early_clobber: PairConsumerBindingAdmission::Rejected,
    };

    /// The rewritten row stays as under [`ISOLATED`](Self::ISOLATED), but
    /// the admitted consumer's operands may carry `fixed_view` bindings —
    /// the register pins a pinned-operand form such as the x86-64 `div`
    /// realization requires. The rewrite rebuilds the operand list from
    /// the unpinned rewritten row, so every pin the folded form needed is
    /// deliberately discarded with it: a surviving operand's register
    /// keeps its other uses' own constraints and gains strictly more
    /// allocation freedom, and a dropped operand's pin dies with the
    /// operand. `tied_to` and `early_clobber` still reject — neither has a
    /// carried meaning once the operand list is rebuilt.
    pub const BOUND_CONSUMER_OPERANDS: Self = Self {
        consumer_fixed_view: PairConsumerBindingAdmission::DroppedWithOperandList,
        ..Self::ISOLATED
    };

    /// The rewritten row stays as under [`ISOLATED`](Self::ISOLATED), but
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
    pub const BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS: Self = Self {
        consumer_early_clobber: PairConsumerBindingAdmission::DroppedWithOperandList,
        ..Self::BOUND_CONSUMER_OPERANDS
    };

    /// Whether the constraint row's instruction-level unit traffic
    /// satisfies the declaration. Implicit *definitions* are the result
    /// channel and stay under `PairResultDisposition`. The requirement is
    /// the same under every axis composition: the rewritten row never
    /// carries unit traffic of its own.
    pub fn admits_row_units(self, row: &RegisterInstructionConstraint) -> bool {
        row.implicit_uses.is_empty() && row.clobbers.is_empty()
    }

    /// Whether one constraint-row operand carries no unit binding. The
    /// requirement is the same under every axis composition: the rewritten
    /// row's operands are rebuilt undecorated.
    pub fn admits_operand(self, operand: &RegisterOperandConstraint) -> bool {
        operand.fixed_view.is_none() && operand.tied_to.is_none() && !operand.early_clobber
    }

    /// Whether the admitted consumer's operand unit bindings survive the
    /// wholesale rebuild from the constraint row: `tied_to` rejects under
    /// every composition, and each other binding kind is admitted only
    /// under its own axis — a `fixed_view` pin under
    /// [`DroppedWithOperandList`](PairConsumerBindingAdmission::DroppedWithOperandList)
    /// on `consumer_fixed_view` because the rewrite deliberately drops it
    /// with the pinned form, an `early_clobber` mark under the
    /// `consumer_early_clobber` axis for the same reason — the hazard it
    /// names exists only inside the dropped operand list.
    pub fn admits_consumer(self, consumer: &SelectedInstruction) -> bool {
        let admits_fixed_view = matches!(
            self.consumer_fixed_view,
            PairConsumerBindingAdmission::DroppedWithOperandList
        );
        let admits_early_clobber = matches!(
            self.consumer_early_clobber,
            PairConsumerBindingAdmission::DroppedWithOperandList
        );
        consumer.operands.iter().all(|operand| {
            operand.tied_to.is_none()
                && (admits_fixed_view || operand.fixed_view.is_none())
                && (admits_early_clobber || !operand.early_clobber)
        })
    }

    /// Whether the eliminated producer's instruction record carries no unit
    /// traffic — implicit uses, definitions, clobbers, or operand bindings —
    /// that removing the instruction would silently drop. The requirement is
    /// the same under every axis composition: the eliminated instruction
    /// never survives in any form.
    pub fn admits_producer(self, producer: &SelectedInstruction) -> bool {
        producer.implicit_uses.is_empty()
            && producer.implicit_defs.is_empty()
            && producer.clobbers.is_empty()
            && producer.operands.iter().all(|operand| {
                operand.fixed_view.is_none() && operand.tied_to.is_none() && !operand.early_clobber
            })
    }
}

/// The machine-effect relationships a producer→consumer pair declares,
/// composed from independent axes rather than one variant per
/// combination.
///
/// `PairUnitEffects` owns the physical-register-unit traffic carried by the
/// selected instruction records and constraint rows (implicit uses,
/// definitions as the declared result channel, clobbers, and operand unit
/// bindings); this descriptor covers the validated
/// [`MachineEffectDeclaration`] surface — memory, trap, stack, control
/// flow, barrier, call, and cleanup — for all three instruction forms the
/// rewrite involves, and the relationship between the consumer's and
/// rewritten form's encoded implicit-unit traffic. Its three axes are
/// independent: `non_unit` relates the consumer's and rewritten form's
/// non-unit declaration surface, `fault` names which surface discharges
/// the consumer's encoded architectural fault, and `unit_defs` names what
/// the rewrite does with the consumer's implicit unit definitions. A new
/// pair relationship is a row in this product — the saturating divide's
/// fold, for example, is `fault` discharged by the folded literal
/// composed with `unit_defs` retiring the condition-state definitions —
/// not a new variant: admission is the conjunction of the per-axis gates
/// below. The producer admits the eliminated producer's, the admitted
/// consumer's, and the rewritten form's catalog declarations through this
/// dimension; the validator re-derives the same requirements from its own
/// matching so a descriptor mistake cannot self-certify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairMachineEffects {
    /// The consumer↔rewritten non-unit declaration surface.
    pub non_unit: PairNonUnitSurface,
    /// Which surface discharges the consumer's encoded architectural
    /// fault, if any.
    pub fault: PairFaultDischarge,
    /// What the rewrite does with the consumer's implicit unit
    /// definitions.
    pub unit_defs: PairUnitDefRelation,
}

/// The consumer↔rewritten relationship on the non-unit declaration
/// surface — memory, trap, stack, control flow, barrier, call, and
/// cleanup — independent of the consumer's fault discharge and
/// implicit-definition disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairNonUnitSurface {
    /// Both declarations are effect-isolated outside their unit surface:
    /// no memory access, no hosted trap, barrier, call, or cleanup
    /// behavior, and every encoded alternative falls through without
    /// memory or stack traffic. Encoded trap behavior stays governed by
    /// the [`PairFaultDischarge`] axis.
    Isolated,
    /// The consumer reads memory through a pointer plus an index operand
    /// the fold removes; the rewritten form reads the same pointer
    /// through a materialized byte offset. `index_operand` is the
    /// consumer operand position the literal victim occupies — it must
    /// equal the operand shape's declared victim position.
    ///
    /// The consumer and rewritten declarations must carry an identical
    /// non-unit surface — pointer-read memory, the same trap, barrier,
    /// call, and cleanup behavior — and every consumer alternative must
    /// be an indexed pointer read whose index is exactly
    /// `index_operand`, matched by a direct pointer read over the same
    /// pointer operand and byte count in every rewritten alternative,
    /// with pairwise-equal stack, trap, and control encodings and the
    /// isolated unit-traffic relation: no implicit uses on either side,
    /// every unit the consumer defines still defined, and no implicit
    /// uses or clobbers on the rewritten form. This relation owns the
    /// whole pairwise surface, so the `fault` and `unit_defs` axes
    /// compose only under [`Isolated`](Self::Isolated).
    IndexedPointerRead {
        /// The consumer operand position carrying the folded index
        /// register.
        index_operand: u16,
    },
}

/// How the consumer's encoded architectural fault — an alternative
/// carrying `MayArchitecturalFaultV1` — is discharged across the fold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairFaultDischarge {
    /// The consumer encodes no architectural fault: every alternative's
    /// trap behavior is `NeverV1`.
    Isolated,
    /// The consumer may architecturally fault and the folded literal is
    /// exactly the value that makes every such fault unreachable:
    /// `EXACT_DIVIDE_ONE_COPY` folds a divisor of one, under which an
    /// unsigned divide can neither divide by zero nor overflow —
    /// provided the auxiliary `Use` operands the shape drops are
    /// provably zero, the operand-shape contract's own requirement —
    /// and `WRAPPING_REMAINDER_ONE_MATERIALIZE` folds a divisor of one,
    /// under which a signed or unsigned remainder can neither divide by
    /// zero nor overflow. Declaring this surface attests that the
    /// rewrite replaces the consumer's trap surface wholesale because
    /// the admitted immediate discharges it: the consumer declaration
    /// must be non-unit isolated, with alternatives that touch no
    /// memory, leave the stack unchanged, fall through, carry no
    /// implicit uses, and encode only `NeverV1` or
    /// `MayArchitecturalFaultV1` trap behavior. The rewritten form is
    /// fully effect-isolated because the discharged fault does not
    /// survive the fold.
    DischargedByLiteral,
    /// The consumer may architecturally fault and the fault is
    /// unreachable under the consumer's own carried obligation rather
    /// than under the folded literal alone:
    /// `WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE` folds a dividend
    /// literal of zero, under which the quotient is zero and cannot
    /// overflow, `EXACT_DIVIDE_ZERO_DIVIDEND_MATERIALIZE` folds the same
    /// dividend literal of an unsigned exact divide, and
    /// `SATURATING_DIVIDE_ZERO_DIVIDEND_MATERIALIZATIONS` the saturating
    /// divide's, while the nonzero-divisor obligation each kind carries
    /// as its accepted fact already excludes the only other reachable
    /// fault — division by zero. Declaring this surface attests that
    /// the rewrite replaces the consumer's trap surface wholesale
    /// because every fault case was already unreachable: the literal
    /// fixes the quotient and the carried obligation fixes the divisor.
    /// Unlike [`DischargedByLiteral`](Self::DischargedByLiteral), the
    /// folded literal is not by itself the discharging value — a
    /// dividend of zero with an unproven divisor would still fault — so
    /// the obligation the consumer's kind names must appear in the
    /// instruction's recorded proof custody. The consumer surface is the
    /// same non-unit isolated, fault-envelope contract
    /// [`DischargedByLiteral`](Self::DischargedByLiteral) names; the
    /// rewritten form is fully effect-isolated.
    DischargedByObligation,
}

/// What the rewrite does with the consumer's implicit unit
/// *definitions*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairUnitDefRelation {
    /// Every unit the consumer defines stays defined by every rewritten
    /// alternative — a definition the rewritten form dropped would leave
    /// its readers observing a stale unit.
    Covered,
    /// The consumer implicitly defines physical units the rewritten
    /// form does not define — the target condition state an aarch64
    /// flag-setting saturating add writes into `nzcv`, which the
    /// isolated `CopyI64` rewrite does not carry — and removing the
    /// consumer retires those definitions. Declaring this surface
    /// attests that the rewrite narrows the defined-unit surface
    /// deliberately: the fold is admitted only while every unit the
    /// consumer record defines is dead in the function — no instruction
    /// or terminator implicitly uses it — so no reader observes a stale
    /// unit once the defining instruction disappears. Coverage by the
    /// rewritten form is not required; it is exactly what the
    /// relationship retires. Implicit uses stay forbidden: one the
    /// rewritten form does not carry would be unit state the rewrite
    /// silently stops observing. Clobbers are unrestricted since
    /// dropping them only narrows what may be destroyed: the x86-64
    /// saturating add's `rflags` clobber retires under this surface
    /// even while a flag-reading branch keeps `rflags` live.
    RetiredWhenDead,
    /// The consumer's implicit definitions stay defined by the
    /// rewritten form while the operand-swapped grammar changes the
    /// relation they encode: `literal - x` rewrites to `x - literal`,
    /// which preserves the zero condition exactly but inverts every
    /// ordering predicate. Declaring this surface attests that keeping
    /// the definitions under the reversed comparison is admitted only
    /// while the inversion is unobservable: every reader each defined
    /// unit can reach through the function's CFG must be
    /// equality-sensing — the `MaterializeBooleanEqual` materialization
    /// or the generic `ConditionalBranchNonZero` terminator — a
    /// record-level fact the `admits_swapped_condition_defs` gate
    /// re-derives from the concrete instruction and function, not a
    /// fact any catalog declaration attests. At the declaration level
    /// the definitions are covered, exactly as under
    /// [`Covered`](Self::Covered).
    OperandSwapped,
}

impl PairMachineEffects {
    /// Every surface isolated: the consumer is effect-isolated outside
    /// its unit surface, encodes no architectural fault, declares no
    /// implicit uses, and every unit it defines stays defined by every
    /// rewritten alternative. Its clobbers are unrestricted: dropping a
    /// clobber only narrows what may be destroyed, which is the
    /// intended refinement when the x86-64 `sub` consumer's `rflags`
    /// clobber disappears under the flag-preserving immediate form.
    pub const ISOLATED: Self = Self {
        non_unit: PairNonUnitSurface::Isolated,
        fault: PairFaultDischarge::Isolated,
        unit_defs: PairUnitDefRelation::Covered,
    };

    /// The indexed pointer read fold: the consumer reads pointer plus
    /// folded index, the rewritten form reads the same pointer through
    /// a materialized byte offset.
    pub const fn indexed_pointer_read_fold(index_operand: u16) -> Self {
        Self {
            non_unit: PairNonUnitSurface::IndexedPointerRead { index_operand },
            fault: PairFaultDischarge::Isolated,
            unit_defs: PairUnitDefRelation::Covered,
        }
    }

    /// The folded literal discharges every encoded fault; implicit
    /// definitions stay covered.
    pub const FAULT_DISCHARGED_BY_LITERAL: Self = Self {
        fault: PairFaultDischarge::DischargedByLiteral,
        ..Self::ISOLATED
    };

    /// The consumer's carried obligation discharges every encoded
    /// fault; implicit definitions stay covered.
    pub const FAULT_DISCHARGED_BY_OBLIGATION: Self = Self {
        fault: PairFaultDischarge::DischargedByObligation,
        ..Self::ISOLATED
    };

    /// The consumer's implicit unit definitions retire under the
    /// record-level deadness gate; no fault surface.
    pub const DEAD_CONSUMER_UNIT_DEFS: Self = Self {
        unit_defs: PairUnitDefRelation::RetiredWhenDead,
        ..Self::ISOLATED
    };

    /// The saturating divide's composition: the folded literal
    /// discharges every encoded fault while the consumer's implicit
    /// unit definitions retire under the deadness gate.
    pub const FAULT_DISCHARGED_BY_LITERAL_DEAD_UNIT_DEFS: Self = Self {
        fault: PairFaultDischarge::DischargedByLiteral,
        unit_defs: PairUnitDefRelation::RetiredWhenDead,
        ..Self::ISOLATED
    };

    /// The zero-dividend saturating divide's composition: the
    /// consumer's carried obligation discharges every encoded fault
    /// while its implicit unit definitions retire under the deadness
    /// gate.
    pub const FAULT_DISCHARGED_BY_OBLIGATION_DEAD_UNIT_DEFS: Self = Self {
        fault: PairFaultDischarge::DischargedByObligation,
        unit_defs: PairUnitDefRelation::RetiredWhenDead,
        ..Self::ISOLATED
    };

    /// The operand-swapped composition: implicit definitions stay
    /// covered while the record-level gate re-derives that every
    /// reachable reader is equality-sensing.
    pub const OPERAND_SWAPPED_UNIT_DEFS: Self = Self {
        unit_defs: PairUnitDefRelation::OperandSwapped,
        ..Self::ISOLATED
    };

    /// Whether the eliminated producer's catalog declaration is
    /// effect-isolated including every implicit unit it could have
    /// written. Every landed pair requires this surface regardless of
    /// the declared axes: removing the producer must drop nothing
    /// machine-visible.
    pub fn admits_producer(self, declaration: &MachineEffectDeclaration) -> bool {
        isolated_declaration(declaration)
            && declaration.alternatives.iter().all(|alternative| {
                isolated_alternative(alternative)
                    && alternative.encoded.implicit_unit_uses.is_empty()
                    && alternative.encoded.implicit_unit_defs.is_empty()
                    && alternative.encoded.implicit_unit_clobbers.is_empty()
            })
    }

    /// Whether the admitted consumer's catalog declaration satisfies the
    /// pair's declared relationship to `rewritten`. Under
    /// [`Isolated`](PairNonUnitSurface::Isolated) the consumer is
    /// effect-isolated outside its unit surface: the encoded trap
    /// envelope the `fault` axis admits and the implicit-definition
    /// coverage the `unit_defs` axis requires compose over it, while
    /// implicit uses stay forbidden — one the rewritten form does not
    /// carry would be unit state the rewrite silently stops observing.
    /// Under [`IndexedPointerRead`](PairNonUnitSurface::IndexedPointerRead)
    /// the consumer is the indexed pointer read: the two declarations
    /// share the same non-unit surface, and every consumer alternative's
    /// encoded indexed read at the folded operand position is matched by
    /// every rewritten alternative's direct read over the same pointer
    /// and byte count.
    pub fn admits_consumer(
        self,
        declaration: &MachineEffectDeclaration,
        rewritten: &MachineEffectDeclaration,
    ) -> bool {
        match self.non_unit {
            PairNonUnitSurface::Isolated => {
                isolated_declaration(declaration)
                    && declaration.alternatives.iter().all(|alternative| {
                        self.fault.consumer_alternative_gate(alternative)
                            && alternative.encoded.implicit_unit_uses.is_empty()
                            && self.unit_defs.defs_gate(alternative, rewritten)
                    })
            }
            PairNonUnitSurface::IndexedPointerRead { index_operand } => {
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

    /// Whether the admitted consumer's instruction record retains the
    /// obligation this discharge relationship depends on. Under
    /// [`DischargedByObligation`](PairFaultDischarge::DischargedByObligation)
    /// the folded literal alone does not discharge the fault: the
    /// obligation the consumer's kind names — the proven nonzero divisor
    /// a `WrappingRemainderI64`, an `ExactDivideU64`, or a
    /// `SaturatingDivide` carries — must appear in the instruction's
    /// recorded proof custody, so an instruction record not retaining it
    /// cannot fold. Every other discharge needs no carried obligation.
    pub fn admits_consumer_obligation(self, consumer: &SelectedInstruction) -> bool {
        if self.fault != PairFaultDischarge::DischargedByObligation {
            return true;
        }
        let obligation = match consumer.kind {
            SelectedInstructionKind::WrappingRemainderI64 { obligation, .. }
            | SelectedInstructionKind::ExactDivideU64 { obligation, .. }
            | SelectedInstructionKind::SaturatingDivide { obligation, .. } => obligation,
            _ => return false,
        };
        consumer.provenance.obligations.contains(&obligation)
    }

    /// Whether the admitted consumer's instruction record retires only
    /// unobserved unit state. Under
    /// [`RetiredWhenDead`](PairUnitDefRelation::RetiredWhenDead) the
    /// record must declare no implicit unit *uses* — one the rewritten
    /// form does not carry would be unit state the rewrite silently
    /// stops observing — and every unit it defines must be dead in the
    /// function, so no reader observes a stale unit once the defining
    /// instruction disappears. Every other relation keeps the defined
    /// units on the rewritten form and needs no record-level gate.
    pub fn admits_dead_consumer_defs(
        self,
        consumer: &SelectedInstruction,
        function: &SelectedFunction,
    ) -> bool {
        match self.unit_defs {
            PairUnitDefRelation::RetiredWhenDead => {
                consumer.implicit_uses.is_empty()
                    && consumer
                        .implicit_defs
                        .iter()
                        .all(|unit| !implicit_unit_used(function, *unit))
            }
            PairUnitDefRelation::Covered | PairUnitDefRelation::OperandSwapped => true,
        }
    }

    /// Whether the operand-swapped rewrite's preserved implicit
    /// definitions stay unobservable to ordering-sensitive readers.
    /// [`OperandSwapped`](PairUnitDefRelation::OperandSwapped) keeps the
    /// consumer's implicit unit *definitions* on the rewritten form but
    /// reverses the comparison's operand order — `literal - x` becomes
    /// `x - literal` — so the zero condition each defined unit's
    /// equality readers observe is identical while every ordering
    /// predicate inverts. The fold is admitted only while every reader
    /// each defined unit reaches through the CFG is equality-sensing: the
    /// record must declare no implicit unit *uses* of its own, and
    /// `swapped_unit_readers_equality_only` walks each defined unit
    /// forward from the consumer until a redefinition or clobber ends the
    /// live range this definition feeds. When both operands name the
    /// same register the subtraction itself is unchanged — `v - v` and
    /// the surviving `v - v` are the identical comparison — so no reader
    /// can observe the rewrite and the audit does not apply. Every other
    /// relation keeps the operand order and needs no flow gate.
    pub fn admits_swapped_condition_defs(
        self,
        consumer: &SelectedInstruction,
        function: &SelectedFunction,
        block_index: usize,
        consumer_index: usize,
    ) -> bool {
        match self.unit_defs {
            PairUnitDefRelation::OperandSwapped => {
                let [left, right, ..] = consumer.operands.as_slice() else {
                    return false;
                };
                consumer.implicit_uses.is_empty()
                    && (left.virtual_register == right.virtual_register
                        || consumer.implicit_defs.iter().all(|unit| {
                            swapped_unit_readers_equality_only(
                                function,
                                block_index,
                                consumer_index,
                                *unit,
                            )
                        }))
            }
            PairUnitDefRelation::Covered | PairUnitDefRelation::RetiredWhenDead => true,
        }
    }

    /// Whether the rewritten form's catalog declaration satisfies the
    /// pair's declared shape. Under
    /// [`Isolated`](PairNonUnitSurface::Isolated) the rewritten form is
    /// fully effect-isolated with no implicit unit uses or clobbers
    /// beyond its result channel: a discharged fault does not reappear
    /// anywhere in the rewrite, and a retired definition or clobber must
    /// not either. Under
    /// [`IndexedPointerRead`](PairNonUnitSurface::IndexedPointerRead)
    /// the rewritten form is the plain pointer read whose alternatives
    /// all read memory through a pointer operand and byte offset, fall
    /// through, leave the stack unchanged, and declare no implicit uses
    /// or clobbers.
    pub fn admits_rewritten(self, declaration: &MachineEffectDeclaration) -> bool {
        match self.non_unit {
            PairNonUnitSurface::Isolated => {
                isolated_declaration(declaration)
                    && declaration.alternatives.iter().all(|alternative| {
                        isolated_alternative(alternative)
                            && alternative.encoded.implicit_unit_uses.is_empty()
                            && alternative.encoded.implicit_unit_clobbers.is_empty()
                    })
            }
            PairNonUnitSurface::IndexedPointerRead { .. } => {
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

impl PairFaultDischarge {
    /// The encoded trap surface this discharge admits on the consumer's
    /// alternatives: a faultless consumer stays fully isolated; a
    /// discharged fault admits the `MayArchitecturalFaultV1` envelope
    /// the named discharge contract retires.
    fn consumer_alternative_gate(self, alternative: &MachineAlternative) -> bool {
        match self {
            Self::Isolated => isolated_alternative(alternative),
            Self::DischargedByLiteral | Self::DischargedByObligation => {
                fault_discharged_alternative(alternative)
            }
        }
    }
}

impl PairUnitDefRelation {
    /// The declaration-level coverage this relation requires of the
    /// consumer's implicit definitions: definitions the rewrite keeps —
    /// whether the operand order is preserved or swapped — must stay
    /// defined by every rewritten alternative, while definitions the
    /// rewrite retires need no coverage: the record-level deadness gate
    /// decides whether retiring them is observable.
    fn defs_gate(
        self,
        consumer: &MachineAlternative,
        rewritten: &MachineEffectDeclaration,
    ) -> bool {
        match self {
            Self::Covered | Self::OperandSwapped => implicit_defs_covered(consumer, rewritten),
            Self::RetiredWhenDead => true,
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
                | SelectedTerminator::Crash { instruction, .. }
                | SelectedTerminator::HostedExitProcess { instruction, .. } => {
                    std::iter::once(instruction)
                }
            })
            .any(|instruction| instruction.implicit_uses.contains(&unit))
    })
}

/// Whether `kind` reads only the zero condition out of the published flag
/// state — the one predicate the operand-swapped subtraction
/// `register - literal` preserves from `literal - register`. The
/// boolean-equal materialization and the generic conditional branch are
/// its only equality consumers in the selected catalog; every ordering
/// predicate — the `*LessThan`/`*LessOrEqual` materializations and the
/// predicate-aware terminators — observes the inverted relation, and any
/// other implicit reader is conservatively refused the same way.
fn equality_sensing_reader(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::MaterializeBooleanEqual
            | SelectedInstructionKind::ConditionalBranchNonZero
    )
}

/// Audit one implicit unit an operand-swapped consumer defines: walk
/// forward from the instruction after the consumer inside
/// `function.blocks[block_index]`, requiring every reached reader of
/// `unit` to be equality-sensing, until an implicit definition or
/// clobber of the unit ends the live range this definition feeds. A unit
/// still live past the terminator resumes at the head of each successor
/// block; an edge naming a block the function does not contain leaves
/// the unit's readers unprovable and refuses. The `(block, start)`
/// visited set bounds the walk — a loop carrying the unit back into the
/// consumer's own block re-scans it from its head, where the consumer's
/// own definition ends the range.
fn swapped_unit_readers_equality_only(
    function: &SelectedFunction,
    block_index: usize,
    consumer_index: usize,
    unit: RegisterUnitId,
) -> bool {
    let mut visited = std::collections::BTreeSet::new();
    let mut frontier = vec![(block_index, consumer_index + 1)];
    while let Some((current, start)) = frontier.pop() {
        if !visited.insert((current, start)) {
            continue;
        }
        let block = &function.blocks[current];
        let mut killed = false;
        for instruction in block_instructions(block).skip(start) {
            if instruction.implicit_uses.contains(&unit)
                && !equality_sensing_reader(instruction.kind)
            {
                return false;
            }
            if instruction.implicit_defs.contains(&unit) || instruction.clobbers.contains(&unit) {
                killed = true;
                break;
            }
        }
        if killed {
            continue;
        }
        for successor in terminator_successors(&block.terminator) {
            let Some(target) = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == successor.block)
            else {
                return false;
            };
            frontier.push((target, 0));
        }
    }
    true
}

/// The consumer operand grammar of a pair rule, declared as a composition
/// of three independent axes rather than one variant per combination.
///
/// The axes are [`PairLiteralPosition`] — which `Use` operand the folded
/// literal occupies — [`PairOperandResult`] — what the rewritten form's
/// result channel computes relative to the non-victim operands — and
/// [`PairTailCustody`] — the custody contract for consumer operands past
/// the scalar `Def` result. A new grammar is an axis tuple (one of the
/// associated constants below, or an inline struct literal when a family
/// composes axes no constant names yet), not a new enum variant, policy
/// bit, or validator shape: admission is the conjunction of the per-axis
/// gates the compute leg reads. The producer admits the operand
/// arrangement matching this declared grammar instead of inferring it
/// from operand counts; the independent validator re-derives the same
/// distinction from the consumer kind alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairOperandShape {
    /// Which `Use` operand of the consumer is the folded literal, and
    /// which operand — if any — survives the fold.
    pub position: PairLiteralPosition,
    /// What the rewritten form's result channel computes relative to the
    /// non-victim operands.
    pub result_kind: PairOperandResult,
    /// The custody contract for consumer operands past the scalar `Def`
    /// result.
    pub tail: PairTailCustody,
}

/// Which `Use` operand of the consumer is the folded literal.
///
/// The position fixes the victim index every gate shares — the compute
/// leg reads it directly rather than re-deriving it per grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairLiteralPosition {
    /// Binary consumer: the literal victim is the right `Use` operand
    /// (operand index 1); operand 0 is the other register input.
    RightOperand,
    /// Binary consumer: the literal victim is the left `Use` operand
    /// (operand index 0); operand 1 is the other register input.
    ///
    /// For a [`PairOperandResult::SurvivingOperand`] grammar, declaring
    /// the left position additionally attests that the fold computes the
    /// same value under operand exchange: the immediate form encodes
    /// `surviving <op> literal` — and the xor-zero and wrapping-add-zero
    /// identity copies bind `surviving` unchanged, which `0 ^ x` and
    /// `x ^ 0` — likewise `0 + x` and `x + 0` — share — so only an
    /// operation exact under commutation may declare it. Exact addition,
    /// bitwise xor, wrapping addition, and the byte-view projection's
    /// modular address addition commute; subtraction and comparison fix
    /// the literal's role, so only those families declare a left-literal
    /// pair. For a [`PairOperandResult::ConstantOfLiteral`] grammar the
    /// position attests nothing about commutation — the operand-0 literal
    /// alone fixes the result (`0 & x` is zero for every `x` under the
    /// bitwise-and annihilator; `0 % x` is zero for every `x` the
    /// remainder's proven nonzero divisor admits).
    LeftOperand,
    /// Unary consumer: the literal victim is the sole `Use` operand
    /// (operand index 0).
    SoleOperand,
}

/// What the rewritten form's result channel computes relative to the
/// non-victim operands — the axis that decides which inputs the fold
/// still observes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairOperandResult {
    /// The non-victim `Use` operand survives into the rewritten row and
    /// produces the result: the immediate-form binary arithmetic and
    /// compare grammars.
    SurvivingOperand,
    /// The literal alone fixes the folded result — a constant the
    /// rewritten form embeds regardless of the non-victim operand's
    /// value: `x % 1` is zero for every `x`, `0 & x` is zero for every
    /// `x`. The non-victim `Use` drops because the constant result never
    /// reads it, and its register carries no custody requirement beyond
    /// that inertness.
    ConstantOfLiteral,
    /// The consumer's result channel is implicit units and the rewritten
    /// row binds the non-victim `Use` under a reversed operand order —
    /// `literal - x` becomes `x - literal` under the left-literal
    /// compare. Declaring this axis value attests only that the swapped
    /// result channel preserves the equality predicate while inverting
    /// every ordering predicate; the companion
    /// [`OperandSwappedUnitDefs`](PairMachineEffects::OPERAND_SWAPPED_UNIT_DEFS)
    /// relationship's record-level reader audit decides whether the
    /// inversion is observable. Only a consumer whose output is the
    /// condition state may declare it — a scalar `Def` result would carry
    /// the inverted subtraction to every register reader with no audit
    /// to refuse it.
    SwappedOperand,
    /// Unary consumer whose rewritten form recomputes a constant output
    /// from the literal itself — the extension-elimination and
    /// copy-materialization rules whose rewritten form is a
    /// `MaterializeI64`. Unlike
    /// [`ConstantOfLiteral`](Self::ConstantOfLiteral), the embedded
    /// payload is derived from the literal's value (the extension's exact
    /// output bits), not a constant the identity fixes.
    LiteralRecompute,
}

/// The custody contract for consumer operands past the scalar `Def`
/// result — every operand the fold drops must be inert under one of
/// these custodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairTailCustody {
    /// The operand list ends at the result; the fold drops nothing.
    Bare,
    /// Every operand past the result is a `Use` the fold drops under
    /// zero-provenance custody: each dropped register must be defined in
    /// the same function only by `MaterializeI64` instructions producing
    /// `Unsigned(0)` — the zeroed high-half input an x86-64 `div`
    /// realization requires, which a divide by one leaves dead. An
    /// operand that is not a dropped `Use` — a `Def`, or a `Use` whose
    /// register is defined any other way — rejects: dropping it would
    /// silently discard a value the consumer observed.
    AuxiliaryUses,
    /// Every operand past the result is a `Def` scratch output the fold
    /// drops under occurrence-free custody: each dropped register must
    /// occur nowhere else in the function — a scratch output another
    /// instruction read or defined would leave a use of a register the
    /// rewrite stopped defining. The bound scratch a clamped saturating
    /// realization computes its saturation bound through is the
    /// representative.
    ScratchDefs,
    /// A mixed drop tail: each operand past the result drops under the
    /// custody its own access declares — a `Use` under the
    /// zero-provenance custody [`AuxiliaryUses`](Self::AuxiliaryUses)
    /// requires, a `Def` under the occurrence-free custody
    /// [`ScratchDefs`](Self::ScratchDefs) requires. The zeroed high-half
    /// dividend an x86-64 `div`/`idiv` realization reads and the bound
    /// scratch `Def` an aarch64 clamped signed-divide realization writes
    /// coexist in the same tail. A `UseDef` operand names neither
    /// custody and rejects.
    AuxiliaryUsesOrScratchDefs,
}

impl PairOperandShape {
    /// Binary consumer: the literal victim is the right `Use` operand
    /// (operand index 1) and the left `Use` operand survives into the
    /// rewritten instruction — the immediate-form binary arithmetic and
    /// compare rules.
    pub const BINARY_RIGHT_LITERAL: Self = Self {
        position: PairLiteralPosition::RightOperand,
        result_kind: PairOperandResult::SurvivingOperand,
        tail: PairTailCustody::Bare,
    };
    /// Commutative binary consumer: the literal victim is the left `Use`
    /// operand (operand index 0) and the right `Use` operand survives into
    /// the rewritten instruction's `Use` position. Declaring this shape
    /// attests that the fold computes the same value under operand exchange:
    /// the immediate form encodes `surviving <op> literal` — and the
    /// xor-zero and wrapping-add-zero identity copies bind `surviving`
    /// unchanged, which `0 ^ x` and `x ^ 0` — likewise `0 + x` and
    /// `x + 0` — share — so only an operation exact under commutation
    /// may declare it. Exact addition, bitwise xor, wrapping addition, and
    /// the byte-view projection's modular address addition commute;
    /// subtraction and comparison fix the literal's role, so
    /// only those families declare a left-literal pair.
    pub const BINARY_LEFT_LITERAL: Self = Self {
        position: PairLiteralPosition::LeftOperand,
        result_kind: PairOperandResult::SurvivingOperand,
        tail: PairTailCustody::Bare,
    };
    /// Binary left-literal consumer whose result channel is implicit
    /// units and whose operands do *not* commute: the literal victim is
    /// the operand-0 `Use`, the operand-1 `Use` survives into the
    /// rewritten row's sole `Use` position, and the rewritten form
    /// computes the operand-swapped operation — `literal - x` becomes
    /// `x - literal` under the left-literal compare. Declaring this shape
    /// attests only that the swapped result channel preserves the
    /// equality predicate while inverting every ordering predicate; the
    /// companion
    /// [`OperandSwappedUnitDefs`](PairMachineEffects::OPERAND_SWAPPED_UNIT_DEFS)
    /// relationship's record-level reader audit decides whether the
    /// inversion is observable. Only a consumer whose output is the
    /// condition state may declare it — a scalar `Def` result would carry
    /// the inverted subtraction to every register reader with no audit
    /// to refuse it.
    pub const BINARY_LEFT_LITERAL_OPERAND_SWAP: Self = Self {
        position: PairLiteralPosition::LeftOperand,
        result_kind: PairOperandResult::SwappedOperand,
        tail: PairTailCustody::Bare,
    };
    /// Unary consumer: the literal victim is the sole `Use` operand (operand
    /// index 0). The rewritten instruction consumes no register input — the
    /// fold recomputes the consumer's constant output directly, as in the
    /// extension-elimination and copy-materialization rules whose rewritten
    /// form is a `MaterializeI64`.
    pub const UNARY_LITERAL: Self = Self {
        position: PairLiteralPosition::SoleOperand,
        result_kind: PairOperandResult::LiteralRecompute,
        tail: PairTailCustody::Bare,
    };
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
    pub const BINARY_RIGHT_LITERAL_AUXILIARY_USES: Self = Self {
        position: PairLiteralPosition::RightOperand,
        result_kind: PairOperandResult::SurvivingOperand,
        tail: PairTailCustody::AuxiliaryUses,
    };
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
    pub const BINARY_RIGHT_LITERAL_CONSTANT_RESULT: Self = Self {
        position: PairLiteralPosition::RightOperand,
        result_kind: PairOperandResult::ConstantOfLiteral,
        tail: PairTailCustody::ScratchDefs,
    };
    /// Binary left-literal consumer whose folded result is a constant of
    /// the literal alone: the literal victim is the operand-0 `Use`,
    /// operand 1 is a `Use` the fold drops because the constant result
    /// never reads it, operand 2 is the `Def` result, and every operand
    /// past the result is a `Def` scratch output the fold drops under the
    /// same occurrence-free custody
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](Self::BINARY_RIGHT_LITERAL_CONSTANT_RESULT)
    /// declares. Unlike [`BINARY_LEFT_LITERAL`](Self::BINARY_LEFT_LITERAL),
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
    pub const BINARY_LEFT_LITERAL_CONSTANT_RESULT: Self = Self {
        position: PairLiteralPosition::LeftOperand,
        result_kind: PairOperandResult::ConstantOfLiteral,
        tail: PairTailCustody::ScratchDefs,
    };
    /// Binary left-literal consumer whose folded result is a constant of
    /// the literal alone and whose operand list continues past its scalar
    /// `Def` result: the literal victim is the operand-0 `Use`, operand 1
    /// is a `Use` the fold drops because the constant result never reads
    /// it, operand 2 is the `Def` result, and every operand past the
    /// result is a `Use` the fold drops under the same provenance custody
    /// [`BINARY_RIGHT_LITERAL_AUXILIARY_USES`](Self::BINARY_RIGHT_LITERAL_AUXILIARY_USES)
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
    pub const BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES: Self = Self {
        position: PairLiteralPosition::LeftOperand,
        result_kind: PairOperandResult::ConstantOfLiteral,
        tail: PairTailCustody::AuxiliaryUses,
    };
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
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](Self::BINARY_RIGHT_LITERAL_CONSTANT_RESULT)
    /// declares: a scratch output another instruction read or defined
    /// would leave a use of a register the rewrite stopped defining. An
    /// operand that is not in its declared position and access — a `Use`
    /// past the result, or any operand at positions 0 through 2 outside
    /// this grammar — rejects.
    pub const BINARY_RIGHT_LITERAL_SCRATCH_DEFS: Self = Self {
        position: PairLiteralPosition::RightOperand,
        result_kind: PairOperandResult::SurvivingOperand,
        tail: PairTailCustody::ScratchDefs,
    };
    /// Binary left-literal consumer whose operand list continues past its
    /// scalar `Def` result with scratch outputs: the literal victim is the
    /// operand-0 `Use`, operand 1 is the surviving `Use` the rewritten row
    /// binds, operand 2 is the `Def` result, and every operand past the
    /// result is a `Def` scratch output the fold drops under the same
    /// occurrence-free custody
    /// [`BINARY_RIGHT_LITERAL_SCRATCH_DEFS`](Self::BINARY_RIGHT_LITERAL_SCRATCH_DEFS)
    /// declares. Like
    /// [`BINARY_LEFT_LITERAL`](Self::BINARY_LEFT_LITERAL), declaring this
    /// shape attests that the fold computes the same value under operand
    /// exchange — `0 +| x` and `x +| 0` are both `x` inside the carrier's
    /// bounds — so only an operation exact under commutation may declare
    /// it. An operand that is not in its declared position and access — a
    /// `Use` past the result, or any operand at positions 0 through 2
    /// outside this grammar — rejects.
    pub const BINARY_LEFT_LITERAL_SCRATCH_DEFS: Self = Self {
        position: PairLiteralPosition::LeftOperand,
        result_kind: PairOperandResult::SurvivingOperand,
        tail: PairTailCustody::ScratchDefs,
    };
    /// Binary right-literal consumer whose operand list continues past its
    /// scalar `Def` result with a mixed drop tail: the literal victim is
    /// the operand-1 `Use`, operand 0 is the surviving `Use` the rewritten
    /// row binds, operand 2 is the `Def` result, and every operand past
    /// the result drops under the custody its own access declares — each
    /// `Use` under the zero-provenance custody
    /// [`BINARY_RIGHT_LITERAL_AUXILIARY_USES`](Self::BINARY_RIGHT_LITERAL_AUXILIARY_USES)
    /// requires, each `Def` under the occurrence-free custody
    /// [`BINARY_RIGHT_LITERAL_SCRATCH_DEFS`](Self::BINARY_RIGHT_LITERAL_SCRATCH_DEFS)
    /// requires. Declaring this shape attests that the surviving operand
    /// alone produces the folded result — `x /| 1` is `x` inside the
    /// carrier's bounds — and that every tail operand is inert once the
    /// literal folds: the zeroed high-half dividend an x86-64 `div`/`idiv`
    /// realization reads at operand 3, or the bound scratch `Def` an
    /// aarch64 clamped signed-divide realization writes at operand 3. A
    /// `UseDef` operand, a `Use` not defined only by zero
    /// materializations, a `Def` occurring anywhere else in the function,
    /// or any operand at positions 0 through 2 outside this grammar
    /// rejects.
    pub const BINARY_RIGHT_LITERAL_AUXILIARY_USES_OR_SCRATCH_DEFS: Self = Self {
        position: PairLiteralPosition::RightOperand,
        result_kind: PairOperandResult::SurvivingOperand,
        tail: PairTailCustody::AuxiliaryUsesOrScratchDefs,
    };
    /// Binary left-literal consumer whose folded result is a constant of
    /// the literal alone and whose operand list continues past its scalar
    /// `Def` result with a mixed drop tail: the literal victim is the
    /// operand-0 `Use`, operand 1 is a `Use` the fold drops because the
    /// constant result never reads it, operand 2 is the `Def` result, and
    /// every operand past the result drops under the custody its own
    /// access declares — each `Use` under the zero-provenance custody
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES`](Self::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES)
    /// requires, each `Def` under the occurrence-free custody
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT`](Self::BINARY_LEFT_LITERAL_CONSTANT_RESULT)
    /// requires. Declaring this shape attests the operand-0 literal alone
    /// fixes the result — `0 /| x` is zero for every `x` the saturating
    /// divide's proven nonzero divisor admits — and that every tail
    /// operand is inert once the literal folds: the zeroed high-half
    /// dividend `Use` an x86-64 `div`/`idiv` realization reads, or the
    /// bound scratch `Def` an aarch64 clamped signed-divide realization
    /// writes. A `UseDef` operand, a `Use` not defined only by zero
    /// materializations, a `Def` occurring anywhere else in the function,
    /// or any operand at positions 0 through 2 outside this grammar
    /// rejects.
    pub const BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES_OR_SCRATCH_DEFS: Self = Self {
        position: PairLiteralPosition::LeftOperand,
        result_kind: PairOperandResult::ConstantOfLiteral,
        tail: PairTailCustody::AuxiliaryUsesOrScratchDefs,
    };
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
        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
        immediate_bound: PairImmediateBound::Encoding(4095),
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::ISOLATED,
        machine_effects: PairMachineEffects::ISOLATED,
    };
    /// Eliminate `MaterializeI64` feeding the left operand of `ExactAddI64`:
    /// exact addition commutes, so `literal + x` rewrites to the same
    /// `ExactAddI64Immediate` form `x + literal` uses. The same catalog
    /// selection admits both grammars; the pair disambiguates by which `Use`
    /// position the folded literal occupies.
    pub const EXACT_ADD_LEFT_IMMEDIATE_U12: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
        ..Self::EXACT_ADD_IMMEDIATE_U12
    };
    pub const EXACT_SUBTRACT_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::ExactSubtractI64,
        rewritten: MachineSemanticKind::ExactSubtractI64Immediate,
        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
        immediate_bound: PairImmediateBound::Encoding(4095),
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::ISOLATED,
        machine_effects: PairMachineEffects::ISOLATED,
    };
    pub const COMPARE_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::CompareI64,
        rewritten: MachineSemanticKind::CompareI64Immediate,
        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
        immediate_bound: PairImmediateBound::Encoding(4095),
        result: PairResultDisposition::ImplicitUnits,
        unit_effects: PairUnitEffects::ISOLATED,
        machine_effects: PairMachineEffects::ISOLATED,
    };
    /// Eliminate `MaterializeI64` feeding the operand-0 `Use` — the
    /// minuend — of `CompareI64`: `literal - x` rewrites to the
    /// `CompareI64Immediate` form computing `x - literal`, the
    /// operand-swapped subtraction whose zero condition is identical but
    /// whose ordering predicates invert. The same catalog selection
    /// admits both operand positions; the pair disambiguates by which
    /// `Use` position the folded literal occupies. Under
    /// [`OperandSwappedUnitDefs`](PairMachineEffects::OPERAND_SWAPPED_UNIT_DEFS)
    /// the rewrite keeps the consumer's implicit unit definitions — the
    /// target condition state — bit-identical while changing the
    /// relation they encode, admitted only while every reader each
    /// defined unit can reach through the function's CFG is
    /// equality-sensing.
    pub const COMPARE_LEFT_IMMEDIATE_U12: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_OPERAND_SWAP,
        machine_effects: PairMachineEffects::OPERAND_SWAPPED_UNIT_DEFS,
        ..Self::COMPARE_IMMEDIATE_U12
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
        operand_shape: PairOperandShape::UNARY_LITERAL,
        // The folded value always encodes as a `MaterializeI64` constant; no
        // target immediate bound applies to the source literal itself.
        immediate_bound: PairImmediateBound::Encoding(u64::MAX),
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::ISOLATED,
        machine_effects: PairMachineEffects::ISOLATED,
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
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Encoding(4095),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::ISOLATED,
            machine_effects: PairMachineEffects::indexed_pointer_read_fold(1),
        };
        assert!(
            matches!(
                rule.machine_effects.non_unit,
                PairNonUnitSurface::IndexedPointerRead { index_operand }
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
    /// [`Isolated`](PairMachineEffects::ISOLATED) relationship.
    ///
    /// The immediate bound is the narrowest byte-offset field any target's
    /// `AddressOffset` encoder admits: aarch64 `add xD, xN, #imm12` carries a
    /// 12-bit unsigned displacement, so a target-independent rule declares
    /// 4095 even though x86-64's `lea` disp32 form would admit more.
    pub const BYTE_VIEW_ADDRESS_OFFSET_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::ByteViewAddress,
        rewritten: MachineSemanticKind::AddressOffset,
        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
        immediate_bound: PairImmediateBound::Encoding(4095),
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::ISOLATED,
        machine_effects: PairMachineEffects::ISOLATED,
    };
    /// Eliminate `MaterializeI64` feeding the operand-0 backing operand of
    /// `ByteViewAddress`: the projection computes `(backing + offset)`
    /// modulo 2^64 — a commutative modular address addition — so a
    /// materialized backing literal rewrites to the same constant-offset
    /// `AddressOffset` form the operand-1 offset fold uses, with the
    /// operand-1 `Use` surviving as the rewritten row's base. The same
    /// catalog selection admits both operand positions; the pair
    /// disambiguates by which `Use` position the folded literal occupies.
    pub const BYTE_VIEW_ADDRESS_BACKING_U12: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
        ..Self::BYTE_VIEW_ADDRESS_OFFSET_U12
    };

    /// Eliminate `MaterializeI64` feeding the divisor operand of
    /// `ExactDivideU64` when the literal is exactly one: an unsigned divide
    /// by one returns the dividend, so the rewrite is a `CopyI64` of the
    /// surviving operand-0 register. The declared surface carries the
    /// dimensions a `div` realization brings: the consumer may encode an
    /// architectural fault — divide by zero or quotient overflow — which
    /// the folded divisor of one discharges under
    /// [`FaultDischargedByLiteral`](PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL),
    /// its operands may carry the register pins the pinned-operand form
    /// requires under
    /// [`BOUND_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_CONSUMER_OPERANDS),
    /// and every `Use` operand past the operand-2 `Def` result — the
    /// zeroed high-half scratch a realization like x86-64 `div` reads — is
    /// dropped under
    /// [`BINARY_RIGHT_LITERAL_AUXILIARY_USES`](PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES),
    /// which requires each such register to be defined only by zero
    /// materializations. Targets whose divide row carries no auxiliary
    /// `Use` — aarch64's `udiv` — admit the same rule with an empty
    /// auxiliary tail.
    pub const EXACT_DIVIDE_ONE_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::ExactDivideU64,
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES,
            immediate_bound: PairImmediateBound::Exactly(1),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL
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
    /// [`FaultDischargedByObligation`](PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION)
    /// so the descriptor never claims the literal did the obligation's
    /// work. The operands may carry the register pins the pinned-operand
    /// realization requires under
    /// [`BOUND_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_CONSUMER_OPERANDS),
    /// the operand-1 divisor `Use` is dropped with the form because the
    /// constant result never reads it, and every `Use` operand past the
    /// operand-2 `Def` result — the zeroed high-half input an x86-64
    /// `div` realization reads as the dividend's upper half — is dropped
    /// under
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES),
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
            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION
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
    /// [`FaultDischargedByLiteral`](PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL),
    /// its operands may carry the register pins and early-clobber marks a
    /// pinned-scratch realization requires under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS),
    /// and every operand past the operand-2 `Def` result — the dead
    /// quotient scratch an x86-64 `idiv` realization writes — is a `Def`
    /// the fold drops under
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT),
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
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(1),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(1)),
            "the fault discharge holds only for the divisor literal one"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the divisor operand of
    /// `WrappingRemainderI64` when the literal is `u64::MAX` — the
    /// normalized-i64 divisor `-1`: a wrapping remainder by minus one is
    /// always zero — `x % -1` is `0` for every `x`, and `i64::MIN % -1`
    /// is the exceptional case the kind's semantics defines to produce
    /// zero rather than trap — so the rewrite is a `MaterializeI64` of
    /// the constant zero at the consumer's result register. The declared
    /// surface carries the dimensions an `idiv`-class realization brings:
    /// the consumer may encode an architectural fault — divide by zero
    /// or quotient overflow — which the folded divisor of minus one
    /// discharges under
    /// [`FaultDischargedByLiteral`](PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL):
    /// a divisor of `-1` can never divide by zero, and the one dividend
    /// whose `idiv` would overflow is the case the kind defines away —
    /// the x86-64 realization's `-1` guard skips the divide for exactly
    /// it — so the encoded fault cannot fire on this instruction. The
    /// operands may carry the register pins and early-clobber marks a
    /// pinned-scratch realization requires under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS),
    /// the operand-0 dividend `Use` is dropped with the form because the
    /// constant result never reads it, and every operand past the
    /// operand-2 `Def` result — the dead quotient scratch an x86-64
    /// `idiv` realization writes — is a `Def` the fold drops under
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT),
    /// which requires each such register to occur nowhere else in the
    /// function. Targets whose remainder row carries no scratch `Def` —
    /// aarch64's `udiv`/`msub` realization — admit the same rule with an
    /// empty scratch tail. The family shares its consumer kind and
    /// operand position with the divisor-one fold; the grammars stay
    /// disjoint on the folded literal's value — `1` admits only the
    /// divisor-one pair, `u64::MAX` only this one.
    pub const WRAPPING_REMAINDER_MINUS_ONE_MATERIALIZE: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::WrappingRemainderI64,
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(u64::MAX),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(u64::MAX)),
            "the fault discharge holds only for the divisor literal minus one"
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
    /// [`FaultDischargedByObligation`](PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION)
    /// so the descriptor never claims the literal did the obligation's
    /// work. The operands may carry the register pins and early-clobber
    /// marks a pinned-scratch realization requires under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS),
    /// the operand-1 divisor `Use` is dropped with the form because the
    /// constant result never reads it, and every operand past the
    /// operand-2 `Def` result — the dead quotient scratch an x86-64
    /// `idiv` realization writes — is a `Def` the fold drops under
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT),
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
            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION
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
    /// [`Isolated`](PairMachineEffects::ISOLATED) and
    /// [`Isolated`](PairUnitEffects::ISOLATED) surfaces apply. The
    /// operand-0 `Use` is dropped because the constant result never reads
    /// it, under the
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT)
    /// grammar's custody: every `Def` operand past the result must occur
    /// nowhere else in the function.
    pub const BITWISE_AND_ZERO_MATERIALIZE: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::BitwiseAndI64,
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::ISOLATED,
            machine_effects: PairMachineEffects::ISOLATED,
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
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT)
    /// the dropped operand-1 `Use` is never read and every `Def` operand
    /// past the result drops under the same occurrence-free custody the
    /// right grammar declares. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const BITWISE_AND_ZERO_LEFT_MATERIALIZE: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
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
    /// [`Isolated`](PairMachineEffects::ISOLATED) and
    /// [`Isolated`](PairUnitEffects::ISOLATED) surfaces apply. The
    /// operand-0 `Use` survives under the ordinary
    /// [`BINARY_RIGHT_LITERAL`](PairOperandShape::BINARY_RIGHT_LITERAL)
    /// grammar: the rewritten row binds it as its `Use` operand.
    pub const BITWISE_XOR_ZERO_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::BitwiseXorI64,
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::ISOLATED,
            machine_effects: PairMachineEffects::ISOLATED,
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
    /// [`BINARY_LEFT_LITERAL`](PairOperandShape::BINARY_LEFT_LITERAL) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const BITWISE_XOR_ZERO_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
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
    /// [`Isolated`](PairMachineEffects::ISOLATED) and
    /// [`Isolated`](PairUnitEffects::ISOLATED) surfaces apply. The
    /// operand-0 `Use` survives under the ordinary
    /// [`BINARY_RIGHT_LITERAL`](PairOperandShape::BINARY_RIGHT_LITERAL)
    /// grammar: the rewritten row binds it as its `Use` operand.
    pub const WRAPPING_ADD_ZERO_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::WrappingAddI64,
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::ISOLATED,
            machine_effects: PairMachineEffects::ISOLATED,
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
    /// [`BINARY_LEFT_LITERAL`](PairOperandShape::BINARY_LEFT_LITERAL) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const WRAPPING_ADD_ZERO_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
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
    /// [`Isolated`](PairMachineEffects::ISOLATED) and
    /// [`Isolated`](PairUnitEffects::ISOLATED) surfaces apply. The
    /// operand-0 `Use` survives under the ordinary
    /// [`BINARY_RIGHT_LITERAL`](PairOperandShape::BINARY_RIGHT_LITERAL)
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
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Exactly(u64::MAX),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::ISOLATED,
            machine_effects: PairMachineEffects::ISOLATED,
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
    /// [`BINARY_LEFT_LITERAL`](PairOperandShape::BINARY_LEFT_LITERAL) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const BITWISE_AND_ONES_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
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
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// that definition may retire only while it is dead in the function —
    /// a conditional branch reading `nzcv` would go stale — and the
    /// consumer's clobbers retire wholesale, as the x86-64 row's `rflags`
    /// clobber does. The consumer's operands may carry the `early_clobber`
    /// mark the x86-64 saturating realization declares on its result —
    /// the hazard it names exists only inside the dropped operand list —
    /// under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The operand-0 `Use` survives under the ordinary
    /// [`BINARY_RIGHT_LITERAL`](PairOperandShape::BINARY_RIGHT_LITERAL)
    /// grammar: the rewritten row binds it as its `Use` operand. The u64
    /// row carries no operand past its `Def` result; the narrower and
    /// signed carriers bind the clamped row whose bound scratch the
    /// scratch-`Def` grammars below drop.
    pub const SATURATING_ADD_ZERO_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingAdd(SaturatingCarrier::U64),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
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
    /// [`BINARY_LEFT_LITERAL`](PairOperandShape::BINARY_LEFT_LITERAL) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const SATURATING_ADD_ZERO_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
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
    /// [`BINARY_RIGHT_LITERAL_SCRATCH_DEFS`](PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS):
    /// each dropped `Def` register must occur nowhere else in the
    /// function. The unit surface is the family's own: the aarch64
    /// clamped row still defines `nzcv` — retired under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// only while dead in the function — both targets mark the dropped
    /// result and scratch `early_clobber`, admitted under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS),
    /// and the x86-64 row's `rflags` clobber retires unconditionally.
    const fn saturating_add_zero_clamped(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingAdd(carrier),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
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
    /// [`BINARY_LEFT_LITERAL_SCRATCH_DEFS`](PairOperandShape::BINARY_LEFT_LITERAL_SCRATCH_DEFS)
    /// grammar attests that commutation and drops the same bound-scratch
    /// `Def` tail under occurrence-free custody.
    const fn saturating_add_zero_clamped_left(carrier: SaturatingCarrier) -> Self {
        Self {
            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_SCRATCH_DEFS,
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
    /// `SaturatingAdd` on an unsigned carrier when the literal is exactly
    /// the carrier's maximum: `x +| MAX` is `MAX` for every `x` the
    /// carrier admits, because `x + MAX` reaches the carrier's upper
    /// bound and saturates to it — so the rewrite is a `MaterializeI64`
    /// of that maximum at the consumer's result register. Only unsigned
    /// carriers admit the fold: under signed saturation `x +| MAX` is
    /// `x + MAX` unclamped for every negative `x`, not a constant, so the
    /// family binds no signed carrier and no signed carrier ever names an
    /// admitted maximum-literal grammar. The operand-0 `Use` is dropped
    /// with the form because the constant result never reads it, and
    /// every operand past the operand-2 `Def` result — the bound scratch
    /// `Def` a clamped saturating-add realization computes its saturation
    /// bound through — drops under the
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT)
    /// grammar's occurrence-free custody: a scratch output another
    /// instruction read or defined would leave a use of a register the
    /// rewrite stopped defining. The consumer carries the same implicit
    /// unit surface the identity family retires: the saturating-add rows
    /// define `nzcv` on aarch64 — every carrier's realization is
    /// flag-setting — while the isolated `MaterializeI64` defines
    /// nothing, so under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// that definition may retire only while it is dead in the function,
    /// and the consumer's clobbers retire wholesale, as the x86-64 row's
    /// `rflags` clobber does. The consumer's operands may carry the
    /// `early_clobber` marks the saturating realizations declare on their
    /// `Def` outputs — the hazard they name exists only inside the
    /// dropped operand list — under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The family shares its consumer kind and operand positions with the
    /// zero-identity fold: the literal's value names which family a
    /// `SaturatingAdd` fold belongs to, and the grammars stay disjoint on
    /// that value.
    const fn saturating_add_upper_bound(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingAdd(carrier),
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(carrier.maximum_bits()),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
        };
        assert!(
            !carrier.is_signed()
                && matches!(
                    rule.immediate_bound,
                    PairImmediateBound::Exactly(bound) if bound == carrier.maximum_bits()
                ),
            "the upper-bound fold holds only for an unsigned carrier's maximum literal"
        );
        rule
    }

    /// The left-operand upper-bound fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of `SaturatingAdd` on an unsigned carrier when the
    /// literal is exactly the carrier's maximum — `MAX +| x` is `MAX`
    /// for every `x`. The
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT)
    /// grammar attests the operand-0 literal alone fixes the result —
    /// saturating addition's commutation makes `MAX +| x` and `x +| MAX`
    /// the same fold, but the constant-result grammar needs none of it:
    /// the operand-1 `Use` drops because the constant never reads it, and
    /// every operand past the operand-2 `Def` result drops under the same
    /// occurrence-free custody.
    const fn saturating_add_upper_bound_left(carrier: SaturatingCarrier) -> Self {
        Self {
            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
            ..Self::saturating_add_upper_bound(carrier)
        }
    }

    /// The saturating-add upper-bound rules: one pair per literal `Use`
    /// position for each unsigned carrier — `x +| MAX` and `MAX +| x`
    /// are both `MAX` under unsigned saturation. The u64 carrier binds
    /// the three-operand row; every other unsigned carrier binds the
    /// clamped row whose bound scratch `Def` drops under the
    /// constant-result grammar's scratch-defs custody. Signed carriers
    /// admit no maximum fold — `x +| MAX` there is `x + MAX` unclamped
    /// for every negative `x` — so the family declares no signed pair.
    /// The family shares its consumer kind and operand positions with
    /// the zero-identity fold; the grammars stay disjoint on the folded
    /// literal's value.
    pub const SATURATING_ADD_UPPER_BOUND_MATERIALIZATIONS: [Self; 8] = [
        Self::saturating_add_upper_bound(SaturatingCarrier::U8),
        Self::saturating_add_upper_bound_left(SaturatingCarrier::U8),
        Self::saturating_add_upper_bound(SaturatingCarrier::U16),
        Self::saturating_add_upper_bound_left(SaturatingCarrier::U16),
        Self::saturating_add_upper_bound(SaturatingCarrier::U32),
        Self::saturating_add_upper_bound_left(SaturatingCarrier::U32),
        Self::saturating_add_upper_bound(SaturatingCarrier::U64),
        Self::saturating_add_upper_bound_left(SaturatingCarrier::U64),
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
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// that definition may retire only while it is dead in the function —
    /// a conditional branch reading `nzcv` would go stale — and the
    /// consumer's clobbers retire wholesale, as the x86-64 row's `rflags`
    /// clobber does. The consumer's operands may carry the `early_clobber`
    /// mark the x86-64 saturating realization declares on its result —
    /// the hazard it names exists only inside the dropped operand list —
    /// under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The operand-0 `Use` survives under the ordinary
    /// [`BINARY_RIGHT_LITERAL`](PairOperandShape::BINARY_RIGHT_LITERAL)
    /// grammar: the rewritten row binds it as its `Use` operand.
    const fn saturating_subtract_zero_unsigned(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
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
    /// [`BINARY_RIGHT_LITERAL_SCRATCH_DEFS`](PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS):
    /// each dropped `Def` register must occur nowhere else in the
    /// function. The unit surface is the family's own: the aarch64
    /// clamped row still defines `nzcv` — retired under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// only while dead in the function — both targets mark the dropped
    /// result and scratch `early_clobber`, admitted under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS),
    /// and the x86-64 row's `rflags` clobber retires unconditionally.
    const fn saturating_subtract_zero_clamped(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
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
    /// The unsigned `0 -| x` constant fold is a separate family below,
    /// not a left-literal member of this one.
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

    /// Eliminate `MaterializeI64` feeding the operand-0 `Use` — the
    /// minuend — of `SaturatingSubtract` on an unsigned carrier when the
    /// literal is exactly zero: `0 -| x` is `0` for every `x`, because
    /// `0 - x` underflows the carrier's lower bound and saturates to it —
    /// so the rewrite is a `MaterializeI64` of the constant zero at the
    /// consumer's result register. Only unsigned carriers admit the fold:
    /// under signed saturation `0 -| x` is `-x` clamped to the carrier's
    /// bounds, not a constant, so the family binds the three-operand
    /// unsigned row alone and no signed carrier ever names an admitted
    /// operand-0 grammar. The operand-1 subtrahend `Use` is dropped with
    /// the form because the constant result never reads it, and every
    /// operand past the operand-2 `Def` result — none on the unsigned
    /// row — would drop under the
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT)
    /// grammar's occurrence-free custody. The consumer carries the same
    /// implicit unit surface the identity family retires: the unsigned
    /// saturating-subtract row defines `nzcv` on aarch64 — its
    /// flag-setting `subs` realization — while the isolated
    /// `MaterializeI64` defines nothing, so under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// that definition may retire only while it is dead in the function,
    /// and the consumer's clobbers retire wholesale, as the x86-64 row's
    /// `rflags` clobber does. The consumer's operands may carry the
    /// `early_clobber` mark the x86-64 saturating realization declares on
    /// its result — the hazard it names exists only inside the dropped
    /// operand list — under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The family shares its consumer kind with the right-zero identity
    /// fold; the grammars stay disjoint on the folded literal's operand
    /// position.
    const fn saturating_subtract_zero_minuend(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
        };
        assert!(
            !carrier.is_signed() && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the zero-minuend fold holds only for an unsigned carrier's zero literal"
        );
        rule
    }

    /// The saturating-subtract zero-minuend rules: one left-literal pair
    /// for each unsigned carrier's three-operand row — `0 -| x` is `0`
    /// under unsigned saturation. Signed carriers admit no operand-0 fold
    /// — `0 -| x` there is `-x` clamped to the carrier's bounds, not a
    /// constant — so the family declares no signed pair at all. The
    /// family shares its consumer kind with the right-zero identity fold;
    /// the grammars stay disjoint on the folded literal's operand
    /// position.
    pub const SATURATING_SUBTRACT_ZERO_MINUEND_MATERIALIZATIONS: [Self; 4] = [
        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U8),
        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U16),
        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U32),
        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U64),
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` — the
    /// subtrahend — of `SaturatingSubtract` on an unsigned carrier when
    /// the literal is exactly the carrier's maximum: `x -| MAX` is `0`
    /// for every `x` the carrier admits, because `x - MAX` underflows
    /// the carrier's lower bound — saturating to it — for every
    /// `x < MAX` and is exactly zero at `x == MAX` — so the rewrite is a
    /// `MaterializeI64` of the constant zero at the consumer's result
    /// register. Only unsigned carriers admit the fold: under signed
    /// saturation `x -| MAX` is `x - MAX` clamped to the carrier's lower
    /// bound for every negative `x`, not a constant, so the family binds
    /// the three-operand unsigned row alone and no signed carrier ever
    /// names an admitted maximum-subtrahend grammar. The grammar is
    /// deliberately asymmetric: `MAX -| x` is `MAX - x`, not a constant,
    /// so the family declares no left-literal pair and a maximum literal
    /// recorded at operand 0 names no admitted grammar — the unsigned
    /// `0 -| x` constant fold is the zero-minuend family, disjoint on
    /// the folded literal's operand position. The operand-0 minuend
    /// `Use` is dropped with the form because the constant result never
    /// reads it, and every operand past the operand-2 `Def` result —
    /// none on the unsigned row — would drop under the
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT)
    /// grammar's occurrence-free custody. The consumer carries the same
    /// implicit unit surface the sibling families retire: the unsigned
    /// saturating-subtract row defines `nzcv` on aarch64 — its
    /// flag-setting `subs` realization — while the isolated
    /// `MaterializeI64` defines nothing, so under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// that definition may retire only while it is dead in the function,
    /// and the consumer's clobbers retire wholesale, as the x86-64 row's
    /// `rflags` clobber does. The consumer's operands may carry the
    /// `early_clobber` mark the x86-64 saturating realization declares on
    /// its result — the hazard it names exists only inside the dropped
    /// operand list — under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The family shares its consumer kind and operand position with the
    /// right-zero identity fold: the folded literal's value names which
    /// `SaturatingSubtract` subtrahend family a fold belongs to.
    const fn saturating_subtract_upper_bound(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(carrier.maximum_bits()),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
        };
        assert!(
            !carrier.is_signed()
                && matches!(
                    rule.immediate_bound,
                    PairImmediateBound::Exactly(bound) if bound == carrier.maximum_bits()
                ),
            "the upper-bound subtrahend fold holds only for an unsigned carrier's maximum literal"
        );
        rule
    }

    /// The saturating-subtract upper-bound rules: one right-literal pair
    /// for each unsigned carrier's three-operand row — `x -| MAX` is `0`
    /// under unsigned saturation. Signed carriers admit no
    /// maximum-subtrahend fold — `x -| MAX` there is `x - MAX` clamped
    /// to the carrier's lower bound for every negative `x`, not a
    /// constant — so the family declares no signed pair at all. The
    /// family shares its consumer kind and operand position with the
    /// right-zero identity fold; the grammars stay disjoint on the
    /// folded literal's value.
    pub const SATURATING_SUBTRACT_UPPER_BOUND_MATERIALIZATIONS: [Self; 4] = [
        Self::saturating_subtract_upper_bound(SaturatingCarrier::U8),
        Self::saturating_subtract_upper_bound(SaturatingCarrier::U16),
        Self::saturating_subtract_upper_bound(SaturatingCarrier::U32),
        Self::saturating_subtract_upper_bound(SaturatingCarrier::U64),
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` — the
    /// divisor — of `SaturatingDivide` on `carrier` when the literal is
    /// exactly one: a saturating divide by one is the dividend — `x /| 1`
    /// is `x` inside every carrier's bounds, and the signed `MIN /| -1`
    /// clamp lies outside the divisor this grammar admits — so the
    /// rewrite is a `CopyI64` of the surviving operand-0 register at the
    /// consumer's result register. The grammar is deliberately
    /// asymmetric: `1 /| x` is not `x`, so the family declares no
    /// left-literal pair and a literal recorded at operand 0 names no
    /// admitted grammar.
    ///
    /// This is the first family whose consumer both may architecturally
    /// fault *and* retires implicit unit definitions the rewritten form
    /// does not carry, so it declares
    /// [`FaultDischargedByLiteralDeadUnitDefs`](PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL_DEAD_UNIT_DEFS):
    /// the divisor literal of one discharges the encoded
    /// `MayArchitecturalFaultV1` an x86-64 `div`/`idiv` realization
    /// carries — a divide by one can neither divide by zero nor overflow,
    /// and unlike the exact-divide family the kind's carried
    /// fault-recovery obligation is not the discharger — while every
    /// implicit unit the consumer record defines, the `nzcv` an aarch64
    /// signed row writes, retires only while dead in the function. The
    /// consumer's operands may carry the register pins the x86-64 `div`
    /// realization requires and the `early_clobber` marks the clamped
    /// aarch64 signed rows declare on their `Def` outputs under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The operand-0 `Use` survives into the rewritten row; every operand
    /// past the operand-2 `Def` result drops under
    /// [`BINARY_RIGHT_LITERAL_AUXILIARY_USES_OR_SCRATCH_DEFS`](PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES_OR_SCRATCH_DEFS) —
    /// the zeroed high-half dividend `Use` an x86-64 `div`/`idiv` reads,
    /// provably defined only by zero materializations, or the bound
    /// scratch `Def` an aarch64 clamped signed row writes, occurring
    /// nowhere else in the function. The unsigned carriers bind the
    /// `divide_u64` row — the bare three-operand `udiv` on aarch64, the
    /// pinned four-operand `div` row carrying its zeroed-rdx auxiliary
    /// `Use` on x86-64 — while every signed carrier binds the
    /// `saturating_divide_signed` row whose tail the grammar drops under
    /// its own access's custody: the pinned `idiv` row's zeroed-rdx
    /// auxiliary `Use` on x86-64, the clamped row's bound scratch `Def`
    /// on aarch64.
    const fn saturating_divide_one(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingDivide(carrier),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES_OR_SCRATCH_DEFS,
            immediate_bound: PairImmediateBound::Exactly(1),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL_DEAD_UNIT_DEFS,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(1)),
            "the fault discharge holds only for the divisor literal one"
        );
        rule
    }

    /// The saturating-divide identity rules: one right-literal pair for
    /// every carrier — `x /| 1` is `x` under signed or unsigned
    /// saturation. Saturating division does not commute — `1 /| x` is not
    /// `x` — so the family declares no left-literal pair. The
    /// unsigned-carrier rules bind the `divide_u64` row and the
    /// signed-carrier rules the `saturating_divide_signed` row; the mixed
    /// drop-tail grammar covers every realization's operand-3 role — the
    /// auxiliary `Use` x86-64's pinned divide rows read and the bound
    /// scratch `Def` aarch64's clamped signed row writes — as well as the
    /// empty tail aarch64's `udiv` leaves.
    pub const SATURATING_DIVIDE_ONE_COPIES: [Self; 8] = [
        Self::saturating_divide_one(SaturatingCarrier::U8),
        Self::saturating_divide_one(SaturatingCarrier::U16),
        Self::saturating_divide_one(SaturatingCarrier::U32),
        Self::saturating_divide_one(SaturatingCarrier::U64),
        Self::saturating_divide_one(SaturatingCarrier::I8),
        Self::saturating_divide_one(SaturatingCarrier::I16),
        Self::saturating_divide_one(SaturatingCarrier::I32),
        Self::saturating_divide_one(SaturatingCarrier::I64),
    ];

    /// Eliminate `MaterializeI64` feeding the operand-0 `Use` — the
    /// dividend — of `SaturatingDivide` on `carrier` when the literal is
    /// exactly zero: a saturating divide of a zero dividend is always
    /// zero — `0 /| x` is `0` inside every carrier's bounds, and a zero
    /// quotient reaches no saturation edge — so the rewrite is a
    /// `MaterializeI64` of the constant zero at the consumer's result
    /// register. The folded dividend is not the value that discharges
    /// the consumer's encoded architectural fault: a quotient of zero
    /// can never overflow, but the divide-by-zero case the encoding
    /// could still name is unreachable only because the
    /// `SaturatingDivide` kind carries its proven nonzero divisor as an
    /// accepted obligation — the fold declares
    /// [`FaultDischargedByObligationDeadUnitDefs`](PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION_DEAD_UNIT_DEFS)
    /// so the descriptor never claims the literal did the obligation's
    /// work, and every implicit unit the consumer record defines — the
    /// `nzcv` an aarch64 signed row writes — retires only while dead in
    /// the function. The consumer's operands may carry the register pins
    /// the x86-64 `div`/`idiv` realizations require and the
    /// `early_clobber` marks the clamped aarch64 signed rows declare on
    /// their `Def` outputs under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The operand-1 divisor `Use` is dropped with the form because the
    /// constant result never reads it; every operand past the operand-2
    /// `Def` result drops under
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES_OR_SCRATCH_DEFS`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES_OR_SCRATCH_DEFS)
    /// — each `Use`, the zeroed high-half dividend an x86-64 `div`/`idiv`
    /// realization reads, provably defined only by zero materializations
    /// since a literal of zero fixes only the low dividend half, and each
    /// `Def`, the bound scratch an aarch64 clamped signed-divide
    /// realization writes, occurring nowhere else in the function. The
    /// unsigned carriers bind the `divide_u64` row — the bare
    /// three-operand `udiv` on aarch64, the pinned four-operand `div`
    /// row carrying its zeroed-rdx auxiliary `Use` on x86-64 — while
    /// every signed carrier binds the `saturating_divide_signed` row
    /// whose tail the grammar drops under its own access's custody: the
    /// pinned `idiv` row's zeroed-rdx auxiliary `Use` on x86-64, the
    /// clamped row's bound scratch `Def` on aarch64. The family shares
    /// its consumer kind with the divisor-one fold; the grammars stay
    /// disjoint on the folded literal's operand position.
    const fn saturating_divide_zero_dividend(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingDivide(carrier),
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape:
                PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES_OR_SCRATCH_DEFS,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION_DEAD_UNIT_DEFS,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION_DEAD_UNIT_DEFS
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the obligation discharge holds only for the dividend literal zero"
        );
        rule
    }

    /// The saturating-divide zero-dividend rules: one left-literal pair
    /// for every carrier — `0 /| x` is `0` under signed or unsigned
    /// saturation. The unsigned-carrier rules bind the `divide_u64` row
    /// and the signed-carrier rules the `saturating_divide_signed` row;
    /// the mixed drop-tail grammar covers every realization's operand-3
    /// role — the auxiliary `Use` x86-64's pinned divide rows read and
    /// the bound scratch `Def` aarch64's clamped signed row writes — as
    /// well as the empty tail aarch64's `udiv` leaves.
    pub const SATURATING_DIVIDE_ZERO_DIVIDEND_MATERIALIZATIONS: [Self; 8] = [
        Self::saturating_divide_zero_dividend(SaturatingCarrier::U8),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::U16),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::U32),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::U64),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::I8),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::I16),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::I32),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::I64),
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

    /// The consumer operand index the literal victim must occupy — the
    /// position axis alone decides it.
    pub const fn victim_operand(self) -> u16 {
        match self.operand_shape.position {
            PairLiteralPosition::RightOperand => 1,
            PairLiteralPosition::LeftOperand | PairLiteralPosition::SoleOperand => 0,
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
    /// the literal itself for the immediate forms and the copy fold, the
    /// extension's exact output bits for the unary extension folds, or the
    /// constant a constant-result grammar fixes — zero for the
    /// zero-producing families and the carrier maximum for the unsigned
    /// saturating-add upper-bound fold. The result is the recorded
    /// `immediate` in [`crate::LiteralFoldAction`].
    pub fn fold_immediate(self, literal: u64) -> Option<u64> {
        match self.operand_shape.result_kind {
            // Surviving-operand and operand-swapped grammars embed the
            // literal itself.
            PairOperandResult::SurvivingOperand | PairOperandResult::SwappedOperand => {
                Some(literal)
            }
            // The constant-result grammars record the constant the
            // rewritten `MaterializeI64` embeds: a remainder by one or of
            // a zero dividend is always zero, an unsigned or saturating
            // divide of a zero dividend is always zero, whatever the
            // folded literal was, a bitwise-and with a zero literal is
            // always zero at either `Use` position, an unsigned
            // saturating subtract whose subtrahend literal is the
            // carrier's maximum saturates to zero, and an unsigned
            // saturating add whose literal is the carrier's maximum
            // saturates to that maximum at either `Use` position — the
            // admitted literal itself already carries the bound.
            PairOperandResult::ConstantOfLiteral => match self.consumer {
                MachineSemanticKind::SaturatingAdd(carrier) => Some(carrier.maximum_bits()),
                _ => Some(0),
            },
            PairOperandResult::LiteralRecompute => match self.consumer {
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
            // with a zero literal, a saturating subtract with a zero
            // right literal, or a saturating divide by a divisor literal
            // of one is the other operand — `x ^ 0` and `0 ^ x`
            // are both `x`, `x + 0` and `0 + x` are both `x` modulo 2^64,
            // `x & MAX` and `MAX & x` are both `x`, `x +| 0` and `0 +| x`
            // are both `x` inside the carrier's bounds, `x -| 0` is
            // `x` inside the carrier's bounds, and `x /| 1` is `x` under
            // either signedness: the `CopyI64` rewrite
            // binds the surviving register the recorded action names. The
            // consumer guard keeps each rule bound to its own consumer
            // kind — the xor rule never rewrites an add, the add rule
            // never rewrites an and, the and-ones rule never rewrites
            // either, each saturating-add rule rewrites only the carrier
            // kind its pair admits, and each saturating-subtract or
            // saturating-divide rule likewise — while the subtraction and
            // division families stay bound to the right-literal grammar
            // alone: `0 -| x` is not `x` and `1 /| x` is not `x`.
            (
                MachineSemanticKind::CopyI64,
                kind @ (SelectedInstructionKind::BitwiseXorI64
                | SelectedInstructionKind::WrappingAddI64
                | SelectedInstructionKind::BitwiseAndI64
                | SelectedInstructionKind::SaturatingAdd { .. }
                | SelectedInstructionKind::SaturatingSubtract { .. }
                | SelectedInstructionKind::SaturatingDivide { .. }),
            ) if machine_semantic_kind(kind) == self.consumer => {
                Some(SelectedInstructionKind::CopyI64)
            }
            // A remainder by one or of a zero dividend is always zero, an
            // unsigned divide of a zero dividend is always zero, a
            // saturating divide of a zero dividend is always zero inside
            // the carrier's bounds, an unsigned saturating subtract of a
            // zero minuend is always zero — `0 -| x` saturates to the
            // carrier's lower bound — and an unsigned saturating
            // subtract by the carrier's maximum is always zero — `x -|
            // MAX` saturates to the same lower bound — a bitwise-and
            // with a zero literal is always zero at either `Use`
            // position, and an unsigned saturating add with the carrier's
            // maximum literal is always that maximum at either `Use`
            // position — `x +| MAX`
            // saturates to the carrier's upper bound: the `MaterializeI64`
            // rewrite materializes the folded constant at the result
            // register, sign-matched and admitted by its scalar type. The
            // consumer guard keeps each rule bound to its own consumer
            // kind — the remainder rules never rewrite an and or a
            // divide, the and-zero rule never rewrites a remainder or a
            // divide, the exact-divide zero-dividend rule never rewrites
            // either, each saturating-divide zero-dividend rule rewrites
            // only the carrier kind its pair admits, each
            // saturating-subtract zero-minuend or upper-bound rule
            // likewise, and each
            // saturating-add upper-bound rule likewise — while each pair
            // sharing a kind legitimately coexists: admission already
            // fixed which grammar applies by the folded literal's operand
            // position or value, and the coexisting constant-result pairs
            // of a kind materialize the same constant.
            (
                MachineSemanticKind::MaterializeI64,
                kind @ (SelectedInstructionKind::WrappingRemainderI64 { .. }
                | SelectedInstructionKind::ExactDivideU64 { .. }
                | SelectedInstructionKind::SaturatingDivide { .. }
                | SelectedInstructionKind::SaturatingSubtract { .. }
                | SelectedInstructionKind::SaturatingAdd { .. }
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
