use crate::{Site, ValueType};

#[derive(Debug, Eq, PartialEq)]
pub struct TypedExpression {
    pub node: omega_resolver::TreeNodeId,
    pub site: Site,
    pub ty: ValueType,
}
