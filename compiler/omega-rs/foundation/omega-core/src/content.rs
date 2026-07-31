//! Compiler-owned resource-content algebra and normalized projection plans.
//!
//! These records are semantic facts, not runtime values. Field symbols remain
//! available to checked consumers, while fingerprints fold stable field names
//! and normalized type identities rather than arena-local handles.

use crate::semantics::SemanticDomainId;
use crate::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentAlgebraIdentity {
    Interval { coordinate_space: String },
    CountedQuantity { unit: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFieldSegment {
    pub symbol: SymbolHandle,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentArithmeticOperator {
    Add,
    Subtract,
    Multiply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentScalarExpression {
    SubjectField(Vec<ContentFieldSegment>),
    Natural(String),
    Successor(Box<ContentScalarExpression>),
    Arithmetic {
        operator: ContentArithmeticOperator,
        left: Box<ContentScalarExpression>,
        right: Box<ContentScalarExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentProjectionExpression {
    Interval {
        start: ContentScalarExpression,
        end: ContentScalarExpression,
    },
    CountedQuantity {
        magnitude: ContentScalarExpression,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentProjectionPlan {
    pub domain: SymbolHandle,
    pub semantic_domain: SemanticDomainId,
    pub carrier_identity: String,
    pub machine: SymbolHandle,
    pub algebra: ContentAlgebraIdentity,
    pub expression: ContentProjectionExpression,
    pub fingerprint: u64,
}

pub fn projection_fingerprint(
    algebra: &ContentAlgebraIdentity,
    expression: &ContentProjectionExpression,
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut bytes = Vec::new();
    encode_algebra(algebra, &mut bytes);
    encode_projection(expression, &mut bytes);
    bytes.into_iter().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(byte);
        hash.wrapping_mul(PRIME)
    })
}

fn encode_algebra(algebra: &ContentAlgebraIdentity, output: &mut Vec<u8>) {
    match algebra {
        ContentAlgebraIdentity::Interval { coordinate_space } => {
            output.push(1);
            encode_string(coordinate_space, output);
        }
        ContentAlgebraIdentity::CountedQuantity { unit } => {
            output.push(2);
            encode_string(unit, output);
        }
    }
}

fn encode_projection(expression: &ContentProjectionExpression, output: &mut Vec<u8>) {
    match expression {
        ContentProjectionExpression::Interval { start, end } => {
            output.push(1);
            encode_scalar(start, output);
            encode_scalar(end, output);
        }
        ContentProjectionExpression::CountedQuantity { magnitude } => {
            output.push(2);
            encode_scalar(magnitude, output);
        }
    }
}

fn encode_scalar(expression: &ContentScalarExpression, output: &mut Vec<u8>) {
    match expression {
        ContentScalarExpression::SubjectField(path) => {
            output.push(1);
            output.extend_from_slice(&(path.len() as u64).to_le_bytes());
            for segment in path {
                encode_string(&segment.name, output);
            }
        }
        ContentScalarExpression::Natural(value) => {
            output.push(2);
            encode_string(value, output);
        }
        ContentScalarExpression::Successor(value) => {
            output.push(3);
            encode_scalar(value, output);
        }
        ContentScalarExpression::Arithmetic {
            operator,
            left,
            right,
        } => {
            output.push(4);
            output.push(match operator {
                ContentArithmeticOperator::Add => 1,
                ContentArithmeticOperator::Subtract => 2,
                ContentArithmeticOperator::Multiply => 3,
            });
            encode_scalar(left, output);
            encode_scalar(right, output);
        }
    }
}

fn encode_string(value: &str, output: &mut Vec<u8>) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_fingerprint_ignores_arena_local_field_symbols() {
        let expression = |index| ContentProjectionExpression::CountedQuantity {
            magnitude: ContentScalarExpression::SubjectField(vec![ContentFieldSegment {
                symbol: SymbolHandle::from_arena_index(index),
                name: "remaining".to_owned(),
            }]),
        };
        let algebra = ContentAlgebraIdentity::CountedQuantity {
            unit: "named(name(Byte))".to_owned(),
        };

        assert_eq!(
            projection_fingerprint(&algebra, &expression(7)),
            projection_fingerprint(&algebra, &expression(91))
        );
    }
}
