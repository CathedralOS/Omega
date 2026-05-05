use crate::ir::command::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub name: String,
    pub commands: Vec<Command>,
}
