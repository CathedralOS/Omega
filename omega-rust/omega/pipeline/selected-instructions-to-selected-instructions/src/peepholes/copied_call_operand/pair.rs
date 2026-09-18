//! Declarative copied-call-operand pair descriptors: producer, consumer,
//! and the rewritten form plus the admissibility axes each must satisfy.
//!
//! The [`literal_fold`](crate::rewrites::selected_lowering) pair grammar
//! describes a materialized literal folded into a consumer operand, and the
//! [`projected_access`](super::projected_access) grammar describes a
//! register-only producer and consumer whose rebind is safe because neither
//! record carries a unit or machine surface. Neither vocabulary can name
//! the pair a transparent register copy forms with the direct internal call
//! that reads its result: the `CopyI64` producer's `Def` is an ordinary
//! register — no literal to substitute, no displacement to absorb — and the
//! consumer is a call constraint row whose operand list is pinned to ABI
//! views, whose implicit surface carries the stack pointer and the complete
//! caller-saved clobber roster, and whose encoded alternatives carry the
//! activation-stack lifecycle. This module declares that relationship. A
//! [`CopiedCallOperandRule`] declares one (producer kind, consumer kind)
//! pair and the axes the relationship must satisfy; the producer consults
//! its own descriptor when admitting a candidate, while the independent
//! replay re-derives the whole grammar — the call's operand roster, the
//! copy's last-definition custody, and the retained call surface — from the
//! instruction records alone and never reads a rule.
//!
//! The landed family is the copied-call-operand reroute: a `CallUnit`,
//! `CallScalar`, or `CallAggregate` whose `Use` argument operands read the
//! register a clean `CopyI64` defines becomes the same call reading the
//! copy's source register at every such operand — the transformation
//! `rewrites::copy_removal` cannot perform, because its substitution
//! refuses call contracts outright. The rewrite binds the call's own
//! constraint row, so the rewritten form keeps the callee, the ABI views,
//! and every declared effect verbatim; only the operand registers change.
//! The producer stays: its defined register remains published for every
//! other reader the function still holds, and producer elimination stays
//! with the producer-elimination rules.
//!
//! This is the first descriptor whose consumer carries a stack effect:
//! under [`CopiedCallOperandEffects::CallLifecycleRetained`] the
//! rewritten row's encoded alternatives must declare the direct internal
//! call's complete activation contract — `DirectRelativeCallV1` control,
//! the `Call` barrier, the `DirectInternalNormalReturnV1` call effect, and
//! the target's stack lifecycle, `WriteReturnAddressBelowStackPointerV1`
//! paired with `CallReturnAddressLifecycleV1` on stack-pushing targets or
//! `NoneV1` paired with `UnchangedV1` on link-register targets — verbatim,
//! because the reroute changes only which virtual register each argument
//! operand names, never what the call does.

use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior, SelectedInstruction, SelectedInstructionKind,
    SelectedOperand,
};

use crate::machine_semantic_kind;

/// The operand relationship the rewrite asserts: which operand positions of
/// the call may rebind to the copy's source register and what the rewritten
/// record carries — the operand-shape axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CopiedCallOperandShape {
    /// The call's operand roster is the dense leading-`Use`-arguments,
    /// trailing-`Def`-results shape the direct-call contract publishes:
    /// operand numbers equal their positions, every operand pins its ABI
    /// `fixed_view`, and no operand carries `tied_to` or `early_clobber`.
    /// `results` names the result roster the consumer's semantic owns. The
    /// producer's operand 1 `Def` names the destination register the
    /// call's `Use` operands may read; the producer's operand 0 `Use`
    /// names the referent source. The producer must be the destination's
    /// last definition before the consumer in the same block, the source
    /// must be a different register than the destination, at least one
    /// `Use` operand must read the destination, and no instruction between
    /// them may redefine the source: a rebound operand observes the
    /// source's value at the call's position, which must be the value the
    /// copy captured. The rewritten record rebinds every `Use` operand
    /// reading the destination to the source register and carries
    /// everything else — identity, kind, constraint row, operand order,
    /// ABI views, implicit surfaces, provenance — verbatim.
    CopiedUseOperand { results: CallResultRoster },
}

/// The trailing `Def` roster a call semantic owns — part of the operand
/// shape the pair declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CallResultRoster {
    /// Every operand is a `Use` argument: the register-passed Unit call.
    Empty,
    /// Exactly one trailing `Def`: the scalar call's result.
    Scalar,
    /// One or two trailing `Def` fragments — never zero operands overall:
    /// the direct aggregate call's result roster.
    Aggregate,
}

/// The implicit physical-unit relationship the rewrite asserts — the
/// unit-effects axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CopiedCallOperandUnitFlow {
    /// The consumer and rewritten records carry the identical implicit
    /// uses, implicit definitions, and caller-saved clobber roster — the
    /// call's stack-pointer read, return-unit write, and register
    /// preservation surface — and every operand binding carries verbatim:
    /// the ABI `fixed_view` pins place each argument and result while no
    /// `tied_to` or `early_clobber` decorates a call operand. The only
    /// dataflow change is the operand registers the rebound positions
    /// name. The producer is register-only: a `CopyI64` publishes no
    /// implicit traffic and no operand binding, so a decorated or
    /// unit-carrying copy cannot name this pair.
    CallSurfaceRetained,
}

/// The machine-effect relationship the rewrite carries between the
/// producer's, the consumer's, and the rewritten form's catalog
/// declarations — memory, trap, stack, control flow, barrier, call, and
/// cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CopiedCallOperandEffects {
    /// Stack lifecycle retention — the first declared axis admitting a
    /// non-`UnchangedV1` stack effect. The rewritten form performs the
    /// consumer's call unchanged, so its complete activation surface is
    /// *retained*, not rewritten away: rebinding argument operands drops
    /// only the copy's value forwarding, never the return-address
    /// lifecycle the call itself performs.
    ///
    /// The consumer's declaration — which is also the rewritten form's,
    /// the rewrite binding the same constraint row — carries `NoneV1`
    /// memory and `NeverV1` trap at the declaration level (the lifecycle
    /// lives on the encoded surface), the `Call` barrier, the
    /// `DirectInternalNormalReturnV1` call effect, and no cleanup. Every
    /// encoded alternative restates the constraint row's read, write, and
    /// implicit-unit roster exactly and carries `DirectRelativeCallV1`
    /// control, the `MayArchitecturalFaultV1` surface the call retains,
    /// and the target's stack contract: `WriteReturnAddressBelowStackPointerV1`
    /// memory paired with `CallReturnAddressLifecycleV1` stack at matching
    /// nonzero byte counts on a stack-pushing target, or `NoneV1` memory
    /// paired with `UnchangedV1` stack where the link register holds the
    /// return address. The retained `CopyI64` producer is fully
    /// effect-isolated. A target whose copy or call rows encode anything
    /// else cannot admit the pair.
    CallLifecycleRetained,
}

/// One declarative copied-call-operand rule: the `CopyI64` producer kind,
/// the direct-call consumer kind, the rewritten form — the consumer's own
/// row with its copied operands rebound — and the axes the relationship
/// must satisfy.
///
/// The descriptor declares admissibility; it never certifies it. The
/// producer consults its own rule through [`copied_call_operand_for`] and
/// the axes' `admits_*` predicates; the replay in `replay` re-derives the
/// grammar from the instruction records without reading this table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CopiedCallOperandRule {
    producer: MachineSemanticKind,
    consumer: MachineSemanticKind,
    operand_shape: CopiedCallOperandShape,
    unit_flow: CopiedCallOperandUnitFlow,
    effects: CopiedCallOperandEffects,
}

impl CopiedCallOperandRule {
    /// Shared declaration of every copied-call-operand pair: the producer
    /// is the `CopyI64` transparent move, the copied operands are the
    /// call's `Use` positions, the unit surface retains the call's whole
    /// roster verbatim, and the call's lifecycle surface is retained.
    const COPIED_CALL: Self = Self {
        producer: MachineSemanticKind::CopyI64,
        consumer: MachineSemanticKind::CallUnit,
        operand_shape: CopiedCallOperandShape::CopiedUseOperand {
            results: CallResultRoster::Empty,
        },
        unit_flow: CopiedCallOperandUnitFlow::CallSurfaceRetained,
        effects: CopiedCallOperandEffects::CallLifecycleRetained,
    };

    /// `copy_i64 source, destination` feeding `call_unit …, destination`.
    pub(super) const COPY_CALL_UNIT: Self = Self::COPIED_CALL;
    /// `copy_i64 source, destination` feeding `call_scalar …, destination,
    /// result`.
    pub(super) const COPY_CALL_SCALAR: Self = Self {
        consumer: MachineSemanticKind::CallScalar,
        operand_shape: CopiedCallOperandShape::CopiedUseOperand {
            results: CallResultRoster::Scalar,
        },
        ..Self::COPIED_CALL
    };
    /// `copy_i64 source, destination` feeding `call_aggregate …,
    /// destination, fragments…`.
    pub(super) const COPY_CALL_AGGREGATE: Self = Self {
        consumer: MachineSemanticKind::CallAggregate,
        operand_shape: CopiedCallOperandShape::CopiedUseOperand {
            results: CallResultRoster::Aggregate,
        },
        ..Self::COPIED_CALL
    };

    pub(super) const fn producer(self) -> MachineSemanticKind {
        self.producer
    }
    pub(super) const fn consumer(self) -> MachineSemanticKind {
        self.consumer
    }
    /// The rewritten form every declared pair publishes: the consumer's own
    /// semantic — the rewrite binds the same constraint row, so the call
    /// keeps its declared effect surface verbatim.
    pub(super) const fn rewritten(self) -> MachineSemanticKind {
        self.consumer
    }

    /// Whether `kind` is the consumer this rule declares.
    pub(super) fn matches_consumer(&self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.consumer
    }

    /// Whether `kind` is the producer this rule declares.
    pub(super) fn matches_producer(&self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.producer
    }

    /// The declared operand roster, checked against one operand list —
    /// record or constraint row — projected to its `(operand, access,
    /// tied_to, early_clobber)` fields: dense operand numbers, the leading
    /// `Use` arguments ahead of the result roster the consumer's semantic
    /// owns — none for `Empty`, exactly one for `Scalar`, one or
    /// two for `Aggregate` with at least one operand overall — and
    /// no `tied_to` or `early_clobber` binding on any operand.
    fn admits_roster_shape(
        &self,
        operands: impl Iterator<Item = (u16, RegisterOperandAccess, Option<u16>, bool)>,
    ) -> bool {
        let CopiedCallOperandShape::CopiedUseOperand { results } = self.operand_shape;
        let fields: Vec<(u16, RegisterOperandAccess, Option<u16>, bool)> = operands.collect();
        let arity = fields
            .iter()
            .take_while(|(_, access, _, _)| *access == RegisterOperandAccess::Use)
            .count();
        let (arguments, results_operands) = fields.split_at(arity);
        let roster = match results {
            CallResultRoster::Empty => results_operands.is_empty(),
            CallResultRoster::Scalar => results_operands.len() == 1,
            CallResultRoster::Aggregate => {
                !results_operands.is_empty() && results_operands.len() <= 2 && !fields.is_empty()
            }
        };
        roster
            && results_operands
                .iter()
                .all(|(_, access, _, _)| *access == RegisterOperandAccess::Def)
            && fields.iter().enumerate().all(|(position, field)| {
                field.0 as usize == position && field.2.is_none() && !field.3
            })
            && !arguments
                .iter()
                .any(|(_, access, _, _)| *access != RegisterOperandAccess::Use)
    }

    /// The roster the consumer's constraint row must declare: the same
    /// dense `Use`-arguments/`Def`-results shape the record carries, every
    /// operand pinned to its ABI `fixed_view`, no `tied_to` or
    /// `early_clobber`. The implicit uses, implicit definitions, and
    /// caller-saved clobber roster are free — the rewritten record
    /// republishes them verbatim.
    pub(super) fn admits_consumer_row(&self, row: &RegisterInstructionConstraint) -> bool {
        self.admits_roster_shape(row.operands.iter().map(|operand| {
            (
                operand.operand,
                operand.access,
                operand.tied_to,
                operand.early_clobber,
            )
        })) && row
            .operands
            .iter()
            .all(|operand| operand.fixed_view.is_some())
    }

    /// The roster the consumer's operand records must carry: the same
    /// dense `Use`-arguments/`Def`-results shape the row declares. Binding
    /// fields are compared against the row in admission rather than here —
    /// the record must restate the row's `fixed_view` pins exactly.
    pub(super) fn admits_consumer_operands(&self, operands: &[SelectedOperand]) -> bool {
        self.admits_roster_shape(operands.iter().map(|operand| {
            (
                operand.operand,
                operand.access,
                operand.tied_to,
                operand.early_clobber,
            )
        }))
    }

    /// The register-only surface the `CopyI64` producer must carry: no
    /// implicit uses, definitions, or clobbers and no operand bindings.
    pub(super) fn admits_producer_record(&self, instruction: &SelectedInstruction) -> bool {
        match self.unit_flow {
            CopiedCallOperandUnitFlow::CallSurfaceRetained => {
                instruction.implicit_uses.is_empty()
                    && instruction.implicit_defs.is_empty()
                    && instruction.clobbers.is_empty()
                    && instruction.operands.iter().all(|operand| {
                        operand.fixed_view.is_none()
                            && operand.tied_to.is_none()
                            && !operand.early_clobber
                    })
            }
        }
    }

    /// The register-only surface the producer's constraint row must
    /// declare: no implicit unit traffic at all.
    pub(super) fn admits_producer_row(&self, row: &RegisterInstructionConstraint) -> bool {
        match self.unit_flow {
            CopiedCallOperandUnitFlow::CallSurfaceRetained => {
                row.implicit_uses.is_empty()
                    && row.implicit_defs.is_empty()
                    && row.clobbers.is_empty()
            }
        }
    }

    /// The effect-declaration surface the rule admits between the copy's
    /// and the call's catalog declarations — the call's declaration
    /// doubles as the rewritten form's, the rewrite binding the same
    /// constraint row. Under `CallLifecycleRetained` the producer is fully
    /// effect-isolated while the call carries its complete activation
    /// contract verbatim: `Call` barrier, `DirectInternalNormalReturnV1`
    /// call effect, `DirectRelativeCallV1` control, the
    /// `MayArchitecturalFaultV1` surface, the row's read/write/implicit
    /// roster restated, and the target's stack lifecycle —
    /// `CallReturnAddressLifecycleV1` under a pushed return address or
    /// `UnchangedV1` under a link register.
    pub(super) fn admits_declarations(
        &self,
        producer: &MachineEffectDeclaration,
        call: &MachineEffectDeclaration,
        row: &RegisterInstructionConstraint,
    ) -> bool {
        match self.effects {
            CopiedCallOperandEffects::CallLifecycleRetained => {
                isolated_copy(producer) && retained_call(call, row)
            }
        }
    }
}

/// Every copied-call-operand pair the family declares: the `CopyI64`
/// producer against each direct internal call semantic.
pub(super) const COPIED_CALL_OPERAND_RULES: &[CopiedCallOperandRule] = &[
    CopiedCallOperandRule::COPY_CALL_UNIT,
    CopiedCallOperandRule::COPY_CALL_SCALAR,
    CopiedCallOperandRule::COPY_CALL_AGGREGATE,
];

/// The declared pair for one (producer, consumer) kind combination, or none
/// when the family declares no such relationship — several rules matching
/// one combination is a catalog defect and refuses rather than preferring
/// table order.
pub(super) fn copied_call_operand_for(
    producer: MachineSemanticKind,
    consumer: MachineSemanticKind,
) -> Option<&'static CopiedCallOperandRule> {
    let mut matches = COPIED_CALL_OPERAND_RULES
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
/// admitted one whose copy flow or surface refuses. The normalized foreign
/// call is deliberately absent: its boundary custody places it outside the
/// direct internal call contract, and no effect-catalog declaration exists
/// to attest its surface.
pub(super) fn declared_consumer(kind: SelectedInstructionKind) -> bool {
    COPIED_CALL_OPERAND_RULES
        .iter()
        .any(|rule| rule.matches_consumer(kind))
}

/// The declared surface the retained `CopyI64` producer must carry: the
/// copy observes no memory, never traps, and publishes no barrier, call, or
/// cleanup — and every alternative encodes no memory, unchanged stack,
/// `NeverV1`, fall-through, and no implicit unit traffic.
fn isolated_copy(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::None
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
        && declaration.alternatives.iter().all(copy_alternative)
}

/// The encoded surface a copy alternative must carry: no memory access,
/// unchanged stack, no trap surface, a plain fall-through, and no implicit
/// unit traffic at all.
fn copy_alternative(alternative: &MachineAlternative) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::NeverV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
        && encoded.implicit_unit_uses.is_empty()
        && encoded.implicit_unit_defs.is_empty()
        && encoded.implicit_unit_clobbers.is_empty()
}

/// The declared surface the call row — the consumer's and the rewritten
/// form's alike — must carry: `NoneV1` memory and `NeverV1` trap at the
/// declaration level, the `Call` barrier, the `DirectInternalNormalReturnV1`
/// call effect, no cleanup, and every alternative encoding the constraint
/// row's operand and implicit-unit roster exactly, `DirectRelativeCallV1`
/// control, the retained `MayArchitecturalFaultV1` trap, and the target's
/// stack lifecycle.
fn retained_call(
    declaration: &MachineEffectDeclaration,
    row: &RegisterInstructionConstraint,
) -> bool {
    let reads: Vec<u16> = row
        .operands
        .iter()
        .filter(|operand| operand.access == RegisterOperandAccess::Use)
        .map(|operand| operand.operand)
        .collect();
    let writes: Vec<u16> = row
        .operands
        .iter()
        .filter(|operand| operand.access == RegisterOperandAccess::Def)
        .map(|operand| operand.operand)
        .collect();
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::Call
        && matches!(
            declaration.call,
            MachineCallEffect::DirectInternalNormalReturnV1 { .. }
        )
        && declaration.cleanup == MachineCleanupEffect::NoneV1
        && declaration
            .alternatives
            .iter()
            .all(|alternative| call_alternative(alternative, &reads, &writes, row))
}

/// The encoded surface a retained-call alternative must carry: the
/// constraint row's operand read/write and implicit-unit rosters restated
/// exactly, the `MayArchitecturalFaultV1` trap the call retains,
/// `DirectRelativeCallV1` control, and the activation-stack lifecycle —
/// `WriteReturnAddressBelowStackPointerV1` memory paired with
/// `CallReturnAddressLifecycleV1` stack at matching nonzero byte counts on
/// a stack-pushing target, or no memory and `UnchangedV1` stack where the
/// link register holds the return address.
fn call_alternative(
    alternative: &MachineAlternative,
    reads: &[u16],
    writes: &[u16],
    row: &RegisterInstructionConstraint,
) -> bool {
    let encoded = &alternative.encoded;
    let lifecycle = match (encoded.memory, encoded.stack) {
        (
            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer: memory_pointer,
                byte_count,
            },
            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                stack_pointer,
                return_address_byte_count,
            },
        ) => {
            memory_pointer == stack_pointer
                && byte_count == return_address_byte_count
                && return_address_byte_count != 0
        }
        (MachineEncodedMemoryEffect::NoneV1, MachineEncodedStackEffect::UnchangedV1) => true,
        _ => false,
    };
    lifecycle
        && encoded.external_operand_reads == reads
        && encoded.external_operand_writes == writes
        && encoded.implicit_unit_uses == row.implicit_uses
        && encoded.implicit_unit_defs == row.implicit_defs
        && encoded.implicit_unit_clobbers == row.clobbers
        && encoded.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        && encoded.control == MachineEncodedControlEffect::DirectRelativeCallV1
}
