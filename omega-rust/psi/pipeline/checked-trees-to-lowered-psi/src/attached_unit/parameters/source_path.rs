//! Authored structural paths shared by parameter and call-result operands.

use super::*;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};

enum SourceProjection {
    Field(ExpressionHandle, symbols::SymbolHandle),
    Index(u64),
}

pub(crate) fn source_path(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    source_type: TypeReferenceHandle,
    mut expression: ExpressionHandle,
) -> Result<
    (
        symbols::SymbolHandle,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        Option<checked_trees::CheckedStructuralAccess>,
    ),
    LoweringError,
> {
    let mut projections = Vec::new();
    let mut access = None;
    let mut visited = Vec::new();
    let root = loop {
        if !checked.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            return unsupported("scalar wrapper structural argument has a stale or cyclic source");
        }
        visited.push(expression);
        match checked.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) if access.is_none() && projections.is_empty() => {
                access = Some(match borrow.access {
                    language_core::ReferenceAccess::Shared => {
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                    }
                    language_core::ReferenceAccess::Mutable => {
                        checked_trees::CheckedStructuralAccess::MutableBorrow
                    }
                    language_core::ReferenceAccess::WriteOnly => {
                        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
                    }
                });
                expression = borrow.target;
            }
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                projections.push(SourceProjection::Field(expression, member.member_symbol));
                expression = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) =
                    checked.expression_table.expression(indexed.index)
                else {
                    return unsupported(
                        "scalar wrapper structural argument requires a literal index",
                    );
                };
                let index = index
                    .value_bignum()
                    .and_then(|value| value.to_u64())
                    .ok_or(LoweringError::Unsupported(
                        "scalar wrapper structural index exceeds u64",
                    ))?;
                projections.push(SourceProjection::Index(index));
                expression = indexed.collection;
            }
            ExpressionNode::Name(name)
                if name.symbol.is_valid()
                    && name.symbol == name.head_symbol
                    && checked
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1 =>
            {
                break name.symbol;
            }
            _ => return unsupported("scalar wrapper structural argument is not a parameter place"),
        }
    };
    let mut type_reference = source_type;
    let mut path = Vec::new();
    for projection in projections.into_iter().rev() {
        type_reference = unqualified_source_type(checked, type_reference)?;
        match projection {
            SourceProjection::Field(expression, symbol) => {
                let owner = match checked.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Named { symbol, .. } => *symbol,
                    TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                    _ => return unsupported("scalar wrapper field has no declared record owner"),
                };
                let data = checked
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == owner)
                    .ok_or(LoweringError::Unsupported(
                        "scalar wrapper field owner is absent",
                    ))?;
                let symbol = validation::exact_self_field(&checked.typed, machine, expression)
                    .map_or(symbol, |field| field.symbol);
                let ExpressionNode::Member(authored) =
                    checked.expression_table.expression(expression)
                else {
                    unreachable!("field projection retained its member expression")
                };
                // Local field selection can remain unresolved in typed syntax.
                // Its declared owner and unique authored field name resolve it;
                // a present field symbol must still match exactly.
                let mut fields =
                    checked
                        .data_members(data)
                        .iter()
                        .filter_map(|member| match member {
                            checked_trees::data::DataMember::Field(field)
                                if if symbol.is_valid() {
                                    field.symbol == symbol
                                } else {
                                    field.name.as_str() == authored.member.as_str()
                                } =>
                            {
                                Some(field)
                            }
                            _ => None,
                        });
                let field = fields.next().ok_or(LoweringError::Unsupported(
                    "scalar wrapper field substituted its declaration owner",
                ))?;
                if fields.next().is_some() {
                    return unsupported(
                        "structural source field is ambiguous in its declared owner",
                    );
                }
                path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ));
                type_reference = field.type_reference;
            }
            SourceProjection::Index(index) => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: checked_trees::types::FixedArrayLength::Literal(length),
                } = checked.type_reference_table.type_reference(type_reference)
                else {
                    return unsupported("scalar wrapper index has no literal fixed-array owner");
                };
                if usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index >= *length)
                {
                    return unsupported("scalar wrapper structural index is out of bounds");
                }
                path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    index,
                ));
                type_reference = *element_type;
            }
        }
    }
    Ok((root, path, access))
}

fn unqualified_source_type(
    checked: &CheckedTrees,
    mut reference: TypeReferenceHandle,
) -> Result<TypeReferenceHandle, LoweringError> {
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return unsupported("scalar wrapper source has an invalid type chain");
        }
        visited.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            _ => return Ok(reference),
        }
    }
}
