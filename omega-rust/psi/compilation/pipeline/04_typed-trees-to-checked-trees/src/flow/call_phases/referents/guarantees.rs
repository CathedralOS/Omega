//! Prepare immutable returned guarantees once, not once per incoming fact.
//!
//! Exact static places admit a collision-checked scratch bucket join. A captured
//! runtime selector does not become a static identity: those rows retain the
//! existing conditional occurrence judgment. Neither index caches a live
//! preservation verdict or excuses any affecting writer.

use facts::PlaceSegment;
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;
use validation::CanonicalPlace;

pub(super) struct ReturnedGuarantee {
    pub parameter: SymbolHandle,
    pub place: CanonicalPlace,
    pub domain: SymbolHandle,
    pub identity: SemanticDomainId,
}

pub(super) struct PreparedGuarantees {
    exact: Vec<ReturnedGuarantee>,
    exact_positions: facts::places::PlaceBucketIndex,
    selected: Vec<ReturnedGuarantee>,
}

impl PreparedGuarantees {
    pub fn prepare(rows: Vec<ReturnedGuarantee>) -> Self {
        let (exact, selected): (Vec<_>, Vec<_>) = rows.into_iter().partition(|row| {
            row.place
                .segments
                .iter()
                .all(crate::flow::place_segment_has_static_identity)
        });
        let exact_positions = facts::places::PlaceBucketIndex::new(
            exact
                .iter()
                .map(|row| facts::places::place_bucket_hash(row.place.root, &row.place.segments)),
        );
        Self {
            exact,
            exact_positions,
            selected,
        }
    }

    /// The caller has independently established the concrete fact's static
    /// identity and this writer's overlap/confinement. Match only that formal,
    /// domain instance and occurrence; a guarantee on another writer is inert.
    pub fn restores(
        &self,
        program: &dyn validation::CheckingInput,
        parameter: Option<SymbolHandle>,
        concrete: &CanonicalPlace,
        domain: SymbolHandle,
        identity: SemanticDomainId,
    ) -> bool {
        let matches_contract = |row: &ReturnedGuarantee| {
            parameter == Some(row.parameter) && domain == row.domain && identity == row.identity
        };
        let key = facts::places::place_bucket_hash(concrete.root, &concrete.segments);
        self.exact_positions.candidates(key).any(|position| {
            let row = &self.exact[position];
            row.place == *concrete && matches_contract(row)
        }) || self.selected.iter().any(|row| {
            matches_contract(row) && same_occurrence_when_selected(program, concrete, &row.place)
        })
    }
}

/// Conditional identity of a concrete row and a captured selected occurrence.
/// An unknown single-element coordinate may select that concrete index; it
/// does not equate two separately evaluated selectors or a whole range.
fn same_occurrence_when_selected(
    program: &dyn validation::CheckingInput,
    concrete: &CanonicalPlace,
    selected: &CanonicalPlace,
) -> bool {
    concrete.root == selected.root
        && concrete.segments.len() == selected.segments.len()
        && concrete
            .segments
            .iter()
            .zip(&selected.segments)
            .all(|(left, right)| match (*left, *right) {
                (PlaceSegment::FixedIndex { .. }, PlaceSegment::Index { expression }) => {
                    expression.is_valid()
                        && program.expression_table.range_node(expression).is_none()
                }
                _ => crate::flow::static_place_segment_pair_equal(*left, *right)
                    .unwrap_or(*left == *right),
            })
}
