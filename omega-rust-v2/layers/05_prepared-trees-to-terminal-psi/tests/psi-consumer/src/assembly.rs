/// Follow a conditional assembly edge to the operation that produced its Boolean,
/// then read the selected flag and the destinations recorded by the producer.
pub fn assembly_branch<'a>(
    body: &'a omega_psi::MachineBody,
    block: &'a omega_psi::BranchBlock,
) -> Option<(&'a str, omega_psi::BlockId, omega_psi::BlockId)> {
    use omega_psi::{AssemblyOperand, BlockTerminator, InstructionExit, InstructionInput, OperationKind, OperationResult};
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &block.terminator
    else {
        return None;
    };
    let omega_psi::ValueOrigin::Operation(id) = body.values.get(condition.index())?.origin else {
        return None;
    };
    let producer = body.operations.get(id.index())?;
    if producer.result != OperationResult::Scalar(*condition) {
        return None;
    }
    let OperationKind::AssemblyInstruction(selected) = &producer.kind else {
        return None;
    };
    let InstructionExit::Branch {
        condition: InstructionInput::Register(flag),
        target,
    } = &selected.semantics.exit
    else {
        return None;
    };
    let Some(AssemblyOperand::LocalBlock(taken)) = selected.operands.get(*target) else {
        return None;
    };
    if *taken != when_true.target || when_true.requires_adjacency || !when_false.requires_adjacency {
        return None;
    }
    Some((flag, *taken, when_false.target))
}
