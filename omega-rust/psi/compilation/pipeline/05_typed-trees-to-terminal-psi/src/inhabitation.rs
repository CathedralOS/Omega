//! Borrow the source type contracts for the shared emptiness query. The
//! identities and predicates are the same rows lowering publishes; a type's
//! spelling and a callee's private body never participate in this question.

use semantic_vocabulary::{IdTable, Proposition, StructuralTypeId};
use terminal_semantics::inhabitation::{Inhabitation, Status, TypeContracts};

use crate::body::{RecordType, StorageShape, ValueType};

pub(crate) struct SourceContracts<'a>(pub &'a IdTable<StructuralTypeId, RecordType>);

impl SourceContracts<'_> {
    fn record(&self, index: usize) -> &RecordType {
        self.0
            .get(self.type_id(index))
            .expect("source structural identities are dense")
    }
}

impl TypeContracts for SourceContracts<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn type_id(&self, index: usize) -> StructuralTypeId {
        StructuralTypeId::new(index as u64 + 1).expect("source type position is nonzero")
    }

    fn requires(&self, index: usize) -> &[Proposition] {
        &self.record(index).requires
    }

    fn cases(&self, index: usize) -> Option<usize> {
        match &self.record(index).shape {
            StorageShape::Sum(cases) => Some(cases.len()),
            _ => None,
        }
    }

    fn visit_owned_fields(
        &self,
        index: usize,
        mut visit: impl FnMut(Option<usize>, StructuralTypeId),
    ) {
        let mut field = |case, value_type| {
            if let ValueType::Record(child, _, _) = value_type {
                visit(case, child);
            }
        };
        match &self.record(index).shape {
            StorageShape::Record(fields) => {
                for member in fields {
                    field(None, member.value_type);
                }
            }
            StorageShape::Sum(cases) => {
                for (case, members) in cases.iter().enumerate() {
                    for member in &members.fields {
                        field(Some(case), member.value_type);
                    }
                }
            }
            StorageShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } if *length > 0 => visit(None, *element),
            // A descriptor carries no owned referent value. In particular,
            // neither an empty referent nor an empty view proves that the
            // descriptor itself is uninhabited.
            StorageShape::Opaque
            | StorageShape::ByteSequence(_)
            | StorageShape::Reference { .. }
            | StorageShape::ElementView { .. }
            | StorageShape::FixedArray { .. }
            | StorageShape::PrimitiveScalar { .. } => {}
        }
    }
}

pub(crate) fn excludes_result(types: &Inhabitation, result: Option<ValueType>) -> bool {
    matches!(result, Some(ValueType::Record(id, _, _)) if types.status(id) == Status::ProvedEmpty)
}
