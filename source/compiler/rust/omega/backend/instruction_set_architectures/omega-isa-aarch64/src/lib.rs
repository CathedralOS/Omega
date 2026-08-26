pub mod aarch64;
mod operand;

pub use aarch64::*;
pub use omega_terminal_isa_aarch64::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_RETURN, AARCH64_ADD_I64, AARCH64_COMPARE_I64_ZERO,
    AARCH64_CONDITIONAL_BRANCH, AARCH64_COPY_I64, AARCH64_DARWIN_CALL, AARCH64_DARWIN_RETURN,
    AARCH64_INLINE_ASSEMBLY_DEFAULT, AARCH64_LINUX_SYSTEM_CALL, AARCH64_MATERIALIZE_I64,
    AARCH64_REQUIRED_REGISTER_CONSTRAINTS, AARCH64_SUBTRACT_I64,
    Aarch64RegisterConstraintCatalogValidationError, aarch64_physical_register_model,
    aarch64_register_constraint_catalog, validate_aarch64_register_constraint_catalog,
};
pub use operand::Aarch64CallOperand;
