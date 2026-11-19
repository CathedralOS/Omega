use crate::{Site, ValueType};

#[derive(Debug, Eq, PartialEq)]
pub struct TypedExpression {
    pub site: Site,
    pub ty: ValueType,
}
