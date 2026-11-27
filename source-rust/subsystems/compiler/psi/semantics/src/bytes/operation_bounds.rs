use crate::{OperationSemanticError, OperationSemanticTag, ValueTypes};
use semantic_vocabulary::{
    IntegerSign, IntegerType, ObligationId, Proposition, ScalarTerm, ScalarType,
};
use terminal_psi::{Operation, OperationKind};

/// The goal owned by an operation, shared by checking and evidence
/// publication. Collection bounds do not establish their own source, access or
/// measurement correspondence; those remain separate storage judgments.
pub fn operation_safety_goal(
    operation: &Operation,
    types: &impl ValueTypes,
) -> Result<Option<(ObligationId, Proposition)>, OperationSemanticError> {
    if let Some(semantics) = crate::proof_bearing_scalar_leaf_semantics(operation, types)? {
        return Ok(Some((
            semantics.obligation(),
            semantics.canonical_goal().kernel_proposition()?,
        )));
    }
    let count =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    let value = |id, tag| {
        let actual = types
            .value_type(id)
            .ok_or(OperationSemanticError::UnknownValue(id))?;
        if actual != count {
            return Err(OperationSemanticError::OperandShapeMismatch(tag));
        }
        Ok(ScalarTerm::value(id, count))
    };
    let (obligation, goal) = match &operation.kind {
        OperationKind::ByteSequenceRead {
            index,
            length,
            obligation,
            ..
        }
        | OperationKind::StructuralByteSequenceFieldRead {
            index,
            length,
            obligation,
            ..
        }
        | OperationKind::ByteSequenceWrite {
            index,
            length,
            obligation,
            ..
        }
        | OperationKind::ElementViewRead {
            index,
            length,
            obligation,
            ..
        } => {
            let tag = OperationSemanticTag::for_operation(&operation.kind);
            (
                *obligation,
                Proposition::LessThan(value(*index, tag)?, value(*length, tag)?),
            )
        }
        OperationKind::ByteSequenceSubslice {
            start,
            end,
            length,
            obligation,
            ..
        }
        | OperationKind::ElementViewSubslice {
            start,
            end,
            length,
            obligation,
            ..
        } => {
            let tag = OperationSemanticTag::for_operation(&operation.kind);
            let (start, end, length) =
                (value(*start, tag)?, value(*end, tag)?, value(*length, tag)?);
            (
                *obligation,
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(start, end.clone()),
                    Proposition::LessOrEqual(end, length),
                ]),
            )
        }
        OperationKind::ElementViewFixedArray {
            length,
            extent,
            obligation,
            ..
        } => {
            let tag = OperationSemanticTag::for_operation(&operation.kind);
            (
                *obligation,
                Proposition::Equal(
                    value(*length, tag)?,
                    ScalarTerm::integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"),
                        semantic_vocabulary::IntegerValue::Unsigned(u128::from(*extent)),
                    )
                    .map_err(OperationSemanticError::InvalidProposition)?,
                ),
            )
        }
        _ => return Ok(None),
    };
    Ok(Some((obligation, goal)))
}
