/*
The closed scalar body: what `read` resolves a machine's typed trees into and
what `lower` consumes. Every name here is already an identity (a slot or a
`MachineId`) and every type a `ScalarType`, so lowering decides nothing from a
typed tree and nothing from text. The variants are the subset: a construct the
reader cannot express here is refused there, never approximated.
*/

use semantic_vocabulary::{IntegerValue, MachineId, Proposition, ScalarType};

use crate::SourceOrigin;

/// A machine's published signature as lowering and its callers need it.
#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    /// Formal types in position order. Lowering mints formal `i` as the
    /// machine's value `i + 1`, so clause propositions name formals by those
    /// identities.
    pub parameters: Vec<ScalarType>,
    /// `None` is a Unit machine.
    pub result: Option<ScalarType>,
    /// Entry `requires` clauses over the formals, in Terminal Psi's canonical
    /// order (`MachineContract::requires` must strictly increase). A clause's
    /// position here is its position in every `CallRequirement`.
    pub requires: Vec<Proposition>,
    /// Normal-return guarantees over the formals, in authored order.
    pub ensures: Vec<Proposition>,
}

/// A machine body: slot types (formals first, then locals in binding order)
/// and its statements in authored order.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    pub slots: Vec<ScalarType>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Evaluate `value` and bind `slot` to it: a local's initializer or an
    /// assignment, which rebinds an existing slot.
    Bind { slot: usize, value: Expression },
    /// Evaluate for effect and discard the result.
    Evaluate(Expression),
    /// One transition arm. With a guard, the arm runs when it is true and the
    /// following statements run when it is false; without one, the arm runs
    /// and nothing follows it.
    Arm {
        guard: Option<Expression>,
        target: Target,
        source: SourceOrigin,
    },
}

/// What an arm completes the machine with.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A value, or a terminal call whose result completes the machine.
    Value(Expression),
    /// Unit completion (`{}` arm).
    Unit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    pub kind: ExpressionKind,
    /// `None` only for a call of a Unit machine.
    pub result: Option<ScalarType>,
    pub source: SourceOrigin,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionKind {
    Integer(IntegerValue),
    Boolean(bool),
    Slot(usize),
    /// Both operands, left then right, then the operation.
    Strict {
        operation: Strict,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Not(Box<Expression>),
    BitwiseNot(Box<Expression>),
    /// `&&` when `conjunction`, else `||`: the right operand runs only when
    /// the left does not decide the result.
    ShortCircuit {
        conjunction: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    /// A direct call: all arguments left to right, then the invocation.
    Call {
        callee: MachineId,
        arguments: Vec<Expression>,
    },
}

/// Builtin strict binary operations on `Exact` integers and `bool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strict {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}
