use super::RangeFacts;

impl RangeFacts<'_> {
    pub(in crate::checks::ranges) fn alias_collection(&mut self, original: &str, alias: &str) {
        if original == alias {
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
    }

    pub(in crate::checks::ranges) fn alias_index(&mut self, original: &str, alias: &str) {
        if original == alias {
            return;
        }

        for (_, upper_bound) in self
            .proven_index_upper_bounds
            .clone()
            .into_iter()
            .filter(|(index, _)| index == original)
        {
            self.prove_index_upper_bound(alias.to_owned(), upper_bound);
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
        for (lower, upper) in self
            .proven_orderings
            .clone()
            .into_iter()
            .filter(|(lower, upper)| lower == original || upper == original)
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
            self.prove_at_most(lower, upper);
        }
    }

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

    pub(in crate::checks::ranges) fn index_value_is_proven(
        &self,
        collection: &str,
        index: i64,
    ) -> bool {
        index >= 0
            && self
                .minimum_length(collection)
                .is_some_and(|length| index < length)
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
        bound >= 0
            && self
                .minimum_length(collection)
                .is_some_and(|length| bound <= length)
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

    fn minimum_length(&self, collection: &str) -> Option<i64> {
        self.minimum_lengths
            .iter()
            .find_map(|(known_collection, minimum_length)| {
                (known_collection == collection).then_some(*minimum_length)
            })
    }
}
