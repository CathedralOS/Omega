use crate::{Site, ValueType};

#[derive(Debug, Eq, PartialEq)]
pub struct PackageConstruction {
    pub site: Site,
    pub carrier: ValueType,
    pub witnesses: Vec<ValueType>,
}
