use crate::{Site, ValueType};
use omega_program_contracts::CollectionPrimitive;
use omega_resolved::{DeclarationId, TreeNodeId};

#[derive(Debug, Eq, PartialEq)]
pub struct CollectionAccess {
    pub site: Site,
    /// The index expression retains its receiver and written selector in the syntax tree.
    pub node: TreeNodeId,
    pub machine: DeclarationId,
    pub primitive: CollectionPrimitive,
    /// Formal index or range-bound carriers in selector order.
    pub selector_carriers: Vec<ValueType>,
    pub element: ValueType,
    pub result: ValueType,
}
