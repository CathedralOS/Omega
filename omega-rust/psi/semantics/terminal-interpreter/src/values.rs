//! The runtime values an execution starts from and produces: scalars,
//! structural values, case values and their runtime places.

use semantic_vocabulary::{
    IeeeFloatValue, IntegerType, IntegerValue, ScalarType, StructuralCaseId, StructuralDomainId,
    StructuralFieldId, StructuralTypeId,
};
use terminal_psi::StructuralPathSegment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalScalarValue {
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    IeeeFloat(IeeeFloatValue),
}

/// Opaque target-neutral runtime carrier for one structural argument.
///
/// `opaque_identity` is chosen by the embedding host and is only preserved for
/// argument forwarding and deterministic effect observation. Psi never treats
/// it as an address or layout. Qualification IDs are semantic runtime facts
/// supplied by the root installation and must be strictly increasing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalStructuralValue {
    pub opaque_identity: u64,
    pub structural_type: StructuralTypeId,
    pub qualifications: Vec<StructuralDomainId>,
    pub path: Vec<StructuralPathSegment>,
}

/// Exact target-neutral runtime carrier for a selected scalar sum case.
///
/// This is deliberately distinct from an opaque host structural value: a
/// producer-created case has no host identity to preserve or invent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarCaseValue {
    pub structural_type: StructuralTypeId,
    pub result_case: StructuralCaseId,
    pub fields: Vec<(semantic_vocabulary::StructuralFieldId, TerminalScalarValue)>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalStructuralBooleanFieldValue {
    pub argument_index: u32,
    /// Structural path from the entry argument to the record containing
    /// `field`. Empty retains the original direct-field input form.
    pub path: Vec<StructuralPathSegment>,
    pub field: StructuralFieldId,
    pub value: bool,
}

/// Existing target-neutral value for one direct primitive structural entry
/// argument. `argument_index` is the dense structural-parameter position, not
/// a scalar parameter or a machine-local place identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalStructuralPrimitiveValue {
    pub argument_index: u32,
    pub value: TerminalScalarValue,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StructuralRuntimePlace {
    pub(crate) opaque_identity: u64,
    pub(crate) path: Vec<StructuralPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StructuralScalarRuntimeField {
    pub(crate) parent: StructuralRuntimePlace,
    pub(crate) field: StructuralFieldId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StructuralByteSequenceRuntimeField {
    pub(crate) parent: StructuralRuntimePlace,
    pub(crate) field: StructuralFieldId,
}

impl From<&TerminalStructuralValue> for StructuralRuntimePlace {
    fn from(value: &TerminalStructuralValue) -> Self {
        Self {
            opaque_identity: value.opaque_identity,
            path: value.path.clone(),
        }
    }
}

impl TerminalScalarValue {
    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::IeeeFloat(value) => ScalarType::IeeeFloat(value.format()),
        }
    }
}
