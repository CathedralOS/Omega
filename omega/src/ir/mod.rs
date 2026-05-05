pub mod command;
pub mod expression;
pub mod lowering;
pub mod machine;
pub mod platform;
pub mod state;
pub mod statement;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Program {
    pub machines: Vec<machine::Machine>,
    pub platforms: Vec<platform::Platform>,
}
