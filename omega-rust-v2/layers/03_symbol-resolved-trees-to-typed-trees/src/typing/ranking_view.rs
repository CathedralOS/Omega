use crate::ValueType;
use omega_resolver::DeclarationId;
/// Input and rank carriers are supplied by the admitted ranking theory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankingView {
    pub declaration: DeclarationId,
    pub subject: ValueType,
    pub parameters: Vec<ValueType>,
    pub rank: ValueType,
    pub canonical_default: bool,
}
