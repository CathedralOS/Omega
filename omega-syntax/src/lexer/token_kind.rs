#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Integer,
    String,
    Symbol,
}
