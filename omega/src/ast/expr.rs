#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Integer(i64),
    String(String),
}
