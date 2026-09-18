//! Declarative projected-access pair descriptors: producer, consumer, and
//! the rewritten form plus the admissibility axes each must satisfy.
//!
//! The [`literal_fold`](crate::rewrites::selected_lowering) pair grammar
//! describes a materialized literal folded into a consumer operand, and the
//! [`terminator_pair`](super::terminator_pair) and
//! [`condition_materialization`](super::condition_materialization) grammars
//! describe condition-state readers. Neither vocabulary can name the pair
//! an address projection forms with the access that consumes it: the
//! `AddressOffset` producer's `Def` is an ordinary register — no literal to
//! substitute — and the consumer absorbs the projection's displacement into
//! the `byte_offset` its own kind already carries, so the rewritten form is
//! the consumer's *same* constraint row at a new displacement and pointer
//! register. This module declares that relationship. A
//! [`ProjectedAccessRule`] declares one (producer kind, consumer kind,
//! rewritten kind) triple and the axes the relationship must satisfy; the
//! producer consults its own descriptor when admitting a candidate, while
//! the independent replay re-derives the whole grammar — consumer kind and
//! operand shape, the pointer's last definition, the combined-displacement
//! bound, and the effect surface — from the instruction records alone and
//! never reads a rule.
//!
//! The landed family is the projected-access fold: a `Load8`, `Load16`,
//! `Load32`, `Load64`, or `Store` whose operand-0 pointer is the block's
//! last definition of an `AddressOffset` result becomes the same access
//! reading the projection's base register at the combined displacement —
//! the bespoke [`address_fold`](crate::rewrites::address_fold) transform,
//! restated as a declared pair with the machine-effect binding the bespoke
//! rewrite never consulted. The rewrite is the first descriptor whose
//! consumer and rewritten kind are identical, and it exercises the
//! machine-effect axes the board still names open: the *trap-preserving*
//! relationship [`ProjectedAccessEffects::AccessFaultRetained`] — the
//! dereference's `MayArchitecturalFaultV1` surface rides the fold verbatim
//! because the rewritten form performs the identical access — and, through
//! the `Store` consumer, the first declared pair whose consumer writes
//! memory (`WritePointerV1`). The producer stays: its projected register
//! remains published for every other reader the function still holds, so
//! the fold only changes which register the access reads and which
//! displacement it carries.

use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior, SelectedInstruction, SelectedInstructionKind,
};

use crate::machine_semantic_kind;

/// The operand relationship the rewrite asserts: how the producer's
/// projected register reaches the consumer's pointer operand and what the
/// rewritten record carries — the operand-shape axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProjectedAccessOperandShape {
    /// The consumer's operand 0 — a `Use` — reads the register the
    /// producer's operand 1 defines, and the producer's operand 0 `Use`
    /// names the referent base. The producer must be the pointer's last
    /// definition before the consumer in the same block, the base must be
    /// a different register than the pointer, and no instruction between
    /// them may redefine the base: the rebound operand observes the base's
    /// value at the consumer's position, which must be the value the
    /// producer projected. The rewritten record rebinds operand 0 to the
    /// base register and carries the summed `byte_offset` on the consumer's
    /// own kind; operand 1 — the load's `Def` result or the store's `Use`
    /// value — carries verbatim, as do the instruction identity, the
    /// constraint row, the implicit surfaces, and the provenance.
    PointerOperandProjection,
}

/// The displacement bound the summed `byte_offset` must satisfy — the
/// immediate-bound axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProjectedAccessDisplacement {
    /// The combined displacement must encode under the scaled unsigned
    /// immediate every target's access forms share: a multiple of the
    /// access width — one byte for `Load8`, two for `Load16`, four for
    /// `Load32`, eight for `Load64`, the kind's `byte_size` for `Store` —
    /// with a quotient of at most 4095. The AArch64 scaled immediate is
    /// the tighter bound; the wider x86-64 disp32 admits it in full, so the
    /// shared bound admits a pair on every target the family declares.
    AccessScaledImmediate,
}

/// The implicit physical-unit relationship the rewrite asserts — the
/// unit-effects axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProjectedAccessUnitFlow {
    /// Producer, consumer, and rewritten records carry no implicit uses,
    /// definitions, or clobbers, and no operand on either side carries a
    /// `fixed_view`, `tied_to`, or `early_clobber` binding: the projection
    /// and the access forms are register-only outside their explicit
    /// operands, so the fold's only dataflow change is the operand-0
    /// register rebind and the kind's displacement. A form whose row
    /// publishes implicit traffic — the frame-relative `Store64`'s
    /// stack-pointer use, a packed access's early-clobber scratch — cannot
    /// name this pair.
    RegisterOnly,
}

/// The machine-effect relationship the rewrite carries between the
/// producer's, the consumer's, and the rewritten form's catalog
/// declarations — memory, trap, stack, control flow, barrier, call, and
/// cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProjectedAccessEffects {
    /// Trap preservation — the relationship beyond `FaultDischargedBy*`
    /// this family declares. The rewritten form performs the consumer's
    /// dereference unchanged, so its `MayArchitecturalFaultV1` surface is
    /// *retained*, not discharged: rebinding the pointer operand drops only
    /// the trap-free projection, never the fault the access itself may
    /// raise at the combined displacement.
    ///
    /// The consumer's declaration — which is also the rewritten form's,
    /// the rewrite binding the same constraint row — carries the access's
    /// memory effect (`ReadPointerV1` for the loads, `WritePointerV1` for
    /// the store) and the `MayArchitecturalFaultV1` trap the dereference
    /// can raise, with no barrier, call, or cleanup. Every encoded
    /// alternative carries the matching pointer access over operand 0 —
    /// `ReadPointerV1` at the access's byte width for a load,
    /// `WritePointerV1` for the store — `MayArchitecturalFaultV1`,
    /// unchanged stack, fall-through control, and no implicit unit
    /// traffic. The retained producer is fully effect-isolated: `NoneV1`
    /// memory, `NeverV1` trap, no barrier, call, or cleanup, and every
    /// alternative encodes no memory, unchanged stack, `NeverV1`,
    /// fall-through, and no implicit units. A target whose projection or
    /// access rows encode anything else cannot admit the pair.
    AccessFaultRetained,
}

/// One declarative projected-access rule: the `AddressOffset` producer
/// kind, the dereferencing consumer kind, the rewritten form — the
/// consumer's own row at the combined displacement — and the axes the
/// relationship must satisfy.
///
/// The descriptor declares admissibility; it never certifies it. The
/// producer consults its own rule through [`projected_access_for`] and the
/// axes' `admits_*` predicates; the replay in `replay` re-derives the
/// grammar from the instruction records without reading this table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProjectedAccessRule {
    producer: MachineSemanticKind,
    consumer: MachineSemanticKind,
    operand_shape: ProjectedAccessOperandShape,
    /// The access the consumer's operand 1 declares: the `Def` result a
    /// load publishes or the `Use` value a store reads — carried verbatim
    /// onto the rewritten record.
    tail_access: RegisterOperandAccess,
    bound: ProjectedAccessDisplacement,
    unit_flow: ProjectedAccessUnitFlow,
    effects: ProjectedAccessEffects,
}

impl ProjectedAccessRule {
    /// Shared declaration of every projected-access pair: the producer is
    /// the `AddressOffset` projection, operand 0 is the folded pointer,
    /// operand 1 is the load's `Def` result, the displacement is the
    /// access-scaled immediate, the unit surface is register-only, and the
    /// access's fault surface is retained.
    const PROJECTED_LOAD: Self = Self {
        producer: MachineSemanticKind::AddressOffset,
        consumer: MachineSemanticKind::Load8,
        operand_shape: ProjectedAccessOperandShape::PointerOperandProjection,
        tail_access: RegisterOperandAccess::Def,
        bound: ProjectedAccessDisplacement::AccessScaledImmediate,
        unit_flow: ProjectedAccessUnitFlow::RegisterOnly,
        effects: ProjectedAccessEffects::AccessFaultRetained,
    };

    /// `address_offset + off1` feeding `load8 [pointer + off2]`.
    pub(super) const ADDRESS_OFFSET_LOAD8: Self = Self::PROJECTED_LOAD;
    /// `address_offset + off1` feeding `load16 [pointer + off2]`.
    pub(super) const ADDRESS_OFFSET_LOAD16: Self = Self {
        consumer: MachineSemanticKind::Load16,
        ..Self::PROJECTED_LOAD
    };
    /// `address_offset + off1` feeding `load32 [pointer + off2]`.
    pub(super) const ADDRESS_OFFSET_LOAD32: Self = Self {
        consumer: MachineSemanticKind::Load32,
        ..Self::PROJECTED_LOAD
    };
    /// `address_offset + off1` feeding `load64 [pointer + off2]`.
    pub(super) const ADDRESS_OFFSET_LOAD64: Self = Self {
        consumer: MachineSemanticKind::Load64,
        ..Self::PROJECTED_LOAD
    };
    /// `address_offset + off1` feeding `store [pointer + off2], value` —
    /// the store's operand 1 is the stored `Use`, not a result `Def`.
    pub(super) const ADDRESS_OFFSET_STORE: Self = Self {
        consumer: MachineSemanticKind::Store,
        tail_access: RegisterOperandAccess::Use,
        ..Self::PROJECTED_LOAD
    };

    pub(super) const fn producer(self) -> MachineSemanticKind {
        self.producer
    }
    pub(super) const fn consumer(self) -> MachineSemanticKind {
        self.consumer
    }
    /// The rewritten form every declared pair publishes: the consumer's own
    /// semantic — the rewrite binds the same constraint row, so the folded
    /// access keeps its declared effect surface verbatim.
    pub(super) const fn rewritten(self) -> MachineSemanticKind {
        self.consumer
    }
    pub(super) const fn tail_access(self) -> RegisterOperandAccess {
        self.tail_access
    }

    /// Whether `kind` is the consumer this rule declares.
    pub(super) fn matches_consumer(&self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.consumer
    }

    /// Whether `kind` is the producer this rule declares.
    pub(super) fn matches_producer(&self, kind: SelectedInstructionKind) -> bool {
        machine_semantic_kind(kind) == self.producer
    }

    /// The declared displacement bound, checked against the combined
    /// `byte_offset` and the access width the consumer's kind fixes: under
    /// `AccessScaledImmediate` the sum must be a multiple of the width with
    /// a quotient of at most 4095 — the scaled unsigned immediate every
    /// target's access form shares.
    pub(super) fn admits_displacement(&self, combined: u32, width: u32) -> bool {
        match self.bound {
            ProjectedAccessDisplacement::AccessScaledImmediate => {
                combined.is_multiple_of(width) && combined / width <= 4095
            }
        }
    }

    /// The implicit-unit declaration surface the rule admits: under
    /// `RegisterOnly` producer, consumer, and rewritten rows carry no
    /// implicit uses, definitions, or clobbers at all — the pair's whole
    /// unit traffic is the explicit operand-0 rebind.
    pub(super) fn admits_row_units(&self, row: &RegisterInstructionConstraint) -> bool {
        match self.unit_flow {
            ProjectedAccessUnitFlow::RegisterOnly => {
                row.implicit_uses.is_empty()
                    && row.implicit_defs.is_empty()
                    && row.clobbers.is_empty()
            }
        }
    }

    /// The operand-binding surface the rule admits on one instruction's
    /// records: under `RegisterOnly` no operand carries a `fixed_view`,
    /// `tied_to`, or `early_clobber` binding and the instruction's own
    /// implicit lists are empty — a binding the fold does not name would be
    /// silently kept on the rebuilt operand.
    pub(super) fn admits_record(&self, instruction: &SelectedInstruction) -> bool {
        match self.unit_flow {
            ProjectedAccessUnitFlow::RegisterOnly => {
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

    /// The effect-declaration surface the rule admits between the
    /// producer's and the access's catalog declarations — the access's
    /// declaration doubles as the rewritten form's, the rewrite binding
    /// the same constraint row. Under `AccessFaultRetained` the producer is
    /// fully effect-isolated while the access carries its dereference
    /// verbatim: `ReadPointerV1`/`WritePointerV1` memory over operand 0,
    /// the `MayArchitecturalFaultV1` trap surface, unchanged stack,
    /// fall-through control, and no implicit unit traffic on any
    /// alternative.
    pub(super) fn admits_declarations(
        &self,
        producer: &MachineEffectDeclaration,
        access: &MachineEffectDeclaration,
        width: u32,
    ) -> bool {
        match self.effects {
            ProjectedAccessEffects::AccessFaultRetained => {
                isolated_projection(producer) && retained_access(access, self.consumer, width)
            }
        }
    }
}

/// Every projected-access pair the family declares: the `AddressOffset`
/// producer against each dereferencing consumer kind.
pub(super) const PROJECTED_ACCESS_RULES: &[ProjectedAccessRule] = &[
    ProjectedAccessRule::ADDRESS_OFFSET_LOAD8,
    ProjectedAccessRule::ADDRESS_OFFSET_LOAD16,
    ProjectedAccessRule::ADDRESS_OFFSET_LOAD32,
    ProjectedAccessRule::ADDRESS_OFFSET_LOAD64,
    ProjectedAccessRule::ADDRESS_OFFSET_STORE,
];

/// The declared pair for one (producer, consumer) kind combination, or none
/// when the family declares no such relationship — several rules matching
/// one combination is a catalog defect and refuses rather than preferring
/// table order.
pub(super) fn projected_access_for(
    producer: MachineSemanticKind,
    consumer: MachineSemanticKind,
) -> Option<&'static ProjectedAccessRule> {
    let mut matches = PROJECTED_ACCESS_RULES
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
/// admitted one whose pointer flow or displacement refuses.
pub(super) fn declared_consumer(kind: SelectedInstructionKind) -> bool {
    PROJECTED_ACCESS_RULES
        .iter()
        .any(|rule| rule.matches_consumer(kind))
}

/// The byte width the kind's access covers — the scale the declared
/// displacement bound multiplies. `Store` carries its width as
/// `byte_size`; the referent store exists only at the byte-addressable
/// widths every target encodes.
pub(super) fn access_width(kind: SelectedInstructionKind) -> Option<u32> {
    match kind {
        SelectedInstructionKind::Load8 { .. } => Some(1),
        SelectedInstructionKind::Load16 { .. } => Some(2),
        SelectedInstructionKind::Load32 { .. } => Some(4),
        SelectedInstructionKind::Load64 { .. } => Some(8),
        SelectedInstructionKind::Store { byte_size, .. } => match byte_size {
            1 | 2 | 4 | 8 => Some(u32::from(byte_size)),
            _ => None,
        },
        _ => None,
    }
}

/// The declared surface the retained `AddressOffset` producer must carry:
/// the projection observes no memory, never traps, and publishes no
/// barrier, call, or cleanup — and every alternative encodes no memory,
/// unchanged stack, `NeverV1`, fall-through, and no implicit unit traffic.
fn isolated_projection(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::None
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
        && declaration.alternatives.iter().all(projection_alternative)
}

/// The encoded surface a projection alternative must carry: no memory
/// access, unchanged stack, no trap surface, a plain fall-through, and no
/// implicit unit traffic at all.
fn projection_alternative(alternative: &MachineAlternative) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::NeverV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
        && encoded.implicit_unit_uses.is_empty()
        && encoded.implicit_unit_defs.is_empty()
        && encoded.implicit_unit_clobbers.is_empty()
}

/// The declared surface the access row — the consumer's and the rewritten
/// form's alike — must carry: the dereference's own memory effect over
/// operand 0 (`ReadPointerV1` at the access's byte width for a load,
/// `WritePointerV1` for the store), the `MayArchitecturalFaultV1` trap the
/// access can raise, no barrier, call, or cleanup, and every alternative
/// encoding the matching pointer access, `MayArchitecturalFaultV1`,
/// unchanged stack, fall-through control, and no implicit unit traffic.
fn retained_access(
    declaration: &MachineEffectDeclaration,
    consumer: MachineSemanticKind,
    width: u32,
) -> bool {
    let (memory, encoded_memory, reads, writes) = match consumer {
        MachineSemanticKind::Store => (
            MachineMemoryEffect::WritePointerV1,
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            vec![0, 1],
            Vec::new(),
        ),
        _ => (
            MachineMemoryEffect::ReadPointerV1,
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: width as u16,
            },
            vec![0],
            vec![1],
        ),
    };
    declaration.memory == memory
        && declaration.trap == MachineTrapBehavior::MayArchitecturalFaultV1
        && declaration.barrier == MachineBarrier::None
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
        && declaration
            .alternatives
            .iter()
            .all(|alternative| access_alternative(alternative, encoded_memory, &reads, &writes))
}

/// The encoded surface a retained-access alternative must carry: the
/// pointer access over operand 0 the declaration-level memory effect names
/// — `ReadPointerV1` at the access's byte width or `WritePointerV1` — the
/// operand read/write roster the form publishes, the
/// `MayArchitecturalFaultV1` trap the dereference retains, unchanged stack,
/// fall-through control, and no implicit unit traffic.
fn access_alternative(
    alternative: &MachineAlternative,
    encoded_memory: MachineEncodedMemoryEffect,
    reads: &[u16],
    writes: &[u16],
) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == encoded_memory
        && encoded.external_operand_reads == reads
        && encoded.external_operand_writes == writes
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
        && encoded.implicit_unit_uses.is_empty()
        && encoded.implicit_unit_defs.is_empty()
        && encoded.implicit_unit_clobbers.is_empty()
}
