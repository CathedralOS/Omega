//! Operand probes every rewrite spells the same way: the sole catalog
//! declaration for a semantic+constraint pair, a register's last write
//! before a position, and whether an encoded alternative carries no
//! machine effect at all.

use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration,
    MachineMemoryEffect, MachineSemanticKind, MachineTrapBehavior, SelectedInstruction,
    ValidatedMachineEffectCatalog, VirtualRegisterId,
};

pub(super) fn effect_declaration(
    catalog: &ValidatedMachineEffectCatalog,
    semantic: MachineSemanticKind,
    constraint: register_model::RegisterConstraintKey,
) -> Option<&MachineEffectDeclaration> {
    let mut matches = catalog.catalog().declarations.iter().filter(|declaration| {
        declaration.semantic == semantic && declaration.constraint == constraint
    });
    let declaration = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(declaration)
}

pub(super) fn last_definition_before(
    instructions: &[SelectedInstruction],
    position: usize,
    register: VirtualRegisterId,
) -> Option<usize> {
    instructions[..position].iter().rposition(|instruction| {
        instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
        })
    })
}

/// The non-unit declaration surface an isolated form must carry: no memory
/// access, no trap, no barrier, no call, no cleanup.
pub(crate) fn isolated_declaration(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::None
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
}
