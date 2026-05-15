use crate::{data, expression, signature, snapshot, state, statement, tables, types};
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolTable;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Index};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTrees {
    pub roots: SymbolResolvedRoots,
    pub tables: SymbolResolvedTableStorage,
    pub symbols: SymbolTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedRoots {
    pub data_definitions: OrderedRootArena<crate::data::DataDefinition>,
    pub invariant_definitions: OrderedRootArena<crate::invariant::InvariantDefinition>,
    pub machines: OrderedRootArena<crate::machine::Machine>,
    pub platforms: OrderedRootArena<crate::platform::Platform>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedRootArena<T: Default> {
    handles: Arena<Handle<T>>,
    roots: HandleSpan<Handle<T>>,
    storage: Arena<T>,
}

impl<T: Default> OrderedRootArena<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, value: T) -> Handle<T> {
        let value = self.storage.append(value);
        let handle = self.handles.append(value);

        self.roots = if self.roots.is_empty() {
            HandleSpan::from_parts(handle, 1)
        } else {
            HandleSpan::from_parts(
                self.roots.start(),
                self.roots
                    .count()
                    .checked_add(1)
                    .expect("ordered root span count overflow"),
            )
        };

        value
    }

    pub fn len(&self) -> usize {
        self.roots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    pub fn first(&self) -> Option<&T> {
        self.iter().next()
    }

    pub fn iter(&self) -> OrderedRootArenaIter<'_, T> {
        OrderedRootArenaIter {
            storage: &self.storage,
            handles: self.handles.span_or_empty(self.roots).iter(),
            marker: PhantomData,
        }
    }

    pub fn for_each_mut(&mut self, mut visit: impl FnMut(&mut T)) {
        let handles = self.handles.span_or_empty(self.roots);
        let storage = &mut self.storage;

        for handle in handles {
            visit(storage.get_mut(*handle));
        }
    }

    pub fn find_mut(&mut self, mut matches: impl FnMut(&T) -> bool) -> Option<&mut T> {
        let handles = self.handles.span_or_empty(self.roots);
        let storage = &mut self.storage;

        for handle in handles {
            if matches(storage.get(*handle)) {
                return Some(storage.get_mut(*handle));
            }
        }

        None
    }
}

impl<T: Default> Default for OrderedRootArena<T> {
    fn default() -> Self {
        Self {
            handles: Arena::new(),
            roots: HandleSpan::empty(),
            storage: Arena::new(),
        }
    }
}

impl<T: Default> Index<usize> for OrderedRootArena<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        let handle = self.handles.span_or_empty(self.roots)[index];

        self.storage.get(handle)
    }
}

impl<'arena, T: Default> IntoIterator for &'arena OrderedRootArena<T> {
    type Item = &'arena T;
    type IntoIter = OrderedRootArenaIter<'arena, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct OrderedRootArenaIter<'arena, T: Default> {
    storage: &'arena Arena<T>,
    handles: std::slice::Iter<'arena, Handle<T>>,
    marker: PhantomData<&'arena T>,
}

impl<'arena, T: Default> Iterator for OrderedRootArenaIter<'arena, T> {
    type Item = &'arena T;

    fn next(&mut self) -> Option<Self::Item> {
        let handle = self.handles.next()?;

        Some(self.storage.get(*handle))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTableStorage {
    pub declarations: SymbolResolvedDeclarationStorage,
    pub bodies: SymbolResolvedBodyStorage,
    pub types: SymbolResolvedTypeStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedDeclarationStorage {
    pub data_members: Arena<data::DataMember>,
    pub data_type_parameters: Arena<data::TypeParameter>,
    pub machine_contained_objects: Arena<crate::machine::ContainedObject>,
    pub machine_owned_data: Arena<crate::machine::OwnedData>,
    pub machine_state_handles: Arena<Handle<state::State>>,
    pub machine_states: Arena<state::State>,
    pub platform_state_signatures: Arena<signature::StateSignature>,
    pub state_parameters: Arena<signature::StateParameter>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTypeStorage {
    pub constraints: Arena<types::TypeConstraint>,
    pub references: types::TypeReferenceTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedBodyStorage {
    pub expressions: expression::ExpressionTable,
    pub statements: statement::StatementTable,
}

impl SymbolResolvedTrees {
    pub fn data_members(&self, span: HandleSpan<data::DataMember>) -> &[data::DataMember] {
        self.tables.declarations.data_members.span_or_empty(span)
    }

    pub fn data_type_parameters(
        &self,
        span: HandleSpan<data::TypeParameter>,
    ) -> &[data::TypeParameter] {
        self.tables
            .declarations
            .data_type_parameters
            .span_or_empty(span)
    }

    pub fn platform_state_signatures(
        &self,
        span: HandleSpan<signature::StateSignature>,
    ) -> &[signature::StateSignature] {
        self.tables
            .declarations
            .platform_state_signatures
            .span_or_empty(span)
    }

    pub fn state_parameters(
        &self,
        span: HandleSpan<signature::StateParameter>,
    ) -> &[signature::StateParameter] {
        self.tables
            .declarations
            .state_parameters
            .span_or_empty(span)
    }

    pub fn machine_state_handles(
        &self,
        span: HandleSpan<Handle<state::State>>,
    ) -> &[Handle<state::State>] {
        self.tables
            .declarations
            .machine_state_handles
            .span_or_empty(span)
    }

    pub fn machine_state(&self, handle: Handle<state::State>) -> &state::State {
        self.tables.declarations.machine_states.get(handle)
    }

    pub fn machine_contained_objects(
        &self,
        span: HandleSpan<crate::machine::ContainedObject>,
    ) -> &[crate::machine::ContainedObject] {
        self.tables
            .declarations
            .machine_contained_objects
            .span_or_empty(span)
    }

    pub fn machine_owned_data(
        &self,
        span: HandleSpan<crate::machine::OwnedData>,
    ) -> &[crate::machine::OwnedData] {
        self.tables
            .declarations
            .machine_owned_data
            .span_or_empty(span)
    }

    pub fn rebuild_tables(&mut self) {
        let tables =
            tables::SymbolResolvedTreeTables::from_symbol_resolved_trees_with_state_spans(self);
        self.tables.bodies.expressions = tables.bodies.expressions;
        self.tables.bodies.statements = tables.bodies.statements;
        self.tables.types.references = tables.types.references;
    }

    pub fn snapshot(&self) -> snapshot::SymbolResolvedTreesSnapshot {
        snapshot::SymbolResolvedTreesSnapshot::from_symbol_resolved_trees(self)
    }

    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json()
    }

    pub fn snapshot_json_pretty(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json_pretty()
    }
}

impl Deref for SymbolResolvedTrees {
    type Target = SymbolResolvedRoots;

    fn deref(&self) -> &Self::Target {
        &self.roots
    }
}

impl DerefMut for SymbolResolvedTrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.roots
    }
}
