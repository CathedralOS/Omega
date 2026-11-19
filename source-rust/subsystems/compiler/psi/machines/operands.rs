use crate::StructuralArgument;
use semantic_vocabulary::{ObligationId, PlaceId, ScalarType, StructuralFieldId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralStoreDisposition {
    Vacant,
    ReplaceDiscardable,
}

impl StructuralStoreDisposition {
    pub fn required_access(self) -> crate::StructuralAccess {
        match self {
            Self::Vacant => crate::StructuralAccess::MutableBorrow,
            Self::ReplaceDiscardable => crate::StructuralAccess::WriteOnlyBorrow,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByteSequenceOperand {
    Whole {
        source: StructuralArgument,
        length: ValueId,
    },
    Field {
        source: StructuralArgument,
        field: StructuralFieldId,
        length: ValueId,
    },
}

impl ByteSequenceOperand {
    pub const fn length(&self) -> ValueId {
        match self {
            Self::Whole { length, .. } | Self::Field { length, .. } => *length,
        }
    }

    pub const fn source(&self) -> PlaceId {
        match self {
            Self::Whole { source, .. } | Self::Field { source, .. } => source.place,
        }
    }

    pub const fn argument(&self) -> &StructuralArgument {
        match self {
            Self::Whole { source, .. } | Self::Field { source, .. } => source,
        }
    }

    /// Field operands keep their final selector outside the common
    /// argument path. All storage consumers need that same complete operand.
    pub fn storage_projection(&self) -> crate::OperationProjection<'_> {
        let source = self.argument();
        crate::OperationProjection {
            root: source.place,
            path: &source.path,
            field: match self {
                Self::Whole { .. } => None,
                Self::Field { field, .. } => Some(*field),
            },
        }
    }

    pub fn argument_mut(&mut self) -> &mut StructuralArgument {
        match self {
            Self::Whole { source, .. } | Self::Field { source, .. } => source,
        }
    }

    pub fn remap_length(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
        match self {
            Self::Whole { length, .. } | Self::Field { length, .. } => *length = map(*length),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ElementViewRegion {
    WholeCollection,
    WholeRepresentation {
        source_length: ValueId,
        obligation: ObligationId,
    },
    RepresentationBytes {
        start: ValueId,
        end: ValueId,
        obligation: ObligationId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceStorageView {
    pub byte_offset: ValueId,
    pub stored_scalar: Option<ScalarType>,
}
