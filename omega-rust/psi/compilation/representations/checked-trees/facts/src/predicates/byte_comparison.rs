//! The pure content query over an already evaluated byte carrier.
//! Length equality guards every byte observation. Operand preparation belongs
//! to the executable computation; this template grants no place or loan custody.
use super::{CheckedBooleanExpression, CheckedScalarExpression, CheckedStorageRoot};
use crate::{CheckedIntegerComparisonKind, CheckedStructuralPredicatePathSegment};
use typed_trees::types::PrimitiveType;
impl CheckedBooleanExpression {
    pub fn byte_literal_equality(
        root: CheckedStorageRoot,
        path: Vec<CheckedStructuralPredicatePathSegment>,
        bytes: &[u8],
    ) -> Option<Self> {
        Self::byte_literal_equality_in_extent(root, path, bytes, None)
    }

    pub fn byte_literal_equality_in_extent(
        root: CheckedStorageRoot,
        path: Vec<CheckedStructuralPredicatePathSegment>,
        bytes: &[u8],
        maximum_length: Option<u64>,
    ) -> Option<Self> {
        let mut equality = CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::Equal,
            left: Box::new(CheckedScalarExpression::StructuralSequenceLength {
                root,
                path: path
                    .iter()
                    .cloned()
                    .map(super::CheckedScalarReadProjection::Static)
                    .collect(),
            }),
            right: Box::new(literal(
                u64::try_from(bytes.len()).ok()?,
                PrimitiveType::U64,
            )?),
        };
        for (index, byte) in bytes.iter().enumerate().take(
            maximum_length
                .and_then(|length| usize::try_from(length).ok())
                .unwrap_or(bytes.len()),
        ) {
            let read = CheckedScalarExpression::StructuralParameterIndexedRead {
                root,
                path: path.clone(),
                index: Box::new(super::CheckedScalarSelector {
                    expression: literal(u64::try_from(index).ok()?, PrimitiveType::U64)?,
                    occurrence: typed_trees::expression::ExpressionHandle::invalid(),
                }),
                element_path: Vec::new(),
                primitive_type: PrimitiveType::U8,
            };
            let comparison = CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::Equal,
                left: Box::new(read),
                right: Box::new(literal(u64::from(*byte), PrimitiveType::U8)?),
            };
            equality = CheckedBooleanExpression::And {
                left: Box::new(equality),
                right: Box::new(comparison),
            };
        }
        Some(equality)
    }
}
fn literal(value: u64, primitive_type: PrimitiveType) -> Option<CheckedScalarExpression> {
    super::u64_integer_literal(value, primitive_type)
}
