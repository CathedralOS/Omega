#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Integer(i64),
    Mutable(Box<Expression>),
    Name(Vec<String>),
    String(String),
}
