pub mod command;
pub mod machine;
pub mod platform;
pub mod state;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Program {
    pub machines: Vec<machine::Machine>,
    pub platforms: Vec<platform::Platform>,
}
