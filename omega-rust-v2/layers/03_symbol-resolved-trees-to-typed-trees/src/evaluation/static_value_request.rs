use crate::{Site, ValueType};
use omega_resolver::{IndexedSource, TreeNodeId};
/// A source occurrence in the current admitted stage that needs a static value from evaluation.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StaticValueRequest {
    /// Exact occurrence in the resolved forest shared by typing and preparation.
    pub node: TreeNodeId,
    pub expression: Site,
    pub carrier: ValueType,
}

impl StaticValueRequest {
    pub(crate) fn at(
        source: &IndexedSource<'_>,
        node: usize,
        carrier: &ValueType,
    ) -> Self {
        Self {
            node: source.node_id(node),
            expression: Site::at(source, node),
            carrier: carrier.clone(),
        }
    }
}
