use crate::{CrashCause, InvocationTarget, Site, ValueType};
use omega_resolver::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct GuardedCall {
    pub call: Site,
    pub specialization: Option<Site>,
    pub caller: DeclarationId,
    pub guard_contract: Site,
    pub guard: Option<Site>,
    pub cause: CrashCause,
    pub operands: Vec<Site>,
    pub generic_arguments: Vec<ValueType>,
    pub entry: Option<InvocationTarget>,
}
