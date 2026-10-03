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

/// The declaration namespace a runtime scalar cell's field identity belongs
/// to. A record's fields and a mixed shape's common fields sit directly on
/// their parent place. A case payload overlays that same place -- the stored
/// discriminator names which case is live -- but Terminal declares its field
/// identities apart from the common ones, so a verified mixed shape may
/// number a payload field exactly like a common field (`Event { consumed:
/// bool; case Treasure(gold: i32); }` may give both id 1; verification
/// matches the establishment roster by position). The namespace keeps those
/// two cells apart; the field identity alone cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ScalarFieldNamespace {
    /// A record field or a mixed shape's common field.
    Member,
    /// A payload field of the case active at the parent place.
    CasePayload,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StructuralScalarRuntimeField {
    pub(crate) parent: StructuralRuntimePlace,
    pub(crate) namespace: ScalarFieldNamespace,
    pub(crate) field: StructuralFieldId,
}

impl StructuralScalarRuntimeField {
    /// A record field, or a mixed shape's common field, of `parent`.
    pub(crate) fn member(parent: StructuralRuntimePlace, field: StructuralFieldId) -> Self {
        Self {
            parent,
            namespace: ScalarFieldNamespace::Member,
            field,
        }
    }

    /// A payload field of the case active at `parent`.
    pub(crate) fn case_payload(parent: StructuralRuntimePlace, field: StructuralFieldId) -> Self {
        Self {
            parent,
            namespace: ScalarFieldNamespace::CasePayload,
            field,
        }
    }

    /// The same cell under a relocated parent. Copies and moves of a
    /// subtree keep each cell's namespace; rebuilding the key from the field
    /// identity alone would fold a payload cell onto a common one.
    pub(crate) fn relocated(&self, parent: StructuralRuntimePlace) -> Self {
        Self {
            parent,
            namespace: self.namespace,
            field: self.field,
        }
    }
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
    /// Supplied runtime contents must satisfy the declared predicate as well as
    /// the storage carrier. A declaration alone never authenticates host data.
    pub(crate) fn satisfies_field(self, field: &terminal_psi::StructuralFieldType) -> bool {
        let literal = match self {
            Self::Boolean(value) => semantic_vocabulary::ScalarTerm::Boolean(value),
            Self::Integer { scalar_type, value } => {
                semantic_vocabulary::ScalarTerm::Integer { scalar_type, value }
            }
            Self::IeeeFloat(value) => semantic_vocabulary::ScalarTerm::IeeeFloatLiteral(value),
        };
        terminal_semantics::scalar_field_accepts_literal(field, &literal)
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::IeeeFloat(value) => ScalarType::IeeeFloat(value.format()),
        }
    }
}
