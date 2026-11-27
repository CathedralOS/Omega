//! Reconstruct the selected case of an ordinary, local sum from previously
//! observed scalars. Only whole-value copies and moves preserve this fact.

use super::Facts;
use semantic_vocabulary::{PlaceId, StructuralCaseId, StructuralFieldId, ValueId};
use terminal_psi::{OperationKind, RecordFieldValue};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct CaseValue {
    case: StructuralCaseId,
    fields: Vec<(StructuralFieldId, ValueId)>,
}

impl CaseValue {
    pub(super) fn from_operation(
        operation: &OperationKind,
        cases: &Facts<PlaceId, Self>,
    ) -> Option<Self> {
        match operation {
            OperationKind::EstablishScalarCase {
                result_case,
                fields,
            } => Some(Self {
                case: *result_case,
                fields: fields
                    .iter()
                    .map(|field| (field.field, field.value))
                    .collect(),
            }),
            OperationKind::EstablishStructuralCase {
                result_case,
                fields,
            } => Some(Self {
                case: *result_case,
                fields: fields
                    .iter()
                    .map(|field| {
                        let RecordFieldValue::Scalar { value, .. } = field.value else {
                            return None;
                        };
                        Some((field.field, value))
                    })
                    .collect::<Option<_>>()?,
            }),
            OperationKind::StructuralLeafCopy { source, path } if path.is_empty() => {
                cases.get(source).cloned()
            }
            OperationKind::MoveStructuralValue { source } if source.path.is_empty() => {
                cases.get(&source.place).cloned()
            }
            _ => None,
        }
    }

    pub(super) fn is(&self, case: StructuralCaseId) -> bool {
        self.case == case
    }

    pub(super) fn has_known_fields(&self, known: impl Fn(ValueId) -> bool) -> bool {
        self.fields.iter().all(|(_, id)| known(*id))
    }

    pub(super) fn payload(&self, selected: &[StructuralFieldId]) -> Option<Vec<ValueId>> {
        selected
            .iter()
            .map(|field| {
                self.fields
                    .iter()
                    .find(|(id, _)| id == field)
                    .map(|(_, value)| *value)
            })
            .collect()
    }
}
