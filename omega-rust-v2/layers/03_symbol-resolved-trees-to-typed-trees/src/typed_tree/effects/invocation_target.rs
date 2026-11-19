use crate::{RequirementSlot, Site, ValueType};
use omega_resolver::DeclarationId;

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub enum InvocationTarget {
    Declaration(RequirementSlot),
    AppliedMachine { declaration: DeclarationId, arguments: Vec<ValueType> },
    RuntimeParameter { owner: DeclarationId, index: usize },
    StaticParameter { owner: DeclarationId, index: usize },
    StoredCallable(Site),
}
