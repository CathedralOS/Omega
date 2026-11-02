//! Rejoin a demanded declaration read with one admitted invocation observation.
//! The declaration's selector uses its supplying telescope, while the selected
//! field keeps its own storage, entry-bound and scalar-value receipts. Resolving
//! a selector cannot invent an observation or grant access to its record.

use super::scalar_fields::{capture_path, same_projection, source_path};
use super::{CurrentScalarValues, ScalarProjection};
use validation::{DeclaredInputRead, RankSubjectCoordinate};

impl CurrentScalarValues<'_> {
    pub(crate) fn declared_read_path(
        &self,
        root: &RankSubjectCoordinate,
        read: &DeclaredInputRead,
    ) -> Option<Vec<facts::PlaceSegment>> {
        let selected = self.declared_projection(root, read, &read.coordinate, 0)?;
        self.fields
            .iter()
            .find(|field| {
                field.owner == root.parameter
                    && field.carrier == read.primitive_type
                    && field.value.is_some()
                    && same_projection(self.occurrence(), &field.path, &selected)
            })
            .map(|field| source_path(&field.path))
    }

    fn declared_projection(
        &self,
        root: &RankSubjectCoordinate,
        read: &DeclaredInputRead,
        coordinate: &RankSubjectCoordinate,
        depth: usize,
    ) -> Option<Vec<ScalarProjection>> {
        if depth >= 64 || coordinate.parameter != read.coordinate.parameter {
            return None;
        }
        let mut prefix = capture_path(&root.path, |selector| {
            super::scalar_fields::coordinates::selector_value(
                self.occurrence(),
                selector,
                &self.current,
                &self.fields,
            )
        })?;
        prefix.extend(capture_path(&coordinate.path, |selector| {
            read.selector_value(self.program, selector, &mut |input| {
                let selected = self.declared_projection(root, read, input, depth + 1)?;
                self.fields
                    .iter()
                    .find(|field| {
                        field.owner == root.parameter
                            && same_projection(self.occurrence(), &field.path, &selected)
                    })?
                    .value
                    .clone()
            })
        })?);
        Some(prefix)
    }
}
