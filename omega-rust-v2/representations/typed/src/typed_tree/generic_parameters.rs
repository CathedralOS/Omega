use crate::ValueType;
use omega_syntax::NodeKind;

/// A specialization supplies the public parameters in order. Existential witnesses are private to the data value.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GenericParameters {
    pub parameters: Vec<GenericParameter>,
    pub witness_count: usize,
    pub equations: Vec<(usize, ValueType)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericParameter {
    pub kind: NodeKind,
    pub multiplicity: Option<crate::Multiplicity>,
    pub carrier: Option<Box<ValueType>>,
}
