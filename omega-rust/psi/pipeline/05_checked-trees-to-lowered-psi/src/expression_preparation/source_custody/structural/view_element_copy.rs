//! Replay a `[copy]` record element copied out of a borrowed view.
//!
//! The checked plan retains one element read per scalar field of the copied
//! record. Replay requires the authored initializer to be exactly
//! `collection[selector]` over the bare name the reads root at, every read to
//! share that root and selector, and the reads to name the declared record's
//! fields once each in declaration order. The view keeps its loan: like a
//! shared borrow, the copy is provenance pass-through for owned custody.

use super::{CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, unsupported};
use checked_trees::{
    CheckedScalarExpression, CheckedStorageRoot, CheckedStructuralPredicatePathSegment,
};

pub(super) fn validate(
    checked: &CheckedTrees,
    authored: &checked_trees::state::State,
    expression: ExpressionHandle,
    reference: checked_trees::types::TypeReferenceHandle,
    reads: &[CheckedScalarExpression],
) -> Result<(), LoweringError> {
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(expression) else {
        return unsupported("view element copy lost its authored element selection");
    };
    let ExpressionNode::Name(name) = checked.expression_table.expression(indexed.collection) else {
        return unsupported("view element copy does not select from a bare view name");
    };
    if matches!(
        checked.expression_table.expression(indexed.index),
        ExpressionNode::Range(_)
    ) || name.head_symbol != name.symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return unsupported("view element copy does not select one element of a named view");
    }
    let Some(CheckedScalarExpression::StructuralParameterIndexedRead { root, index, .. }) =
        reads.first()
    else {
        return unsupported("view element copy reads no element field");
    };
    let names_root = match *root {
        CheckedStorageRoot::ViewLocal { symbol } => symbol == name.symbol,
        CheckedStorageRoot::Parameter { index: position } => checked
            .state_parameters(authored)
            .get(position as usize)
            .is_some_and(|parameter| parameter.symbol == name.symbol),
    };
    if !names_root {
        return unsupported("view element copy reads another view than it names");
    }
    let record =
        validation::unwrapped_type_reference(&checked.typed, reference).and_then(|reference| {
            match checked.type_reference_table.type_reference(reference) {
                checked_trees::types::TypeReferenceNode::Named { symbol, .. } => checked
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == *symbol),
                _ => None,
            }
        });
    let Some(record) = record else {
        return unsupported("view element copy establishes no declared record");
    };
    let fields = checked
        .data_members(record)
        .iter()
        .map(|member| match member {
            checked_trees::data::DataMember::Field(field) => Some(
                field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned()),
            ),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(LoweringError::Unsupported(
            "view element copy establishes a record with non-field members",
        ))?;
    if fields.len() != reads.len() {
        return unsupported("view element copy does not read every field once");
    }
    for (field, read) in fields.iter().zip(reads) {
        let CheckedScalarExpression::StructuralParameterIndexedRead {
            root: read_root,
            path,
            index: read_index,
            element_path,
            ..
        } = read
        else {
            return unsupported("view element copy field is not an element read");
        };
        if read_root != root
            || read_index != index
            || !path.is_empty()
            || element_path.as_slice()
                != [CheckedStructuralPredicatePathSegment::Field(field.clone())]
        {
            return unsupported("view element copy field reads another element or leaf");
        }
    }
    Ok(())
}
