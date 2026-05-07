#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Float,
    Identifier,
    Integer,
    String,
    Symbol,
}
