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
    /// The callee's checked normal return independently proves this row:
    /// declared child obligations and authored ensures hold on every exit of
    /// a callable callee. `false` marks the referent's own declared root
    /// constraints, which no callee exit re-proves; they restore a caller row
    /// only under a write region covering that row's exact place.
    pub proven_at_exit: bool,
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

    pub fn has_domain(&self, domain: SymbolHandle, identity: SemanticDomainId) -> bool {
        self.exact
            .iter()
            .any(|row| row.domain == domain && row.identity == identity)
            || self
                .selected
                .iter()
                .any(|row| row.domain == domain && row.identity == identity)
    }

    pub fn can_restore_place(
        &self,
        program: &dyn validation::CheckingInput,
        concrete: &CanonicalPlace,
        domain: SymbolHandle,
        identity: SemanticDomainId,
    ) -> bool {
        let matches_domain =
            |row: &ReturnedGuarantee| domain == row.domain && identity == row.identity;
        let key = facts::places::place_bucket_hash(concrete.root, &concrete.segments);
        self.exact_positions.candidates(key).any(|position| {
            let row = &self.exact[position];
            row.place == *concrete && matches_domain(row)
        }) || self.selected.iter().any(|row| {
            matches_domain(row) && same_occurrence_when_selected(program, concrete, &row.place)
        })
    }

    /// The caller has independently established the concrete fact's static
    /// identity and this writer's overlap/confinement. Match only that formal,
    /// domain instance and occurrence; a guarantee on another writer is inert.
    /// A row no callee exit proves additionally requires `write_region` to
    /// cover the concrete place: a checked whole write re-established the
    /// constraint, while a deeper element write could still violate it.
    pub fn restores(
        &self,
        program: &dyn validation::CheckingInput,
        parameter: Option<SymbolHandle>,
        concrete: &CanonicalPlace,
        domain: SymbolHandle,
        identity: SemanticDomainId,
        write_region: Option<&CanonicalPlace>,
    ) -> bool {
        let matches_contract = |row: &ReturnedGuarantee| {
            parameter == Some(row.parameter)
                && domain == row.domain
                && identity == row.identity
                && (row.proven_at_exit
                    || write_region
                        .is_some_and(|region| same_occurrence_covered(program, region, concrete)))
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
            .all(|(left, right)| same_occurrence_segment(program, *left, *right))
}

/// A write region covers a concrete fact place when every leading segment of
/// the region selects that fact's coordinates: an unresolved index covers
/// whichever element it denotes, and a shorter region covers everything below.
fn same_occurrence_covered(
    program: &dyn validation::CheckingInput,
    region: &CanonicalPlace,
    concrete: &CanonicalPlace,
) -> bool {
    region.root == concrete.root
        && region.segments.len() <= concrete.segments.len()
        && region
            .segments
            .iter()
            .zip(&concrete.segments)
            .all(|(region, concrete)| same_occurrence_segment(program, *concrete, *region))
}

fn same_occurrence_segment(
    program: &dyn validation::CheckingInput,
    concrete: PlaceSegment,
    selected: PlaceSegment,
) -> bool {
    match (concrete, selected) {
        (PlaceSegment::FixedIndex { .. }, PlaceSegment::Index { expression }) => {
            expression.is_valid() && program.expression_table.range_node(expression).is_none()
        }
        _ => crate::flow::static_place_segment_pair_equal(concrete, selected)
            .unwrap_or(concrete == selected),
    }
}
