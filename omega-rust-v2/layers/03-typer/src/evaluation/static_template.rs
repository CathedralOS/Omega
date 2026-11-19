use super::integer_arithmetic;
use super::specialized_indices::SyntaxKey;
use crate::{IntegerIndex, Site, ValueType};
use num_bigint::BigInt;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

pub struct StaticTemplate {
    pub(super) key: SyntaxKey,
    pub site: Site,
    pub(super) carrier: ValueType,
    pub(super) values: Arc<HashMap<Vec<ValueType>, BigInt>>,
}
impl std::fmt::Debug for StaticTemplate {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        self.key.fmt(f)
    }
}
impl PartialEq for StaticTemplate {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.key == other.key && self.carrier == other.carrier
    }
}
impl Eq for StaticTemplate {}
impl Hash for StaticTemplate {
    fn hash<H: Hasher>(
        &self,
        h: &mut H,
    ) {
        self.key.hash(h);
        self.carrier.hash(h)
    }
}
impl StaticTemplate {
    pub(crate) fn apply(
        self: &Arc<Self>,
        arguments: Vec<ValueType>,
    ) -> IntegerIndex {
        if arguments.iter().any(ValueType::contains_parameter) {
            return IntegerIndex::Deferred {
                template: self.clone(),
                arguments,
            };
        }
        match self.values.get(&arguments) {
            Some(v) if integer_arithmetic::fits(v, &self.carrier) => IntegerIndex::Value(v.clone()),
            Some(v) => IntegerIndex::Invalid(format!("static index {v} exceeds {}", self.carrier)),
            None => IntegerIndex::Invalid(format!(
                "specialized static expression in {} needs an admitted evaluation result",
                self.key.owner.path
            )),
        }
    }
}
