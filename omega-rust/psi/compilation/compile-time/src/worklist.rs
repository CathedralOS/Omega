//! Worklist draining shared by the reachability walks.
//!
//! Every walk that visits a frontier once keeps its own `visited` rows so a
//! symbol, expression or type reference reached twice is expanded once. The
//! frontier and the visited rows stay caller-owned: a walk may extend its
//! frontier while this drains it.

/// The next frontier item not yet visited, recorded as visited.
pub(crate) fn unvisited<T: PartialEq + Copy>(
    frontier: &mut Vec<T>,
    visited: &mut Vec<T>,
) -> Option<T> {
    while let Some(item) = frontier.pop() {
        if !visited.contains(&item) {
            visited.push(item);
            return Some(item);
        }
    }
    None
}
