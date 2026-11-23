use omega_psi::{
    BlockTerminator, CleanupAction, CrashCause, EdgeId, LoweredPackage, MachineBody, OperationId, OperationKind, OperationResult, ReturnEdge, ScalarType,
    TrappingIntegerPrimitive, TypeShape, ValueId,
};

#[derive(Debug, Eq, PartialEq)]
pub enum DivisionOutcome<'a> {
    Crash { operation: OperationId, cause: CrashCause },
    Return { edge: EdgeId, value: u8, cleanup: &'a [CleanupAction] },
}

/// Evaluate the single unsigned byte division in this fixture using only
/// Psi's inputs and selected primitive. A crash stops at the operation.
pub fn observe_byte_division<'a>(
    package: &LoweredPackage,
    body: &'a MachineBody,
    inputs: &[(ValueId, u8)],
) -> Option<DivisionOutcome<'a>> {
    let [block] = body.blocks.as_slice() else { return None };
    let [op] = body.operations.get(block.operations.clone())? else { return None };
    let OperationKind::TrappingInteger {
        primitive: TrappingIntegerPrimitive::Divide,
        left,
        right,
    } = op.kind
    else {
        return None;
    };
    let OperationResult::Scalar(result) = op.result else { return None };
    let byte = TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 8 });
    if [left, right, result].into_iter().any(|id| {
        body.values
            .get(id.index())
            .and_then(|value| package.types.get(value.ty))
            != Some(&byte)
    }) {
        return None;
    }
    let input = |id| {
        inputs
            .iter()
            .find(|(parameter, _)| *parameter == id)
            .map(|(_, value)| *value)
    };
    let Some(value) = input(left)?.checked_div(input(right)?) else {
        return Some(DivisionOutcome::Crash {
            operation: op.id,
            cause: CrashCause::Trap,
        });
    };
    let BlockTerminator::Return {
        id,
        result: ReturnEdge::Scalar(returned),
        cleanup,
    } = &block.terminator
    else {
        return None;
    };
    if *returned != result {
        return None;
    }
    Some(DivisionOutcome::Return { edge: *id, value, cleanup })
}
