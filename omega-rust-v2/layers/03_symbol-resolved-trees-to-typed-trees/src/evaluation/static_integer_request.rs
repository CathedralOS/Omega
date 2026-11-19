use crate::{Site, ValueType};
/// A source occurrence in the current admitted stage that needs a closed integer from evaluation.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StaticIntegerRequest {
    pub expression: Site,
    pub carrier: ValueType,
}
