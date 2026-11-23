//! Check each declaration once its direct dependencies have been checked.
use crate::TypeError;
use std::collections::{HashSet, VecDeque};

pub(super) fn check_in_dependency_order(
    dependencies: impl ExactSizeIterator<Item = HashSet<usize>>,
    mut check: impl FnMut(usize) -> Result<(), TypeError>,
    cycle: impl FnOnce(usize) -> TypeError,
) -> Result<(), TypeError> {
    let mut users = vec![Vec::new(); dependencies.len()];
    let mut remaining = Vec::with_capacity(users.len());
    let mut ready = VecDeque::new();
    for (user, parents) in dependencies.enumerate() {
        remaining.push(parents.len());
        if parents.is_empty() {
            ready.push_back(user);
        }
        for parent in parents {
            users[parent].push(user);
        }
    }
    while let Some(index) = ready.pop_front() {
        check(index)?;
        for &user in &users[index] {
            remaining[user] -= 1;
            if remaining[user] == 0 {
                ready.push_back(user);
            }
        }
    }
    match remaining.iter().position(|&count| count != 0) {
        Some(index) => Err(cycle(index)),
        None => Ok(()),
    }
}
