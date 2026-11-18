//! Target semantics, register effects and size declarations for selected forms.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingRegister;

pub mod atomic;
pub mod atomic_fence;
pub mod crash;
pub mod float_result_policy;
pub mod floating_control;
pub mod hosted_exit_process;
pub mod hosted_read_byte;
pub mod hosted_write_byte;
pub mod ieee_float_arithmetic;
pub mod installed_interrupt;
pub mod memory_fence;
pub mod normalized_foreign_call;
pub mod port_write;
pub mod register_access;
pub mod saturating_forms;
pub mod table_call;
pub mod trapping_forms;
