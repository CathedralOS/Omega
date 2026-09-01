use super::{
    MatchedPhysicalRead, OperandCoordinate, OperandRelation, PairInstruction, TerminalPairPattern,
};

pub(super) fn failed_relations(
    pattern: &TerminalPairPattern,
    first: &[MatchedPhysicalRead],
    second: &[MatchedPhysicalRead],
) -> Vec<OperandRelation> {
    pattern
        .relations()
        .iter()
        .copied()
        .filter(|relation| !relation_holds(*relation, first, second))
        .collect()
}

fn relation_holds(
    relation: OperandRelation,
    first: &[MatchedPhysicalRead],
    second: &[MatchedPhysicalRead],
) -> bool {
    let (left, right, physical) = match relation {
        OperandRelation::SameVirtualRegister(left, right) => (left, right, false),
        OperandRelation::SamePhysicalViewAndStorageUnits(left, right) => (left, right, true),
    };
    let Some(left) = operand(left, first, second) else {
        return false;
    };
    let Some(right) = operand(right, first, second) else {
        return false;
    };
    if physical {
        left.view == right.view && left.storage_units == right.storage_units
    } else {
        left.virtual_register == right.virtual_register
    }
}

pub(super) fn operand<'a>(
    coordinate: OperandCoordinate,
    first: &'a [MatchedPhysicalRead],
    second: &'a [MatchedPhysicalRead],
) -> Option<&'a MatchedPhysicalRead> {
    let operands = match coordinate.instruction {
        PairInstruction::First => first,
        PairInstruction::Second => second,
    };
    operands
        .iter()
        .find(|operand| operand.operand == coordinate.operand)
}
