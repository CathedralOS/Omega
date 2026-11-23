//! Lookup indexes owned by the resolved program and tied to its backing storage.
use crate::source_resolved::{signature, statement};
use arena::{Handle, HandleKeyMap, OwnedIndex};
use record_storage::{RecordError, RecordReader, RecordWriter};
use symbols::SymbolHandle;

#[derive(Clone, Default)]
pub struct GenericOriginIndex {
    pub origins_by_symbol: HandleKeyMap<
        SymbolHandle,
        Vec<Handle<crate::source_resolved::types::GenericApplicationOrigin>>,
    >,
    pub definitions_by_symbol: HandleKeyMap<SymbolHandle, Vec<usize>>,
    pub malformed: bool,
}

#[derive(Clone, Default)]
pub struct ResolvedIndexes {
    generic_origins: OwnedIndex<GenericOriginIndex>,
    data_positions: OwnedIndex<HandleKeyMap<SymbolHandle, u32>>,
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
                let mut positions = HandleKeyMap::default();
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

    // The resolver also passes temporary and growing slices here. Their address
    // identifies neither the scope nor its current declaration order.
    pub fn local_statement_position(
        &self,
        statements: &[statement::Statement],
        symbol: SymbolHandle,
    ) -> Option<u32> {
        let position = statements.iter().position(
            |row| matches!(row, statement::Statement::LocalData(local) if local.symbol == symbol),
        )?;
        u32::try_from(position).ok()
    }

    pub fn parameter_position(
        &self,
        parameters: &[signature::StateParameter],
        symbol: SymbolHandle,
    ) -> Option<u32> {
        let position = parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)?;
        u32::try_from(position).ok()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookups_follow_the_current_binder_order() {
        let indexes = ResolvedIndexes::default();
        let symbols = [1, 2, 3, 4, 5].map(SymbolHandle::from_arena_index);
        let mut parameters = symbols.map(|symbol| signature::StateParameter {
            symbol,
            ..Default::default()
        });
        let mut locals = symbols.map(|symbol| {
            statement::Statement::LocalData(statement::LocalData {
                symbol,
                name: Default::default(),
                storage: Default::default(),
            })
        });
        assert_eq!(indexes.parameter_position(&parameters, symbols[1]), Some(1));
        assert_eq!(
            indexes.local_statement_position(&locals, symbols[1]),
            Some(1)
        );
        parameters.swap(1, 3);
        locals.swap(1, 3);
        assert_eq!(
            (
                indexes.parameter_position(&parameters, symbols[1]),
                indexes.local_statement_position(&locals, symbols[1])
            ),
            (Some(3), Some(3))
        );
    }
}
