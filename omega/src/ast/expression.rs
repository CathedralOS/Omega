#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Integer(i64),
    String(String),
}
