//! Scalar qualifications, their establishment routes and value ranges.

use crate::TerminalIdentityIndex;
use semantic_vocabulary::{
    DomainSemanticId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType, IntegerValue,
    MachineId, ScalarDomainId, ScalarQualificationSetId, ScalarType, ValueId,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarQualificationCatalog {
    pub fields: Vec<ScalarFieldQualification>,
    /// Complete scalar types of addressable primitive storage. A
    /// reference or scalar home cannot erase its referent's domains.
    pub primitives: Vec<ScalarPrimitiveQualification>,
    pub returns: Vec<ScalarReturnQualification>,
    pub domains: Vec<ScalarDomainDeclaration>,
    pub sets: Vec<ScalarQualificationSet>,
    pub coercions: Vec<ScalarQualificationCoercion>,
    pub float_entry_ranges: Vec<ScalarFloatRange>,
    pub integer_entry_ranges: Vec<ScalarIntegerRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarFieldQualification {
    pub field: semantic_vocabulary::StructuralFieldId,
    pub value_type: semantic_vocabulary::QualifiedScalarType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarPrimitiveQualification {
    pub structural_type: semantic_vocabulary::StructuralTypeId,
    pub value_type: semantic_vocabulary::QualifiedScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarDomainDeclaration {
    pub required_memberships: Option<Vec<DomainSemanticId>>,
    pub predicate: Option<ScalarDomainPredicate>,
    pub id: ScalarDomainId,
    pub semantic_domain: DomainSemanticId,
    pub identity: TerminalIdentityIndex,
    pub carrier: ScalarType,
    /// None adds no issuer restriction; Some(empty) permits nobody.
    pub establishment_routes: Option<Vec<ScalarDomainEstablishmentRoute>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarDomainPredicate {
    pub subject: ValueId,
    pub proposition: semantic_vocabulary::Proposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScalarReturnOrigin {
    Forwarded,
    Established {
        route: u32,
    },
    Vacuous,
    /// An unrestricted predicate domain introduced at the exact
    /// completion whose arrival proves the retained domain predicate.
    Proved,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarReturnSite {
    NormalExit,
    Completion { argument_ordinal: u32 },
}

pub use super::StructuralEstablishmentRoute as ScalarDomainEstablishmentRoute;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarFloatRange {
    pub machine: MachineId,
    pub parameter: ValueId,
    pub minimum: IeeeFloatValue,
    pub maximum: IeeeFloatValue,
    pub maximum_inclusive: bool,
}

impl ScalarFloatRange {
    fn decode(value: IeeeFloatValue) -> f64 {
        match value {
            IeeeFloatValue::Binary32(bits) => f32::from_bits(bits) as f64,
            IeeeFloatValue::Binary64(bits) => f64::from_bits(bits),
        }
    }

    pub fn format(&self) -> IeeeFloatFormat {
        self.minimum.format()
    }

    pub fn ordered(&self) -> bool {
        self.minimum.format() == self.maximum.format()
            && Self::decode(self.minimum) <= Self::decode(self.maximum)
    }

    pub fn contains(&self, value: IeeeFloatValue) -> bool {
        if value.format() != self.format() {
            return false;
        }
        let candidate = Self::decode(value);
        candidate >= Self::decode(self.minimum)
            && (candidate < Self::decode(self.maximum)
                || (self.maximum_inclusive && candidate == Self::decode(self.maximum)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarIntegerRange {
    pub machine: MachineId,
    pub parameter: ValueId,
    pub integer_type: IntegerType,
    pub minimum: IntegerValue,
    pub maximum: IntegerValue,
}

impl ScalarIntegerRange {
    pub fn ordered(&self) -> bool {
        !self.integer_type.is_address()
            && self
                .integer_type
                .compare(self.minimum, self.maximum)
                .is_some_and(|ordering| !ordering.is_gt())
    }

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
}
