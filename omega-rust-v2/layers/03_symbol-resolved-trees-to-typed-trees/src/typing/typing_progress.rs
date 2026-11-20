use crate::{StaticValueDemand, TypedTrees};

#[derive(Debug)]
pub enum TypingProgress<'text> {
    Complete(TypedTrees<'text>),
    Pending(PendingTyping<'text>),
}

/// The typed facts are provisional. Only the listed demands and their
/// dependencies have completed typing and can be evaluated.
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
