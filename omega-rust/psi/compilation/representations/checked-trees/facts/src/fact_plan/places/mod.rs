pub mod resolution;
mod view_coordinates;
pub mod write_frame;

pub use view_coordinates::{compose_captured_view_segments, project_captured_view_segments};

use crate::PlaceHandle;
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PlaceRoot {
    #[default]
    Unknown,
    Symbol(SymbolHandle),
    Expression(ExpressionHandle),
    TypeReference(TypeReferenceHandle),
}

impl PlaceRoot {
    /// Whether the root retains a nonzero identity. The owning arena still
    /// establishes validity; absence cannot establish storage compatibility.
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Symbol(symbol) => symbol.is_valid(),
            Self::Expression(expression) => expression.is_valid(),
            Self::TypeReference(reference) => reference.is_valid(),
        }
    }

    /// The symbol root, if this place is rooted at one.
    pub fn as_symbol(&self) -> Option<SymbolHandle> {
        let PlaceRoot::Symbol(symbol) = self else {
            return None;
        };
        Some(*symbol)
    }

    /// The expression root, if this place is rooted at one.
    pub fn as_expression(&self) -> Option<ExpressionHandle> {
        let PlaceRoot::Expression(expression) = self else {
            return None;
        };
        Some(*expression)
    }

    /// The type-reference root, if this place is rooted at one.
    pub fn as_type_reference(&self) -> Option<TypeReferenceHandle> {
        let PlaceRoot::TypeReference(reference) = self else {
            return None;
        };
        Some(*reference)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// A captured collection window whose bounds belong to another invocation.
    /// This preserves the collection dimension, unlike an unknown element.
    /// It supplies neither exact extent nor executable coordinates; overlap
    /// remains conservative and access still requires the captured loan.
    OpaqueRange,
    /// A runtime or otherwise non-normalized index expression. Ownership
    /// decomposition treats this conservatively as potentially selecting any
    /// element.
    Index {
        expression: ExpressionHandle,
    },
}

impl PlaceSegment {
    /// Erase an executable selector without erasing whether it selected a
    /// collection window or one element. The owning typed-expression catalog
    /// classifies a live range before its callee-local handle is discarded.
    pub fn erase_runtime_selector(self, is_range: impl FnOnce(ExpressionHandle) -> bool) -> Self {
        match self {
            Self::Index { expression } if expression.is_valid() && is_range(expression) => {
                Self::OpaqueRange
            }
            Self::Index { .. } => Self::Index {
                expression: ExpressionHandle::invalid(),
            },
            _ => self,
        }
    }
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

impl FactPlace {
    /// The place handle, if this fact place names one.
    pub fn as_place(&self) -> Option<PlaceHandle> {
        let FactPlace::Place(place) = self else {
            return None;
        };
        Some(*place)
    }

    /// The symbol, if this fact place names one.
    pub fn as_symbol(&self) -> Option<SymbolHandle> {
        let FactPlace::Symbol(symbol) = self else {
            return None;
        };
        Some(*symbol)
    }

    /// The expression handle, if this fact place names one.
    pub fn as_expression(&self) -> Option<ExpressionHandle> {
        let FactPlace::Expression(expression) = self else {
            return None;
        };
        Some(*expression)
    }

    /// The type-reference handle, if this fact place names one.
    pub fn as_type_reference(&self) -> Option<TypeReferenceHandle> {
        let FactPlace::TypeReference(reference) = self else {
            return None;
        };
        Some(*reference)
    }
}
