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
    BoundaryMachineId, CrashCause, IdTable, IntegerValue, MachineId, PlaceId, Proposition,
    ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId, TargetMachineCallable,
    ValueId,
};
use terminal_check_tables::values::RigidType;
use terminal_psi::CrashRouteBucket;

use crate::SourceOrigin;

/// The type of a formal, local, result or expression: a carrier (a builtin
/// scalar, a record type by its dense identity in the module's record table,
/// or a rigid type parameter) and the domains the type carries on it.
/// `wiki/spec/terminal-psi/calls_and_outcomes.md`: "Scalar values and
/// structural places retain separate namespaces".
///
/// `wiki/spec/language/domains.md`: "Qualification never selects or changes
/// storage representation", so everything that stores, moves or lowers a
/// value reads the carrier alone. The qualification is part of the type all
/// the same: two types with the same carrier and different domains are
/// different types, and every typing rule that compares two types (a `let`,
/// an argument, a field initializer, a return, the arms of a join) tells
/// them apart ("the supplied expression's static type must carry exactly the
/// domains the destination declares").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Scalar(ScalarType, Qualification),
    Record(StructuralTypeId, Qualification),
    /// A value of a type parameter that a generic check form leaves rigid.
    /// It is neither a scalar nor a record: no operator, field or literal
    /// applies to it, so every typing rule that asks for one refuses it. It
    /// can be named, passed to a call and returned.
    Rigid(RigidType),
}

/// The domains a type carries, one mechanism for every kind of domain: the
/// arithmetic policy of an integer (`u32 in Wrapping`) and the declared
/// domains (`i32 in Km`, `Seal in Issued`) are members of the same set, and
/// the same casts add and remove them.
///
/// The policy has its own field because a type carries at most one
/// (`wiki/spec/language/numeric_values.md`, "Overflow policies": "A type may
/// carry any number of value qualifications but at most one arithmetic
/// policy") and it alone chooses what a builtin operator means. A policy is
/// only ever carried by a `ScalarType::Integer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Qualification {
    pub policy: Option<Policy>,
    pub domains: DomainSet,
}

impl Qualification {
    /// The bare carrier: no policy and no declared domain.
    pub const NONE: Self = Self {
        policy: None,
        domains: DomainSet::EMPTY,
    };

    pub const fn of_policy(policy: Policy) -> Self {
        Self {
            policy: Some(policy),
            domains: DomainSet::EMPTY,
        }
    }

    pub fn is_none(self) -> bool {
        self == Self::NONE
    }
}

/// The declared domains of one type, each by its declaration's symbol, kept
/// sorted so that equal sets are equal values. The set is held inline, as
/// the two numbers of each symbol handle, which keeps a type `Copy` and lets
/// `Qualification::NONE` be matched as a pattern; a type naming more than
/// `DomainSet::CAPACITY` declared domains is refused by the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DomainSet {
    len: u8,
    members: [(u32, u32); Self::CAPACITY],
}

impl DomainSet {
    pub const CAPACITY: usize = 4;
    pub const EMPTY: Self = Self {
        len: 0,
        members: [(0, 0); Self::CAPACITY],
    };

    const fn key(domain: symbols::SymbolHandle) -> (u32, u32) {
        (domain.arena_index(), domain.generation())
    }

    pub fn members(&self) -> impl Iterator<Item = symbols::SymbolHandle> + '_ {
        self.members[..usize::from(self.len)]
            .iter()
            .map(|(index, generation)| symbols::SymbolHandle::from_parts(*index, *generation))
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn contains(&self, domain: symbols::SymbolHandle) -> bool {
        self.members[..usize::from(self.len)].contains(&Self::key(domain))
    }

    /// Whether every member of this set is a member of `other`.
    pub fn is_subset_of(&self, other: &Self) -> bool {
        self.members().all(|domain| other.contains(domain))
    }

    /// This set with `domain` added: `None` when it is already a member or
    /// the set is full.
    pub fn with(mut self, domain: symbols::SymbolHandle) -> Option<Self> {
        let count = usize::from(self.len);
        if self.contains(domain) || count == Self::CAPACITY {
            return None;
        }
        let key = Self::key(domain);
        let position = self.members[..count].partition_point(|member| *member < key);
        self.members.copy_within(position..count, position + 1);
        self.members[position] = key;
        self.len += 1;
        Some(self)
    }
}

/// An arithmetic policy a type can carry. `Exact` is no policy; `Trapping`
/// is refused by the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Policy {
    /// Overflow reduces modulo the carrier.
    Wrapping,
    /// Overflow clamps to the carrier's bounds.
    Saturating,
}

impl From<ScalarType> for ValueType {
    fn from(scalar_type: ScalarType) -> Self {
        Self::plain(scalar_type)
    }
}

impl ValueType {
    /// The bare scalar carrier, with no domain.
    pub const fn plain(scalar_type: ScalarType) -> Self {
        Self::Scalar(scalar_type, Qualification::NONE)
    }

    /// The bare record carrier, with no domain.
    pub const fn record(record: StructuralTypeId) -> Self {
        Self::Record(record, Qualification::NONE)
    }

    /// The scalar a value of this type is stored as, whatever it carries.
    pub const fn scalar(self) -> Option<ScalarType> {
        match self {
            Self::Scalar(scalar_type, _) => Some(scalar_type),
            Self::Record(..) | Self::Rigid(_) => None,
        }
    }

    /// The domains this type carries; a rigid type parameter carries none.
    pub const fn qualification(self) -> Qualification {
        match self {
            Self::Scalar(_, qualification) | Self::Record(_, qualification) => qualification,
            Self::Rigid(_) => Qualification::NONE,
        }
    }

    /// This type's carrier with `qualification` in place of its own; a rigid
    /// type parameter has no qualified form.
    pub const fn qualified(self, qualification: Qualification) -> Option<Self> {
        match self {
            Self::Scalar(scalar_type, _) => Some(Self::Scalar(scalar_type, qualification)),
            Self::Record(record, _) => Some(Self::Record(record, qualification)),
            Self::Rigid(_) => None,
        }
    }

    /// The arithmetic policy this type carries, if it carries one.
    pub const fn policy(self) -> Option<Policy> {
        self.qualification().policy
    }

    /// How a value of this type is declared on the lowered form, when it is
    /// a value there and not a place: its declared type, and the rigid type
    /// that declaration stands in for.
    pub const fn declared(self) -> Option<(ScalarType, Option<RigidType>)> {
        match self {
            Self::Scalar(scalar_type, _) => Some((scalar_type, None)),
            Self::Rigid(rigid) => Some((RigidType::DECLARED_AS, Some(rigid))),
            Self::Record(..) => None,
        }
    }
}

/// One plain record type: its fields in declaration order and its authored
/// copy permission. Plain owned records remain affine even with scalar fields.
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
/// The shape is explicit so an array cannot become a record with numbered
/// fields. The row lowers into the corresponding existing Terminal storage
/// shape; its identity remains the type the reader actually resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordType {
    /// Actual source linear custody, including owned nested claims.
    pub owns_linear_claim: bool,
    pub copyable: bool,
    pub identity: String,
    pub shape: StorageShape,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageShape {
    Record(Vec<RecordField>),
    Sum(Vec<CaseType>),
    ByteSequence(terminal_psi::ByteSequenceCarrier),
    FixedArray {
        element: StructuralTypeId,
        length: terminal_psi::ArrayExtent,
    },
    PrimitiveScalar {
        scalar_type: ScalarType,
        policy: Option<Policy>,
    },
}

impl RecordType {
    pub fn fields(&self) -> &[RecordField] {
        match &self.shape {
            StorageShape::Record(fields) => fields,
            _ => &[],
        }
    }

    pub fn cases(&self) -> &[CaseType] {
        match &self.shape {
            StorageShape::Sum(cases) => cases,
            _ => &[],
        }
    }
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
    /// The formals passed by reference, `&T` or `&mut T`, in strictly
    /// increasing position order. `parameters` holds each one's referent
    /// type. A borrowed formal is a place, never a value, whatever its
    /// referent: the root of a structural parameter whose declared access
    /// is the reference's, holding the caller's storage for the activation
    /// (`wiki/spec/language/ownership.md`, "Borrows and aliases": "Borrowed
    /// arguments continue to denote the original storage, not a caller-side
    /// snapshot chosen because the referent is small.").
    pub borrowed: Vec<Borrowed>,
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
    /// What the header says about service reach.
    pub reaches: Ceiling,
    /// What the header says about synchronous entry (`invokes`).
    pub invokes: Ceiling,
    pub termination: Termination,
}

/// Borrow the one signature representation across the two actual declaration
/// namespaces. A boundary signature remains a contract; looking it up neither
/// creates a machine body nor selects a provider.
#[derive(Clone, Copy)]
pub(crate) struct Signatures<'a> {
    pub machines: &'a IdTable<MachineId, Signature>,
    pub boundaries: &'a IdTable<BoundaryMachineId, Signature>,
    /// How an invocation of each boundary requirement ends when it does not
    /// crash. The canonical process-exit requirement "produces no normal
    /// result or continuation" (`wiki/spec/language/process_exit.md`);
    /// every other requirement returns. It is the leaf fact of
    /// `Outcome::completes_normally`: whether a machine with a body returns
    /// is derived from the calls of that body, so a machine has no row.
    pub completions: &'a IdTable<BoundaryMachineId, semantic_vocabulary::BoundaryCompletion>,
}

impl<'a> Signatures<'a> {
    /// The declared completion of a boundary requirement.
    pub fn completion(
        self,
        boundary: BoundaryMachineId,
        caller: MachineId,
    ) -> terminal_check_tables::Checked<semantic_vocabulary::BoundaryCompletion> {
        terminal_check_tables::row(
            self.completions,
            boundary,
            caller,
            terminal_check_tables::Construct::Machine,
        )
        .copied()
    }

    pub fn get(
        self,
        callable: TargetMachineCallable,
        caller: MachineId,
    ) -> terminal_check_tables::Checked<&'a Signature> {
        match callable {
            TargetMachineCallable::Machine(machine) => terminal_check_tables::row(
                self.machines,
                machine,
                caller,
                terminal_check_tables::Construct::Machine,
            ),
            TargetMachineCallable::Boundary(boundary) => terminal_check_tables::row(
                self.boundaries,
                boundary,
                caller,
                terminal_check_tables::Construct::Machine,
            ),
        }
    }
}

/// One formal passed by reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Borrowed {
    /// The formal's position in `Signature::parameters`.
    pub position: usize,
    /// `Shared` for `&T`, `Mutable` for `&mut T`. `&write T` is refused by
    /// the reader.
    pub access: language_core::ReferenceAccess,
    /// For a scalar referent, the text identity of its type as the typed
    /// trees render it: Terminal Psi declares the primitive type of the
    /// formal's place with an interned text identity, as it does a scalar
    /// home's (`Statement::Declare`). `None` for a record referent, whose
    /// `RecordType` holds its own. Nothing compares it.
    pub home: Option<String>,
}

/// What the machine's header says about ending.
/// `wiki/spec/language/termination.md`: "Bare `terminates` authors the
/// promise", and `terminates by subject` supplies "a witness [that]
/// discharges that promise rather than redefining it".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// An interface cannot infer a ceiling from a body it does not own.
    /// The selected declaration must supply the published routes explicitly.
    pub fn declared_crash_routes(&self) -> Option<&[CrashRouteBucket]> {
        match &self.crashes {
            CrashCeiling::DeclaredNone => Some(&[]),
            CrashCeiling::Authored(routes) => Some(routes),
            CrashCeiling::Inferred => None,
        }
    }

    pub fn is_erased(&self, position: usize) -> bool {
        self.erased.binary_search(&position).is_ok()
    }

    /// The reference the formal at `position` is passed by, if it is one.
    pub fn borrowed(&self, position: usize) -> Option<&Borrowed> {
        self.borrowed
            .binary_search_by_key(&position, |borrowed| borrowed.position)
            .ok()
            .and_then(|found| self.borrowed.get(found))
    }

    /// Whether the formal at `position` is a structural place: a record
    /// formal, or any borrowed formal. The `k`-th of them, counting from
    /// zero in position order, is the machine's place `k + 1`, which is how
    /// a clause names it.
    pub fn is_place(&self, position: usize) -> bool {
        self.borrowed(position).is_some()
            || matches!(self.parameters.get(position), Some(ValueType::Record(..)))
    }

    /// The value identity of each formal, by position; `None` for a record
    /// formal, which is a place. See `formal_values`.
    pub fn formal_values(&self) -> Vec<Option<ValueId>> {
        formal_values(
            self.parameters
                .iter()
                .enumerate()
                .map(|(position, value_type)| {
                    (
                        *value_type,
                        self.is_erased(position),
                        self.borrowed(position).is_some(),
                    )
                }),
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
/// `MachineContract::erased_scalar_formals`. A borrowed formal (the third
/// flag) is a place whatever its referent, so it has no value identity.
pub fn formal_values(
    formals: impl Iterator<Item = (ValueType, bool, bool)> + Clone,
    result: Option<ValueType>,
) -> Vec<Option<ValueId>> {
    let relevant = formals
        .clone()
        .filter(|(value_type, erased, borrowed)| {
            value_type.declared().is_some() && !erased && !borrowed
        })
        .count() as u64;
    let result = u64::from(result.and_then(ValueType::declared).is_some());
    let (mut next_relevant, mut next_erased) = (0, relevant + result);
    formals
        .map(|(value_type, erased, borrowed)| {
            if borrowed {
                return None;
            }
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

/// What a header says on one ceiling axis: service reach (`reaches`) or
/// synchronous entry (`invokes`). Both are a set of service identities that
/// bounds what the machine's calls contribute, published to callers, so one
/// type, one derivation (`outcome`) and one judgment serve both.
/// `wiki/spec/language/effects.md`: "A present clause is an upper bound, not
/// an assertion of exact equality and not an additive contribution", and
/// "Omission requests inference and is distinct from an explicit empty
/// clause."
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ceiling {
    /// No clause on a declaration whose row is derived: a checked body
    /// publishes what its calls contribute (`outcome`). A boundary
    /// requirement must not omit `reaches`; a requirement that omits
    /// `invokes` is not this case (see `Axis::Entries`).
    Inferred,
    /// The written row, strictly increasing. For reach, the services the
    /// clause names "and their boundary parents"; empty is `reaches none`.
    /// For entries, the requirement identities the clause names, with no
    /// parent closure.
    Authored(Vec<semantic_vocabulary::ServiceId>),
}

/// The two ceiling axes a header may bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// `reaches`: the boundary services a machine's calls may reach.
    Reach,
    /// `invokes`: the callbacks and inbound entries control may arrive at
    /// before the machine returns. effects.md, "Direct synchronous
    /// invocation": a bodyless requirement that omits the clause has "No
    /// synchronous entry", so its row is `Authored` and empty, never open.
    Entries,
}

impl Axis {
    pub const ALL: [Axis; 2] = [Axis::Reach, Axis::Entries];
}

impl Signature {
    pub fn ceiling(&self, axis: Axis) -> &Ceiling {
        match axis {
            Axis::Reach => &self.reaches,
            Axis::Entries => &self.invokes,
        }
    }
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
    /// Capture the existing destination expression as a place, then evaluate
    /// `value` and store it there. Field and index expressions retain the
    /// same meaning as in a read or loan; no second destination path exists.
    /// `wiki/spec/language/state_contracts.md`: "Assignments first capture
    /// their destination place ... They then evaluate the right-hand side".
    /// A destination made of a slot and fields has nothing to evaluate.
    Store {
        destination: Expression,
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
    /// The enclosing match's already captured place, never its selector again.
    SavedSubject(arena::Handle<typed_trees::expression::CompletedValueCapture>),
    Integer(IntegerValue),
    Boolean(bool),
    /// Actual immutable octets established at this expression's operation.
    ByteLiteral(Vec<u8>),
    Array {
        elements: Vec<Expression>,
    },
    Index {
        base: Box<Expression>,
        index: Box<Expression>,
    },
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
        capture: Option<arena::Handle<typed_trees::expression::CompletedValueCapture>>,
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
    /// A cast that adds or removes an arithmetic policy and keeps the carrier,
    /// `x as u32 in Wrapping` or `w as u32`: the operand, whose value is the
    /// result. Only `result` differs from the operand, and only in its
    /// policy, so no operation is emitted and the operand keeps the
    /// operations its own type chose.
    Requalify(Box<Expression>),
    /// `&&` when `conjunction`, else `||`: the right operand runs only when
    /// the left does not decide the result.
    ShortCircuit {
        conjunction: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    /// `&place` or `&mut place` as the argument for a borrowed formal, the
    /// one position the reader serves it in: the place a slot, or a member
    /// path beneath one, names, lent to the call for the call's duration.
    /// `result` is the referent's type. A borrowed formal named whole as the
    /// argument for another borrowed formal is the same expression, a
    /// reborrow of its referent. `wiki/spec/terminal-psi/loans.md`: "Loan
    /// formation captures a place, not a live selector expression."
    ///
    /// `home` is set when the place is a scalar local that is an SSA value
    /// here: lending it needs storage, so lowering gives it a home holding
    /// its current value first, and this is the text identity of that
    /// home's type, as `Statement::Declare` carries one.
    Borrow {
        access: language_core::ReferenceAccess,
        place: Box<Expression>,
        home: Option<String>,
    },
    /// A direct call: all arguments left to right, then the invocation.
    ///
    /// `designated` is set on a call of the toolchain's Build selection
    /// protocol (`Build::bind(builder, slot, entry)` as typing spells it):
    /// the declarations the call names beside its ordinary arguments.
    /// scoped_execution.md, "Selecting product declarations without
    /// executing them": "No product receiver, function body, initializer,
    /// or provider is executed." So it is no operand: the call computes
    /// with its arguments alone, and lowering records which emitted
    /// operation the reference stands beside (`MachineSources::designated`)
    /// so the Build host can receive it.
    Call {
        callee: TargetMachineCallable,
        arguments: Vec<Expression>,
        designated: Option<DesignatedOperands>,
    },
}

/// The designated operands of one Build selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignatedOperands {
    /// The request as the typed trees hold it
    /// (`ExpressionTable::build_selection_request`).
    pub request: arena::Handle<typed_trees::expression::BuildSelectionRequest>,
    /// The machine whose source wrote the selection. The declarations a
    /// selection names are resolved for the place that wrote it
    /// (scoped_execution.md: "using the occurrence's package authority,
    /// exact target, and complete expected slot/requirement signature"),
    /// and that is not always the machine whose lowered body holds the
    /// call: a call component lowers its members into one machine.
    pub selecting: MachineId,
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

#[cfg(test)]
mod qualification_tests {
    use super::{DomainSet, Policy, Qualification};
    use symbols::SymbolHandle;

    /// Equal sets are equal values whatever order their members were added
    /// in, a member is added once, and the policy is part of the same
    /// qualification as the declared domains.
    #[test]
    fn a_domain_set_is_one_value_per_set() {
        let [km, positive] = [7, 3].map(SymbolHandle::from_arena_index);
        let forward = DomainSet::EMPTY.with(km).unwrap().with(positive).unwrap();
        let backward = DomainSet::EMPTY.with(positive).unwrap().with(km).unwrap();
        assert_eq!(forward, backward);
        assert_eq!(forward.members().collect::<Vec<_>>(), [positive, km]);
        assert!(forward.with(km).is_none());
        assert!(DomainSet::EMPTY.with(km).unwrap().is_subset_of(&forward));
        assert!(!forward.is_subset_of(&DomainSet::EMPTY.with(km).unwrap()));

        let plain = Qualification::NONE;
        assert!(plain.is_none());
        let wrapping = Qualification::of_policy(Policy::Wrapping);
        let qualified = Qualification {
            domains: forward,
            ..wrapping
        };
        assert!(!wrapping.is_none() && wrapping != qualified);
        // Past the inline capacity the set refuses, never drops a member.
        let mut full = DomainSet::EMPTY;
        for index in 0..DomainSet::CAPACITY as u32 {
            full = full
                .with(SymbolHandle::from_arena_index(index + 20))
                .unwrap();
        }
        assert!(full.with(km).is_none());
    }
}
