#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    CommandCall(String),
    Transition(String),
}
