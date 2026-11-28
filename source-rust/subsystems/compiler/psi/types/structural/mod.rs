//! Structural type declarations and the field, case and referent shapes they carry.

mod atomic_outcomes;
mod fields;
mod patterns;
mod queries;
#[cfg(test)]
mod tests;

pub use atomic_outcomes::{atomic_once_outcome_cases, atomic_once_result_is_plain};
pub use fields::{ByteSequenceCarrier, StructuralFieldType};
pub use queries::*;

use crate::TerminalIdentityIndex;
use language_core::BindingRelevance;
use semantic_vocabulary::{
    ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralTypeDeclaration {
    /// Exact source owner; absent for compiler-generated declarations.
    pub declaration_identity: TerminalIdentityIndex,
    pub quotient_carrier: Option<StructuralTypeId>,
    pub id: StructuralTypeId,
    pub identity: TerminalIdentityIndex,
    /// This declaration's intrinsic root debt, independent of its
    /// declared multiplicity and claims retained in owned components.
    pub owns_linear_claim: bool,
    pub multiplicity: crate::StructuralMultiplicity,
    pub no_code_affine_cleanup: bool,
    pub descriptor_cleanup: bool,
    pub nominal_cleanup: Option<crate::NominalCleanupSelection>,
    pub shape: StructuralTypeShape,
    pub scalar_restriction: Option<StructuralFieldType>,
    /// The data declaration's `requires` clauses, in authored order:
    /// the declaration-owned requirement every construction and
    /// scalar-field replacement proves and every validated read
    /// instantiates (structural_access.md, "Complete record
    /// construction"). A clause names the declaration's own fields
    /// by `StructuralFieldId`, with `self` as place 1, and its cases
    /// by `StructuralCaseId`. Empty for every type of a module
    /// whose producer proves them elsewhere; only a record or a sum
    /// declares any.
    pub requires: Vec<semantic_vocabulary::Proposition>,
}

impl StructuralTypeDeclaration {
    pub const fn is_copyable(&self) -> bool {
        matches!(
            self.multiplicity,
            crate::StructuralMultiplicity::Unrestricted
        )
    }

    pub fn is_float_class_shape(&self) -> bool {
        let StructuralTypeShape::Sum { cases } = &self.shape else {
            return false;
        };
        let [nan, infinity, normal, subnormal, zero] = cases.as_slice() else {
            return false;
        };
        nan.fields.is_empty()
            && [infinity, normal, subnormal, zero].iter().all(|case| {
                matches!(case.fields.as_slice(), [field] if !field.relevance.is_erased()
                && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralReferentIdentity {
    pub id: StructuralTypeId,
    pub identity: TerminalIdentityIndex,
    pub foreign_descriptor: bool,
    pub abstract_opaque: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralTypeShape {
    /// A nominal value supplied across an admitted boundary. It has no
    /// guest-visible fields or implied storage geometry; moving it
    /// preserves the supplying host's identity, rather than constructing
    /// an empty record with the same name.
    Opaque,
    /// The value owns hidden contents bounded by these declared lifetime
    /// parameters. Moving it transfers its existing loan dependencies; it
    /// supplies neither a reference nor a way to inspect those contents.
    BorrowingOpaque { lifetime_count: std::num::NonZeroU32 },
    Reference {
        referent: StructuralTypeId,
        access: crate::StructuralAccess,
    },
    PrimitiveScalar(ScalarType),
    ByteSequence(ByteSequenceCarrier),
    ElementView {
        element: StructuralTypeId,
        access: Option<crate::StructuralAccess>,
    },
    Record {
        fields: Vec<StructuralFieldDeclaration>,
        storage: crate::StructuralRecordStorage,
    },
    FixedArray {
        element: StructuralTypeId,
        length: crate::ArrayExtent,
    },
    Sum {
        cases: Vec<StructuralCaseDeclaration>,
    },
    Mixed {
        fields: Vec<StructuralFieldDeclaration>,
        cases: Vec<StructuralCaseDeclaration>,
    },
    AtomicCell(semantic_vocabulary::AtomicCellType),
    /// A checking form retains this exact source type parameter. Its
    /// contents and geometry remain unknown until specialization.
    TypeParameter,
}

impl StructuralTypeShape {
    pub fn cases(&self) -> &[StructuralCaseDeclaration] {
        match self {
            Self::Sum { cases } | Self::Mixed { cases, .. } => cases,
            _ => &[],
        }
    }

    pub fn cases_mut(&mut self) -> &mut [StructuralCaseDeclaration] {
        match self {
            Self::Sum { cases } | Self::Mixed { cases, .. } => cases,
            _ => &mut [],
        }
    }

    pub fn fields_with_case(
        &self,
    ) -> impl Iterator<
        Item = (
            Option<&StructuralCaseDeclaration>,
            &StructuralFieldDeclaration,
        ),
    > {
        let fields = match self {
            Self::Record { fields, .. } | Self::Mixed { fields, .. } => fields.as_slice(),
            _ => &[],
        };
        fields.iter().map(|field| (None, field)).chain(
            self.cases()
                .iter()
                .flat_map(|case| case.fields.iter().map(move |field| (Some(case), field))),
        )
    }

    pub fn fields(&self) -> impl Iterator<Item = &StructuralFieldDeclaration> {
        self.fields_with_case().map(|(_, field)| field)
    }

    pub fn fields_mut(&mut self) -> impl Iterator<Item = &mut StructuralFieldDeclaration> {
        let (fields, cases): (
            &mut [StructuralFieldDeclaration],
            &mut [StructuralCaseDeclaration],
        ) = match self {
            Self::Record { fields, .. } => (fields, &mut []),
            Self::Sum { cases } => (&mut [], cases),
            Self::Mixed { fields, cases } => (fields, cases),
            _ => (&mut [], &mut []),
        };
        fields
            .iter_mut()
            .chain(cases.iter_mut().flat_map(|case| &mut case.fields))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralCaseDeclaration {
    /// Exact source owner; absent for compiler-generated declarations.
    pub declaration_identity: TerminalIdentityIndex,
    pub id: StructuralCaseId,
    pub identity: TerminalIdentityIndex,
    pub fields: Vec<StructuralFieldDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralFieldDeclaration {
    /// A compiler-generated field has no source declaration and cannot
    /// satisfy a policy naming a source field, even if its name matches.
    pub declaration_identity: TerminalIdentityIndex,
    pub inline_carrier: Option<StructuralTypeId>,
    pub id: StructuralFieldId,
    pub identity: TerminalIdentityIndex,
    pub relevance: BindingRelevance,
    pub field_type: StructuralFieldType,
    pub storage: Option<semantic_vocabulary::AuthoredFieldStorage>,
}

impl StructuralTypeDeclaration {
    pub fn is_record_shape(&self) -> bool {
        matches!(self.shape, StructuralTypeShape::Record { .. })
    }

    pub fn is_bounded_owned_byte_sequence(&self) -> bool {
        matches!(
            self.shape,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
        )
    }
}

impl StructuralTypeShape {
    /// A view value carries its descriptor, including retained payload
    /// access. A reference to descriptor storage has a Reference shape.
    pub const fn is_view_descriptor(&self) -> bool {
        self.is_borrowed_byte_carrier() || matches!(self, Self::ElementView { .. })
    }

    /// Descriptors own a carrier whose retirement ends its retained permission.
    pub const fn is_borrowed_descriptor(&self) -> bool {
        matches!(self, Self::Reference { .. }) || self.is_view_descriptor()
    }

    pub fn is_canonical_view_descriptor(&self) -> bool {
        matches!(
            self,
            StructuralTypeShape::ElementView { access: None, .. }
                | StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BorrowedView { access: None }
                        | ByteSequenceCarrier::BoundedView { access: None, .. }
                )
        )
    }

    pub fn is_composite_shape(&self) -> bool {
        matches!(
            self,
            crate::record_or_fixed_array_shape_pattern!()
                | crate::sum_or_mixed_shape_pattern!()
        )
    }

    pub fn is_borrowed_view_byte_sequence(&self) -> bool {
        matches!(
            self,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        )
    }
}

impl StructuralTypeShape {
    pub fn direct_fields(&self) -> Option<&[StructuralFieldDeclaration]> {
        match self {
            Self::Record { fields, .. } | Self::Mixed { fields, .. } => Some(fields),
            _ => None,
        }
    }

    pub fn record_fields(&self) -> Option<&[StructuralFieldDeclaration]> {
        match self {
            Self::Record { fields, .. } => Some(fields),
            _ => None,
        }
    }

    pub fn sum_cases(&self) -> Option<&[StructuralCaseDeclaration]> {
        match self {
            Self::Sum { cases } | Self::Mixed { cases, .. } => Some(cases),
            _ => None,
        }
    }

    pub const fn is_bounded_owned_byte_sequence(&self) -> bool {
        matches!(
            self,
            Self::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
        )
    }

    pub const fn is_shared_readable_byte_view(&self) -> bool {
        matches!(
            self,
            Self::ByteSequence(carrier) if carrier.is_shared_readable_view()
        )
    }

    pub const fn is_borrowed_byte_view(&self) -> bool {
        matches!(
            self,
            Self::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        )
    }

    pub const fn is_borrowed_byte_carrier(&self) -> bool {
        matches!(
            self,
            Self::ByteSequence(crate::byte_sequence_view_carrier_pattern!())
        )
    }

    pub const fn is_owned_or_shared_byte_carrier(&self) -> bool {
        matches!(
            self,
            Self::ByteSequence(carrier) if carrier.is_owned_or_shared_byte_carrier()
        )
    }
}
