use crate::{CallUse, PauseEffects, Site, ValueType};
use omega_resolver::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct TypedCall {
    pub node: omega_resolver::TreeNodeId,
    pub use_site: CallUse,
    pub site: Site,
    pub machine: DeclarationId,
    pub type_arguments: Vec<ValueType>,
    pub operands: Vec<omega_resolver::TreeNodeId>,
    pub enclosing_arguments: Vec<(DeclarationId, Vec<ValueType>)>,
    pub caller: Option<DeclarationId>,
    pub acknowledgements: PauseEffects,
    pub evidence: Option<ValueType>,
    pub result: ValueType,
}
