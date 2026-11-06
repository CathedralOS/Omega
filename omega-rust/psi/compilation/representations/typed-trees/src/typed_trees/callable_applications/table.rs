//! Applied-callable storage owns its invocation association. The flat index
//! keeps complete handles and arena order within each invocation. Mutable
//! access clears it before exposing the arena; clones and replacements carry
//! only the association belonging to their own stored rows.
use super::{AppliedCallableOwner, AppliedCallableSignature};
use crate::static_applications::StaticMachineApplicationHandle;
use arena::{Arena, Handle, OwnedIndex};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Default)]
struct InvocationIndex {
    rows: OnceLock<Arc<[InvocationRow]>>,
}
type InvocationRow = (
    StaticMachineApplicationHandle,
    Handle<AppliedCallableSignature>,
);

#[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub(in crate::typed_trees) struct AppliedCallableTable {
    arena: Arena<AppliedCallableSignature>,
    index: OwnedIndex<InvocationIndex, ()>,
}

impl From<Arena<AppliedCallableSignature>> for AppliedCallableTable {
    fn from(arena: Arena<AppliedCallableSignature>) -> Self {
        Self {
            arena,
            index: OwnedIndex::default(),
        }
    }
}
impl std::fmt::Debug for AppliedCallableTable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.arena.fmt(formatter)
    }
}
impl std::ops::Deref for AppliedCallableTable {
    type Target = Arena<AppliedCallableSignature>;
    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}
impl std::ops::DerefMut for AppliedCallableTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.index.clear();
        &mut self.arena
    }
}
impl AppliedCallableTable {
    pub(super) fn prepare_index(&self) {
        self.index.with((), InvocationIndex::default, |index| {
            // Construction lookups may already have allocated an empty cache.
            // Preparing rows must fill it without another arena mutation.
            index.rows.get_or_init(|| {
                let mut rows = self
                    .arena
                    .iter()
                    .filter_map(|(handle, row)| {
                        let AppliedCallableOwner::MachineSelection { invocation, .. } = &row.owner
                        else {
                            return None;
                        };
                        Some((*invocation, handle))
                    })
                    .collect::<Vec<_>>();
                rows.sort_by_key(|(invocation, _)| {
                    (invocation.arena_index(), invocation.generation())
                });
                rows.into()
            });
        });
    }

    pub(super) fn machine_rows(
        &self,
        invocation: StaticMachineApplicationHandle,
    ) -> impl Iterator<Item = (Handle<AppliedCallableSignature>, &AppliedCallableSignature)> {
        let key = (invocation.arena_index(), invocation.generation());
        let indexed = self.index.with((), InvocationIndex::default, |index| {
            let rows = index.rows.get()?;
            let start =
                rows.partition_point(|(owner, _)| (owner.arena_index(), owner.generation()) < key);
            let end =
                rows.partition_point(|(owner, _)| (owner.arena_index(), owner.generation()) <= key);
            Some((Arc::clone(rows), start..end))
        });
        let mut source = self.arena.iter();
        let mut indexed = indexed;
        std::iter::from_fn(move || {
            if let Some((rows, positions)) = &mut indexed {
                return positions.next().map(|position| {
                    let handle = rows[position].1;
                    (handle, self.arena.get(handle))
                });
            }
            // Production interleaves lookup with row mutation. Preserve its
            // original walk rather than sorting every growing prefix. Frozen
            // readers and private-use batches prepare the association once.
            source.find(|(_, row)| match &row.owner {
                AppliedCallableOwner::MachineSelection {
                    invocation: owner, ..
                } => *owner == invocation,
                _ => false,
            })
        })
    }

    pub(in crate::typed_trees) fn seal(&mut self) {
        self.prepare_index();
        self.index.seal();
    }
    pub(in crate::typed_trees) fn unseal(&mut self) {
        self.index.unseal();
    }
}
