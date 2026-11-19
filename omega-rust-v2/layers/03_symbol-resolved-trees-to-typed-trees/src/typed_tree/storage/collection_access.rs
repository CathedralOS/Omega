use crate::{Site, ValueType};
use omega_resolver::{DeclarationId, TreeNodeId};

#[derive(Debug, Eq, PartialEq)]
pub struct CollectionAccess {
    pub site: Site,
    /// The index expression retains its receiver and written selector in the syntax tree.
    pub node: TreeNodeId,
    pub machine: DeclarationId,
    pub element: ValueType,
    pub result: ValueType,
}
