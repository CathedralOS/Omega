use crate::{MachineApplication, RequirementSlot, Site};
use omega_resolved::DeclarationId;

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub enum InvocationTarget {
    Declaration(RequirementSlot),
    AppliedMachine(MachineApplication),
    RuntimeParameter { owner: DeclarationId, index: usize },
    StaticParameter { owner: DeclarationId, index: usize },
    StoredCallable(Site),
}
