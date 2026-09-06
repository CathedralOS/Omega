use super::RangeFacts;

mod aliases;

impl RangeFacts<'_> {
    pub(in crate::checks::ranges) fn prove_index(&mut self, collection: String, index: String) {
        if !self
            .proven_indexes
            .iter()
            .any(|(known_collection, known_index)| {
                known_collection == &collection && known_index == &index
            })
        {
            self.proven_indexes.push((collection, index));
        }
    }

    pub(in crate::checks::ranges) fn index_is_proven(&self, collection: &str, index: &str) -> bool {
        self.proven_indexes
            .iter()
            .any(|(known_collection, known_index)| {
                known_collection == collection && known_index == index
            })
    }

    pub(in crate::checks::ranges) fn prove_index_upper_bound(
        &mut self,
        index: String,
        exclusive_upper_bound: i64,
    ) {
        if exclusive_upper_bound <= 0 {
            return;
        }

        if let Some((_, known_upper_bound)) = self
            .proven_index_upper_bounds
            .iter_mut()
            .find(|(known_index, _)| known_index == &index)
        {
            *known_upper_bound = (*known_upper_bound).min(exclusive_upper_bound);
            return;
        }

        self.proven_index_upper_bounds
            .push((index, exclusive_upper_bound));
    }

    pub(in crate::checks::ranges) fn index_upper_bound_is_proven(
        &self,
        index: &str,
        length: usize,
    ) -> bool {
        let Ok(length) = i64::try_from(length) else {
            return false;
        };

        self.proven_index_upper_bounds
            .iter()
            .any(|(known_index, upper_bound)| known_index == index && *upper_bound <= length)
    }

    /// The tightest (smallest) proven EXCLUSIVE upper bound for `index`, if any.
    /// Used to carry a bound across `field = otherfield + const`.
    pub(in crate::checks::ranges) fn proven_index_upper_bound(&self, index: &str) -> Option<i64> {
        self.proven_index_upper_bounds
            .iter()
            .filter(|(known_index, _)| known_index == index)
            .map(|(_, upper_bound)| *upper_bound)
            .min()
    }

    pub(in crate::checks::ranges) fn index_value_is_proven(
        &self,
        collection: &str,
        index: i64,
    ) -> bool {
        if index < 0 {
            return false;
        }
        // A `minimum_length` floor proves a literal index in-range because the
        // collection has *at least* that many elements. An `exact_length`
        // additionally pins the length precisely; either lower-bound on the
        // length suffices for the strict `index < len` index obligation.
        self.minimum_length(collection)
            .into_iter()
            .chain(self.exact_length(collection))
            .any(|length| index < length)
    }

    /// Records that `name` is provably `>= 0`.
    pub(in crate::checks::ranges) fn prove_non_negative(&mut self, name: String) {
        if !self.proven_non_negatives.iter().any(|known| known == &name) {
            self.proven_non_negatives.push(name);
        }
    }

    pub(in crate::checks::ranges) fn non_negative_is_proven(&self, name: &str) -> bool {
        self.proven_non_negatives.iter().any(|known| known == name)
    }

    pub(in crate::checks::ranges) fn prove_at_most(&mut self, lower: String, upper: String) {
        if !self
            .proven_orderings
            .iter()
            .any(|(known_lower, known_upper)| known_lower == &lower && known_upper == &upper)
        {
            self.proven_orderings.push((lower, upper));
        }
    }

    pub(in crate::checks::ranges) fn at_most_is_proven(&self, lower: &str, upper: &str) -> bool {
        self.proven_orderings
            .iter()
            .any(|(known_lower, known_upper)| known_lower == lower && known_upper == upper)
    }

    /// Proves `index < length` by chaining an ordering with a bound: if
    /// `index <= j` (an `at_most` fact, e.g. from a two-pointer loop guard
    /// `index < j`) and `j` has a proven exclusive upper bound `<= length`, then
    /// `index <= j < length`, so `index < length`. One level of chaining --
    /// enough for the palindrome / two-pointer case where the increasing pointer
    /// is bounded only relative to the decreasing pointer (which carries the loop
    /// invariant). Sound because `at_most` is seeded from `<`/`<=` guards and
    /// dropped on reassignment, so it reflects a fact live at the access.
    pub(in crate::checks::ranges) fn index_upper_bound_is_proven_via_ordering(
        &self,
        index: &str,
        length: usize,
    ) -> bool {
        self.proven_orderings
            .iter()
            .filter(|(lower, _)| lower == index)
            .any(|(_, upper)| self.index_upper_bound_is_proven(upper, length))
    }

    /// Proves `index >= 0` by chaining an ordering with non-negativity: if `x <= index`
    /// (an `at_most` fact, e.g. from a two-pointer guard `x < index`) and `x` is proven
    /// non-negative, then `index >= x >= 0`. The symmetric counterpart of
    /// `index_upper_bound_is_proven_via_ordering` -- it carries the DECREASING pointer's
    /// lower bound from the INCREASING pointer that sits below it (the palindrome /
    /// two-pointer case where `j` runs down but stays `> i >= 0`). Sound because
    /// `at_most` is seeded from `<`/`<=` guards and dropped on reassignment, so it
    /// reflects a relation live at the access.
    pub(in crate::checks::ranges) fn non_negative_is_proven_via_ordering(
        &self,
        index: &str,
    ) -> bool {
        self.proven_orderings
            .iter()
            .filter(|(_, upper)| upper == index)
            .any(|(lower, _)| self.non_negative_is_proven(lower))
    }

    pub(in crate::checks::ranges) fn prove_range_bound(
        &mut self,
        collection: String,
        bound: String,
    ) {
        if !self
            .proven_range_bounds
            .iter()
            .any(|(known_collection, known_bound)| {
                known_collection == &collection && known_bound == &bound
            })
        {
            self.proven_range_bounds.push((collection, bound));
        }
    }

    pub(in crate::checks::ranges) fn range_bound_is_proven(
        &self,
        collection: &str,
        bound: &str,
    ) -> bool {
        self.proven_range_bounds
            .iter()
            .any(|(known_collection, known_bound)| {
                known_collection == collection && known_bound == bound
            })
    }

    pub(in crate::checks::ranges) fn range_bound_value_is_proven(
        &self,
        collection: &str,
        bound: i64,
    ) -> bool {
        if bound < 0 {
            return false;
        }
        // A range bound is the *exclusive* end of a subslice, so it is valid up
        // to and including the length (`bound <= len`). A `minimum_length` floor
        // proves `bound <= floor <= len`; an `exact_length` proves it precisely.
        self.minimum_length(collection)
            .into_iter()
            .chain(self.exact_length(collection))
            .any(|length| bound <= length)
    }

    pub(in crate::checks::ranges) fn prove_minimum_length(
        &mut self,
        collection: String,
        minimum_length: i64,
    ) {
        if minimum_length <= 0 {
            return;
        }

        if let Some((_, known_minimum)) = self
            .minimum_lengths
            .iter_mut()
            .find(|(known_collection, _)| known_collection == &collection)
        {
            *known_minimum = (*known_minimum).max(minimum_length);
            return;
        }

        self.minimum_lengths.push((collection, minimum_length));
    }

    pub(in crate::checks::ranges) fn minimum_length(&self, collection: &str) -> Option<i64> {
        self.minimum_lengths
            .iter()
            .find_map(|(known_collection, minimum_length)| {
                (known_collection == collection).then_some(*minimum_length)
            })
    }

    /// Records an exact-length fact: `collection` provably has exactly
    /// `length` elements (e.g. the subslice `a..b` over constant bounds has
    /// length `b - a`). A zero/negative length carries no useful obligation
    /// vocabulary, so it is dropped.
    pub(in crate::checks::ranges) fn prove_exact_length(
        &mut self,
        collection: String,
        length: i64,
    ) {
        if length < 0 {
            return;
        }

        if let Some((_, known_length)) = self
            .exact_lengths
            .iter_mut()
            .find(|(known_collection, _)| known_collection == &collection)
        {
            // An exact length should be stable; if a second derivation disagrees
            // keep the tighter (smaller) value so proofs stay sound.
            *known_length = (*known_length).min(length);
            return;
        }

        self.exact_lengths.push((collection, length));
    }

    pub(in crate::checks::ranges) fn exact_length(&self, collection: &str) -> Option<i64> {
        self.exact_lengths
            .iter()
            .find_map(|(known_collection, length)| {
                (known_collection == collection).then_some(*length)
            })
    }

    /// Forgets every label-keyed collection fact about `collection`: length
    /// floors, exact lengths, the window-parent relation it was carved with,
    /// and element-position proofs keyed on it. Called when a local collection
    /// is reassigned — the facts described the old value, and keeping them
    /// (the merge in `prove_minimum_length` keeps the larger floor) would let a
    /// stale floor prove indexes into the new, possibly shorter value.
    pub(in crate::checks::ranges) fn forget_collection_facts(&mut self, collection: &str) {
        self.minimum_lengths
            .retain(|(known, _)| known != collection);
        self.exact_lengths.retain(|(known, _)| known != collection);
        self.window_parents
            .retain(|(child, _, _)| child != collection);
        self.proven_indexes.retain(|(known, _)| known != collection);
        self.proven_range_bounds
            .retain(|(known, _)| known != collection);
    }

    /// Derives the carved tail window's length facts from its parent's length
    /// facts (window-shrinking length vocabulary): `parent[start..]` drops
    /// exactly `start` elements, so a parent with at least `m` elements leaves
    /// at least `m - start`, and a parent with exactly `n` elements leaves
    /// exactly `n - start`. Only meaningful for a start-only window — a
    /// constant-bounded window `[a..b)` already pins its exact length `b - a`
    /// independent of the parent.
    pub(in crate::checks::ranges) fn prove_shrunk_window_length(
        &mut self,
        child: &str,
        parent: &str,
        start: i64,
    ) {
        if start < 0 {
            return;
        }
        if let Some(exact) = self.exact_length(parent)
            && exact >= start
        {
            self.prove_exact_length(child.to_owned(), exact - start);
        }
        if let Some(minimum) = self.minimum_length(parent) {
            // `prove_minimum_length` drops non-positive floors, so a shrink
            // past the known floor simply records nothing.
            self.prove_minimum_length(child.to_owned(), minimum - start);
        }
    }

    /// Records a window-shrinking fact: `child` is a subslice of `parent` and is
    /// therefore no longer than it. `bounds` are the child's constant `[start,
    /// end)` offsets into the parent when known. The relation is the basis for
    /// conservative subslice-overlap reasoning (two windows of the same base are
    /// assumed to overlap unless proven disjoint).
    pub(in crate::checks::ranges) fn prove_window_parent(
        &mut self,
        child: String,
        parent: String,
        bounds: Option<(i64, i64)>,
    ) {
        if child == parent {
            return;
        }
        if let Some((known_parent, known_bounds)) =
            self.window_parents
                .iter_mut()
                .find_map(|(known_child, known_parent, known_bounds)| {
                    (known_child == &child).then_some((known_parent, known_bounds))
                })
        {
            *known_parent = parent;
            *known_bounds = bounds.or(*known_bounds);
            return;
        }
        self.window_parents.push((child, parent, bounds));
    }

    /// The recorded parent (base) collection a window was carved from, if any.
    pub(in crate::checks::ranges) fn window_parent(&self, child: &str) -> Option<&str> {
        self.window_parents
            .iter()
            .find_map(|(known_child, known_parent, _)| {
                (known_child == child).then_some(known_parent.as_str())
            })
    }

    /// The recorded `[start, end)` offsets of a window into its base, if known.
    fn window_bounds(&self, child: &str) -> Option<(i64, i64)> {
        self.window_parents
            .iter()
            .find_map(|(known_child, _, bounds)| (known_child == child).then_some(*bounds))
            .flatten()
    }

    /// Conservative subslice overlap: two windows/indices of the *same* base are
    /// assumed to overlap unless they are provably disjoint. Distinct bases never
    /// overlap; identical labels always overlap.
    ///
    /// The sharpened provable-disjoint cases derive each side's resolved `[start,
    /// end)` over the shared base from the recorded window bounds (literal vs.
    /// literal, disjoint windows). Any side with unknown bounds falls back to the
    /// conservative assumption that the windows overlap.
    pub(in crate::checks::ranges) fn windows_may_overlap(&self, left: &str, right: &str) -> bool {
        if left == right {
            return true;
        }

        let left_base = self.window_parent(left).unwrap_or(left);
        let right_base = self.window_parent(right).unwrap_or(right);
        if left_base != right_base {
            // Different bases: subslices/indices cannot alias. Provably disjoint.
            return false;
        }

        // Same base. Default conservative: assume overlap unless both resolved
        // windows are known and provably disjoint (`left.end <= right.start` or
        // the symmetric case).
        match (self.window_bounds(left), self.window_bounds(right)) {
            (Some((left_start, left_end)), Some((right_start, right_end))) => {
                !(left_end <= right_start || right_end <= left_start)
            }
            _ => true,
        }
    }
}
