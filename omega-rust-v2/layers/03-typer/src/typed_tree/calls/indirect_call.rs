use crate::{CallUse, CallableType, InvocationTarget, Site, ValueType};
use omega_resolver::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct IndirectCall {
    pub use_site: CallUse,
    pub operands: Vec<Site>,
    pub entry: InvocationTarget,
    pub site: Site,
    pub callee: Site,
    pub caller: Option<DeclarationId>,
    pub contract: CallableType,
    pub binder: Option<ValueType>,
    pub guarantee: bool,
}
