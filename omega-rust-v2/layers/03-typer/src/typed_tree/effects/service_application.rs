use crate::ValueType;
use omega_resolver::DeclarationId;

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub struct ServiceApplication {
    pub service: DeclarationId,
    pub arguments: Vec<ValueType>,
}
