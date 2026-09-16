//! Source-free scalar membership vocabulary and exact edge coercions.
//!
//! This closed catalog carries predicate-free, route-free scalar tags and the
//! retained authored IEEE floating entry ranges. An explicit edge coercion
//! adds or removes strict same-carrier membership. Removal visibly erases
//! non-owning meaning; neither direction changes payload bits, executes an
//! operation, or proves a predicate. A float entry range is not a tag: it is
//! a closed membership requirement on the values a call may deliver to one
//! direct scalar parameter.

use semantic_vocabulary::{
    DomainSemanticId, EdgeId, IeeeFloatFormat, IeeeFloatValue, MachineId, ScalarDomainId,
    ScalarQualificationSetId, ScalarType, ValueId,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarQualificationCatalog {
    pub domains: Vec<ScalarDomainDeclaration>,
    pub sets: Vec<ScalarQualificationSet>,
    pub coercions: Vec<ScalarQualificationCoercion>,
    /// Retained authored floating ranges, strictly ordered by
    /// `(machine, parameter)`. Each row constrains the exact IEEE values a
    /// call may deliver to that direct scalar parameter.
    pub float_entry_ranges: Vec<ScalarFloatRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarDomainDeclaration {
    pub id: ScalarDomainId,
    pub semantic_domain: DomainSemanticId,
    pub identity: String,
    pub carrier: ScalarType,
}

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
