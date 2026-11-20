use crate::ValueType;
use omega_resolved::DeclarationId;

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub struct ServiceApplication {
    pub service: DeclarationId,
    pub arguments: Vec<ValueType>,
}
