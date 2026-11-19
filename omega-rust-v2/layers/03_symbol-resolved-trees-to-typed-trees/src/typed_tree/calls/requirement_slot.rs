use crate::ValueType;
use omega_resolver::DeclarationId;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RequirementSlot {
    pub declaration: DeclarationId,
    pub trait_arguments: Vec<ValueType>,
}
