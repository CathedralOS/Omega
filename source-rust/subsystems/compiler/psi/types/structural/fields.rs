//! Field representations and byte-sequence carriers.

use super::{StructuralTypeDeclaration, StructuralTypeShape};
use crate::TerminalIdentityIndex;
use semantic_vocabulary::{IeeeFloatFormat, ScalarType, StructuralTypeId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralFieldType {
    Scalar(ScalarType),
    Atomic(ScalarType),
    AtomicBoundedInteger(semantic_vocabulary::BoundedIntegerType),
    BoundedInteger(semantic_vocabulary::BoundedIntegerType),
    RefinedScalar(semantic_vocabulary::ScalarRefinement),
    IeeeFloat(IeeeFloatFormat),
    ByteSequence(ByteSequenceCarrier),
    PredicateByteSequence {
        carrier: ByteSequenceCarrier,
        predicates: crate::ByteSequencePredicateSet,
    },
    Structural(StructuralTypeId),
    Erased {
        type_identity: TerminalIdentityIndex,
        scalar: Option<ScalarType>,
    },
}

impl From<semantic_vocabulary::AtomicCellType> for StructuralFieldType {
    fn from(cell: semantic_vocabulary::AtomicCellType) -> Self {
        match cell {
            semantic_vocabulary::AtomicCellType::BoundedInteger(bounds) => {
                Self::AtomicBoundedInteger(bounds)
            }
            cell => Self::Atomic(cell.scalar_type()),
        }
    }
}

impl StructuralFieldType {
    pub fn atomic_cell_type(&self) -> Option<semantic_vocabulary::AtomicCellType> {
        use semantic_vocabulary::AtomicCellType;
        let cell = match self {
            Self::Atomic(scalar) => AtomicCellType::from_scalar(*scalar)?,
            Self::AtomicBoundedInteger(bounds) => AtomicCellType::BoundedInteger(*bounds),
            _ => return None,
        };
        cell.is_supported().then_some(cell)
    }

    pub fn as_structural(&self) -> Option<StructuralTypeId> {
        let StructuralFieldType::Structural(structural) = self else {
            return None;
        };
        Some(*structural)
    }

    pub fn as_scalar(&self) -> Option<ScalarType> {
        let StructuralFieldType::Scalar(scalar) = self else {
            return None;
        };
        Some(*scalar)
    }
}

impl StructuralFieldType {
    pub fn atomic_scalar_type(&self) -> Option<ScalarType> {
        self.atomic_cell_type().map(|cell| cell.scalar_type())
    }

    pub const fn byte_sequence_carrier(&self) -> Option<ByteSequenceCarrier> {
        match self {
            Self::ByteSequence(carrier) | Self::PredicateByteSequence { carrier, .. } => {
                Some(*carrier)
            }
            _ => None,
        }
    }

    pub const fn byte_predicates(&self) -> crate::ByteSequencePredicateSet {
        match self {
            Self::PredicateByteSequence { predicates, .. } => *predicates,
            _ => crate::ByteSequencePredicateSet::EMPTY,
        }
    }

    pub fn scalar_type(&self) -> Option<ScalarType> {
        match self {
            Self::Scalar(scalar_type) | Self::Atomic(scalar_type) => Some(*scalar_type),
            Self::BoundedInteger(integer) | Self::AtomicBoundedInteger(integer) => {
                Some(ScalarType::Integer(integer.integer_type()))
            }
            Self::RefinedScalar(refinement) => Some(refinement.carrier()),
            Self::IeeeFloat(format) => Some(ScalarType::IeeeFloat(*format)),
            Self::ByteSequence(_)
            | Self::PredicateByteSequence { .. }
            | Self::Structural(_)
            | Self::Erased { .. } => None,
        }
    }

    pub fn canonical_leaf_shape(&self) -> Option<StructuralTypeShape> {
        match self {
            Self::Scalar(scalar_type) => Some(StructuralTypeShape::PrimitiveScalar(*scalar_type)),
            Self::IeeeFloat(format) => Some(StructuralTypeShape::PrimitiveScalar(
                ScalarType::IeeeFloat(*format),
            )),
            Self::ByteSequence(_)
            | Self::PredicateByteSequence { .. }
            | Self::BoundedInteger(_)
            | Self::RefinedScalar(_)
            | Self::Atomic(_)
            | Self::AtomicBoundedInteger(_)
            | Self::Structural(_)
            | Self::Erased { .. } => None,
        }
    }

    pub fn canonical_leaf_type(&self) -> Option<Self> {
        match self {
            Self::BoundedInteger(_) | Self::RefinedScalar(_) => Some(self.clone()),
            _ => self.canonical_leaf_shape().and_then(Self::from_leaf_shape),
        }
    }

    pub fn shared_observation_type(&self) -> Option<Self> {
        self.shared_observation_shape()
            .and_then(Self::from_leaf_shape)
    }

    pub fn borrowed_presentation_type(
        &self,
        access: crate::StructuralAccess,
        expected: Option<&StructuralTypeDeclaration>,
    ) -> Option<Self> {
        if access != crate::StructuralAccess::Owned
            && let Some(cell) = self.atomic_cell_type()
            && let Some(expected) = expected
            && expected.shape == StructuralTypeShape::AtomicCell(cell)
            && expected.scalar_restriction.is_none()
        {
            return Some(Self::Structural(expected.id));
        }
        if access != crate::StructuralAccess::Owned
            && let Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) =
                self.byte_sequence_carrier()
            && let Some(expected) = expected
            && expected.shape == StructuralTypeShape::ByteSequence(carrier)
        {
            return Some(Self::Structural(expected.id));
        }
        if let Some(expected) = expected
            && expected.scalar_restriction.as_ref() == Some(self)
        {
            return Some(Self::Structural(expected.id));
        }
        let presentation = if access == crate::StructuralAccess::SharedBorrow {
            match self.byte_sequence_carrier() {
                Some(ByteSequenceCarrier::BorrowedView {
                    access: Some(crate::StructuralAccess::SharedBorrow),
                }) => Some(Self::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None,
                })),
                _ => self.shared_observation_type(),
            }
        } else if access == crate::StructuralAccess::Owned {
            None
        } else {
            match self {
                Self::BoundedInteger(_) | Self::RefinedScalar(_) => self.shared_observation_type(),
                _ => self.canonical_leaf_type(),
            }
        }?;
        if let Some(expected) = expected
            && presentation.declares_leaf(expected)
        {
            Some(Self::Structural(expected.id))
        } else {
            Some(presentation)
        }
    }

    pub fn leaf_copy_type(&self, expected: Option<&StructuralTypeDeclaration>) -> Option<Self> {
        let contract = self.leaf_copy_shape().and_then(Self::from_leaf_shape)?;
        if let Some(expected) = expected
            && (contract.declares_leaf(expected)
                || self
                    .borrowed_presentation_type(
                        crate::StructuralAccess::SharedBorrow,
                        Some(expected),
                    )
                    .is_some_and(|presentation| presentation.declares_leaf(expected)))
        {
            Some(Self::Structural(expected.id))
        } else {
            Some(contract)
        }
    }

    fn from_leaf_shape(shape: StructuralTypeShape) -> Option<Self> {
        match shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => Some(Self::Scalar(scalar)),
            StructuralTypeShape::ByteSequence(carrier) => Some(Self::ByteSequence(carrier)),
            _ => None,
        }
    }

    pub fn leaf_declaration<'a>(
        &self,
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration>,
    ) -> Option<&'a StructuralTypeDeclaration> {
        let mut matching = declarations.filter(|declaration| self.declares_leaf(declaration));
        let declaration = matching.next()?;
        matching.next().is_none().then_some(declaration)
    }

    pub fn declares_leaf(&self, declaration: &StructuralTypeDeclaration) -> bool {
        if let Self::Structural(identity) = self {
            return *identity == declaration.id;
        }
        let Some((shape, restriction)) = (match self {
            Self::BoundedInteger(_) | Self::RefinedScalar(_) => self
                .scalar_type()
                .map(|scalar| (StructuralTypeShape::PrimitiveScalar(scalar), Some(self))),
            Self::ByteSequence(carrier) => {
                Some((StructuralTypeShape::ByteSequence(*carrier), None))
            }
            _ => self.canonical_leaf_shape().map(|shape| (shape, None)),
        }) else {
            return false;
        };
        declaration.shape == shape && declaration.scalar_restriction.as_ref() == restriction
    }

    pub fn shared_observation_shape(&self) -> Option<StructuralTypeShape> {
        match self {
            Self::BoundedInteger(integer) => Some(StructuralTypeShape::PrimitiveScalar(
                ScalarType::Integer(integer.integer_type()),
            )),
            Self::RefinedScalar(refinement) => {
                Some(StructuralTypeShape::PrimitiveScalar(refinement.carrier()))
            }
            _ => self.canonical_leaf_shape(),
        }
    }

    pub fn leaf_copy_shape(&self) -> Option<StructuralTypeShape> {
        match self.byte_sequence_carrier() {
            Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) => {
                Some(StructuralTypeShape::ByteSequence(carrier))
            }
            Some(carrier @ ByteSequenceCarrier::BorrowedView { .. }) => {
                Some(StructuralTypeShape::ByteSequence(carrier))
            }
            _ => self.canonical_leaf_shape(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ByteSequenceCarrier {
    BorrowedView {
        access: Option<crate::access::StructuralAccess>,
    },
    BoundedView {
        access: Option<crate::access::StructuralAccess>,
        capacity: u64,
    },
    BoundedOwned {
        capacity: u64,
    },
}

impl ByteSequenceCarrier {
    pub const fn supports_field_predicates(&self) -> bool {
        matches!(
            self,
            Self::BoundedOwned { .. }
                | Self::BorrowedView {
                    access: Some(crate::StructuralAccess::SharedBorrow),
                }
        )
    }

    pub const fn is_borrowed(&self) -> bool {
        matches!(self, Self::BorrowedView { .. } | Self::BoundedView { .. })
    }

    pub const fn is_shared_readable_view(&self) -> bool {
        matches!(
            self,
            Self::BorrowedView {
                access: None | Some(crate::StructuralAccess::SharedBorrow),
            }
        )
    }

    pub const fn is_owned_or_shared_byte_carrier(&self) -> bool {
        matches!(
            self,
            Self::BoundedOwned { .. }
                | Self::BorrowedView {
                    access: None | Some(crate::StructuralAccess::SharedBorrow),
                }
        )
    }
}
