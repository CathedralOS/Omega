use super::super::RangeFacts;

impl RangeFacts<'_> {
    pub(in crate::checks::ranges) fn alias_collection(&mut self, original: &str, alias: &str) {
        if original == alias {
            return;
        }

        // Conservative subslice overlap: an index/range proof on `original` only
        // transfers to `alias` if the two may refer to overlapping elements.
        // Identical labels and any pair that is not a provably-disjoint pair of
        // same-base windows are assumed to overlap (the safe default); only a
        // provably-disjoint window pair blocks the transfer.
        if !self.windows_may_overlap(original, alias) {
            // Length facts still transfer — a disjoint window is no longer than
            // the base — but element-position proofs (indexes / range bounds) do
            // not, because they may name positions outside the disjoint window.
            if let Some(minimum_length) = self.minimum_length(original) {
                self.prove_minimum_length(alias.to_owned(), minimum_length);
            }
            if let Some(exact_length) = self.exact_length(original) {
                self.prove_exact_length(alias.to_owned(), exact_length);
            }
            return;
        }

        for (_, index) in self
            .proven_indexes
            .clone()
            .into_iter()
            .filter(|(collection, _)| collection == original)
        {
            self.prove_index(alias.to_owned(), index);
        }
        for (_, bound) in self
            .proven_range_bounds
            .clone()
            .into_iter()
            .filter(|(collection, _)| collection == original)
        {
            self.prove_range_bound(alias.to_owned(), bound);
        }
        if let Some(minimum_length) = self.minimum_length(original) {
            self.prove_minimum_length(alias.to_owned(), minimum_length);
        }
        if let Some(exact_length) = self.exact_length(original) {
            self.prove_exact_length(alias.to_owned(), exact_length);
        }

        // Descendants are joined by typed place equality in place_aliases,
        // never by replacing a prefix in a rendered label here.
    }

    pub(in crate::checks::ranges) fn alias_index(&mut self, original: &str, alias: &str) {
        if original == alias {
            return;
        }

        // This fact describes the copied value, not the source's future value.
        if self.non_negative_is_proven(original)
            || self.non_negative_is_proven_via_ordering(original)
        {
            self.prove_non_negative(alias.to_owned());
        }

        for (_, upper_bound) in self
            .proven_index_upper_bounds
            .clone()
            .into_iter()
            .filter(|(index, _)| index == original)
        {
            self.prove_index_upper_bound(alias.to_owned(), upper_bound);
        }
        for (_, binder) in self
            .below_const_binders
            .clone()
            .into_iter()
            .filter(|(index, _)| index == original)
        {
            self.prove_below_const_binder(alias.to_owned(), binder);
        }
        for (collection, _) in self
            .proven_indexes
            .clone()
            .into_iter()
            .filter(|(_, index)| index == original)
        {
            self.prove_index(collection, alias.to_owned());
        }
        for (collection, _) in self
            .proven_range_bounds
            .clone()
            .into_iter()
            .filter(|(_, bound)| bound == original)
        {
            self.prove_range_bound(collection, alias.to_owned());
        }
        for (lower, upper, strict) in self
            .proven_orderings
            .clone()
            .into_iter()
            .filter(|(lower, upper, _)| lower == original || upper == original)
        {
            let lower = if lower == original {
                alias.to_owned()
            } else {
                lower
            };
            let upper = if upper == original {
                alias.to_owned()
            } else {
                upper
            };
            if strict {
                self.prove_strictly_less(lower, upper);
            } else {
                self.prove_at_most(lower, upper);
            }
        }
    }
}
