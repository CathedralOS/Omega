use crate::{Site, ValueType};
use omega_resolver::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct AccessPolicySelection {
    pub site: Site,
    pub domain: DeclarationId,
    pub policy: DeclarationId,
    pub arguments: Vec<ValueType>,
    pub plan_machine: DeclarationId,
}
