use crate::{Carry, Multiplicity};

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct DataProperties {
    pub multiplicity: Multiplicity,
    pub carry: Option<Carry>,
}
