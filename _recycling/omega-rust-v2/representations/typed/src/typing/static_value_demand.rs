use crate::{StaticValueRequest, ValueType};

/// A typed occurrence whose value is needed to finish typing.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum StaticValueDemand {
    Closed(StaticValueRequest),
    Specialized { request: StaticValueRequest, arguments: Vec<ValueType> },
}
impl StaticValueDemand {
    pub fn request(&self) -> &StaticValueRequest {
        match self {
            Self::Closed(request) | Self::Specialized { request, .. } => request,
        }
    }
}
