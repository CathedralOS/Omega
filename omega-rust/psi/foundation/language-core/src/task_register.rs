//! Task-register operations keep their checked table geometry with the operation.

pub const TASK_REGISTER_CORE_SOURCE: &str = "task_register.omg";

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum TaskRegisterOperation {
    /// Check the loaded table and task descriptor before changing TR.
    Activate { entries: u16 },
    /// Observe the selector currently retained by TR.
    Selector,
}
