//! Source-free scalar membership vocabulary and exact edge coercions.
//!
//! This closed catalog carries obligation-free tags and routed scalar theories.
//! A routed theory retains its predicate and permitted issuers independently;
//! exact normal-return rows retain checked establishment or forwarding, not
//! permission inferred from the result annotation. Numeric result contracts
//! publish those same predicates over the actual returned value.
//! Explicit edge coercions introduce only obligation-free tags, or erase strict
//! same-carrier membership. Removal visibly erases non-owning meaning; neither
//! direction changes payload bits, executes an operation, or proves a
//! predicate. The catalog also retains authored numeric entry ranges. An entry
//! range is not a tag: it is a closed membership
//! requirement on the values a call may deliver to one direct scalar
//! parameter.

use crate::terminal_module::TerminalIdentityIndex;
use semantic_vocabulary::{
    DomainSemanticId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType, IntegerValue,
    MachineId, ScalarDomainId, ScalarQualificationSetId, ScalarType, ValueId,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarQualificationCatalog {
    /// Exact scalar receiving memberships for declared record/case fields.
    /// Every initializer and replacement supplies these from its actual SSA
    /// value; a readable field observation captures them on a fresh snapshot.
    /// Numeric field restrictions remain independent and grant no membership.
    pub fields: Vec<ScalarFieldQualification>,
    /// Source-checked result membership bound to exact normal return edges.
    /// These are independent of both the domain's predicate and its routes.
    pub returns: Vec<ScalarReturnQualification>,
    pub domains: Vec<ScalarDomainDeclaration>,
    pub sets: Vec<ScalarQualificationSet>,
    pub coercions: Vec<ScalarQualificationCoercion>,
    /// Retained authored floating ranges, strictly ordered by
    /// `(machine, parameter)`. Each row constrains the exact IEEE values a
    /// call may deliver to that direct scalar parameter.
    pub float_entry_ranges: Vec<ScalarFloatRange>,
    /// Retained authored integer ranges, strictly ordered by
    /// `(machine, parameter)`. Each row constrains the exact values a call
    /// may deliver to that direct scalar parameter; the same inclusive
    /// bounds also publish as `requires` propositions on the owner contract,
    /// so an independently replayed call edge proves delivery.
    pub integer_entry_ranges: Vec<ScalarIntegerRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarFieldQualification {
    pub field: semantic_vocabulary::StructuralFieldId,
    pub value_type: semantic_vocabulary::QualifiedScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarDomainDeclaration {
    /// Complete predicate component, in a domain-local scalar namespace.
    /// It states an obligation, never an assumption establishing membership.
    pub predicate: Option<ScalarDomainPredicate>,
    pub id: ScalarDomainId,
    pub semantic_domain: DomainSemanticId,
    pub identity: TerminalIdentityIndex,
    pub carrier: ScalarType,
    /// The authored declaration's establishment routes, canonically ordered
    /// and deduplicated. Each row names one authorized issuer by the
    /// requirement or machine identity the module itself must carry; the
    /// verifier replays those referents rather than trusting the rows.
    /// Routes grant no consumer call authority on their own.
    pub establishment_routes: Vec<ScalarDomainEstablishmentRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarDomainPredicate {
    pub subject: ValueId,
    pub proposition: semantic_vocabulary::Proposition,
}

/// All possible checked origins of one completed result. Joining control flow
/// retains alternatives; it does not promote a forwarding wrapper to issuer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScalarReturnOrigin {
    Forwarded,
    Established {
        route: u32,
    },
    /// A fresh result-completion binding adds an obligation-free tag.
    /// This does not authorize a predicate or routed domain.
    Vacuous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarReturnQualification {
    pub site: ScalarReturnSite,
    pub machine: MachineId,
    pub edge: EdgeId,
    pub source: ValueId,
    pub result: ValueId,
    pub domain: ScalarDomainId,
    pub origins: Vec<ScalarReturnOrigin>,
}

/// The exported return and the earlier fresh completion binding are distinct
/// occurrences. A completion row names where checked membership first enters
/// a join, not permission for arbitrary ordinary successor coercions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarReturnSite {
    NormalExit,
    Completion { argument_ordinal: u32 },
}

/// Scalar and structural presentations retain one owner-bound issuer protocol.
/// Carrier presentation cannot weaken the identity of an authorized declaration.
pub use super::qualifications::StructuralEstablishmentRoute as ScalarDomainEstablishmentRoute;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarQualificationSet {
    pub id: ScalarQualificationSetId,
    pub domains: Vec<ScalarDomainId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarQualificationCoercion {
    pub machine: MachineId,
    pub edge: EdgeId,
    pub argument_ordinal: u32,
    pub source: ValueId,
    pub destination: ValueId,
}

/// One authored IEEE floating entry range retained against a machine's direct
/// scalar parameter. `minimum` and `maximum` hold the exact authored
/// interchange bits at the parameter's declared carrier — not decoded host
/// floats — so IEEE order and signed zero survive publication. The minimum
/// is inclusive; `maximum_inclusive` keeps the authored boundary, and an
/// exclusive endpoint is preserved verbatim: integer predecessor arithmetic
/// is not floating range normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarFloatRange {
    /// Machine whose direct scalar parameter carries the range.
    pub machine: MachineId,
    /// The machine's direct scalar parameter carrying the range.
    pub parameter: ValueId,
    /// Authored inclusive minimum as exact interchange bits.
    pub minimum: IeeeFloatValue,
    /// Authored maximum as exact interchange bits.
    pub maximum: IeeeFloatValue,
    /// Whether the authored maximum admits the endpoint itself.
    pub maximum_inclusive: bool,
}

impl ScalarFloatRange {
    /// IEEE comparison over retained interchange bits. Widening `f32` bits
    /// to `f64` is lossless and preserves order and NaN, so both formats
    /// share one comparison.
    fn decode(value: IeeeFloatValue) -> f64 {
        match value {
            IeeeFloatValue::Binary32(bits) => f32::from_bits(bits) as f64,
            IeeeFloatValue::Binary64(bits) => f64::from_bits(bits),
        }
    }

    /// The IEEE carrier format the endpoints retain.
    pub fn format(&self) -> IeeeFloatFormat {
        self.minimum.format()
    }

    /// Whether the authored endpoints share one format and IEEE-order as
    /// `minimum <= maximum`; NaN endpoints are never ordered.
    pub fn ordered(&self) -> bool {
        self.minimum.format() == self.maximum.format()
            && Self::decode(self.minimum) <= Self::decode(self.maximum)
    }

    /// Whether a scalar delivery of `value` satisfies the retained range at
    /// its declared format: rejects NaN, requires `value >= minimum`, and
    /// requires `value < maximum` — or `value <= maximum` when the authored
    /// endpoint is inclusive.
    pub fn contains(&self, value: IeeeFloatValue) -> bool {
        if value.format() != self.format() {
            return false;
        }
        let candidate = Self::decode(value);
        candidate >= Self::decode(self.minimum)
            && (candidate < Self::decode(self.maximum)
                || (self.maximum_inclusive && candidate == Self::decode(self.maximum)))
    }

    /// Whether every value `inner` admits also lies inside `self` under the
    /// authored IEEE endpoints — `inner ⊆ self`. This is how a ranged caller
    /// parameter discharges a ranged callee parameter: `inner.minimum` must
    /// be at or above `self.minimum`, and `inner.maximum` must fall strictly
    /// below `self.maximum` unless the endpoints coincide and `self` is at
    /// least as permissive about the boundary.
    pub fn contains_range(&self, inner: &Self) -> bool {
        if inner.format() != self.format() {
            return false;
        }
        let inner_minimum = Self::decode(inner.minimum);
        let outer_minimum = Self::decode(self.minimum);
        let inner_maximum = Self::decode(inner.maximum);
        let outer_maximum = Self::decode(self.maximum);
        inner_minimum >= outer_minimum
            && (inner_maximum < outer_maximum
                || (inner_maximum == outer_maximum
                    && (self.maximum_inclusive || !inner.maximum_inclusive)))
    }
}

/// One authored integer entry range retained against a machine's direct
/// scalar parameter. `integer_type` is the declared fixed-width integer
/// carrier — an address carrier is not an entry-range carrier — and both
/// endpoints are inclusive values admitted by that carrier. An authored
/// exclusive maximum already became its predecessor before retention, so
/// the row always reads `minimum <= x <= maximum` under the carrier's
/// signedness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarIntegerRange {
    /// Machine whose direct scalar parameter carries the range.
    pub machine: MachineId,
    /// The machine's direct scalar parameter carrying the range.
    pub parameter: ValueId,
    /// The declared fixed integer carrier of the parameter and endpoints.
    pub integer_type: IntegerType,
    /// Inclusive minimum admitted by `integer_type`.
    pub minimum: IntegerValue,
    /// Inclusive maximum admitted by `integer_type`.
    pub maximum: IntegerValue,
}

impl ScalarIntegerRange {
    /// Whether the carrier is fixed-width, both endpoints are admitted by
    /// it, and they order as `minimum <= maximum` under its signedness.
    pub fn ordered(&self) -> bool {
        !self.integer_type.is_address()
            && self
                .integer_type
                .compare(self.minimum, self.maximum)
                .is_some_and(|ordering| !ordering.is_gt())
    }

    /// Whether a scalar delivery of `value` satisfies the retained range:
    /// the carrier admits it and `minimum <= value <= maximum`.
    pub fn contains(&self, value: IntegerValue) -> bool {
        self.integer_type.admits(value)
            && self
                .integer_type
                .compare(self.minimum, value)
                .is_some_and(|ordering| !ordering.is_gt())
            && self
                .integer_type
                .compare(value, self.maximum)
                .is_some_and(|ordering| !ordering.is_gt())
    }

    /// Whether every value `inner` admits also lies inside `self` —
    /// `inner ⊆ self` at one shared carrier. This is how a ranged caller
    /// parameter discharges a ranged callee parameter.
    pub fn contains_range(&self, inner: &Self) -> bool {
        self.integer_type == inner.integer_type
            && self
                .integer_type
                .compare(self.minimum, inner.minimum)
                .is_some_and(|ordering| !ordering.is_gt())
            && self
                .integer_type
                .compare(inner.maximum, self.maximum)
                .is_some_and(|ordering| !ordering.is_gt())
    }
}
