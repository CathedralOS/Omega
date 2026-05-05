pub mod command;
pub mod data;
pub mod expression;
pub mod lowering;
pub mod machine;
pub mod platform;
pub mod state;
pub mod statement;
pub mod types;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Program {
    pub data_definitions: Vec<data::DataDefinition>,
    pub machines: Vec<machine::Machine>,
    pub platforms: Vec<platform::Platform>,
}
