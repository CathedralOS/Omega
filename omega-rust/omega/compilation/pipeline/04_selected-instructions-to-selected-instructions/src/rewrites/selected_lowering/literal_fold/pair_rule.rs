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
//! any literal up to an encoding limit is admitted, whether that window
//! excludes the zero a dedicated constant-form pair binds, or the fold's
//! correctness requires one exact literal value — and, for families that
//! share one consumer kind and operand position, keeps the grammars
//! disjoint on the literal's value: the bitwise-and annihilator and
//! identity selections both fold `BitwiseAndI64` at either `Use`
//! position, and the compare family's immediate and zero realizations
//! both fold `CompareI64` at either `Use` position, so pair selection
//! matches the bound as well as the kind and
//! position. When a rule needs shape
//! data beyond those — a further non-isolated effect relationship or
//! further operand roles — extend this struct rather than re-inlining kind
//! matches in compute.

use register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
    RegisterOperandConstraint, RegisterUnitId, TargetRegisterEnvironmentConstraintKeys,
};
use selected_instructions::{
    MachineAlternative, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineMemoryEffect, MachineSemanticKind, SaturatingCarrier, SelectedFunction,
    SelectedInstruction, SelectedInstructionKind, SelectedOperand, VirtualRegisterId,
};
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

use crate::analyses::machine_effects::machine_semantic_kind;
use crate::rewrites::block_edges::{block_instructions, terminator_successors};
use crate::rewrites::operand_probes::isolated_declaration;

mod predicates;
mod rules;

use predicates::{
    auxiliary_uses, dead_scratch, droppable_tail, fault_discharged_alternative,
    implicit_defs_covered, implicit_unit_used, indexed_pointer_read_matches, isolated_alternative,
    swapped_unit_readers_equality_only,
};

/// How a pair rule's rewritten instruction delivers its output.
///
/// The producer admits the constraint-row shape matching this declared
/// channel instead of inferring it from operand counts; the independent
/// validator re-derives the same distinction from the consumer kind and row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PairResultDisposition {
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
pub(crate) struct PairUnitEffects {
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
pub(crate) enum PairConsumerBindingAdmission {
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
    pub(crate) const ISOLATED: Self = Self {
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
    pub(crate) const BOUND_CONSUMER_OPERANDS: Self = Self {
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
    pub(crate) const BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS: Self = Self {
        consumer_early_clobber: PairConsumerBindingAdmission::DroppedWithOperandList,
        ..Self::BOUND_CONSUMER_OPERANDS
    };

    /// Whether the constraint row's instruction-level unit traffic
    /// satisfies the declaration. Implicit *definitions* are the result
    /// channel and stay under `PairResultDisposition`. The requirement is
    /// the same under every axis composition: the rewritten row never
    /// carries unit traffic of its own.
    pub(crate) fn admits_row_units(self, row: &RegisterInstructionConstraint) -> bool {
        row.implicit_uses.is_empty() && row.clobbers.is_empty()
    }

    /// Whether one constraint-row operand carries no unit binding. The
    /// requirement is the same under every axis composition: the rewritten
    /// row's operands are rebuilt undecorated.
    pub(crate) fn admits_operand(self, operand: &RegisterOperandConstraint) -> bool {
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
    pub(crate) fn admits_consumer(self, consumer: &SelectedInstruction) -> bool {
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
    pub(crate) fn admits_producer(self, producer: &SelectedInstruction) -> bool {
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
pub(crate) struct PairMachineEffects {
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
pub(crate) enum PairNonUnitSurface {
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
pub(crate) enum PairFaultDischarge {
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
pub(crate) enum PairUnitDefRelation {
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
    pub(crate) const ISOLATED: Self = Self {
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
    pub(crate) const FAULT_DISCHARGED_BY_LITERAL: Self = Self {
        fault: PairFaultDischarge::DischargedByLiteral,
        ..Self::ISOLATED
    };

    /// The consumer's carried obligation discharges every encoded
    /// fault; implicit definitions stay covered.
    pub(crate) const FAULT_DISCHARGED_BY_OBLIGATION: Self = Self {
        fault: PairFaultDischarge::DischargedByObligation,
        ..Self::ISOLATED
    };

    /// The consumer's implicit unit definitions retire under the
    /// record-level deadness gate; no fault surface.
    pub(crate) const DEAD_CONSUMER_UNIT_DEFS: Self = Self {
        unit_defs: PairUnitDefRelation::RetiredWhenDead,
        ..Self::ISOLATED
    };

    /// The saturating divide's composition: the folded literal
    /// discharges every encoded fault while the consumer's implicit
    /// unit definitions retire under the deadness gate.
    pub(crate) const FAULT_DISCHARGED_BY_LITERAL_DEAD_UNIT_DEFS: Self = Self {
        fault: PairFaultDischarge::DischargedByLiteral,
        unit_defs: PairUnitDefRelation::RetiredWhenDead,
        ..Self::ISOLATED
    };

    /// The zero-dividend saturating divide's composition: the
    /// consumer's carried obligation discharges every encoded fault
    /// while its implicit unit definitions retire under the deadness
    /// gate.
    pub(crate) const FAULT_DISCHARGED_BY_OBLIGATION_DEAD_UNIT_DEFS: Self = Self {
        fault: PairFaultDischarge::DischargedByObligation,
        unit_defs: PairUnitDefRelation::RetiredWhenDead,
        ..Self::ISOLATED
    };

    /// The operand-swapped composition: implicit definitions stay
    /// covered while the record-level gate re-derives that every
    /// reachable reader is equality-sensing.
    pub(crate) const OPERAND_SWAPPED_UNIT_DEFS: Self = Self {
        unit_defs: PairUnitDefRelation::OperandSwapped,
        ..Self::ISOLATED
    };

    /// Whether the eliminated producer's catalog declaration is
    /// effect-isolated including every implicit unit it could have
    /// written. Every landed pair requires this surface regardless of
    /// the declared axes: removing the producer must drop nothing
    /// machine-visible.
    pub(crate) fn admits_producer(self, declaration: &MachineEffectDeclaration) -> bool {
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
    pub(crate) fn admits_consumer(
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
    pub(crate) fn admits_consumer_obligation(self, consumer: &SelectedInstruction) -> bool {
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
    pub(crate) fn admits_dead_consumer_defs(
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
    pub(crate) fn admits_swapped_condition_defs(
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
    pub(crate) fn admits_rewritten(self, declaration: &MachineEffectDeclaration) -> bool {
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
pub(crate) struct PairOperandShape {
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
pub(crate) enum PairLiteralPosition {
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

impl PairLiteralPosition {
    /// The consumer operand index the folded literal occupies.
    pub(crate) const fn victim_operand(self) -> u16 {
        match self {
            Self::RightOperand => 1,
            Self::LeftOperand | Self::SoleOperand => 0,
        }
    }

    /// The consumer operand index of the non-victim register input, when
    /// the grammar has one: a binary consumer's other `Use`. The unary
    /// grammar consumes only the folded input.
    pub(crate) const fn survivor_operand(self) -> Option<u16> {
        match self {
            Self::RightOperand => Some(0),
            Self::LeftOperand => Some(1),
            Self::SoleOperand => None,
        }
    }
}

/// What the rewritten form's result channel computes relative to the
/// non-victim operands — the axis that decides which inputs the fold
/// still observes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PairOperandResult {
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
pub(crate) enum PairTailCustody {
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

impl PairTailCustody {
    /// Whether every operand past the grammar's fixed roles satisfies this
    /// custody contract: a bare tail carries none, and each drop custody
    /// admits only the accesses whose death the fold proved.
    pub(crate) fn admits(self, function: &SelectedFunction, operands: &[SelectedOperand]) -> bool {
        match self {
            Self::Bare => operands.is_empty(),
            Self::AuxiliaryUses => auxiliary_uses(function, operands),
            Self::ScratchDefs => dead_scratch(function, operands),
            Self::AuxiliaryUsesOrScratchDefs => droppable_tail(function, operands),
        }
    }
}

impl PairOperandShape {
    /// Binary consumer: the literal victim is the right `Use` operand
    /// (operand index 1) and the left `Use` operand survives into the
    /// rewritten instruction — the immediate-form binary arithmetic and
    /// compare rules.
    pub(crate) const BINARY_RIGHT_LITERAL: Self = Self {
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
    pub(crate) const BINARY_LEFT_LITERAL: Self = Self {
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
    pub(crate) const BINARY_LEFT_LITERAL_OPERAND_SWAP: Self = Self {
        position: PairLiteralPosition::LeftOperand,
        result_kind: PairOperandResult::SwappedOperand,
        tail: PairTailCustody::Bare,
    };
    /// Unary consumer: the literal victim is the sole `Use` operand (operand
    /// index 0). The rewritten instruction consumes no register input — the
    /// fold recomputes the consumer's constant output directly, as in the
    /// extension-elimination and copy-materialization rules whose rewritten
    /// form is a `MaterializeI64`.
    pub(crate) const UNARY_LITERAL: Self = Self {
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
    pub(crate) const BINARY_RIGHT_LITERAL_AUXILIARY_USES: Self = Self {
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
    pub(crate) const BINARY_RIGHT_LITERAL_CONSTANT_RESULT: Self = Self {
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
    pub(crate) const BINARY_LEFT_LITERAL_CONSTANT_RESULT: Self = Self {
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
    pub(crate) const BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES: Self = Self {
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
    pub(crate) const BINARY_RIGHT_LITERAL_SCRATCH_DEFS: Self = Self {
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
    pub(crate) const BINARY_LEFT_LITERAL_SCRATCH_DEFS: Self = Self {
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
    pub(crate) const BINARY_RIGHT_LITERAL_AUXILIARY_USES_OR_SCRATCH_DEFS: Self = Self {
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
    pub(crate) const BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES_OR_SCRATCH_DEFS: Self =
        Self {
            position: PairLiteralPosition::LeftOperand,
            result_kind: PairOperandResult::ConstantOfLiteral,
            tail: PairTailCustody::AuxiliaryUsesOrScratchDefs,
        };

    /// The consumer operand index the folded literal occupies.
    pub(crate) const fn victim_operand(self) -> u16 {
        self.position.victim_operand()
    }

    /// The consumer operand index of the non-victim register input — the
    /// operand a surviving-`Use` grammar binds into the rewritten row —
    /// when the consumer has one.
    pub(crate) const fn survivor_operand(self) -> Option<u16> {
        self.position.survivor_operand()
    }

    /// The consumer operand index of the scalar `Def` result under a
    /// scalar result channel; the implicit-unit channel carries no `Def`
    /// operand at all. A sole-`Use` grammar carries its result right after
    /// the folded input, a binary grammar after both inputs.
    pub(crate) const fn result_operand(self, result: PairResultDisposition) -> Option<u16> {
        match (self.position, result) {
            (_, PairResultDisposition::ImplicitUnits) => None,
            (PairLiteralPosition::SoleOperand, PairResultDisposition::ScalarRegister) => Some(1),
            (
                PairLiteralPosition::RightOperand | PairLiteralPosition::LeftOperand,
                PairResultDisposition::ScalarRegister,
            ) => Some(2),
        }
    }
}

/// The literal values a pair's fold admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PairImmediateBound {
    /// Any unsigned literal up to the encoding limit the rewritten form's
    /// immediate field admits — the folded value only needs to fit.
    Encoding(u64),
    /// Any *nonzero* unsigned literal up to the encoding limit the
    /// rewritten form's immediate field admits — the folded value must
    /// fit and must not name the dedicated constant form the same
    /// consumer kind's zero-specific pair binds, so the two grammars stay
    /// disjoint on the literal's value: the compare family's immediate
    /// pairs carry this bound while the zero pairs carry
    /// [`Exactly(0)`](Self::Exactly).
    EncodingNonZero(u64),
    /// Exactly one literal value: the fold is an algebraic identity whose
    /// correctness depends on the value itself — the divide-by-one divisor
    /// — not merely on fitting an immediate field.
    Exactly(u64),
}

/// The symbolic instruction triple, immediate bound, result channel,
/// unit-effect surface, machine-effect surface, and consumer operand grammar
/// of one lowering rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectedInstructionPairRule {
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
        self.operand_shape.victim_operand()
    }

    /// The rewritten row's declared operand roles: whether the row opens
    /// with the surviving `Use` operand, and whether it closes with the
    /// scalar `Def` result. `None` when the declared axes name no row of
    /// their own — a swapped result channel with a scalar output would
    /// publish the inverted difference to every register reader with no
    /// reader audit to refuse it, and the constant-result channels always
    /// materialize their scalar constant, so neither declares a form.
    pub(crate) const fn rewritten_row_roles(self) -> Option<(bool, bool)> {
        match (self.operand_shape.result_kind, self.result) {
            (PairOperandResult::SurvivingOperand, PairResultDisposition::ScalarRegister) => {
                Some((true, true))
            }
            (PairOperandResult::SurvivingOperand, PairResultDisposition::ImplicitUnits) => {
                Some((true, false))
            }
            (PairOperandResult::SwappedOperand, PairResultDisposition::ImplicitUnits) => {
                Some((true, false))
            }
            (
                PairOperandResult::ConstantOfLiteral | PairOperandResult::LiteralRecompute,
                PairResultDisposition::ScalarRegister,
            ) => Some((false, true)),
            _ => None,
        }
    }
    pub const fn admits_immediate(self, value: u64) -> bool {
        match self.immediate_bound {
            PairImmediateBound::Encoding(limit) => value <= limit,
            PairImmediateBound::EncodingNonZero(limit) => value != 0 && value <= limit,
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
    pub(crate) fn fold_immediate(self, literal: u64) -> Option<u64> {
        Some(match self.operand_shape.result_kind {
            // Surviving-operand and operand-swapped grammars embed the
            // literal itself.
            PairOperandResult::SurvivingOperand | PairOperandResult::SwappedOperand => literal,
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
                MachineSemanticKind::SaturatingAdd(carrier) => carrier.maximum_bits(),
                _ => 0,
            },
            PairOperandResult::LiteralRecompute => match self.consumer {
                MachineSemanticKind::CopyI64 => literal,
                MachineSemanticKind::ZeroExtendU8 => literal & 0xFF,
                MachineSemanticKind::ZeroExtendU16 => literal & 0xFFFF,
                MachineSemanticKind::ZeroExtendU32 => literal & 0xFFFF_FFFF,
                MachineSemanticKind::SignExtendI8 => (literal & 0xFF) as u8 as i8 as i64 as u64,
                MachineSemanticKind::SignExtendI16 => {
                    (literal & 0xFFFF) as u16 as i16 as i64 as u64
                }
                MachineSemanticKind::SignExtendI32 => {
                    (literal & 0xFFFF_FFFF) as u32 as i32 as i64 as u64
                }
                _ => return None,
            },
        })
    }

    pub(crate) fn matches_producer(self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.producer
    }

    pub(crate) fn matches_consumer(self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.consumer
    }

    /// The constraint-catalog key of this rule's rewritten instruction form.
    pub(crate) fn immediate_constraint_key(
        self,
        keys: &TargetRegisterEnvironmentConstraintKeys,
    ) -> Option<RegisterConstraintKey> {
        match self.rewritten {
            MachineSemanticKind::ExactAddI64Immediate => Some(keys.add_i64_immediate),
            MachineSemanticKind::ExactSubtractI64Immediate => Some(keys.subtract_i64_immediate),
            MachineSemanticKind::CompareI64Immediate => Some(keys.compare_i64_immediate),
            MachineSemanticKind::CompareI64Zero => Some(keys.compare_i64_zero),
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
    pub(crate) fn rewrite_consumer(
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
            // A compare whose folded literal is exactly zero rewrites to
            // the dedicated zero form — `x - 0` publishes the same
            // condition state through the narrower realization — under
            // either operand grammar: `0 - x` is the same operand-swapped
            // realization refinement `x - 0` performs, admitted under the
            // same reader-flow audit the left-immediate grammar carries.
            // The pair's `Exactly(0)` bound already fixed the value, so
            // the rewrite embeds no immediate field.
            (MachineSemanticKind::CompareI64Zero, SelectedInstructionKind::CompareI64) => {
                Some(SelectedInstructionKind::CompareI64Zero)
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
                | selected_instructions::saturating_add_or_subtract_pattern!()
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
    let integer = scalar.as_integer()?;
    if integer.is_address() || integer.bits() > 64 {
        return None;
    }
    let value = match integer.sign() {
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(bits)),
        IntegerSign::Signed => IntegerValue::Signed(i128::from(bits as i64)),
    };
    integer.admits(value).then_some(value)
}
