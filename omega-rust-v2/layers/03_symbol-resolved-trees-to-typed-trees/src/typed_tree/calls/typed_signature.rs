use crate::{Site, ValueType};
use omega_lexer::SourceSpan;
use omega_resolver::DeclarationId;

#[derive(Debug, Eq, PartialEq)]
pub struct TypedSignature {
    pub site: Site,
    pub machine: DeclarationId,
    pub parameters: Vec<(SourceSpan, ValueType)>,
    pub result: ValueType,
}
