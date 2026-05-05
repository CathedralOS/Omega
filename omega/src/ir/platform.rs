use crate::ir::command::CommandSignature;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub name: String,
    pub commands: Vec<CommandSignature>,
}
