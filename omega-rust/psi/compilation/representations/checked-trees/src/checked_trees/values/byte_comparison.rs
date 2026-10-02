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
        let mut equality = CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::Equal,
            left: Box::new(CheckedScalarExpression::StructuralParameterByteLength {
                root,
                path: path.clone(),
            }),
            right: Box::new(literal(
                u64::try_from(bytes.len()).ok()?,
                PrimitiveType::U64,
            )?),
        };
        for (index, byte) in bytes.iter().enumerate() {
            let read = CheckedScalarExpression::StructuralParameterIndexedRead {
                root,
                path: path.clone(),
                index: Box::new(literal(u64::try_from(index).ok()?, PrimitiveType::U64)?),
                inner_indexes: Vec::new(),
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
    let landed_type = match primitive_type {
        PrimitiveType::U8 => numerics::literals::LandedIntegerType::U8,
        PrimitiveType::U64 => numerics::literals::LandedIntegerType::U64,
        _ => return None,
    };
    let literal = numerics::literals::IntegerLiteral::from_parts(
        false,
        numerics::literals::IntegerRadix::Decimal,
        &value.to_string(),
    )
    .ok()?
    .with_landing(numerics::literals::IntegerLanding {
        landed_type,
        domain: numerics::arithmetic::ArithmeticDomain::Exact,
    });
    Some(CheckedScalarExpression::IntegerLiteral { literal })
}
