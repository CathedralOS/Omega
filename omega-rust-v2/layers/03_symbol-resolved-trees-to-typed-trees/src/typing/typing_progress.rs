use crate::{StaticValueRequest, TypedTrees, ValueType};

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

#[derive(Debug)]
pub enum TypingProgress<'text> {
    Complete(TypedTrees<'text>),
    Pending(PendingTyping<'text>),
}

/// The typed facts are provisional until the selected static expressions are evaluated.
#[derive(Debug)]
pub struct PendingTyping<'text> {
    pub(crate) trees: TypedTrees<'text>,
    pub(crate) demands: Vec<StaticValueDemand>,
}
impl<'text> PendingTyping<'text> {
    pub fn trees(&self) -> &TypedTrees<'text> {
        &self.trees
    }
    pub fn demands(&self) -> &[StaticValueDemand] {
        &self.demands
    }
}
