use crate::{RequirementSlot, SlotDirection, ValueType};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundarySlot {
    pub requirement: RequirementSlot,
    pub arguments: Vec<ValueType>,
    pub direction: SlotDirection,
}
