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
    /// A checked full-carrier integer widening. These are the source/target
    /// `PrimitiveType` and `ArithmeticDomain` tags, like the operator tags
    /// above: identity data, not permission to evaluate a general cast.
    /// Keep placement and policy even though widening preserves the value:
    /// `(x + 1) as u64` and `(x as u64) + 1` need not do the same arithmetic.
    IntegerWiden {
        source_type: u8,
        target_type: u8,
        domain: u8,
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
        // An explicit worklist keeps deep predicates off the call stack,
        // matching the sibling validators in semantic-vocabulary and
        // terminal-codec: `Visit` pushes a `Finalize` step behind the node's
        // children so the rebuilt parent is constructed only after every
        // child has produced its substituted tree on the value stack.
        // Children push in reverse, so the left operand still rebuilds
        // first — substitution itself has no side effects, but the order
        // keeps the value-stack invariant obvious. Every node pushes
        // exactly one rebuilt tree, so `built` holds the root when the
        // worklist drains.
        enum Step<'a> {
            Visit(&'a CrashPredicateExpression),
            Finalize(&'a CrashPredicateExpression),
        }
        let mut pending = vec![Step::Visit(self)];
        let mut built: Vec<Self> = Vec::new();
        while let Some(step) = pending.pop() {
            match step {
                Step::Visit(term) => match term {
                    Self::Parameter(index) => built.push(
                        arguments
                            .get(*index as usize)
                            .and_then(Clone::clone)
                            .unwrap_or_else(|| term.clone()),
                    ),
                    Self::Binary { left, right, .. } => {
                        pending.push(Step::Finalize(term));
                        pending.push(Step::Visit(right));
                        pending.push(Step::Visit(left));
                    }
                    Self::Unary { operand, .. } | Self::IntegerWiden { operand, .. } => {
                        pending.push(Step::Finalize(term));
                        pending.push(Step::Visit(operand));
                    }
                    Self::Member { receiver, .. } => {
                        pending.push(Step::Finalize(term));
                        pending.push(Step::Visit(receiver));
                    }
                    Self::Indexed { collection, index } => {
                        pending.push(Step::Finalize(term));
                        pending.push(Step::Visit(index));
                        pending.push(Step::Visit(collection));
                    }
                    Self::Range { start, end, .. } => {
                        pending.push(Step::Finalize(term));
                        pending.push(Step::Visit(end));
                        pending.push(Step::Visit(start));
                    }
                    Self::Call {
                        receiver,
                        arguments: nested,
                        ..
                    } => {
                        pending.push(Step::Finalize(term));
                        for argument in nested.iter().rev() {
                            pending.push(Step::Visit(argument));
                        }
                        pending.push(Step::Visit(receiver));
                    }
                    Self::Invalid
                    | Self::Integer(_)
                    | Self::Float(_)
                    | Self::Boolean(_)
                    | Self::Name(_)
                    | Self::Opaque(_)
                    | Self::ContentConservation(_) => built.push(term.clone()),
                },
                Step::Finalize(term) => {
                    let rebuilt = match term {
                        Self::Binary { operator, .. } => {
                            let right = built.pop().expect("right child rebuilt before its parent");
                            let left = built.pop().expect("left child rebuilt before its parent");
                            Self::Binary {
                                operator: *operator,
                                left: Box::new(left),
                                right: Box::new(right),
                            }
                        }
                        Self::Unary { operator, .. } => {
                            let operand = built.pop().expect("operand rebuilt before its parent");
                            Self::Unary {
                                operator: *operator,
                                operand: Box::new(operand),
                            }
                        }
                        Self::IntegerWiden {
                            source_type,
                            target_type,
                            domain,
                            ..
                        } => {
                            let operand = built.pop().expect("operand rebuilt before its parent");
                            Self::IntegerWiden {
                                source_type: *source_type,
                                target_type: *target_type,
                                domain: *domain,
                                operand: Box::new(operand),
                            }
                        }
                        Self::Member { member, .. } => {
                            let receiver = built.pop().expect("receiver rebuilt before its parent");
                            Self::Member {
                                receiver: Box::new(receiver),
                                member: member.clone(),
                            }
                        }
                        Self::Indexed { .. } => {
                            let index = built.pop().expect("index rebuilt before its parent");
                            let collection =
                                built.pop().expect("collection rebuilt before its parent");
                            Self::Indexed {
                                collection: Box::new(collection),
                                index: Box::new(index),
                            }
                        }
                        Self::Range { end_inclusive, .. } => {
                            let end = built.pop().expect("range end rebuilt before its parent");
                            let start = built.pop().expect("range start rebuilt before its parent");
                            Self::Range {
                                start: Box::new(start),
                                end: Box::new(end),
                                end_inclusive: *end_inclusive,
                            }
                        }
                        Self::Call {
                            target,
                            arguments: nested,
                            ..
                        } => {
                            let mut arguments = Vec::with_capacity(nested.len());
                            for _ in nested {
                                arguments.push(
                                    built
                                        .pop()
                                        .expect("call argument rebuilt before its parent"),
                                );
                            }
                            arguments.reverse();
                            let receiver = built
                                .pop()
                                .expect("call receiver rebuilt before its parent");
                            Self::Call {
                                target: target.clone(),
                                receiver: Box::new(receiver),
                                arguments,
                            }
                        }
                        _ => unreachable!("childless nodes finish inside Visit"),
                    };
                    built.push(rebuilt);
                }
            }
        }
        debug_assert_eq!(built.len(), 1);
        built.pop().expect("substitution finishes its root")
    }

    pub fn boolean_value(&self) -> Option<bool> {
        use typed_trees::expression::{BinaryOperator, UnaryOperator};

        // The same Visit/Finalize worklist as `substitute`: only the
        // boolean subset evaluates, and every other form answers `None`
        // inline — the recursive `_ => None` made any non-evaluable node
        // fail the whole tree, so rejecting at `Visit` keeps the identical
        // result while no descendant of a failed node is ever walked.
        // Operators outside the boolean subset (`And`/`Or`/`LogicalNot`)
        // reject the same way — the visit arms carry the guards the match
        // used to.
        enum Step<'a> {
            Visit(&'a CrashPredicateExpression),
            Finalize(&'a CrashPredicateExpression),
        }
        let mut pending = vec![Step::Visit(self)];
        let mut values: Vec<bool> = Vec::new();
        while let Some(step) = pending.pop() {
            match step {
                Step::Visit(term) => match term {
                    Self::Boolean(value) => values.push(*value),
                    Self::Unary { operator, operand }
                        if *operator == UnaryOperator::LogicalNot as u8 =>
                    {
                        pending.push(Step::Finalize(term));
                        pending.push(Step::Visit(operand));
                    }
                    Self::Binary {
                        operator,
                        left,
                        right,
                        ..
                    } if *operator == BinaryOperator::And as u8
                        || *operator == BinaryOperator::Or as u8 =>
                    {
                        pending.push(Step::Finalize(term));
                        pending.push(Step::Visit(right));
                        pending.push(Step::Visit(left));
                    }
                    _ => return None,
                },
                Step::Finalize(term) => match term {
                    Self::Unary { .. } => {
                        let operand = values.pop().expect("operand evaluated before its parent");
                        values.push(!operand);
                    }
                    Self::Binary { operator, .. } if *operator == BinaryOperator::And as u8 => {
                        let right = values
                            .pop()
                            .expect("right operand evaluated before its parent");
                        let left = values
                            .pop()
                            .expect("left operand evaluated before its parent");
                        values.push(left && right);
                    }
                    Self::Binary { operator, .. } if *operator == BinaryOperator::Or as u8 => {
                        let right = values
                            .pop()
                            .expect("right operand evaluated before its parent");
                        let left = values
                            .pop()
                            .expect("left operand evaluated before its parent");
                        values.push(left || right);
                    }
                    _ => unreachable!("only boolean-subset nodes reach Finalize"),
                },
            }
        }
        debug_assert_eq!(values.len(), 1);
        values.pop()
    }

    fn write_canonical(&self, out: &mut Vec<u8>) {
        // Pre-order serialization on an explicit stack, so deep predicates
        // no longer spend call stack. A node's tag and inline fields emit
        // before its children push in reverse, keeping the byte stream in
        // declaration order; `Emit` and `EmitBytes` carry the writes that
        // used to run after a recursive call returned (the member name and
        // the call terminator).
        enum Step<'a> {
            Node(&'a CrashPredicateExpression),
            Emit(u8),
            EmitBytes(&'a [u8]),
        }
        let mut pending = vec![Step::Node(self)];
        while let Some(step) = pending.pop() {
            match step {
                Step::Node(term) => match term {
                    Self::Invalid => out.push(0),
                    Self::Binary {
                        operator,
                        left,
                        right,
                    } => {
                        out.push(1);
                        out.push(*operator);
                        pending.push(Step::Node(right));
                        pending.push(Step::Node(left));
                    }
                    Self::Unary { operator, operand } => {
                        out.push(2);
                        out.push(*operator);
                        pending.push(Step::Node(operand));
                    }
                    Self::IntegerWiden {
                        source_type,
                        target_type,
                        domain,
                        operand,
                    } => {
                        out.extend([0x0e, *source_type, *target_type, *domain]);
                        pending.push(Step::Node(operand));
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
                        pending.push(Step::Emit(0));
                        pending.push(Step::EmitBytes(member.as_bytes()));
                        pending.push(Step::Node(receiver));
                    }
                    Self::Indexed { collection, index } => {
                        out.push(0x0b);
                        pending.push(Step::Node(index));
                        pending.push(Step::Node(collection));
                    }
                    Self::Range {
                        start,
                        end,
                        end_inclusive,
                    } => {
                        out.push(0x0d);
                        out.push(u8::from(*end_inclusive));
                        pending.push(Step::Node(end));
                        pending.push(Step::Node(start));
                    }
                    Self::Call {
                        target,
                        receiver,
                        arguments,
                    } => {
                        out.push(7);
                        out.extend(target.as_bytes());
                        out.push(0);
                        pending.push(Step::Emit(0xfe));
                        for argument in arguments.iter().rev() {
                            pending.push(Step::Node(argument));
                        }
                        pending.push(Step::Node(receiver));
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
                },
                Step::Emit(byte) => out.push(byte),
                Step::EmitBytes(bytes) => out.extend(bytes),
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
