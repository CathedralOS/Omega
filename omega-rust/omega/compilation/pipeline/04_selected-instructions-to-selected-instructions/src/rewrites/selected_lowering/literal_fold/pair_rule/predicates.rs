//! Admission predicates the effect impls share: pointer-read equivalence,
//! isolated-effect surfaces, fault discharge, implicit-unit use, and the
//! equality-sensing reader tests behind operand-swap admission.

use super::*;

/// The indexed pointer read a `consumer` alternative encodes is the read
/// `rewritten`'s alternative performs once the fold replaces the index
/// register with a byte offset: same pointer operand, same byte count, the
/// folded operand position as the index, and pairwise-identical stack,
/// trap, control, and implicit-unit surface — no implicit uses on either
/// side, every unit the consumer defines still defined, and no clobbers on
/// the rewritten form.
pub(super) fn indexed_pointer_read_matches(
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
pub(super) fn implicit_defs_covered(
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

/// The non-unit encoded surface an isolated pair form's alternatives must
/// carry: no memory access, unchanged stack, never traps, falls through.
pub(super) fn isolated_alternative(alternative: &MachineAlternative) -> bool {
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
pub(super) fn fault_discharged_alternative(alternative: &MachineAlternative) -> bool {
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
pub(super) fn implicit_unit_used(function: &SelectedFunction, unit: RegisterUnitId) -> bool {
    function.blocks.iter().any(|block| {
        block_instructions(block).any(|instruction| instruction.implicit_uses.contains(&unit))
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
pub(super) fn equality_sensing_reader(kind: SelectedInstructionKind) -> bool {
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
pub(super) fn swapped_unit_readers_equality_only(
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
            let Some(target) = function.block_index(successor.block) else {
                return false;
            };
            frontier.push((target, 0));
        }
    }
    true
}

/// Whether every scratch operand past the folded result is a `Def` whose
/// register dies with this rewrite — the occurrence-free custody the
/// scratch-defs grammar requires of every dropped definition: a scratch
/// output another instruction read or defined would leave a use of a
/// register the rewrite stopped defining.
pub(super) fn dead_scratch(function: &SelectedFunction, operands: &[SelectedOperand]) -> bool {
    operands.iter().all(|operand| {
        operand.access == RegisterOperandAccess::Def
            && dropped_def_is_dead(function, operand.virtual_register)
    })
}

/// Every auxiliary operand past the folded result is a `Use` whose register
/// is defined in this function solely by zero materializations.
pub(super) fn auxiliary_uses(function: &SelectedFunction, operands: &[SelectedOperand]) -> bool {
    operands.iter().all(|operand| {
        operand.access == RegisterOperandAccess::Use
            && auxiliary_zero_defined(function, operand.virtual_register)
    })
}

/// A mixed drop tail: each operand's own access declares the custody it
/// drops under — a `Use` must be auxiliary-zero defined, a `Def` must die
/// with the rewrite, any other access rejects.
pub(super) fn droppable_tail(function: &SelectedFunction, operands: &[SelectedOperand]) -> bool {
    operands.iter().all(|operand| match operand.access {
        RegisterOperandAccess::Use => auxiliary_zero_defined(function, operand.virtual_register),
        RegisterOperandAccess::Def => dropped_def_is_dead(function, operand.virtual_register),
        _ => false,
    })
}

/// Whether `register` is defined in `function` only by `MaterializeI64`
/// instructions producing `Unsigned(0)` — the provenance the auxiliary-`Use`
/// operand grammar requires of every operand it drops. A register with no
/// definition, or any definition that is not a zero materialization, fails:
/// the dropped operand would carry a value the folded form silently stopped
/// observing.
fn auxiliary_zero_defined(function: &SelectedFunction, register: VirtualRegisterId) -> bool {
    let mut definitions = function.instructions().filter(|instruction| {
        instruction.operands.iter().any(|operand| {
            operand.access == RegisterOperandAccess::Def && operand.virtual_register == register
        })
    });
    let mut saw_definition = false;
    definitions.all(|instruction| {
        saw_definition = true;
        matches!(
            instruction.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0)
            }
        )
    }) && saw_definition
}

/// Whether `register`'s only occurrence in `function` is one `Def` operand —
/// the custody the scratch-defs grammar requires of every scratch `Def` it
/// drops. The operand itself is that one occurrence: any other operand
/// position, terminator operand, or successor transport naming the register
/// would leave the fold removing a definition that a surviving read or a
/// second definition still observes.
fn dropped_def_is_dead(function: &SelectedFunction, register: VirtualRegisterId) -> bool {
    crate::rewrites::register_occurs_once(function, register)
}
