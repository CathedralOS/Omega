/// Handle-keyed maps take the integer hasher; none is iterated for output order.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;
use std::fmt;
use std::sync::Arc;

type Arena<T> = arena::PagedArenaSet<T>;

use super::{FactContext, ProgramPoint};
use crate::FactContextHandle;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod measurements;

/// Derived lookup storage, never fact identity. Dense group ordinals keep
/// prepared selections independent of the point map and stable across sealing.
#[derive(Clone, Default)]
struct ContextIndex {
    points: HashMap<ProgramPoint, FactContextGroup>,
    members: Vec<Vec<FactContextHandle>>,
    maximum_generation: u32,
}

/// Append-ordered contexts with a shared immutable index and local additions.
///
/// Flow groups read the same declaration baseline but append disjoint pages.
/// A linked index used to rewrite baseline tails on append and copy the entire
/// lookup map on every group reset. Sealing shares the baseline once; extensions
/// record only their own memberships without touching those shared rows.
///
/// Membership vectors are derived lookup storage, not a second fact graph.
/// They preserve per-point encounter order, including reverse page adoption;
/// hash iteration never selects fact order. Actual facts remain arena-owned.
#[derive(Default)]
pub struct FactContexts {
    contexts: Arena<FactContext>,
    baseline: Arc<ContextIndex>,
    groups: HashMap<ProgramPoint, FactContextGroup>,
    members: Vec<Vec<FactContextHandle>>,
    extensions: Vec<Vec<FactContextHandle>>,
}

impl Clone for FactContexts {
    fn clone(&self) -> Self {
        Self {
            contexts: self.contexts.clone(),
            baseline: self.baseline.clone(),
            groups: self.groups.clone(),
            members: self.members.clone(),
            extensions: self.extensions.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        // Restore the complete owner, including provisional point membership
        // and frees. A frozen source lends its pages and index, not its changes.
        self.contexts.clone_from(&source.contexts);
        self.baseline.clone_from(&source.baseline);
        self.groups.clone_from(&source.groups);
        self.members.clone_from(&source.members);
        self.extensions.clone_from(&source.extensions);
    }
}

/// One historically introduced group in this owner or matching clones.
/// Zero is absence: an empty selection cannot observe a later introduction.
/// Existing selections observe later appends, even after every old row is freed.
/// Sealing preserves ordinals; selections after sibling divergence are local
/// to that sibling, just like the contexts they name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FactContextGroup {
    ordinal: usize,
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
            ..Self::default()
        }
    }

    /// Publish current pages and index once before actual sibling fan-out.
    /// Later appends stay local; a subsequent seal retains existing selections
    /// and their complete encounter order rather than rebuilding from page order.
    pub fn share_pages(&mut self) {
        self.contexts.share_pages();
        if self.groups.is_empty() && self.extensions.iter().all(Vec::is_empty) {
            return;
        }
        let baseline = Arc::make_mut(&mut self.baseline);
        for (position, additions) in self.extensions.iter_mut().enumerate() {
            for handle in additions.iter() {
                baseline.maximum_generation = baseline.maximum_generation.max(handle.generation());
            }
            baseline.members[position].append(additions);
        }
        for group in &self.members {
            for handle in group {
                baseline.maximum_generation = baseline.maximum_generation.max(handle.generation());
            }
        }
        baseline.members.append(&mut self.members);
        baseline.points.extend(self.groups.drain());
        self.extensions.clear();
    }

    /// Append later contexts without rewriting any retained baseline row.
    pub fn begin_page(&mut self, generation: u32) {
        self.contexts.begin_page(generation);
    }

    /// Transfer later pages, retaining only first-page memberships in this owner.
    /// Historical group ordinals survive, but removed handles must not revive if
    /// this owner subsequently starts another page with the same generation.
    /// The returned page handoff has no index; adoption indexes its live rows.
    pub fn take_additional_pages(&mut self) -> Self {
        let first_generation = self.contexts.first_generation();
        let contexts = self.contexts.take_additional_pages();
        if self.baseline.maximum_generation > first_generation {
            let baseline = Arc::make_mut(&mut self.baseline);
            for members in &mut baseline.members {
                members.retain(|handle| handle.generation() == first_generation);
            }
            baseline.maximum_generation = if baseline.members.iter().all(Vec::is_empty) {
                0
            } else {
                first_generation
            };
        }
        for members in self.members.iter_mut().chain(&mut self.extensions) {
            members.retain(|handle| handle.generation() == first_generation);
        }
        Self {
            contexts,
            ..Self::default()
        }
    }

    /// Adopt taken rows under their minted handles. Each adoption encounters
    /// pages in generation order; separate adoptions retain their call order,
    /// including when multiple workers extend the same point.
    pub fn adopt_additional_pages(&mut self, other: Self) {
        let adopted = other
            .contexts
            .iter()
            .map(|(handle, context)| (handle, context.point))
            .collect::<Vec<_>>();
        self.contexts.adopt_additional_pages(other.contexts);
        for (context, point) in adopted {
            self.index_context(context, point);
        }
    }

    fn index_context(&mut self, context: FactContextHandle, point: ProgramPoint) {
        if let Some(group) = self.baseline.points.get(&point) {
            let position = group.ordinal - 1;
            if self.extensions.len() <= position {
                self.extensions.resize_with(position + 1, Vec::new);
            }
            self.extensions[position].push(context);
        } else if let Some(group) = self.groups.get(&point) {
            let position = group.ordinal - self.baseline.members.len() - 1;
            self.members[position].push(context);
        } else {
            let ordinal = self.baseline.members.len() + self.members.len() + 1;
            self.groups.insert(point, FactContextGroup { ordinal });
            self.members.push(vec![context]);
        }
    }

    pub fn append(&mut self, context: FactContext) -> FactContextHandle {
        let point = context.point;
        let context = self.contexts.append(context);
        self.index_context(context, point);
        context
    }

    #[inline]
    pub fn get(&self, context: FactContextHandle) -> &FactContext {
        self.contexts.get(context)
    }

    /// Freeing changes this owner's arena, not shared membership history.
    /// Slot reuse is forbidden; lookup filters against this owner's live rows.
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
        self.baseline
            .points
            .get(&point)
            .or_else(|| self.groups.get(&point))
            .copied()
            .unwrap_or_default()
    }

    pub(in crate::fact_plan) fn handles_in_group(
        &self,
        group: FactContextGroup,
    ) -> impl Iterator<Item = FactContextHandle> + '_ {
        let empty: &[FactContextHandle] = &[];
        let (baseline, additions) = match group.ordinal.checked_sub(1) {
            Some(position) if position < self.baseline.members.len() => (
                self.baseline.members[position].as_slice(),
                self.extensions.get(position).map_or(empty, Vec::as_slice),
            ),
            Some(position) => (
                empty,
                self.members
                    .get(position - self.baseline.members.len())
                    .map_or(empty, Vec::as_slice),
            ),
            None => (empty, empty),
        };
        baseline
            .iter()
            .chain(additions)
            .copied()
            .filter(|context| self.contexts.is_valid(*context))
    }
}
