use crate::{Site, ValueType};
use omega_lexer::SourceSpan;
use omega_resolver::DeclarationId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalHypothesis {
    pub site: Site,
    pub parent: Option<Site>,
    pub owner: DeclarationId,
    pub name: String,
    pub parameters: Vec<(SourceSpan, ValueType)>,
    pub result: ValueType,
    pub premises: Vec<Site>,
    pub conclusions: Vec<Site>,
}
