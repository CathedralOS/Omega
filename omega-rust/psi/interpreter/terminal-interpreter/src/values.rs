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

/// Exact target-neutral finite scalar record tree or selected scalar sum-case contents.
///
/// A missing discriminator denotes a record; a present one denotes a sum case.
/// This payload contains no storage identity or custody. Import validates it
/// before publishing an ordinary placed home; export snapshots that home's
/// actual optional discriminator and scalar cells rather than keeping a second carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarAggregateValue {
    pub structural_type: StructuralTypeId,
    pub result_case: Option<StructuralCaseId>,
    /// Complete ordered typed paths from the result root to its scalar leaves.
    pub fields: Vec<(
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        TerminalScalarValue,
    )>,
}

mod aggregates;
pub(crate) use aggregates::record_scalar_fields;

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
                        vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                            StructuralFieldId::new(id).unwrap(),
                        )],
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
        assert!(p.validate_against(&d, |_| None).is_ok());
        let mut bad = p.clone();
        bad.fields.swap(0, 1);
        assert!(bad.validate_against(&d, |_| None).is_err());
        let mut bad = p.clone();
        bad.fields.pop();
        assert!(bad.validate_against(&d, |_| None).is_err());
        let mut bad = p.clone();
        bad.fields.push(p.fields[0].clone());
        assert!(bad.validate_against(&d, |_| None).is_err());
        let mut bad = p.clone();
        bad.fields[0].1 = TerminalScalarValue::Boolean(true);
        assert!(bad.validate_against(&d, |_| None).is_err());
        let mut bad = p.clone();
        bad.fields[0].1 = TerminalScalarValue::Integer {
            scalar_type: integer(),
            value: IntegerValue::Signed(i128::MAX),
        };
        assert!(bad.validate_against(&d, |_| None).is_err());
        let mut bad = p.clone();
        bad.structural_type = StructuralTypeId::new(2).unwrap();
        assert!(bad.validate_against(&d, |_| None).is_err());
    }
    #[test]
    fn record_and_sum_discriminators_cannot_be_interchanged() {
        let record = declaration();
        let p = payload();
        let case = StructuralCaseId::new(1).unwrap();
        let mut tagged = p.clone();
        tagged.result_case = Some(case);
        assert!(tagged.validate_against(&record, |_| None).is_err());
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
        assert!(p.validate_against(&sum, |_| None).is_err());
        assert!(tagged.validate_against(&sum, |_| None).is_ok());
        tagged.result_case = Some(StructuralCaseId::new(2).unwrap());
        assert!(tagged.validate_against(&sum, |_| None).is_err());
    }
    #[test]
    fn nested_scalar_record_binds_complete_containing_paths() {
        use semantic_vocabulary::CanonicalStructuralPathSegment as Path;
        let mut child = declaration();
        child.id = StructuralTypeId::new(2).unwrap();
        let mut root = declaration();
        let terminal_psi::StructuralTypeShape::Record { fields, .. } = &mut root.shape else {
            unreachable!()
        };
        fields.truncate(1);
        fields[0].field_type = terminal_psi::StructuralFieldType::Structural(child.id);
        let mut value = payload();
        for (path, _) in &mut value.fields {
            path.insert(0, Path::Field(StructuralFieldId::new(1).unwrap()));
        }
        let lookup = |identity| (identity == child.id).then_some(&child);
        assert!(value.validate_against(&root, lookup).is_ok());
        assert!(value.validate_against(&root, |_| None).is_err());
        let mut flat = value.clone();
        for (path, _) in &mut flat.fields {
            path.remove(0);
        }
        assert!(flat.validate_against(&root, lookup).is_err());
        let mut changed = value.clone();
        changed.fields[0].0[0] = Path::Field(StructuralFieldId::new(2).unwrap());
        assert!(changed.validate_against(&root, lookup).is_err());
        let mut missing = value.clone();
        missing.fields.pop();
        assert!(missing.validate_against(&root, lookup).is_err());
        let mut repeated = value.clone();
        repeated.fields[1] = repeated.fields[0].clone();
        assert!(repeated.validate_against(&root, lookup).is_err());
        let terminal_psi::StructuralTypeShape::Record { fields, .. } = &mut child.shape else {
            unreachable!()
        };
        fields.clear();
        value.fields.clear();
        assert!(
            value
                .validate_against(&root, |identity| (identity == child.id).then_some(&child))
                .is_ok()
        );
        let terminal_psi::StructuralTypeShape::Record { fields, .. } = &mut child.shape else {
            unreachable!()
        };
        let terminal_psi::StructuralTypeShape::Record {
            fields: root_fields,
            ..
        } = &root.shape
        else {
            unreachable!()
        };
        fields.push(root_fields[0].clone());
        assert!(
            value
                .validate_against(&root, |identity| (identity == child.id).then_some(&child))
                .is_err()
        );
    }
}
