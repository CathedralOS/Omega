//! Resolve finite service unions after callers have named exact dependency rows.
//! Selection and authority stay with the caller; an unresolved row or cycle
//! never supplies an empty service set.

pub struct ReachRow<T> {
    pub fixed: Vec<T>,
    pub dependencies: Vec<Option<usize>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReachResolutionError {
    InvalidDependency { row: usize, dependency: usize },
    Unresolved(Vec<(usize, usize)>),
}

/// Keep each result in its input position. A row is canonicalized once, after
/// its children complete, and its contribution is copied only to its parents.
pub fn resolve<T: Ord + Clone>(
    mut rows: Vec<ReachRow<T>>,
) -> Result<Vec<Vec<T>>, ReachResolutionError> {
    let count = rows.len();
    let mut parents = vec![Vec::new(); count];
    let mut remaining = Vec::with_capacity(count);
    let mut ready = std::collections::VecDeque::with_capacity(count);
    for (position, row) in rows.iter_mut().enumerate() {
        row.dependencies.sort_unstable();
        row.dependencies
            .dedup_by(|left, right| left.is_some() && left == right);
        for dependency in row.dependencies.iter().flatten().copied() {
            let Some(parent_rows) = parents.get_mut(dependency) else {
                return Err(ReachResolutionError::InvalidDependency {
                    row: position,
                    dependency,
                });
            };
            parent_rows.push(position);
        }
        remaining.push(row.dependencies.len());
        if row.dependencies.is_empty() {
            ready.push_back(position);
        }
    }
    let mut completed = 0;
    while let Some(position) = ready.pop_front() {
        let mut resolved = std::mem::take(&mut rows[position].fixed);
        resolved.sort_unstable();
        resolved.dedup();
        for &parent in &parents[position] {
            rows[parent].fixed.extend(resolved.iter().cloned());
            remaining[parent] -= 1;
            if remaining[parent] == 0 {
                ready.push_back(parent);
            }
        }
        rows[position].fixed = resolved;
        completed += 1;
    }
    if completed != count {
        return Err(ReachResolutionError::Unresolved(
            remaining
                .iter()
                .enumerate()
                .filter_map(|(position, &remaining)| {
                    (remaining != 0).then_some((position, remaining))
                })
                .collect(),
        ));
    }
    Ok(rows.into_iter().map(|row| row.fixed).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(fixed: &[u32], dependencies: &[Option<usize>]) -> ReachRow<u32> {
        ReachRow {
            fixed: fixed.to_vec(),
            dependencies: dependencies.to_vec(),
        }
    }

    #[test]
    fn staggered_children_keep_the_earlier_contribution() {
        let result = resolve(vec![
            row(&[], &[Some(1), Some(2)]),
            row(&[11], &[]),
            row(&[], &[Some(3)]),
            row(&[22], &[]),
        ])
        .unwrap();
        assert_eq!(result, [vec![11, 22], vec![11], vec![22], vec![22]]);
    }

    #[test]
    fn shared_children_and_duplicate_edges_do_not_drop_or_repeat_services() {
        let result = resolve(vec![
            row(&[3], &[Some(1), Some(2), Some(1)]),
            row(&[1], &[Some(3)]),
            row(&[2], &[Some(3)]),
            row(&[1, 1], &[]),
        ])
        .unwrap();
        assert_eq!(result[0], [1, 2, 3]);
    }

    #[test]
    fn cycles_and_missing_children_remain_unresolved() {
        for rows in [
            vec![row(&[], &[Some(1)]), row(&[], &[Some(0)])],
            vec![row(&[], &[None]), row(&[1], &[])],
        ] {
            assert!(matches!(
                resolve(rows),
                Err(ReachResolutionError::Unresolved(_))
            ));
        }
        assert_eq!(
            resolve(vec![row(&[], &[Some(1)])]),
            Err(ReachResolutionError::InvalidDependency {
                row: 0,
                dependency: 1
            })
        );
    }
}
