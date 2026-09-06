pub mod resolution;
pub mod write_frame;

use crate::PlaceHandle;
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlaceRoot {
    #[default]
    Unknown,
    Symbol(SymbolHandle),
    Expression(ExpressionHandle),
    TypeReference(TypeReferenceHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceSegment {
    Field {
        symbol: SymbolHandle,
    },
    /// Compiler-normalized identity for one statically selected sum case.
    /// Payload fields follow this segment, so otherwise identical field
    /// spellings in distinct variants cannot alias.
    Case {
        variant: SymbolHandle,
    },
    /// Compiler-normalized identity for one statically known fixed-array
    /// element. Unlike `Index`, this is independent of expression handles and
    /// can therefore appear in a type-derived ownership frontier.
    FixedIndex {
        index: usize,
    },
    /// One compiler-normalized half-open window selected from a collection.
    /// The bounds are element ordinals, not byte offsets; `start == end`
    /// denotes the empty window. Keeping the window structural lets mutation,
    /// loan-overlap, and caller-frame reasoning preserve untouched siblings
    /// without depending on expression-handle identity.
    FixedRange {
        start: usize,
        end: usize,
    },
    /// A runtime or otherwise non-normalized index expression. Ownership
    /// decomposition treats this conservatively as potentially selecting any
    /// element.
    Index {
        expression: ExpressionHandle,
    },
}

impl Default for PlaceSegment {
    fn default() -> Self {
        Self::Field {
            symbol: SymbolHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Place {
    pub root: PlaceRoot,
    pub segments: HandleSpan<PlaceSegment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FactPlace {
    #[default]
    Unknown,
    Place(PlaceHandle),
    Symbol(SymbolHandle),
    Expression(ExpressionHandle),
    TypeReference(TypeReferenceHandle),
}
