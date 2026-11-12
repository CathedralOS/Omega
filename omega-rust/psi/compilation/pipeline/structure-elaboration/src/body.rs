/*
The closed body: what `read` resolves a machine's typed trees into and what
`lower` consumes. Every name here is already an identity (a slot, a
`MachineId`, a record type or a field position in it) and every type a
`ValueType`, so lowering decides nothing from a typed tree and nothing from
text. The variants are the subset: a construct the reader cannot express here
is refused there, never approximated.

A value is a scalar or a value of a known data type: a record, or a sum with
cases. A data value lives in a structural place, so a data-typed expression
names storage: a slot is the root its local or formal holds, a member path a
projection of it, and a literal or call the fresh root its operation
produces. `ValueType::Record` and `RecordType` name both shapes: a record has
fields and no case, a sum has cases and no field.
*/

use semantic_vocabulary::{
    CrashCause, IntegerValue, MachineId, PlaceId, Proposition, ScalarType, StructuralCaseId,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_check_tables::values::RigidType;
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
    /// A value of a type parameter that a generic check form leaves rigid.
    /// It is neither a scalar nor a record: no operator, field or literal
    /// applies to it, so every typing rule that asks for one refuses it. It
    /// can be named, passed to a call and returned.
    Rigid(RigidType),
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
            Self::Record(_) | Self::Rigid(_) => None,
        }
    }

    /// How a value of this type is declared on the lowered form, when it is
    /// a value there and not a place: its declared type, and the rigid type
    /// that declaration stands in for.
    pub const fn declared(self) -> Option<(ScalarType, Option<RigidType>)> {
        match self {
            Self::Scalar(scalar_type) => Some((scalar_type, None)),
            Self::Rigid(rigid) => Some((RigidType::DECLARED_AS, Some(rigid))),
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
///
/// A sum is the same row with `cases` instead of `fields`
/// (`wiki/spec/language/data_and_literals.md`: a sum holds "cases, one of
/// which is active with its payload"). A type with both, common fields
/// beside cases, is refused by the reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordType {
    pub identity: String,
    pub fields: Vec<RecordField>,
    /// The declared cases in declaration order; empty for a record. A case
    /// is named by its position here and by its `StructuralCaseId`, fixed
    /// once by the reader from the case's symbol.
    pub cases: Vec<CaseType>,
    /// The clauses of the declaration's `requires`, in authored order: the
    /// type's default domain (`wiki/spec/language/dependent_values.md`: "A
    /// data declaration's field qualifications and `requires` clauses define
    /// its default domain"). Each is a clause over `self`, in the form
    /// Terminal Psi gives a clause over storage: a field of `self` is a
    /// field term whose root is `self_place()` and whose path is the dense
    /// field and case identities from the value down to the field, and
    /// `self in S::C` is a case-membership proposition on such a path.
    /// Nothing proves or knows a clause in this form: the checker replaces
    /// every field term by the value a place holds, and every case test by
    /// whether the place is that case, before it states a goal
    /// (`lowered-discharge`).
    pub requires: Vec<Proposition>,
}

/// The root that stands for `self` in a data type's `requires` clauses
/// (`RecordType::requires`). A clause belongs to a type, not a machine, so
/// this names no storage: it is the one root such a clause can name, and it
/// is replaced before the clause is used.
pub fn self_place() -> Option<PlaceId> {
    PlaceId::new(1)
}

/// One case of a sum: its payload fields in declaration order. A payload
/// field is the place `Segment::Case(id)` then `Segment::Field(field)`
/// beneath the sum (`wiki/spec/terminal-psi/ownership.md`: "A Case segment
/// guards its payload namespace; it adds neither physical storage nor an
/// independent claim.").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseType {
    /// Unique in the module.
    pub id: StructuralCaseId,
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
    /// The positions in `parameters` of the `[erased]` formals, strictly
    /// increasing. `wiki/spec/proofs/contracts.md`, "Explicit erased
    /// bindings": "The binding remains in checking and semantic identity but
    /// contributes no runtime field, address, read, or cleanup." An erased
    /// formal is scalar here, travels in neither runtime lane, and is a
    /// value only so that clauses can name it: `formal_values` gives it the
    /// identity after the result pseudo-value. A call supplies a term for
    /// it and evaluates nothing.
    pub erased: Vec<usize>,
    /// `None` is a Unit machine. A scalar result's pseudo-value is the value
    /// right after the scalar formals, which is how `ensures` clauses name
    /// `result`. A record result is the machine's result place.
    pub result: Option<ValueType>,
    /// Entry `requires` clauses over the formals, in Terminal Psi's canonical
    /// order (`MachineContract::requires` must strictly increase). A clause's
    /// position here is its position in every `CallRequirement`. A term
    /// that projects a record formal (`p.first`) is a field term whose root
    /// is that formal's place: the `k`-th record formal, counting from zero
    /// and skipping scalar formals, is the machine's place `k + 1`. It
    /// speaks of what the place holds at entry.
    pub requires: Vec<Proposition>,
    /// Normal-return guarantees over the formals and the result pseudo-value,
    /// in authored order.
    pub ensures: Vec<Proposition>,
    pub crashes: CrashCeiling,
    pub termination: Termination,
}

/// What the machine's header says about ending.
/// `wiki/spec/language/termination.md`: "Bare `terminates` authors the
/// promise", and `terminates by subject` supplies "a witness [that]
/// discharges that promise rather than redefining it".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// No clause. termination.md: "A private acyclic body needs no
    /// annotation and derives a local `Terminates` summary", and "A
    /// productive terminal-transfer cycle without a termination promise may
    /// run forever and owes no rank".
    Silent,
    /// The header promises termination and names no ranking.
    Promised,
    /// The header supplies a ranking witness, with or without the promise.
    Witnessed,
}

impl Signature {
    pub fn is_erased(&self, position: usize) -> bool {
        self.erased.binary_search(&position).is_ok()
    }

    /// The value identity of each formal, by position; `None` for a record
    /// formal, which is a place. See `formal_values`.
    pub fn formal_values(&self) -> Vec<Option<ValueId>> {
        formal_values(
            self.parameters
                .iter()
                .enumerate()
                .map(|(position, value_type)| (*value_type, self.is_erased(position))),
            self.result,
        )
    }
}

/// The one numbering of a machine's formal values, shared by the reader
/// (which states clauses over them) and lowering (which mints them). The
/// `k`-th relevant value formal (scalar or of a rigid type) is value
/// `k + 1`; a value result's pseudo-value follows them; the erased scalar formals follow that, in
/// position order. Terminal Psi keeps the same split: runtime formals in
/// `TerminalMachine::parameters`, erased ones in
/// `MachineContract::erased_scalar_formals`.
pub fn formal_values(
    formals: impl Iterator<Item = (ValueType, bool)> + Clone,
    result: Option<ValueType>,
) -> Vec<Option<ValueId>> {
    let relevant = formals
        .clone()
        .filter(|(value_type, erased)| value_type.declared().is_some() && !erased)
        .count() as u64;
    let result = u64::from(result.and_then(ValueType::declared).is_some());
    let (mut next_relevant, mut next_erased) = (0, relevant + result);
    formals
        .map(|(value_type, erased)| {
            value_type.declared()?;
            let next = if erased {
                &mut next_erased
            } else {
                &mut next_relevant
            };
            *next += 1;
            ValueId::new(*next)
        })
        .collect()
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
    /// Evaluate `value` and bind `slot` to it: a local's initializer, or an
    /// assignment of a whole value, which rebinds the slot. A scalar slot
    /// names the value from then on; a record slot names the fresh root that
    /// holds the assigned value, which is how the current pipeline lowers
    /// `r = other;` on a record local. Every arm completes the machine, so
    /// the statements of a body lie on one spine and a slot has one binding
    /// at each of them.
    Bind { slot: usize, value: Expression },
    /// `let x [erased]: T = term;`: the slot names `term` and nothing is
    /// evaluated. contracts.md, "Explicit erased bindings": the binding "may
    /// supply proof computation or static authorization; it cannot determine
    /// runtime data or control." `term` is a literal or a slot, the forms
    /// with no formation condition of their own (contracts.md, "Total
    /// arithmetic": "Exact operations discharge representability before
    /// formation"); the reader refuses any other initializer.
    Witness { slot: usize, term: Expression },
    /// `let x: T;`: a local declared without an initializer.
    ///
    /// A data local is zero-established: its storage holds the zero
    /// representation, a record's scalar fields zero and a sum its first
    /// declared case with a zeroed payload
    /// (`wiki/spec/language/data_and_literals.md`: "The ordinary zero
    /// representation of a sum selects the first declared case and a
    /// recursively zeroed payload."). It is an established value when the
    /// zero satisfies the type's `requires`; otherwise the type is gated
    /// and the storage may not be observed before a store establishes it
    /// (`wiki/spec/language/dependent_values.md`: "If zero satisfies the
    /// default domain, the value is zero-constructible. Otherwise the type
    /// is gated: construction or checked qualification must establish the
    /// domain before observation.").
    ///
    /// A scalar local is zero-constructed by the same rule
    /// (`local-without-initializer`): its storage holds zero, which is an
    /// established value because every scalar type served here (`i8`..`u64`,
    /// `bool`) admits zero. A constrained scalar type is refused by the
    /// reader, so no gated scalar reaches lowering. A scalar slot declared
    /// this way is a storage home, not an SSA value: it is written by
    /// `Store` with an empty path and read through its place.
    ///
    /// `home` is, for a scalar slot, the text identity of its type as the
    /// typed trees render it. Terminal Psi declares the primitive type of
    /// such a home with an interned text identity, as it does a record's
    /// (`RecordType::identity`); lowering copies it there and nothing
    /// compares it. A record slot has `None`: its `RecordType` holds its own.
    Declare {
        slot: usize,
        home: Option<String>,
        source: SourceOrigin,
    },
    /// `slot.path = value;`: evaluate `value`, then store it into exactly the
    /// place `path` selects beneath the slot's root, each step a field
    /// position in the record type the previous step reached. The path is
    /// empty only for a scalar slot that `Declare` homed.
    /// `wiki/spec/language/state_contracts.md`: "Assignments first capture
    /// their destination place ... They then evaluate the right-hand side".
    /// A destination made of a slot and fields has nothing to evaluate.
    Store {
        slot: usize,
        path: Vec<usize>,
        value: Expression,
        source: SourceOrigin,
    },
    /// Evaluate for effect and discard the result.
    Evaluate(Expression),
    /// `transition subject { S::A { .. } -> x  S::B -> y  _ -> z }` over a
    /// sum: the subject is observed once and exactly the selected arm runs
    /// (`wiki/spec/language/patterns.md`: "A dispatch evaluates its subject
    /// once, tests alternatives in authored order, and executes only the
    /// selected arm."). Every arm completes the machine, so nothing follows
    /// the statement. `subject` names storage and is not copied. Each arm is
    /// a case position of the subject's sum, at most once; `default` takes
    /// every case no arm names. A case with neither is a compile error that
    /// lowering reports where it builds the terminator.
    Dispatch {
        subject: Expression,
        arms: Vec<(usize, Target, SourceOrigin)>,
        default: Option<(Target, SourceOrigin)>,
        source: SourceOrigin,
    },
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
    /// A formal or local. A scalar slot is the value it holds, or for a
    /// local declared without an initializer a fresh read of its home; a
    /// record slot is the root place it holds.
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
    /// `S::C { p: x }`, `S::C {}` and `S::C`: each payload operand in
    /// authored order, then one atomic establishment of the sum `result`
    /// names as the case at this position. Each entry is the payload field's
    /// position in the case and its operand; every payload field appears
    /// exactly once. `wiki/spec/language/data_and_literals.md`: "Case values
    /// always use their carrier-qualified member path."
    Case {
        case: usize,
        fields: Vec<(usize, Expression)>,
    },
    /// The payload field at position `field` of case `case` of the sum
    /// `base` names. It is storage that exists only while that case is
    /// selected; whether it is, at this use, is the checker's payload
    /// judgment (`lowered-discharge`, `places`), not something stated here.
    Payload {
        base: Box<Expression>,
        case: usize,
        field: usize,
    },
    /// `subject in S::C`: whether the sum `subject` names currently holds
    /// the case at this position. A Boolean.
    IsCase {
        subject: Box<Expression>,
        case: usize,
    },
    /// Value-position `match` over a sum whose arms are scalar: the subject
    /// is observed once, the selected arm's value is the result
    /// (`wiki/spec/language/expressions.md`: "Value-position `match` |
    /// Subject once; first matching arm only."). `subject` names storage, a
    /// slot or a path beneath one, so observing it evaluates nothing. Arms
    /// and `default` are as in `Statement::Dispatch`.
    Match {
        subject: Box<Expression>,
        arms: Vec<(usize, Expression)>,
        default: Option<Box<Expression>>,
    },
    /// Both operands, left then right, then the operation.
    Strict {
        operation: Strict,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Not(Box<Expression>),
    BitwiseNot(Box<Expression>),
    /// `operand as T` between two integer carriers, neither carrying a
    /// domain: the operand, then the one exact conversion to `result`.
    /// `wiki/spec/language/domains.md`, "Carrier casts": "`value as T`
    /// changes the carrier through a unique exact compiler-derived
    /// transformation: exact widening, or narrowing with representability
    /// proved. The denoted value does not change."
    Cast(Box<Expression>),
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
