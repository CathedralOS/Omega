//! Lookup indexes owned by the resolved program and tied to its backing storage.
use std::sync::Mutex;

use crate::source_resolved::{signature, statement};
use arena::{Handle, OwnedIndex};
use record_storage::{RecordError, RecordReader, RecordWriter};
use symbols::{SymbolHandle, SymbolKeyMap};

type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

#[derive(Clone, Default)]
pub struct GenericOriginIndex {
    pub origins_by_symbol: SymbolKeyMap<
        SymbolHandle,
        Vec<Handle<crate::source_resolved::types::GenericApplicationOrigin>>,
    >,
    pub definitions_by_symbol: SymbolKeyMap<SymbolHandle, Vec<usize>>,
    pub malformed: bool,
}

#[derive(Clone, Default)]
struct PrefixPositions {
    watermark: usize,
    samples: [usize; 3],
    by_symbol: SymbolKeyMap<SymbolHandle, u32>,
}

#[derive(Clone, Default)]
struct PrefixPositionMaps {
    by_start: HashMap<usize, PrefixPositions>,
}

impl PrefixPositionMaps {
    fn position<S>(
        &mut self,
        rows: &[S],
        symbol: SymbolHandle,
        sample: impl Fn(&S) -> usize,
        key: impl Fn(&S) -> Option<SymbolHandle>,
    ) -> Option<u32> {
        let start = rows.as_ptr() as usize;
        let length = rows.len();
        let samples = |watermark: usize| {
            [
                sample(&rows[0]),
                sample(&rows[watermark / 2]),
                sample(&rows[watermark - 1]),
            ]
        };
        let fresh = self.by_start.get(&start).is_some_and(|entry| {
            entry.watermark <= length
                && (entry.watermark == 0 || samples(entry.watermark) == entry.samples)
        });
        if !fresh {
            self.by_start.insert(start, PrefixPositions::default());
        }
        let entry = self.by_start.get_mut(&start).expect("entry just populated");
        for (position, row) in rows.iter().enumerate().skip(entry.watermark) {
            if let Some(row_symbol) = key(row) {
                entry.by_symbol.entry(row_symbol).or_insert(position as u32);
            }
        }
        entry.watermark = length;
        entry.samples = if length == 0 { [0; 3] } else { samples(length) };
        entry.by_symbol.get(&symbol).copied()
    }
}

#[derive(Default)]
pub struct ResolvedIndexes {
    generic_origins: OwnedIndex<GenericOriginIndex>,
    data_positions: OwnedIndex<SymbolKeyMap<SymbolHandle, u32>>,
    statements: Mutex<PrefixPositionMaps>,
    parameters: Mutex<PrefixPositionMaps>,
}

impl Clone for ResolvedIndexes {
    fn clone(&self) -> Self {
        let clone_maps = |maps: &Mutex<PrefixPositionMaps>| {
            Mutex::new(
                maps.lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone(),
            )
        };
        Self {
            generic_origins: self.generic_origins.clone(),
            data_positions: self.data_positions.clone(),
            statements: clone_maps(&self.statements),
            parameters: clone_maps(&self.parameters),
        }
    }
}

impl std::fmt::Debug for ResolvedIndexes {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ResolvedIndexes")
    }
}

impl PartialEq for ResolvedIndexes {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for ResolvedIndexes {}

fn statement_sample(statement: &statement::Statement) -> usize {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::mem::discriminant(statement).hash(&mut hasher);
    if let statement::Statement::LocalData(local) = statement {
        local.symbol.arena_index().hash(&mut hasher);
    }
    hasher.finish() as usize
}

impl ResolvedIndexes {
    pub fn with_generic_origin_index<R>(
        &self,
        revision: u64,
        build: impl FnOnce(&mut GenericOriginIndex),
        run: impl FnOnce(&GenericOriginIndex) -> R,
    ) -> R {
        self.generic_origins.with_mut(
            revision,
            || {
                let mut index = GenericOriginIndex::default();
                build(&mut index);
                index
            },
            |index| run(index),
        )
    }

    pub fn data_definition_position(
        &self,
        definitions: &arena::OrderedRootArena<crate::source_resolved::data::DataDefinition>,
        symbol: SymbolHandle,
    ) -> Option<u32> {
        self.data_positions.with(
            definitions.revision(),
            || {
                let mut positions = SymbolKeyMap::default();
                for (position, definition) in definitions.iter().enumerate() {
                    positions
                        .entry(definition.symbol)
                        .or_insert(position as u32);
                }
                positions
            },
            |positions| positions.get(&symbol).copied(),
        )
    }

    pub fn local_statement_position(
        &self,
        statements: &[statement::Statement],
        symbol: SymbolHandle,
    ) -> Option<u32> {
        self.statements
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .position(
                statements,
                symbol,
                statement_sample,
                |statement| match statement {
                    statement::Statement::LocalData(local) => Some(local.symbol),
                    _ => None,
                },
            )
    }

    pub fn parameter_position(
        &self,
        parameters: &[signature::StateParameter],
        symbol: SymbolHandle,
    ) -> Option<u32> {
        self.parameters
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .position(
                parameters,
                symbol,
                |parameter| parameter.symbol.arena_index() as usize,
                |parameter| Some(parameter.symbol),
            )
    }
}

impl crate::source_resolved::SymbolResolvedTrees {
    pub fn with_generic_origin_index<R>(
        &self,
        build: impl FnOnce(&mut GenericOriginIndex),
        run: impl FnOnce(&GenericOriginIndex) -> R,
    ) -> R {
        let revision = self
            .tables
            .types
            .generic_application_origins
            .revision()
            .wrapping_add(self.declarations().child_type_references.revision())
            .wrapping_add(self.roots.data_definitions.revision());
        self.resolution_indexes
            .with_generic_origin_index(revision, build, run)
    }
}

impl record_storage::Record for ResolvedIndexes {
    fn write(&self, _: &mut RecordWriter) -> Result<(), RecordError> {
        Ok(())
    }
    fn read<Context>(_: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        Ok(Self::default())
    }
}
