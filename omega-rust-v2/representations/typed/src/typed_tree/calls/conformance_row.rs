use crate::{RequirementSlot, Site};
use omega_resolved::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct ConformanceRow {
    pub site: Site,
    pub node: omega_resolved::TreeNodeId,
    /// A named conformance owns a declaration row. An explicit binding expression owns its own selection.
    pub conformance: Option<DeclarationId>,
    pub requirement: RequirementSlot,
    pub implementation: DeclarationId,
}
