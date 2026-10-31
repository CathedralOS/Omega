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

struct Points {
    rows: HashMap<ProgramPoint, Vec<Handle<Fact>>, symbols::BuildSymbolHasher>,
    handles_round_trip: bool,
}

#[derive(Default)]
pub struct FactTable {
    arena: PagedArenaSet<Fact>,
    points: OwnedIndex<Points>,
    revision: OnceLock<u64>,
}

impl FactTable {
    pub fn at_point(&self, point: ProgramPoint) -> Vec<&Fact> {
        self.with_points(|points| {
            points
                .rows
                .get(&point)
                .into_iter()
                .flatten()
                .map(|handle| self.arena.get(*handle))
                .collect()
        })
    }

    /// Raw handles and rows in arena order, including decoded rows whose
    /// occupied slot generation does not resolve through the page lookup.
    /// The grouping borrow ends before any consumer inspects these rows.
    pub fn rows_at_point(&self, point: ProgramPoint) -> Vec<(Handle<Fact>, &Fact)> {
        self.with_points(|points| {
            if points.handles_round_trip {
                points
                    .rows
                    .get(&point)
                    .into_iter()
                    .flatten()
                    .map(|handle| (*handle, self.arena.get(*handle)))
                    .collect()
            } else {
                self.arena
                    .iter()
                    .filter(|(_, row)| row.point == point)
                    .collect()
            }
        })
    }

    fn with_points<Result>(&self, read: impl FnOnce(&Points) -> Result) -> Result {
        self.points.with(
            *self.revision.get_or_init(|| self.arena.revision()),
            || {
                let mut points = Points {
                    rows: HashMap::default(),
                    handles_round_trip: true,
                };
                for (handle, row) in self.arena.iter() {
                    points.rows.entry(row.point).or_default().push(handle);
                    if !self.arena.is_valid(handle) || !std::ptr::eq(self.arena.get(handle), row) {
                        points.handles_round_trip = false;
                    }
                }
                points
            },
            read,
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
            assert_eq!(
                rows.rows_at_point(*point)
                    .into_iter()
                    .map(|(handle, _)| handle)
                    .collect::<Vec<_>>(),
                rows.iter()
                    .filter_map(|(handle, row)| (row.point == *point).then_some(handle))
                    .collect::<Vec<_>>()
            );
            assert!(
                rows.rows_at_point(*point)
                    .iter()
                    .map(|(_, row)| *row)
                    .zip(expected.iter().copied())
                    .all(|(actual, expected)| std::ptr::eq(actual, expected))
            );
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
            ProgramPoint::Definition {
                symbol: symbols::SymbolHandle::from_parts(3, 10),
            },
            ProgramPoint::Definition {
                symbol: symbols::SymbolHandle::invalid(),
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
        for point in &points[2..] {
            rows.append(Fact {
                point: *point,
                ..Default::default()
            });
        }
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

    // Write the existing PagedArenaSet wire shape with an Arena page whose
    // occupied slot generations need not agree with its page generation.
    // The actual FactTable decoder, not a trusted constructor, reads it below.
    struct RawFactPages {
        first: arena::Arena<Fact>,
        other: Option<arena::Arena<Fact>>,
    }

    impl compiler_storage::Record for RawFactPages {
        fn write(
            &self,
            out: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            use compiler_storage::Record;
            out.record(|out| {
                1u32.write(out)?;
                false.write(out)?;
                self.first.write(out)?;
                out.record(|out| {
                    out.sequence(usize::from(self.other.is_some()))?;
                    if let Some(page) = &self.other {
                        out.record(|out| {
                            2u32.write(out)?;
                            false.write(out)?;
                            page.write(out)
                        })?;
                    }
                    Ok(())
                })?;
                (if self.other.is_some() { 2u32 } else { 1u32 }).write(out)?;
                (self.first.len() + self.other.as_ref().map_or(0, arena::Arena::len)).write(out)?;
                Fact::default().write(out)
            })
        }

        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            Err(input.error("raw fact page fixture is write-only"))
        }
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

        let symbol = symbols::SymbolHandle::from_parts(3, 9);
        let point = ProgramPoint::Definition { symbol };
        let fact = Fact {
            point,
            ..Default::default()
        };
        for alias_page in [false, true] {
            let first = arena::Arena::from_storage(
                Fact::default(),
                vec![fact, fact],
                Some(arena::ArenaSlotStorage {
                    generations: vec![1, 2],
                    occupied: vec![true, true],
                    free_indices: Vec::new(),
                }),
                1,
                false,
            )
            .unwrap();
            let other = alias_page.then(|| {
                arena::Arena::from_storage(Fact::default(), vec![Fact::default(); 2], None, 2, true)
                    .unwrap()
            });
            let bytes = compiler_storage::encode(&RawFactPages { first, other }, limits).unwrap();
            let mut decoded: FactTable = compiler_storage::decode(&bytes, limits).unwrap();
            let raw: Vec<_> = decoded
                .iter()
                .filter(|(_, row)| row.point == point)
                .collect();
            assert_eq!(raw.len(), 2);
            let displaced = raw[1].0;
            if alias_page {
                assert!(decoded.is_valid(displaced));
                assert!(!std::ptr::eq(decoded.get(displaced), raw[1].1));
            } else {
                assert!(!decoded.is_valid(displaced));
            }
            for selected in [
                point,
                ProgramPoint::Global,
                ProgramPoint::Definition {
                    symbol: symbols::SymbolHandle::from_parts(3, 10),
                },
            ] {
                let original: Vec<_> = decoded
                    .iter()
                    .filter(|(_, row)| row.point == selected)
                    .collect();
                let actual = decoded.rows_at_point(selected);
                assert_eq!(actual.len(), original.len());
                assert!(
                    actual
                        .iter()
                        .zip(&original)
                        .all(|((a, ar), (b, br))| a == b && std::ptr::eq(*ar, *br))
                );
                // Existing at_point retains its original handle-get behavior,
                // even for these malformed rows; the new reader must not use it.
                let old: Vec<_> = original
                    .iter()
                    .map(|(handle, _)| decoded.get(*handle))
                    .collect();
                assert!(
                    decoded
                        .at_point(selected)
                        .iter()
                        .zip(old)
                        .all(|(a, b)| std::ptr::eq(*a, b))
                );
            }
            assert_eq!(compiler_storage::encode(&decoded, limits).unwrap(), bytes);
            let first = raw[0].0;
            drop(raw);
            decoded.get_mut(first).point = ProgramPoint::Global;
            let remaining = decoded.rows_at_point(point);
            assert_eq!(remaining.len(), 1);
            assert_eq!(remaining[0].0, displaced);
            assert!(std::ptr::eq(
                remaining[0].1,
                decoded
                    .iter()
                    .find(|(handle, _)| *handle == displaced)
                    .unwrap()
                    .1
            ));
        }
    }
}
