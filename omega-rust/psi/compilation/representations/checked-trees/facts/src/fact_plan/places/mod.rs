pub mod resolution;
mod view_coordinates;
pub mod write_frame;

pub use view_coordinates::{compose_captured_view_segments, project_captured_view_segments};

use crate::PlaceHandle;
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

/// A reference written by one performed normal invocation. This identifies
/// the descriptor occurrence, never a new allocation or spatial separation.
/// The existing typed call-site owner resolves this state/statement/ordinal
/// coordinate. Target, expression and applied carrier are not duplicated here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InvocationOutputIdentity {
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub parameter_symbol: SymbolHandle,
}

impl InvocationOutputIdentity {
    pub fn is_valid(self) -> bool {
        self.state_symbol.is_valid() && self.parameter_symbol.is_valid()
    }

    /// Exact compiler identity, independent of physical aliasing.
    pub fn compare_identity(self, other: Self) -> std::cmp::Ordering {
        let key = |value: Self| {
            (
                (
                    value.state_symbol.arena_index(),
                    value.state_symbol.generation(),
                ),
                value.statement_index,
                value.call_ordinal,
                (
                    value.parameter_symbol.arena_index(),
                    value.parameter_symbol.generation(),
                ),
            )
        };
        key(self).cmp(&key(other))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PlaceRoot {
    #[default]
    Unknown,
    Symbol(SymbolHandle),
    /// The storage denoted by an invocation's original reference formal. This
    /// is a proof subject, not an executable binding or an entry-value snapshot.
    /// Keep its reference type/access even after no state parameter carries it.
    InvocationReferent {
        parameter: SymbolHandle,
        type_reference: TypeReferenceHandle,
    },
    /// Opaque provider-supplied backing of a completed output leaf. Unequal
    /// identities do not prove non-aliasing with other output or input storage.
    InvocationOutput(InvocationOutputIdentity),
    Expression(ExpressionHandle),
    TypeReference(TypeReferenceHandle),
}

/// One variant-extraction accessor per root vocabulary entry. Both place
/// vocabularies publish the same spelling (`as_symbol`, `as_expression`, …), so
/// the extraction is written once here and each enum names the variants it
/// carries.
macro_rules! place_root_handle_accessors {
    ($($variant:ident => $method:ident -> $handle:ty),* $(,)?) => {
        $(
            /// The stored handle, when this place names that variant.
            pub fn $method(&self) -> Option<$handle> {
                let Self::$variant(handle) = self else {
                    return None;
                };
                Some(*handle)
            }
        )*
    };
}

impl PlaceRoot {
    /// Whether the root retains a nonzero identity. The owning arena still
    /// establishes validity; absence cannot establish storage compatibility.
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Symbol(symbol) => symbol.is_valid(),
            Self::InvocationReferent {
                parameter,
                type_reference,
            } => parameter.is_valid() && type_reference.is_valid(),
            Self::InvocationOutput(output) => output.is_valid(),
            Self::Expression(expression) => expression.is_valid(),
            Self::TypeReference(reference) => reference.is_valid(),
        }
    }

    place_root_handle_accessors!(
        Symbol => as_symbol -> SymbolHandle,
        Expression => as_expression -> ExpressionHandle,
        TypeReference => as_type_reference -> TypeReferenceHandle,
    );
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
    place_root_handle_accessors!(
        Place => as_place -> PlaceHandle,
        Symbol => as_symbol -> SymbolHandle,
        Expression => as_expression -> ExpressionHandle,
    );
}

/// Scratch partition key for an exact structural place. Equal places produce
/// equal keys, but a key is never identity or aliasing evidence: every candidate
/// lookup must still compare its complete root, path and consumer-owned facts.
pub fn place_bucket_hash(root: PlaceRoot, segments: &[PlaceSegment]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hash);
    segments.hash(&mut hash);
    hash.finish()
}

/// Construction-only positions grouped by a scratch key, in encounter order.
/// Hash collisions retain every candidate. Callers supply full equality; this
/// index never certifies a place, domain, evidence, overlap or storage lifetime.
pub struct PlaceBucketIndex {
    positions: Vec<(u64, usize)>,
}

impl PlaceBucketIndex {
    pub fn new(keys: impl IntoIterator<Item = u64>) -> Self {
        let mut positions: Vec<_> = keys
            .into_iter()
            .enumerate()
            .map(|(position, key)| (key, position))
            .collect();
        positions.sort_unstable();
        Self { positions }
    }

    pub fn candidates(&self, key: u64) -> impl Iterator<Item = usize> + '_ {
        let start = self
            .positions
            .partition_point(|(candidate, _)| *candidate < key);
        let end =
            start + self.positions[start..].partition_point(|(candidate, _)| *candidate == key);
        self.positions[start..end]
            .iter()
            .map(|(_, position)| *position)
    }

    /// First equivalent input position for each input. Equality must imply the
    /// same scratch key; collisions still call `equal` and remain distinct.
    pub fn group_equal_positions(&self, equal: impl Fn(usize, usize) -> bool) -> Vec<usize> {
        let mut first = vec![usize::MAX; self.positions.len()];
        let mut representatives = Vec::new();
        let mut bucket = None;
        let mut bucket_start = 0;
        for (key, position) in &self.positions {
            if bucket != Some(*key) {
                bucket = Some(*key);
                bucket_start = representatives.len();
            }
            let previous = representatives[bucket_start..]
                .iter()
                .copied()
                .find(|previous| equal(*previous, *position));
            first[*position] = if let Some(previous) = previous {
                previous
            } else {
                representatives.push(*position);
                *position
            };
        }
        first
    }
}

#[cfg(test)]
mod bucket_index_tests {
    use super::PlaceBucketIndex;

    #[test]
    fn scratch_buckets_keep_collisions_and_first_encounter_positions() {
        let rows = [
            (8, "left"),
            (3, "other"),
            (8, "right"),
            (8, "left"),
            (3, "other"),
        ];
        let index = PlaceBucketIndex::new(rows.iter().map(|(key, _)| *key));
        assert_eq!(index.candidates(8).collect::<Vec<_>>(), [0, 2, 3]);
        assert!(index.candidates(99).next().is_none());
        let expected = [0, 1, 2, 0, 1];
        assert_eq!(
            index.group_equal_positions(|left, right| rows[left] == rows[right]),
            expected
        );
        // Deliberately collide every key: the equality callback must still
        // keep unequal rows apart and retain their first input positions.
        let collisions = PlaceBucketIndex::new(rows.iter().map(|_| 0));
        assert_eq!(
            collisions.group_equal_positions(|left, right| rows[left] == rows[right]),
            expected
        );
    }
}
