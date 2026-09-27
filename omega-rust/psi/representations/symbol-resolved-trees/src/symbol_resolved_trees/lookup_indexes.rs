//! Declaration and roster indexes owned by the resolved program itself.
//!
//! Resolution asks the same positional questions many times — where a
//! symbol's statement, parameter, or data definition sits — and previously
//! answered them through thread-local caches keyed on slice addresses and
//! content samples. Those caches now live here: the slot is owned by the
//! `SymbolResolvedTrees`, so the addresses inside it are owned by the same
//! program and can never be recycled under the index. Revision counters
//! cover arena-keyed indexes; the slice-keyed position maps still carry
//! their content samples because a slice has no other identity — only the
//! program's ownership, not the samples, bounds the index's lifetime.

use std::sync::Mutex;

use crate::{signature, statement};
use arena::{Handle, OwnedIndex};
use symbols::{SymbolHandle, SymbolKeyMap};

/// Handle-keyed maps take the integer hasher; none is iterated for output
/// order, and `usize` slice-start keys hash through the same mixing.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

/// One whole-source index behind the per-type-reference origin lookup: the
/// generic-application origin roster bucketed by instance symbol, and the
/// data-definition roster bucketed by symbol. The typing stage fills it —
/// the `malformed` flag records an unreadable origin row so callers keep
/// their scanned fallback.
#[derive(Clone, Default)]
pub struct GenericOriginIndex {
    pub origins_by_symbol:
        SymbolKeyMap<SymbolHandle, Vec<Handle<crate::types::GenericApplicationOrigin>>>,
    pub definitions_by_symbol: SymbolKeyMap<SymbolHandle, Vec<usize>>,
    pub malformed: bool,
}

/// Position map for one statement-prefix or parameter slice. The watermark
/// pattern fits stamped prefixes: a state's `prior_statements` strictly
/// appends as statements resolve, so entries indexed below the watermark
/// stay valid for any longer prefix of the same backing slice.
#[derive(Clone, Default)]
struct PrefixPositions {
    watermark: usize,
    samples: [usize; 3],
    by_symbol: SymbolKeyMap<SymbolHandle, u32>,
}

/// Position maps keyed by slice start. Each entry keeps its watermark and
/// content samples: `as_ptr` has no identity of its own, so the samples are
/// what discriminate a prefix of one state's statements from a recycled
/// address or a foreign fixture slice.
#[derive(Clone, Default)]
struct PrefixPositionMaps {
    by_start: HashMap<usize, PrefixPositions>,
}

/// The resolution indexes owned by one `SymbolResolvedTrees`.
#[derive(Default)]
pub struct ResolvedIndexes {
    generic_origins: OwnedIndex<GenericOriginIndex>,
    data_positions: OwnedIndex<SymbolKeyMap<SymbolHandle, u32>>,
    statements: Mutex<PrefixPositionMaps>,
    parameters: Mutex<PrefixPositionMaps>,
}

// A cloned program keeps its source's arena contents and slice storage, so
// the clone starts warm with the same index content.
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

fn prefix_samples(statements: &[statement::Statement], watermark: usize) -> [usize; 3] {
    [
        statement_sample(&statements[0]),
        statement_sample(&statements[watermark / 2]),
        statement_sample(&statements[watermark - 1]),
    ]
}

/// Returns the first `Statement::LocalData` position naming `symbol` within
/// `statements[..len]`; positions below the watermark are already indexed
/// and stay valid for any shorter prefix of the same backing slice.
fn local_statement_position(
    maps: &Mutex<PrefixPositionMaps>,
    statements: &[statement::Statement],
    symbol: SymbolHandle,
) -> Option<u32> {
    let mut maps = maps.lock().unwrap_or_else(|error| error.into_inner());
    let start = statements.as_ptr() as usize;
    let fresh = match maps.by_start.get(&start) {
        Some(entry) => {
            entry.watermark <= statements.len()
                && (entry.watermark == 0
                    || prefix_samples(statements, entry.watermark) == entry.samples)
        }
        None => false,
    };
    if !fresh {
        maps.by_start.insert(start, PrefixPositions::default());
    }
    let entry = maps.by_start.get_mut(&start).expect("entry just populated");
    for (position, statement) in statements.iter().enumerate().skip(entry.watermark) {
        if let statement::Statement::LocalData(local) = statement {
            entry
                .by_symbol
                .entry(local.symbol)
                .or_insert(position as u32);
        }
    }
    entry.watermark = statements.len();
    entry.samples = if entry.watermark == 0 {
        [0; 3]
    } else {
        prefix_samples(statements, entry.watermark)
    };
    entry.by_symbol.get(&symbol).copied()
}

/// Returns `symbol`'s position within `parameters`, indexing the roster
/// once per distinct parameter slice.
fn parameter_position(
    maps: &Mutex<PrefixPositionMaps>,
    parameters: &[signature::StateParameter],
    symbol: SymbolHandle,
) -> Option<u32> {
    let mut maps = maps.lock().unwrap_or_else(|error| error.into_inner());
    let start = parameters.as_ptr() as usize;
    let len = parameters.len();
    let sample = |index: usize| -> usize {
        if index < len {
            parameters[index].symbol.arena_index() as usize
        } else {
            0
        }
    };
    let fingerprint =
        sample(0).rotate_left(11) ^ sample(len / 2).rotate_left(23) ^ sample(len.saturating_sub(1));
    let fresh = matches!(maps.by_start.get(&start), Some(entry)
        if entry.watermark == len && entry.samples == [fingerprint, 0, 0]);
    if !fresh {
        let mut positions = SymbolKeyMap::default();
        for (position, parameter) in parameters.iter().enumerate() {
            positions.entry(parameter.symbol).or_insert(position as u32);
        }
        maps.by_start.insert(
            start,
            PrefixPositions {
                watermark: len,
                samples: [fingerprint, 0, 0],
                by_symbol: positions,
            },
        );
    }
    maps.by_start
        .get(&start)
        .expect("entry just populated")
        .by_symbol
        .get(&symbol)
        .copied()
}

impl ResolvedIndexes {
    /// The generic-origin roster index the typing stage consumes. `revision`
    /// is the summed revision of the arenas the builder reads; `build` fills
    /// the index when it moved.
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

    /// First position of `symbol` in the `definitions` roster — the index
    /// `OrderedRootArena::find` used to rescan per projection hop. The
    /// arena's own revision is the freshness key.
    pub fn data_definition_position(
        &self,
        definitions: &arena::OrderedRootArena<crate::data::DataDefinition>,
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

    /// First `LocalData` position naming `symbol` inside a state's stamped
    /// statement prefix; see [`local_statement_position`].
    pub fn local_statement_position(
        &self,
        statements: &[statement::Statement],
        symbol: SymbolHandle,
    ) -> Option<u32> {
        local_statement_position(&self.statements, statements, symbol)
    }

    /// `symbol`'s position inside a state's parameter roster; see
    /// [`parameter_position`].
    pub fn parameter_position(
        &self,
        parameters: &[signature::StateParameter],
        symbol: SymbolHandle,
    ) -> Option<u32> {
        parameter_position(&self.parameters, parameters, symbol)
    }
}

impl super::SymbolResolvedTrees {
    /// The generic-origin roster index the typing stage consumes. `build`
    /// fills the index whenever the origin or definition arenas' revisions
    /// moved since the stored index was built.
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
            .wrapping_add(self.tables.declarations.child_type_references.revision())
            .wrapping_add(self.roots.data_definitions.revision());
        self.resolution_indexes
            .with_generic_origin_index(revision, build, run)
    }
}
