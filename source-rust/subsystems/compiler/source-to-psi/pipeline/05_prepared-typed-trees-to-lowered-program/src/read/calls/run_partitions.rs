//! A partition consumes the exact applied live owners of its public declaration.
use super::*;
use language_core::sealed_intrinsics::{CompilerPrimitive, SealedIntrinsic};

impl Program<'_> {
    pub(super) fn check_run_partition(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &StaticInvocation<'_>,
    ) -> Checked<()> {
        let typed = self.typed;
        let Some(machine) = self
            .entered_machine(call.target)
            .and_then(|symbol| typed.machine_by_symbol(symbol))
        else {
            return Ok(());
        };
        let Some(SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::RunPartition(_))) = typed
            .symbols
            .sealed_declaration(machine.symbol, machine.name.as_str())
        else {
            return Ok(());
        };
        let unavailable = |reason| refuse_at(id, Unserved::Call(reason));
        self.visible_to(id, machine.symbol)?;
        self.direct_selection(id, call.target, call.selection, call.binding)?;
        if call.unsupported || call.runtime_receiver {
            return Err(unavailable(
                "run partition needs a direct call to its sealed declaration",
            ));
        }
        let header = typed
            .bodyless_callable_signature(call.application, call.target)
            .map_err(unavailable)?
            .ok_or_else(|| unavailable("run partition needs its prepared applied declaration"))?;
        if !self.owns_call_arguments(
            id,
            call.application,
            machine.symbol,
            call.authored_arguments,
        ) {
            return Err(unavailable(
                "run partition operands belong to another application",
            ));
        }
        let (signature, _) = self.declared_requirement_signature(id, header, &[])?;
        let formals = &signature.parameters;
        if formals.len() != call.arguments.len() {
            return Err(unavailable("run partition lost a declared operand"));
        }
        for (&ty, argument) in formals.iter().zip(call.arguments) {
            let actual = self.expression(id, slots, *argument, Some(ty))?;
            if !self.agrees(actual.result, Some(ty)) {
                return Err(self.mistyped(
                    id,
                    argument_source(&actual),
                    super::super::rejections::Destination::Parameter,
                    Some(ty),
                    actual.result,
                    "run partition operand type",
                ));
            }
        }
        Err(unavailable(
            "run partition requires the ordered transfer of its input cell custody and backing",
        ))
    }
}
