//! Function-relative realization and callable-entry custody stages.

pub(crate) mod aarch64_movn_function_relative_realization;
pub(crate) mod active_resident_function_relative_realization;
pub(crate) mod function_relative_realization;
pub(crate) mod ordinary_callable_entry;
pub(crate) mod structural_unit_function_relative_realization;
pub(crate) mod unit_function_relative_realization;

pub use aarch64_movn_function_relative_realization::*;
pub use active_resident_function_relative_realization::*;
pub use function_relative_realization::*;
pub use ordinary_callable_entry::*;
pub use structural_unit_function_relative_realization::*;
pub use unit_function_relative_realization::*;
