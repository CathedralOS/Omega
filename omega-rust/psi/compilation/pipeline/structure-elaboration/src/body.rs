/*
The closed body: what `read` resolves a machine's typed trees into and what
`lower` consumes. Every name here is already an identity (a slot, a
`MachineId`, a record type or a field position in it) and every type a
`ValueType`, so lowering decides nothing from a typed tree and nothing from
text. The variants are the subset: a construct the reader cannot express here
is refused there, never approximated.

A value is a scalar or a record of a known type. A record value lives in a
structural place, so a record-typed expression names storage: a slot is the
root its local or formal holds, a member path a projection of it, and a
literal or call the fresh root its operation produces.
*/

use semantic_vocabulary::{
    CrashCause, IntegerValue, MachineId, Proposition, ScalarType, StructuralFieldId,
    StructuralTypeId,
};
use terminal_psi::CrashRouteBucket;

use crate::SourceOrigin;

/// The type of a formal, local, result or expression: a builtin scalar, or a
/// record type by its dense identity in the module's record table.
/// `wiki/spec/terminal-psi/calls_and_outcomes.md`: "Scalar values and
/// structural places retain separate namespaces".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Scalar(ScalarType),
    Record(StructuralTypeId),
}

impl From<ScalarType> for ValueType {
    fn from(scalar_type: ScalarType) -> Self {
        Self::Scalar(scalar_type)
    }
}

impl ValueType {
    pub const fn scalar(self) -> Option<ScalarType> {
        match self {
            Self::Scalar(scalar_type) => Some(scalar_type),
            Self::Record(_) => None,
        }
    }
}

/// One copy-eligible plain record type: its fields in declaration order.
/// A field is named by its position here and by its `StructuralFieldId`,
/// both fixed once by the reader from the field's symbol.
///
/// The two `identity` texts exist only because Terminal Psi's own type and
/// field declarations carry an interned text identity
/// (`StructuralTypeDeclaration::identity`). Lowering copies them into the
/// module's identity table and nothing in this crate or the checker compares
/// them (`check_on_lowered_form.md`, "Identity of a type on the lowered
/// form").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordType {
    pub identity: String,
    pub fields: Vec<RecordField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordField {
    /// Unique in the module, so a projection is one `(parent, field)` pair.
    pub id: StructuralFieldId,
    pub identity: String,
    pub value_type: ValueType,
}

/// A machine's published signature as lowering and its callers need it.
#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    /// Formal types in position order. Lowering mints the scalar formals
    /// first: the `k`-th scalar formal, counting from zero and skipping
    /// record formals, is the machine's value `k + 1`, so clause propositions
    /// name scalar formals by those identities. A record formal is a root
    /// place and has no value identity.
    pub parameters: Vec<ValueType>,
    /// `None` is a Unit machine. A scalar result's pseudo-value is the value
    /// right after the scalar formals, which is how `ensures` clauses name
    /// `result`. A record result is the machine's result place.
    pub result: Option<ValueType>,
    /// Entry `requires` clauses over the formals, in Terminal Psi's canonical
    /// order (`MachineContract::requires` must strictly increase). A clause's
    /// position here is its position in every `CallRequirement`.
    pub requires: Vec<Proposition>,
    /// Normal-return guarantees over the formals and the result pseudo-value,
    /// in authored order.
    pub ensures: Vec<Proposition>,
    pub crashes: CrashCeiling,
}

/// What the machine's header says about crashing.
/// `wiki/spec/language/effects.md`: "On a public checked body omitted
/// `crashes` means no crash route, exactly as `crashes none`. Private checked
/// bodies may omit any of the three to request inference."
#[derive(Debug, Clone, PartialEq)]
pub enum CrashCeiling {
    /// The authored routes, canonical (one bucket per cause, causes and
    /// alternatives strictly increasing, `Truth` alone when present), over the
    /// formals. Empty is "no crash route".
    Authored(Vec<CrashRouteBucket>),
    /// A private body with no `crashes` clause: its published routes are
    /// derived from its body (`outcome`).
    Inferred,
    /// The header wrote `crashes none`, read from the typed trees'
    /// `Machine::declares_no_crash_route`, not from the absence of a clause.
    /// effects.md: "An explicit empty value, `reaches none`, `crashes none`,
    /// `suspends false` or `blocks false`, promises the empty value and
    /// rejects any body or selection that violates it." It publishes no
    /// route, and a body that structure can reach a crash from is refused
    /// (`outcome`), never given an inferred route.
    DeclaredNone,
}

/// A machine body: slot types (formals first, then locals in binding order)
/// and its statements in authored order.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    pub slots: Vec<ValueType>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Evaluate `value` and bind `slot` to it: a local's initializer or an
    /// assignment, which rebinds an existing scalar slot. A record slot is
    /// bound once, by its local's initializer, to the root that holds it.
    Bind { slot: usize, value: Expression },
    /// Evaluate for effect and discard the result.
    Evaluate(Expression),
    /// One transition arm. With a guard, the arm runs when it is true and the
    /// following statements run when it is false; without one, the arm runs
    /// and nothing follows it. A `crash` statement is an unguarded arm whose
    /// target is `Target::Crash`.
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
    /// A crash with its cause: a no-successor exit, not a completion.
    /// `wiki/spec/terminal-psi/calls_and_outcomes.md`: "`Trap` and `Abort`
    /// are closed causes on distinct no-successor terminators."
    Crash(CrashCause),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    pub kind: ExpressionKind,
    /// `None` only for a call of a Unit machine.
    pub result: Option<ValueType>,
    pub source: SourceOrigin,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionKind {
    Integer(IntegerValue),
    Boolean(bool),
    /// A formal or local. A scalar slot is the value it holds; a record slot
    /// is the root place it holds.
    Slot(usize),
    /// `R { a: x, b: y }`: each operand in authored order, then one atomic
    /// establishment of the record `result` names.
    /// `wiki/spec/terminal-psi/structural_access.md`: "Each operand is
    /// evaluated in authored order before this operation. Establishment is
    /// atomic after all operands are available". Each entry is the field's
    /// position in the record's declaration and its operand; every field
    /// appears exactly once.
    Record {
        fields: Vec<(usize, Expression)>,
    },
    /// `base.field`: the field at this position of the record `base` names.
    /// `wiki/spec/language/expressions.md`: "Dot access remains data-field
    /// projection; it does not select or invoke a machine."
    Field {
        base: Box<Expression>,
        field: usize,
    },
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
