//! Fixture identities shared with the executing owner and corpus inventory.

pub const FREE_MACHINE_NAMED_TRANSITION_REJECTED: &str =
    "calls/free_machine_named_transition_rejected";
pub const STATIC_MACHINE_PARAMETER_CONFIG_COMPILE: &str =
    "build/static_machine_parameter_config_compile";
pub const RUNTIME_SHIFT_COUNT_DOMAIN_EXIT: &str = "arithmetic/runtime_shift_count_domain_exit";
pub const RUNTIME_SHIFT_ATWIDTH_SIGNED_MODULAR_EXIT: &str =
    "arithmetic/runtime_shift_atwidth_signed_modular_exit";
pub const RUNTIME_SHIFT_SUBWORD_MASKED_COUNT_EXIT: &str =
    "arithmetic/runtime_shift_subword_masked_count_exit";
pub const RUNTIME_SHL_SATURATING_EXIT: &str = "arithmetic/runtime_shl_saturating_exit";
pub const RUNTIME_SHIFT_RIGHT_ATWIDTH_EXIT: &str = "arithmetic/runtime_shift_right_atwidth_exit";
pub const RUNTIME_TRAPPING_SHIFT_COUNT_EXIT: &str = "arithmetic/runtime_trapping_shift_count_exit";
pub const RUNTIME_SAT_MIN_IDIOM_EXIT: &str = "arithmetic/runtime_sat_min_idiom_exit";
pub const RUNTIME_WIRE_UTF8_INVALID_REFUSED_EXIT: &str =
    "wire/runtime_wire_utf8_invalid_refused_exit";
pub const EXTERNAL_LEAF_SYSCALL_COMPILE: &str = "providers/external_leaf_syscall_compile";

pub const PASS_CANARIES: &[&str] = &[
    STATIC_MACHINE_PARAMETER_CONFIG_COMPILE,
    RUNTIME_SHIFT_COUNT_DOMAIN_EXIT,
    RUNTIME_SHIFT_ATWIDTH_SIGNED_MODULAR_EXIT,
    RUNTIME_SHIFT_SUBWORD_MASKED_COUNT_EXIT,
    RUNTIME_SHL_SATURATING_EXIT,
    RUNTIME_SHIFT_RIGHT_ATWIDTH_EXIT,
    RUNTIME_TRAPPING_SHIFT_COUNT_EXIT,
    RUNTIME_SAT_MIN_IDIOM_EXIT,
    RUNTIME_WIRE_UTF8_INVALID_REFUSED_EXIT,
    EXTERNAL_LEAF_SYSCALL_COMPILE,
];

pub const FILE_EXPECTATION_FAIL_CANARIES: &[&str] = &[FREE_MACHINE_NAMED_TRANSITION_REJECTED];

pub const RECENT_ENCODER_PASS_CANARIES: &[&str] = &[
    "arithmetic/runtime_float_compare_bool_exit",
    "text/runtime_text_not_equals_exit",
];
