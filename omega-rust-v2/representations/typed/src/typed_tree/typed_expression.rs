use crate::typed_tree::site::site_at;
use crate::{Site, ValueType};

#[derive(Debug, Eq, PartialEq)]
pub struct TypedExpression {
    pub node: omega_resolved::TreeNodeId,
    pub site: Site,
    pub ty: ValueType,
    /// Position of the selected binder in the ordered arguments of the specialization owner.
    pub static_argument_slot: Option<usize>,
}

/// An exact numeric value after the selected destination carrier is checked.
#[derive(Debug, Eq, PartialEq)]
pub struct TypedNumericLanding {
    pub node: omega_resolved::TreeNodeId,
    pub site: Site,
    pub value: num_rational::BigRational,
    pub ty: ValueType,
}

impl TypedExpression {
    pub fn at(
        source: &omega_resolved::IndexedSource<'_>,
        index: usize,
        ty: ValueType,
    ) -> Self {
        Self {
            node: source.node_id(index),
            site: site_at(source, index),
            ty,
            static_argument_slot: None,
        }
    }
}
