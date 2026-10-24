/// Handle-keyed maps take the integer hasher; none is iterated for output order.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;
use std::fmt;

use arena::Handle;
type Arena<T> = arena::PagedArenaSet<T>;

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
            groups: HashMap::default(),
        }
    }

    /// Share the completed context and link pages before independent forks.
    /// Each fork still owns its point groups. Extending an existing group
    /// detaches its old tail link before changing that fork's chain.
    pub(in crate::fact_plan) fn share_pages(&mut self) {
        let Self {
            contexts,
            links,
            groups: _,
        } = self;
        contexts.share_pages();
        links.share_pages();
    }

    /// Append later contexts and their links on page `generation`.
    pub fn begin_page(&mut self, generation: u32) {
        self.contexts.begin_page(generation);
        self.links.begin_page(generation);
    }

    /// The contexts written after the first page. Links and point groups are
    /// derived and may have rewritten a baseline tail, so they stay behind;
    /// adoption relinks the taken contexts instead of copying them.
    pub fn take_additional_pages(&mut self) -> Self {
        let contexts = self.contexts.take_additional_pages();
        self.links.take_additional_pages();
        Self {
            contexts,
            links: Arena::default(),
            groups: HashMap::default(),
        }
    }

    /// Adopt taken contexts under their minted handles, then link each one
    /// into its point group on its own page, exactly as `append` would have.
    pub fn adopt_additional_pages(&mut self, other: Self) {
        let adopted = other
            .contexts
            .iter()
            .map(|(handle, context)| (handle, context.point))
            .collect::<Vec<_>>();
        self.contexts.adopt_additional_pages(other.contexts);
        let mut page = 0;
        for (context, point) in adopted {
            if context.generation() != page {
                page = context.generation();
                self.links.begin_page(page);
            }
            self.link(context, point);
        }
    }

    fn link(&mut self, context: FactContextHandle, point: ProgramPoint) {
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
    }

    pub fn append(&mut self, context: FactContext) -> FactContextHandle {
        let point = context.point;
        let context = self.contexts.append(context);
        self.link(context, point);
        context
    }

    #[inline]
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
