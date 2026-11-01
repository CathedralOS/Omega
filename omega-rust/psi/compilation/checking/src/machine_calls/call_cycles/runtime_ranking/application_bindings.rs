//! Exact formal-to-observed coordinates of a declared application. Retaining
//! and reading the roster changes arena ownership, never its associations.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRankApplicationBinding {
    pub formal: crate::RankSubjectCoordinate,
    pub actual: crate::RankSubjectCoordinate,
    pub entry_indexes: Vec<checked_trees::CheckedEntryIndexBound>,
}

impl RuntimeRankApplicationBinding {
    pub fn retain(
        &self,
        plan: &mut checked_trees::CheckedRankArrivalPlan,
    ) -> checked_trees::CheckedRankCoordinateBinding {
        let entry = plan
            .coordinates
            .insert(checked_trees::CheckedRankCoordinate {
                parameter: self.formal.parameter,
                path: plan
                    .path_segments
                    .insert_many(self.formal.path.iter().cloned()),
                ..Default::default()
            });
        let destination = plan
            .coordinates
            .insert(checked_trees::CheckedRankCoordinate {
                parameter: self.actual.parameter,
                path: plan
                    .path_segments
                    .insert_many(self.actual.path.iter().cloned()),
                entry_indexes: plan
                    .entry_indexes
                    .insert_many(self.entry_indexes.iter().cloned()),
            });
        checked_trees::CheckedRankCoordinateBinding {
            entry,
            destination,
            ..Default::default()
        }
    }

    pub fn from_checked(
        plan: &checked_trees::CheckedRankArrivalPlan,
        binding: &checked_trees::CheckedRankCoordinateBinding,
    ) -> Option<Self> {
        let entry = plan.coordinates.get(binding.entry);
        let destination = plan.coordinates.get(binding.destination);
        Some(Self {
            formal: crate::RankSubjectCoordinate {
                parameter: entry.parameter,
                path: plan.path_segments.span(entry.path)?.to_vec(),
            },
            actual: crate::RankSubjectCoordinate {
                parameter: destination.parameter,
                path: plan.path_segments.span(destination.path)?.to_vec(),
            },
            entry_indexes: plan.entry_indexes.span(destination.entry_indexes)?.to_vec(),
        })
    }
}
