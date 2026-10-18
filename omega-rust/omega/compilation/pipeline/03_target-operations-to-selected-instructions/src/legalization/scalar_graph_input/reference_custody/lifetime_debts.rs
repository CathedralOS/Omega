//! Foreign reference validity is independent of where its pointer points.
//! Capture anchors before completion replaces slots. Copies keep their debts;
//! reborrows inherit through authenticated spatial ancestry, including retired
//! occurrences. Anchor links retain history but never authorize traversal into
//! an anchor as though it contained the provider's referent.
use super::{Custody, Leaf, OccurrenceIdentity, StructuralAccess, descriptor_backing};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Debt {
    pub(super) backing: Vec<descriptor_backing::Backing>,
    pub(super) access: StructuralAccess,
    pub(super) parents: Vec<OccurrenceIdentity>,
}

impl Leaf {
    /// Both sorts of link keep their exact generation alive. Only `parents`
    /// describe a spatial reborrow and may authorize parent traversal.
    pub(super) fn obligations(&self) -> impl Iterator<Item = &OccurrenceIdentity> {
        self.parents
            .iter()
            .chain(self.lifetime_debts.iter().flat_map(|debt| &debt.parents))
    }
}

impl Custody {
    fn occurrence(&self, identity: &OccurrenceIdentity) -> Option<&Leaf> {
        self.leaves
            .values()
            .find(|leaf| &leaf.identity == identity)
            .or_else(|| self.retired.get(identity).map(|retired| &retired.leaf))
    }

    /// Inherit validity through spatial parents without rewriting it to the
    /// child's access. A shared reborrow does not weaken an exclusive debt.
    pub(super) fn captured_lifetime_debts<'a>(&'a self, leaf: &'a Leaf) -> Vec<&'a Debt> {
        let mut pending = vec![leaf];
        let mut visited = BTreeSet::new();
        let mut debts = Vec::new();
        while let Some(current) = pending.pop() {
            if !visited.insert(&current.identity) {
                continue;
            }
            debts.extend(&current.lifetime_debts);
            pending.extend(
                current
                    .obligations()
                    .filter_map(|parent| self.occurrence(parent)),
            );
        }
        debts
    }

    pub(super) fn lifetime_depends_on(&self, leaf: &Leaf, ancestor: &OccurrenceIdentity) -> bool {
        self.lifetime_access_conflicts(leaf, ancestor, StructuralAccess::Owned)
    }

    pub(super) fn lifetime_access_conflicts(
        &self,
        leaf: &Leaf,
        ancestor: &OccurrenceIdentity,
        requested: StructuralAccess,
    ) -> bool {
        self.captured_lifetime_debts(leaf).iter().any(|debt| {
            (requested != StructuralAccess::SharedBorrow
                || debt.access != StructuralAccess::SharedBorrow)
                && debt.parents.iter().any(|parent| {
                    parent == ancestor
                        || self
                            .occurrence(parent)
                            .is_some_and(|parent| self.descends_from(parent, ancestor))
                })
        })
    }
}
