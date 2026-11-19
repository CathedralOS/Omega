use crate::{RequirementSlot, Site};
use omega_resolver::DeclarationId;

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub enum InvocationTarget {
    Declaration(RequirementSlot),
    RuntimeParameter { owner: DeclarationId, index: usize },
    StaticParameter { owner: DeclarationId, index: usize },
    StoredCallable(Site),
}
