use crate::{DomainApplication, Site, ValueType};

#[derive(Debug, Eq, PartialEq)]
pub struct TypedMembership {
    pub node: omega_resolved::TreeNodeId,
    pub site: Site,
    pub subject: ValueType,
    pub domain: DomainApplication,
}
