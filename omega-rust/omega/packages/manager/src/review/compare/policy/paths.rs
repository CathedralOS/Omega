use super::{
    PackagePolicyChangeError as Error, PackagePolicyDependencyPath,
    PackagePolicyDependencyPathStep, limits::Budget,
};
use crate::declarations::PackageKey;
use crate::resolution::graph::CanonicalSourceClosureSubject;

/// First canonical breadth-first path. Diamond edges remain bound by the full
/// source subject even though the diagnostic path chooses one occurrence.
pub(super) struct Paths<'a> {
    subject: &'a CanonicalSourceClosureSubject,
    root: usize,
    parents: Vec<Option<usize>>,
}
impl<'a> Paths<'a> {
    pub(super) fn new(
        subject: &'a CanonicalSourceClosureSubject,
        budget: &mut Budget,
    ) -> Result<Self, Error> {
        let packages = subject.packages();
        let root_key = subject.root().selected().key();
        let root = packages
            .binary_search_by(|package| package.key().cmp(root_key))
            .map_err(|_| Error::InvalidSourcePath {
                package: Box::new(root_key.clone()),
            })?;
        budget.slots::<Option<usize>>(packages.len())?;
        budget.slots::<usize>(packages.len())?;
        let mut parents = Vec::new();
        parents
            .try_reserve_exact(packages.len())
            .map_err(|_| Error::AllocationFailed)?;
        parents.resize(packages.len(), None);
        let mut queue = Vec::new();
        queue
            .try_reserve_exact(packages.len())
            .map_err(|_| Error::AllocationFailed)?;
        queue.push(root);
        let mut position = 0;
        let edges = subject.dependency_requests();
        while position < queue.len() {
            let requester = packages[queue[position]].key();
            position += 1;
            let first = edges.partition_point(|edge| edge.requester() < requester);
            for (index, edge) in edges
                .iter()
                .enumerate()
                .skip(first)
                .take_while(|(_, edge)| edge.requester() == requester)
            {
                let selected = packages
                    .binary_search_by(|package| package.key().cmp(edge.selected().key()))
                    .map_err(|_| Error::InvalidSourcePath {
                        package: Box::new(edge.selected().key().clone()),
                    })?;
                if selected != root && parents[selected].is_none() {
                    parents[selected] = Some(index);
                    queue.push(selected);
                }
            }
        }
        Ok(Self {
            subject,
            root,
            parents,
        })
    }

    pub(super) fn path(
        &self,
        key: &PackageKey,
        budget: &mut Budget,
    ) -> Result<PackagePolicyDependencyPath, Error> {
        let packages = self.subject.packages();
        let mut current = packages
            .binary_search_by(|package| package.key().cmp(key))
            .map_err(|_| Error::InvalidSourcePath {
                package: Box::new(key.clone()),
            })?;
        let start = current;
        let mut count = 0;
        while current != self.root {
            count += 1;
            if count > budget.limits.maximum_dependency_path_steps {
                return Err(Error::LimitExceeded {
                    resource: "dependency path steps",
                    maximum: budget.limits.maximum_dependency_path_steps,
                });
            }
            if count > packages.len() {
                return Err(Error::InvalidSourcePath {
                    package: Box::new(key.clone()),
                });
            }
            let edge = self.edge(current, key)?;
            budget.context(edge.alias().as_str().len())?;
            current = packages
                .binary_search_by(|package| package.key().cmp(edge.requester()))
                .map_err(|_| Error::InvalidSourcePath {
                    package: Box::new(key.clone()),
                })?;
        }
        budget.slots::<PackagePolicyDependencyPathStep>(count)?;
        let mut steps = Vec::new();
        steps
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        current = start;
        while current != self.root {
            let edge = self.edge(current, key)?;
            let mut alias = String::new();
            alias
                .try_reserve_exact(edge.alias().as_str().len())
                .map_err(|_| Error::AllocationFailed)?;
            alias.push_str(edge.alias().as_str());
            steps.push(PackagePolicyDependencyPathStep {
                requester: edge.requester().identity(),
                dependency_index: edge.dependency_index(),
                alias,
                target: edge.selected().key().identity(),
            });
            current = packages
                .binary_search_by(|package| package.key().cmp(edge.requester()))
                .map_err(|_| Error::InvalidSourcePath {
                    package: Box::new(key.clone()),
                })?;
        }
        steps.reverse();
        Ok(PackagePolicyDependencyPath {
            root: packages[self.root].key().identity(),
            steps,
        })
    }

    fn edge(
        &self,
        index: usize,
        key: &PackageKey,
    ) -> Result<&crate::resolution::graph::CanonicalDependencySourceSelection, Error> {
        self.parents[index]
            .and_then(|edge| self.subject.dependency_requests().get(edge))
            .ok_or_else(|| Error::InvalidSourcePath {
                package: Box::new(key.clone()),
            })
    }
}
