use crate::{Site, ValueType};
use omega_resolver::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct CleanupSelection {
    pub site: Site,
    pub owner: DeclarationId,
    pub machine: DeclarationId,
    pub arguments: Vec<ValueType>,
}
