use crate::{Site, ValueType};

#[derive(Debug, Eq, PartialEq)]
pub struct TypedExpression {
    pub node: omega_resolver::TreeNodeId,
    pub site: Site,
    pub ty: ValueType,
}

/// An exact numeric value after the selected destination carrier is checked.
#[derive(Debug, Eq, PartialEq)]
pub struct TypedNumericLanding {
    pub node: omega_resolver::TreeNodeId,
    pub site: Site,
    pub value: num_rational::BigRational,
    pub ty: ValueType,
}

impl TypedExpression {
    pub(crate) fn at(
        source: &omega_resolver::IndexedSource<'_>,
        index: usize,
        ty: ValueType,
    ) -> Self {
        Self {
            node: source.node_id(index),
            site: Site::at(source, index),
            ty,
        }
    }
}
