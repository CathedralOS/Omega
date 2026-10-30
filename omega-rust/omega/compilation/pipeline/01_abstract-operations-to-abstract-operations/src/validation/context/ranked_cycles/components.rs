//! Optimizer module role: algorithm leaf. Cyclic components of the current
//! canonical body graph.

use super::super::super::BTreeMap;
use semantic_vocabulary::BlockId;

use crate::cycles::strongly_connected_components;

pub(super) fn cyclic_components(graph: &BTreeMap<BlockId, Vec<BlockId>>) -> Vec<Vec<BlockId>> {
    strongly_connected_components(graph)
        .into_iter()
        .filter(|component| {
            component.len() > 1
                || component
                    .first()
                    .is_some_and(|block| graph[block].contains(block))
        })
        .collect()
}
