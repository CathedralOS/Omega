use super::specialized_indices::{ClosedArguments, SyntaxKey};
use crate::{IntegerIndex, StaticValue, StaticValueDemand, StaticValueRequest, ValueType};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

pub struct StaticTemplate {
    pub(super) key: SyntaxKey,
    pub(super) request: StaticValueRequest,
    pub(super) allow_pending: bool,
    pub(super) values: Arc<HashMap<ClosedArguments, StaticValue>>,
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
enum Application {
    Open(Vec<ValueType>),
    Ready(StaticValue),
    Pending(Arc<StaticValueDemand>),
    Missing(String),
}

impl StaticTemplate {
    /// Identify the typed occurrence and carrier when returning a prepared value.
    pub fn request(&self) -> StaticValueRequest {
        self.request.clone()
    }

    fn resolve(
        &self,
        arguments: Vec<ValueType>,
    ) -> Application {
        let arguments = match ClosedArguments::try_from(arguments) {
            Ok(arguments) => arguments,
            Err(arguments) => return Application::Open(arguments),
        };
        match self.values.get(&arguments) {
            Some(value) => Application::Ready(value.clone()),
            None if self.allow_pending => Application::Pending(Arc::new(StaticValueDemand::Specialized {
                request: self.request(),
                arguments: arguments.into_values(),
            })),
            None => Application::Missing(format!(
                "specialized static expression in {} needs an admitted evaluation result",
                self.key.owner.path
            )),
        }
    }

    pub(crate) fn apply_integer(
        self: &Arc<Self>,
        arguments: Vec<ValueType>,
    ) -> IntegerIndex {
        match self.resolve(arguments) {
            Application::Open(arguments) => IntegerIndex::Deferred {
                template: self.clone(),
                arguments,
            },
            Application::Ready(StaticValue::Integer(value)) if self.request.carrier.contains_integer(&value) => IntegerIndex::Value(value),
            Application::Ready(StaticValue::Integer(value)) => IntegerIndex::Invalid(format!("static index {value} exceeds {}", self.request.carrier)),
            Application::Ready(_) => IntegerIndex::Invalid("specialized evaluation result does not match its integer carrier".into()),
            Application::Pending(demand) => IntegerIndex::Pending(demand),
            Application::Missing(message) => IntegerIndex::Invalid(message),
        }
    }

    pub(crate) fn apply_value(
        self: &Arc<Self>,
        arguments: Vec<ValueType>,
    ) -> ValueType {
        match self.resolve(arguments) {
            Application::Open(arguments) => ValueType::OpenStatic {
                template: self.clone(),
                arguments,
            },
            Application::Ready(value @ StaticValue::Bool(_)) if self.request.carrier == ValueType::Bool => ValueType::StaticValue {
                carrier: Box::new(self.request.carrier.clone()),
                value,
            },
            Application::Ready(_) => ValueType::InvalidStatic("specialized evaluation result does not match its carrier".into()),
            Application::Pending(demand) => ValueType::PendingStatic(demand),
            Application::Missing(message) => ValueType::InvalidStatic(message),
        }
    }
}
