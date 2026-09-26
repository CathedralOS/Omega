use std::collections::HashMap;
use std::fmt;

use arena::{Arena, Handle};

use super::{FactContext, ProgramPoint};
use crate::FactContextHandle;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod measurements;

/// Append-ordered context storage with exact program-point lookup.
///
/// Program points are sparse tuples (including symbol generations, statement
/// indices and call/arm identities), not dense arena indices. Only the group
/// lookup uses hashing; contexts and group links retain arena handles. Hash
/// iteration never determines fact order. Appends maintain the index eagerly,
/// including facts introduced during flow construction.
#[derive(Default)]
pub struct FactContexts {
    contexts: Arena<FactContext>,
    links: Arena<ContextLink>,
    groups: HashMap<ProgramPoint, ContextGroup>,
}

impl Clone for FactContexts {
    fn clone(&self) -> Self {
        Self {
            contexts: self.contexts.clone(),
            links: self.links.clone(),
            groups: self.groups.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        // Replacing only visible contexts would leave divergent tail links and
        // point groups live. Restore the complete derived index with its owner.
        self.contexts.clone_from(&source.contexts);
        self.links.clone_from(&source.links);
        self.groups.clone_from(&source.groups);
    }
}

#[derive(Clone, Default)]
struct ContextLink {
    context: FactContextHandle,
    next: Handle<ContextLink>,
}

#[derive(Clone, Default)]
struct ContextGroup {
    first: Handle<ContextLink>,
    last: Handle<ContextLink>,
}

/// A selected context chain in one fact plan or clones preserving that chain.
/// Like context handles, these are storage-local: selections acquired after
/// sibling plans diverge are not interchangeable between those plans.
/// Existing chains observe later appends. An empty selection stays empty;
/// it does not reserve a group for a point that has not been introduced yet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FactContextGroup {
    first: Handle<ContextLink>,
}

// Lookup storage is derived, not semantic identity or diagnostic output.
impl PartialEq for FactContexts {
    fn eq(&self, other: &Self) -> bool {
        self.contexts == other.contexts
    }
}

impl Eq for FactContexts {}

impl fmt::Debug for FactContexts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.contexts.fmt(formatter)
    }
}

impl FactContexts {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            contexts: Arena::with_capacity(capacity),
            links: Arena::with_capacity(capacity),
            groups: HashMap::new(),
        }
    }

    pub fn append(&mut self, context: FactContext) -> FactContextHandle {
        let point = context.point;
        let context = self.contexts.append(context);
        let link = self.links.append(ContextLink {
            context,
            next: Handle::invalid(),
        });
        let group = self.groups.entry(point).or_default();
        if group.last.is_valid() {
            self.links.get_mut(group.last).next = link;
        } else {
            group.first = link;
        }
        group.last = link;
        context
    }

    pub fn get(&self, context: FactContextHandle) -> &FactContext {
        self.contexts.get(context)
    }

    /// Freed handles remain absent from indexed lookup, just as from `iter`.
    /// Append never reuses their slots. Links remain valid for later appends.
    pub fn free(&mut self, context: FactContextHandle) -> bool {
        self.contexts.free(context)
    }

    pub fn len(&self) -> usize {
        self.contexts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.contexts.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (FactContextHandle, &FactContext)> {
        self.contexts.iter()
    }

    pub(in crate::fact_plan) fn handles_at_point(
        &self,
        point: ProgramPoint,
    ) -> impl Iterator<Item = FactContextHandle> + '_ {
        self.handles_in_group(self.group_at_point(point))
    }

    pub(in crate::fact_plan) fn group_at_point(&self, point: ProgramPoint) -> FactContextGroup {
        FactContextGroup {
            first: self
                .groups
                .get(&point)
                .map_or(Handle::invalid(), |group| group.first),
        }
    }

    pub(in crate::fact_plan) fn handles_in_group(
        &self,
        group: FactContextGroup,
    ) -> impl Iterator<Item = FactContextHandle> + '_ {
        let mut next = group.first;
        std::iter::from_fn(move || {
            while next.is_valid() {
                let link = self.links.get(next);
                next = link.next;
                if self.contexts.is_valid(link.context) {
                    return Some(link.context);
                }
            }
            None
        })
    }
}
