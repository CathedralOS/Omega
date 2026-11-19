use super::specialized_indices::{ClosedArguments, SyntaxKey};
use crate::{IntegerIndex, StaticIntegerRequest, ValueType};
use num_bigint::BigInt;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

pub struct StaticTemplate {
    pub(super) key: SyntaxKey,
    pub(super) request: StaticIntegerRequest,
    pub(super) values: Arc<HashMap<ClosedArguments, BigInt>>,
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
        self.key == other.key && self.request.carrier == other.request.carrier
    }
}
impl Eq for StaticTemplate {}
impl Hash for StaticTemplate {
    fn hash<H: Hasher>(
        &self,
        h: &mut H,
    ) {
        self.key.hash(h);
        self.request.carrier.hash(h)
    }
}
impl StaticTemplate {
    /// Identify the typed occurrence and carrier when returning a prepared value.
    pub fn request(&self) -> StaticIntegerRequest {
        self.request.clone()
    }

    pub(crate) fn apply(
        self: &Arc<Self>,
        arguments: Vec<ValueType>,
    ) -> IntegerIndex {
        let arguments = match ClosedArguments::try_from(arguments) {
            Ok(arguments) => arguments,
            Err(arguments) => {
                return IntegerIndex::Deferred {
                    template: self.clone(),
                    arguments,
                };
            }
        };
        match self.values.get(&arguments) {
            Some(v) if self.request.carrier.contains_integer(v) => IntegerIndex::Value(v.clone()),
            Some(v) => IntegerIndex::Invalid(format!("static index {v} exceeds {}", self.request.carrier)),
            None => IntegerIndex::Invalid(format!(
                "specialized static expression in {} needs an admitted evaluation result",
                self.key.owner.path
            )),
        }
    }
}
