use crate::{Site, ValueType};
use omega_resolver::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct ConformanceRow {
    pub site: Site,
    pub conformance: DeclarationId,
    pub requirement: DeclarationId,
    pub implementation: DeclarationId,
    pub trait_arguments: Vec<ValueType>,
}
