//! Bind register observations on a linear assembly CFG to the SSA operations
//! that establish them. Physical selection must preserve each observed version
//! at its own edge, even when it is overwritten before the final return.
use operation_operands::values::{AssemblyRegisterMaterialization, AssemblyRegisterVersion};
use semantic_vocabulary::{EdgeId, ValueId};
use terminal_psi::{
    AsmRegisterView, AssemblyExitConstraint, OperationKind, TerminalMachine, Terminator,
};

pub(super) fn derive(
    machine: &TerminalMachine,
    result: AsmRegisterView,
) -> Result<Vec<AssemblyRegisterVersion>, String> {
    if machine.blocks.first().map(|block| block.id) != Some(machine.entry) {
        return Err("the physical assembly range must begin at its callable entry".into());
    }
    let mut versions = Vec::new();
    let mut incoming: Vec<(ValueId, AssemblyRegisterVersion)> = Vec::new();
    for (index, block) in machine.blocks.iter().enumerate() {
        if block
            .operations
            .iter()
            .any(|op| !matches!(op.kind, OperationKind::IntegerConstant { .. }))
        {
            return Err(
                "the assembly instruction's register placement and effects are not yet represented"
                    .into(),
            );
        }
        let observe = |edge: EdgeId, view: AsmRegisterView, value: ValueId| {
            if view != result {
                return Err(
                    "an additional working register needs its own preservation and clobber check",
                );
            }
            if let Some((_, prior)) = incoming.iter().find(|(parameter, _)| *parameter == value) {
                return Ok(AssemblyRegisterVersion {
                    edge,
                    view,
                    ..*prior
                });
            }
            let definition = block
                .operations
                .iter()
                .find(|op| op.result.scalar_ref().is_some_and(|decl| decl.id == value))
                .ok_or("the incoming assembly register has no checked definition")?;
            let OperationKind::IntegerConstant { value: literal } = definition.kind else {
                return Err("the assembly register definition has no physical materialization");
            };
            Ok(AssemblyRegisterVersion {
                edge,
                view,
                value,
                operation: definition.id,
                materialization: AssemblyRegisterMaterialization::IntegerImmediate(literal),
            })
        };
        match (&block.assembly_exit, &block.terminator) {
            (
                Some(AssemblyExitConstraint::Adjacent { target, registers }),
                Terminator::Jump {
                    edge,
                    target: successor,
                    arguments,
                    ..
                },
            ) => {
                let next = machine
                    .blocks
                    .get(index + 1)
                    .filter(|next| next.id == *target && next.id == *successor)
                    .ok_or("required adjacent edge has no contiguous unique destination")?;
                if next.parameters.len() != registers.len() || arguments.len() != registers.len() {
                    return Err(
                        "assembly edge bindings do not cover the receiver's register values".into(),
                    );
                }
                let mut arrived = Vec::with_capacity(registers.len());
                for ((&view, &argument), parameter) in
                    registers.iter().zip(arguments).zip(&next.parameters)
                {
                    let version = observe(*edge, view, argument)?;
                    versions.push(version);
                    arrived.push((parameter.id, version));
                }
                incoming = arrived;
            }
            (
                Some(AssemblyExitConstraint::ScalarReturn(view)),
                Terminator::Return { edge, value, .. },
            ) if index + 1 == machine.blocks.len() => {
                versions.push(observe(*edge, *view, *value)?);
            }
            _ => {
                return Err(
                    "a checked assembly return needs a physically modeled path to each local label"
                        .into(),
                );
            }
        }
    }
    Ok(versions)
}
