#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Use(UseItem),
    Machine(Machine),
    Platform(Platform),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseItem {
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub contains: Vec<Contains>,
    pub states: Vec<State>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contains {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub name: String,
    pub statements: Vec<crate::ast::statement::Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub name: String,
    pub commands: Vec<CommandSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSignature {
    pub name: String,
}
