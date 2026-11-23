use crate::{CallUse, CallableType, InvocationTarget, Site, ValueType};
use omega_resolved::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct IndirectOperand {
    pub node: omega_resolved::TreeNodeId,
    pub site: Site,
    pub subject: ValueType,
    pub callable: Option<InvocationTarget>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct IndirectCall {
    pub node: omega_resolved::TreeNodeId,
    pub use_site: CallUse,
    pub operands: Vec<IndirectOperand>,
    pub entry: InvocationTarget,
    pub acknowledgements: crate::PauseEffects,
    pub site: Site,
    pub callee: Site,
    pub caller: Option<DeclarationId>,
    pub contract: CallableType,
}
