//! The runtime values an execution starts from and produces: scalars,
//! structural values, case values and their runtime places.

use semantic_vocabulary::{
    IeeeFloatValue, IntegerType, IntegerValue, ScalarType, StructuralCaseId, StructuralFieldId,
    StructuralTypeId,
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

pub use terminal_installation::TerminalStructuralValue;

/// Exact target-neutral scalar record or selected scalar sum-case contents.
///
/// A missing discriminator denotes a record; a present one denotes a sum case.
/// This payload contains no storage identity or custody. Import validates it
/// before publishing an ordinary placed home; export snapshots that home's
/// actual optional discriminator and scalar cells rather than keeping a second carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarAggregateValue {
    pub structural_type: StructuralTypeId,
    pub result_case: Option<StructuralCaseId>,
    pub fields: Vec<(semantic_vocabulary::StructuralFieldId, TerminalScalarValue)>,
}

impl TerminalScalarAggregateValue {
    /// Admit actual scalar payload contents, not a tag inferred from an opaque
    /// identity. Both ordinary construction and external completion use this
    /// join; carrier equality alone does not establish bounded field contents.
    pub(crate) fn validate_against(
        &self,
        declaration: &terminal_psi::StructuralTypeDeclaration,
    ) -> Result<(), crate::errors::TerminalInterpretError> {
        use crate::errors::malformed;
        let fields = match (&declaration.shape, self.result_case) {
            (terminal_psi::StructuralTypeShape::Record { fields, storage }, None)
                if !matches!(
                    storage,
                    terminal_psi::StructuralRecordStorage::PlacedView(_)
                        | terminal_psi::StructuralRecordStorage::PlacedAccessor(_)
                ) =>
            {
                fields
            }
            (terminal_psi::StructuralTypeShape::Sum { cases }, Some(case)) => {
                &terminal_psi::find_structural_case(cases, case)
                    .ok_or_else(malformed)?
                    .fields
            }
            _ => return Err(malformed()),
        };
        if declaration.quotient_carrier.is_some()
            || declaration.id != self.structural_type
            || fields.len() != self.fields.len()
            || fields
                .iter()
                .zip(&self.fields)
                .any(|(field, (identity, value))| {
                    field.relevance.is_erased()
                        || field.id != *identity
                        || field.field_type.scalar_type() != Some(value.scalar_type())
                        || !value.satisfies_field(&field.field_type)
                })
        {
            return Err(malformed());
        }
        Ok(())
    }
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
pub(crate) enum StructuralFieldNamespace {
    /// A record field or a mixed shape's common field.
    Member,
    /// A payload field of the case active at the parent place.
    CasePayload,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StructuralScalarRuntimeField {
    pub(crate) parent: StructuralRuntimePlace,
    pub(crate) namespace: StructuralFieldNamespace,
    pub(crate) field: StructuralFieldId,
}

impl StructuralScalarRuntimeField {
    /// A record field, or a mixed shape's common field, of `parent`.
    pub(crate) fn member(parent: StructuralRuntimePlace, field: StructuralFieldId) -> Self {
        Self {
            parent,
            namespace: StructuralFieldNamespace::Member,
            field,
        }
    }

    /// A payload field of the case active at `parent`.
    pub(crate) fn case_payload(parent: StructuralRuntimePlace, field: StructuralFieldId) -> Self {
        Self {
            parent,
            namespace: StructuralFieldNamespace::CasePayload,
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
    pub(crate) namespace: StructuralFieldNamespace,
    pub(crate) field: StructuralFieldId,
}

impl StructuralByteSequenceRuntimeField {
    pub(crate) fn relocated(&self, parent: StructuralRuntimePlace) -> Self {
        Self {
            parent,
            namespace: self.namespace,
            field: self.field,
        }
    }
}

impl StructuralFieldNamespace {
    pub(crate) fn for_carrier(path: &[StructuralPathSegment]) -> Self {
        if matches!(path.last(), Some(StructuralPathSegment::Case(_))) {
            Self::CasePayload
        } else {
            Self::Member
        }
    }
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

/// The semantic view-value form of a runtime scalar.
pub(crate) fn scalar_view_value(
    value: &TerminalScalarValue,
) -> terminal_semantics::ScalarViewValue {
    match value {
        TerminalScalarValue::Boolean(value) => terminal_semantics::ScalarViewValue::Boolean(*value),
        TerminalScalarValue::Integer { value, .. } => {
            terminal_semantics::ScalarViewValue::Integer(*value)
        }
        TerminalScalarValue::IeeeFloat(value) => {
            terminal_semantics::ScalarViewValue::IeeeFloat(*value)
        }
    }
}

#[cfg(test)]
mod scalar_aggregate_tests {
    use super::*;
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
    };

    fn integer() -> IntegerType {
        IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 64).unwrap()
    }
    fn declaration() -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            quotient_carrier: None,
            id: StructuralTypeId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            shape: StructuralTypeShape::Record {
                fields: (1..=2)
                    .map(|id| StructuralFieldDeclaration {
                        id: StructuralFieldId::new(id).unwrap(),
                        identity: TerminalIdentityIndex(id as u32),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer())),
                        storage: None,
                    })
                    .collect(),
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
            scalar_restriction: None,
        }
    }
    fn payload() -> TerminalScalarAggregateValue {
        TerminalScalarAggregateValue {
            structural_type: StructuralTypeId::new(1).unwrap(),
            result_case: None,
            fields: (1..=2)
                .map(|id| {
                    (
                        StructuralFieldId::new(id).unwrap(),
                        TerminalScalarValue::Integer {
                            scalar_type: integer(),
                            value: IntegerValue::Signed(-(id as i128)),
                        },
                    )
                })
                .collect(),
        }
    }
    #[test]
    fn scalar_record_requires_complete_ordered_typed_fields() {
        let d = declaration();
        let p = payload();
        assert!(p.validate_against(&d).is_ok());
        let mut bad = p.clone();
        bad.fields.swap(0, 1);
        assert!(bad.validate_against(&d).is_err());
        let mut bad = p.clone();
        bad.fields.pop();
        assert!(bad.validate_against(&d).is_err());
        let mut bad = p.clone();
        bad.fields.push(p.fields[0]);
        assert!(bad.validate_against(&d).is_err());
        let mut bad = p.clone();
        bad.fields[0].1 = TerminalScalarValue::Boolean(true);
        assert!(bad.validate_against(&d).is_err());
        let mut bad = p.clone();
        bad.fields[0].1 = TerminalScalarValue::Integer {
            scalar_type: integer(),
            value: IntegerValue::Signed(i128::MAX),
        };
        assert!(bad.validate_against(&d).is_err());
        let mut bad = p.clone();
        bad.structural_type = StructuralTypeId::new(2).unwrap();
        assert!(bad.validate_against(&d).is_err());
    }
    #[test]
    fn record_and_sum_discriminators_cannot_be_interchanged() {
        let record = declaration();
        let p = payload();
        let case = StructuralCaseId::new(1).unwrap();
        let mut tagged = p.clone();
        tagged.result_case = Some(case);
        assert!(tagged.validate_against(&record).is_err());
        let StructuralTypeShape::Record { fields, .. } = record.shape.clone() else {
            unreachable!()
        };
        let mut sum = record;
        sum.shape = StructuralTypeShape::Sum {
            cases: vec![terminal_psi::StructuralCaseDeclaration {
                id: case,
                identity: TerminalIdentityIndex(3),
                fields,
            }],
        };
        assert!(p.validate_against(&sum).is_err());
        assert!(tagged.validate_against(&sum).is_ok());
        tagged.result_case = Some(StructuralCaseId::new(2).unwrap());
        assert!(tagged.validate_against(&sum).is_err());
    }
}
