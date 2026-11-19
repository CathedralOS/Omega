use crate::{DomainApplication, Site, ValueType};

#[derive(Debug, Eq, PartialEq)]
pub struct TypedMembership {
    pub site: Site,
    pub subject: ValueType,
    pub domain: DomainApplication,
}
