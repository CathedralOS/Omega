//! Register versions that a checked assembly block must deliver at its exit.
use crate::codec_error::{CodecError, malformed};
use crate::wire::{Reader, Writer};
use crate::wire_fields::{Field, Id, List, wire_enum};
use semantic_vocabulary::{ScalarType, ValueId};
use terminal_psi::{AsmRegisterView, AssemblyExitConstraint, Block, TerminalMachine, Terminator};
use terminal_semantics::MachineScalarIndex;

struct RegisterView;
impl Field<terminal_psi::AsmRegisterView> for RegisterView {
    fn write(writer: &mut Writer, view: &terminal_psi::AsmRegisterView) -> Result<(), CodecError> {
        writer.u8(view.slot());
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<terminal_psi::AsmRegisterView, CodecError> {
        let slot = reader.u8()?;
        terminal_psi::AsmRegisterView::from_scratch_slot(usize::from(slot))
            .ok_or(CodecError::InvalidTag("AsmRegisterView", slot))
    }
}

wire_enum! {
    AssemblyExit: AssemblyExitConstraint, "AssemblyExitConstraint" {
        1 => Adjacent { target: Id, registers: List<RegisterView> },
        2 => ScalarReturn(view: RegisterView),
    }
}

pub(crate) fn validate(
    machine: &TerminalMachine,
    block: &Block,
    scalars: &MachineScalarIndex<'_>,
) -> Result<(), CodecError> {
    match &block.assembly_exit {
        None => Ok(()),
        Some(AssemblyExitConstraint::ScalarReturn(view)) => {
            let Terminator::Return {
                value,
                cleanup_actions,
                ..
            } = &block.terminator
            else {
                return malformed("assembly register return requires a scalar return terminator");
            };
            if !cleanup_actions.is_empty() {
                return malformed("assembly register return has cleanup actions");
            }
            register_value(machine, block, scalars, *view, *value).map(|_| ())
        }
        Some(AssemblyExitConstraint::Adjacent { target, registers }) => {
            let (next, arguments, cleanup) = match &block.terminator {
                Terminator::Jump {
                    target,
                    arguments,
                    cleanup_actions,
                    ..
                } => (*target, arguments, cleanup_actions),
                Terminator::Conditional { when_false, .. } => (
                    when_false.target,
                    &when_false.arguments,
                    &when_false.cleanup_actions,
                ),
                _ => {
                    return malformed(
                        "assembly adjacency requires a jump or conditional terminator",
                    );
                }
            };
            if next != *target || next == block.id || !cleanup.is_empty() {
                return malformed(
                    "assembly adjacency does not describe a distinct successor without cleanup",
                );
            }
            let successor =
                machine
                    .block(target)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "assembly adjacency names an unknown successor",
                    ))?;
            if registers.len() != arguments.len() || registers.len() != successor.parameters.len() {
                return malformed(
                    "assembly register count differs from the edge arguments or successor parameters",
                );
            }
            // The catalog currently models one non-overlapping 32-bit view per GPR.
            let mut seen = [false; language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT];
            for ((&view, &argument), parameter) in
                registers.iter().zip(arguments).zip(&successor.parameters)
            {
                if std::mem::replace(&mut seen[usize::from(view.slot())], true) {
                    return malformed("assembly edge binds a register more than once");
                }
                if register_value(machine, block, scalars, view, argument)? != parameter.scalar_type
                {
                    return malformed(
                        "assembly register value differs from its successor parameter type",
                    );
                }
            }
            Ok(())
        }
    }
}

fn register_value(
    machine: &TerminalMachine,
    block: &Block,
    scalars: &MachineScalarIndex<'_>,
    view: AsmRegisterView,
    value: ValueId,
) -> Result<ScalarType, CodecError> {
    if machine.assembly_target_requirement != view.target() {
        return malformed(
            "assembly register does not belong to the machine's required architecture",
        );
    }
    match scalars.scalar_type_at_block_end(value, block.id) {
        Some(ty @ ScalarType::Integer(integer))
            if !integer.is_address() && integer.bits() == Some(32) =>
        {
            Ok(ty)
        }
        _ => malformed("assembly register requires a live 32-bit integer value at the block exit"),
    }
}

#[cfg(test)]
mod tests;
