//! Raw fact rows and their non-authorizing program-point lookup.
//!
//! Context membership has a different order and coverage. This table groups
//! the original arena directly, retaining every row in arena order. Mutable
//! access discards the grouping, including replacement by an equal-revision
//! arena; clones and decoded tables rebuild it on demand.

use arena::{Handle, OwnedIndex, PagedArenaSet};
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::OnceLock;

use crate::{Fact, ProgramPoint};

type Points = HashMap<ProgramPoint, Vec<Handle<Fact>>, symbols::BuildSymbolHasher>;

#[derive(Default)]
pub struct FactTable {
    arena: PagedArenaSet<Fact>,
    points: OwnedIndex<Points>,
    revision: OnceLock<u64>,
}

impl FactTable {
    pub fn at_point(&self, point: ProgramPoint) -> Vec<&Fact> {
        self.points.with(
            *self.revision.get_or_init(|| self.arena.revision()),
            || {
                let mut points = Points::default();
                for (handle, row) in self.arena.iter() {
                    points.entry(row.point).or_default().push(handle);
                }
                points
            },
            |points| {
                points
                    .get(&point)
                    .into_iter()
                    .flatten()
                    .map(|handle| self.arena.get(*handle))
                    .collect()
            },
        )
    }

    pub fn take_additional_pages(&mut self) -> Self {
        self.deref_mut().take_additional_pages().into()
    }

    pub fn adopt_additional_pages(&mut self, other: Self) {
        self.deref_mut().adopt_additional_pages(other.arena);
    }
}

impl From<PagedArenaSet<Fact>> for FactTable {
    fn from(arena: PagedArenaSet<Fact>) -> Self {
        Self {
            arena,
            ..Self::default()
        }
    }
}

impl std::ops::Deref for FactTable {
    type Target = PagedArenaSet<Fact>;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

impl DerefMut for FactTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.points.clear();
        self.revision.take();
        &mut self.arena
    }
}

impl Clone for FactTable {
    fn clone(&self) -> Self {
        self.arena.clone().into()
    }

    fn clone_from(&mut self, source: &Self) {
        self.deref_mut().clone_from(&source.arena);
    }
}

impl std::fmt::Debug for FactTable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.arena.fmt(formatter)
    }
}

impl PartialEq for FactTable {
    fn eq(&self, other: &Self) -> bool {
        self.arena == other.arena
    }
}

impl Eq for FactTable {}

// Preserve the exact existing arena bytes. The derived grouping and cached
// revision have no encoded identity or admission authority.
impl compiler_storage::Record for FactTable {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        compiler_storage::Record::write(&self.arena, output)
    }

    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        <PagedArenaSet<Fact> as compiler_storage::Record>::read(input).map(Self::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches_scan(rows: &FactTable, points: &[ProgramPoint]) {
        for point in points {
            let expected = rows
                .iter()
                .filter_map(|(_, row)| (row.point == *point).then_some(row))
                .collect::<Vec<_>>();
            let actual = rows.at_point(*point);
            assert_eq!(actual.len(), expected.len());
            assert!(
                actual
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| std::ptr::eq(*actual, expected))
            );
        }
    }

    #[test]
    fn point_rows_preserve_arena_order_and_follow_mutation_and_replacement() {
        let points = [
            ProgramPoint::Global,
            ProgramPoint::Definition {
                symbol: symbols::SymbolHandle::from_parts(3, 9),
            },
        ];
        let mut rows = FactTable::default();
        let first = rows.append(Fact {
            point: points[0],
            ..Default::default()
        });
        let second = rows.append(Fact {
            point: points[1],
            ..Default::default()
        });
        rows.append(Fact {
            point: points[0],
            ..Default::default()
        });
        matches_scan(&rows, &points);
        rows.get_mut(first).point = points[1];
        matches_scan(&rows, &points);
        rows.free(second);
        matches_scan(&rows, &points);
        rows.begin_page(12);
        rows.append(Fact {
            point: points[0],
            ..Default::default()
        });
        matches_scan(&rows, &points);
        let mut baseline = rows.clone();
        let pages = rows.take_additional_pages();
        matches_scan(&rows, &points);
        rows.adopt_additional_pages(pages);
        matches_scan(&rows, &points);
        baseline.clone_from(&rows);
        matches_scan(&baseline, &points);

        let mut left = PagedArenaSet::default();
        left.append(Fact {
            point: points[0],
            ..Default::default()
        });
        let mut right = PagedArenaSet::default();
        right.append(Fact {
            point: points[1],
            ..Default::default()
        });
        assert_eq!(left.revision(), right.revision());
        let mut replaced = FactTable::from(left);
        matches_scan(&replaced, &points);
        *replaced = right;
        matches_scan(&replaced, &points);
    }

    #[test]
    fn point_lookup_keeps_the_exact_arena_encoding_and_decodes_cold() {
        let mut rows = FactTable::default();
        rows.append(Fact::default());
        let limits = compiler_storage::RecordLimits::default();
        let original = compiler_storage::encode(&*rows, limits).unwrap();
        matches_scan(&rows, &[ProgramPoint::Global]);
        assert_eq!(compiler_storage::encode(&rows, limits).unwrap(), original);
        let decoded: FactTable = compiler_storage::decode(&original, limits).unwrap();
        assert!(decoded.revision.get().is_none());
        matches_scan(&decoded, &[ProgramPoint::Global]);
        assert_eq!(decoded, rows);
    }
}
