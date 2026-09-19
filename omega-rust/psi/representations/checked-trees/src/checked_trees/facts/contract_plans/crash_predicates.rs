//! Crash causes, interfaces, predicate expressions and route guards.

use std::cmp::Ordering;
use std::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashCause {
    Trap,
    Abort,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CrashInterface {
    #[default]
    InternalInferred,
    PublishedCeiling,
}

/// Source-independent checked syntax retained for one guarded crash route.
///
/// This is intentionally parameter-relative rather than tied to terminal
/// `ValueId`s. The terminal producer assigns those identities and lowers the
/// supported scalar subset into `semantic_vocabulary::Proposition`; syntax outside that
/// subset remains explicit and fails closed there.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashPredicateExpression {
    Invalid,
    Binary {
        operator: u8,
        left: Box<Self>,
        right: Box<Self>,
    },
    Unary {
        operator: u8,
        operand: Box<Self>,
    },
    Integer(String),
    /// A float literal's exact source spelling (the `FloatLiteral` text:
    /// suffix-free, text-only identity like the integer carrier). The literal
    /// is a closed leaf — no formal can hide inside it — so entry
    /// substitution transports it the same way it transports `Integer`.
    Float(String),
    Boolean(bool),
    Name(Vec<String>),
    Member {
        receiver: Box<Self>,
        member: String,
    },
    /// A `collection[index]` read. Both children are explicit, so entry
    /// substitution transports any formals they carry rather than hiding
    /// them inside a flattened display the way `Opaque` did. The index is
    /// an evaluated operand, not a fixed field projection.
    Indexed {
        collection: Box<Self>,
        index: Box<Self>,
    },
    /// A `start..end` (or `start..=end`) range operand. Both bounds keep
    /// their own expressions so a bound carrying a formal still substitutes;
    /// `end_inclusive` is part of the identity since the bounds alone cannot
    /// distinguish `..` from `..=`.
    Range {
        start: Box<Self>,
        end: Box<Self>,
        end_inclusive: bool,
    },
    Call {
        target: String,
        receiver: Box<Self>,
        arguments: Vec<Self>,
    },
    Opaque(String),
    Parameter(u32),
    ContentConservation(Vec<u8>),
}

impl CrashPredicateExpression {
    pub fn substitute(&self, arguments: &[Option<Self>]) -> Self {
        match self {
            Self::Parameter(index) => arguments
                .get(*index as usize)
                .and_then(Clone::clone)
                .unwrap_or_else(|| self.clone()),
            Self::Binary {
                operator,
                left,
                right,
            } => Self::Binary {
                operator: *operator,
                left: Box::new(left.substitute(arguments)),
                right: Box::new(right.substitute(arguments)),
            },
            Self::Unary { operator, operand } => Self::Unary {
                operator: *operator,
                operand: Box::new(operand.substitute(arguments)),
            },
            Self::Member { receiver, member } => Self::Member {
                receiver: Box::new(receiver.substitute(arguments)),
                member: member.clone(),
            },
            Self::Indexed { collection, index } => Self::Indexed {
                collection: Box::new(collection.substitute(arguments)),
                index: Box::new(index.substitute(arguments)),
            },
            Self::Range {
                start,
                end,
                end_inclusive,
            } => Self::Range {
                start: Box::new(start.substitute(arguments)),
                end: Box::new(end.substitute(arguments)),
                end_inclusive: *end_inclusive,
            },
            Self::Call {
                target,
                receiver,
                arguments: nested,
            } => Self::Call {
                target: target.clone(),
                receiver: Box::new(receiver.substitute(arguments)),
                arguments: nested
                    .iter()
                    .map(|argument| argument.substitute(arguments))
                    .collect(),
            },
            _ => self.clone(),
        }
    }

    pub fn boolean_value(&self) -> Option<bool> {
        use typed_trees::expression::{BinaryOperator, UnaryOperator};

        match self {
            Self::Boolean(value) => Some(*value),
            Self::Unary { operator, operand } if *operator == UnaryOperator::LogicalNot as u8 => {
                operand.boolean_value().map(|value| !value)
            }
            Self::Binary {
                operator,
                left,
                right,
            } if *operator == BinaryOperator::And as u8 => {
                Some(left.boolean_value()? && right.boolean_value()?)
            }
            Self::Binary {
                operator,
                left,
                right,
            } if *operator == BinaryOperator::Or as u8 => {
                Some(left.boolean_value()? || right.boolean_value()?)
            }
            _ => None,
        }
    }

    fn write_canonical(&self, out: &mut Vec<u8>) {
        match self {
            Self::Invalid => out.push(0),
            Self::Binary {
                operator,
                left,
                right,
            } => {
                out.push(1);
                out.push(*operator);
                left.write_canonical(out);
                right.write_canonical(out);
            }
            Self::Unary { operator, operand } => {
                out.push(2);
                out.push(*operator);
                operand.write_canonical(out);
            }
            Self::Integer(value) => {
                out.push(3);
                out.extend(value.as_bytes());
                out.push(0);
            }
            Self::Boolean(value) => {
                out.push(4);
                out.push(u8::from(*value));
            }
            Self::Float(value) => {
                out.push(0x0a);
                out.extend(value.as_bytes());
                out.push(0);
            }
            Self::Name(members) => {
                out.push(5);
                for member in members {
                    out.extend(member.as_bytes());
                    out.push(b'.');
                }
                out.push(0);
            }
            Self::Member { receiver, member } => {
                out.push(6);
                receiver.write_canonical(out);
                out.extend(member.as_bytes());
                out.push(0);
            }
            Self::Indexed { collection, index } => {
                out.push(0x0b);
                collection.write_canonical(out);
                index.write_canonical(out);
            }
            Self::Range {
                start,
                end,
                end_inclusive,
            } => {
                out.push(0x0d);
                out.push(u8::from(*end_inclusive));
                start.write_canonical(out);
                end.write_canonical(out);
            }
            Self::Call {
                target,
                receiver,
                arguments,
            } => {
                out.push(7);
                out.extend(target.as_bytes());
                out.push(0);
                receiver.write_canonical(out);
                for argument in arguments {
                    argument.write_canonical(out);
                }
                out.push(0xfe);
            }
            Self::Opaque(display) => {
                out.push(8);
                out.extend(display.as_bytes());
                out.push(0);
            }
            Self::Parameter(index) => {
                out.push(9);
                out.extend(index.to_le_bytes());
            }
            Self::ContentConservation(bytes) => {
                out.push(0xcc);
                out.extend(bytes);
            }
        }
    }
}

/// Source-independent identity plus the checked syntax that produced it. The
/// canonical bytes remain the equality/hash material used by checked joins;
/// syntax is retained solely so later semantic lowering need not interpret
/// those identity bytes as executable meaning.
#[derive(Debug, Clone)]
pub struct CrashPredicateIdentity {
    canonical_bytes: Vec<u8>,
    expression: Option<CrashPredicateExpression>,
    scalar_expression: Option<crate::CheckedBooleanExpression>,
}

impl CrashPredicateIdentity {
    pub fn from_canonical_bytes(bytes: Vec<u8>) -> Self {
        Self {
            canonical_bytes: bytes,
            expression: None,
            scalar_expression: None,
        }
    }

    pub fn from_expression(expression: CrashPredicateExpression) -> Self {
        let mut canonical_bytes = vec![1]; // ProofFact::Expression
        expression.write_canonical(&mut canonical_bytes);
        Self {
            canonical_bytes,
            expression: Some(expression),
            scalar_expression: None,
        }
    }

    pub fn from_expression_and_scalar(
        expression: CrashPredicateExpression,
        scalar_expression: crate::CheckedBooleanExpression,
    ) -> Self {
        let mut identity = Self::from_expression(expression);
        identity.scalar_expression = Some(scalar_expression);
        identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn expression(&self) -> Option<&CrashPredicateExpression> {
        self.expression.as_ref()
    }

    pub const fn scalar_expression(&self) -> Option<&crate::CheckedBooleanExpression> {
        self.scalar_expression.as_ref()
    }
}

impl PartialEq for CrashPredicateIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_bytes == other.canonical_bytes
    }
}

impl Eq for CrashPredicateIdentity {}

impl PartialOrd for CrashPredicateIdentity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CrashPredicateIdentity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.canonical_bytes.cmp(&other.canonical_bytes)
    }
}

impl Hash for CrashPredicateIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.canonical_bytes.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashRouteGuard {
    /// The canonical route contributed by a route-less clause or an authored
    /// `true` route. It subsumes every guarded alternative in its bucket.
    Truth,
    Predicate(CrashPredicateIdentity),
}

/// Dense, one-based identity of a canonical published route bucket within one
/// machine's crash plan. Bucket normalization happens before these identities
/// are assigned, so clause regrouping and duplicate routes cannot renumber
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashRouteBucketId(u32);

impl CrashRouteBucketId {
    pub(crate) fn from_index(index: usize) -> Self {
        Self(
            u32::try_from(index)
                .expect("published crash bucket count exceeds checked identity range")
                .checked_add(1)
                .expect("published crash bucket identity is one-based"),
        )
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    pub(crate) fn index(self) -> Option<usize> {
        usize::try_from(self.0.checked_sub(1)?).ok()
    }
}
