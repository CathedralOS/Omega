//! Fixture identities shared with the executing owner and corpus inventory.

pub const RUNTIME_ATOMIC_LOAD_STORE_EXIT: &str = "atomics/runtime_atomic_load_store_exit";
pub const RUNTIME_ATOMIC_FETCH_ADD_EXIT: &str = "atomics/runtime_atomic_fetch_add_exit";
pub const RUNTIME_ATOMIC_FETCH_SUB_EXIT: &str = "atomics/runtime_atomic_fetch_sub_exit";
pub const RUNTIME_ATOMIC_FETCH_XOR_EXIT: &str = "atomics/runtime_atomic_fetch_xor_exit";
pub const RUNTIME_ATOMIC_FETCH_OR_EXIT: &str = "atomics/runtime_atomic_fetch_or_exit";
pub const RUNTIME_ATOMIC_FETCH_AND_EXIT: &str = "atomics/runtime_atomic_fetch_and_exit";
pub const RUNTIME_ATOMIC_SWAP_EXIT: &str = "atomics/runtime_atomic_swap_exit";
pub const RUNTIME_ATOMIC_COMPARE_EXCHANGE_EXIT: &str =
    "atomics/runtime_atomic_compare_exchange_exit";
pub const ATOMIC_GLOBAL_ORDER_OPERATIONS: &str = "atomics/atomic_global_order_operations";
pub const SHARED_RECEIVER_ATOMIC_STORE: &str = "atomics/shared_receiver_atomic_store";
pub const RUNTIME_CONSOLE_BYTE_ECHO_EXIT: &str = "host/runtime_console_byte_echo_exit";
pub const EFI_VTABLE_FIELD_CALL: &str = "targets/efi_vtable_field_call";
pub const SYSV_VTABLE_FIELD_CALL: &str = "targets/sysv_vtable_field_call";
pub const EFI_TWO_TABLE_FUNCTION_LEAVES: &str = "targets/efi_two_table_function_leaves";
pub const EFI_OUT_PARAM_CALL: &str = "targets/efi_out_param_call";
pub const CROSS_CONSOLE_BYTE_TARGETS: &str = "host/cross_console_byte_targets";
pub const CONSOLE_BYTE_FIELD_TARGET_REJECTED: &str = "host/console_byte_field_target_rejected";
/// Fuzz pin: scalar host-call results stored straight into a field target
/// are allowed -- only composite byte-op results refuse field targets.
pub const SCALAR_HOST_CALL_FIELD_STORE: &str = "filesystem/scalar_host_call_field_store";
pub const RUNTIME_DUTCH_FLAG_PARTITION_EXIT: &str = "collections/runtime_dutch_flag_partition_exit";
pub const IMMUTABLE_ARG_FOR_MUT_PARAM_REJECTED: &str = "calls/immutable_arg_for_mut_param_rejected";
pub const FLOAT_WRAPPING_DOMAIN_REJECTED: &str = "arithmetic/float_wrapping_domain_rejected";

pub const PASS_CANARIES: &[&str] = &[
    RUNTIME_ATOMIC_LOAD_STORE_EXIT,
    RUNTIME_ATOMIC_FETCH_ADD_EXIT,
    RUNTIME_ATOMIC_FETCH_SUB_EXIT,
    RUNTIME_ATOMIC_FETCH_XOR_EXIT,
    RUNTIME_ATOMIC_FETCH_OR_EXIT,
    RUNTIME_ATOMIC_FETCH_AND_EXIT,
    RUNTIME_ATOMIC_SWAP_EXIT,
    RUNTIME_ATOMIC_COMPARE_EXCHANGE_EXIT,
    ATOMIC_GLOBAL_ORDER_OPERATIONS,
    SHARED_RECEIVER_ATOMIC_STORE,
    RUNTIME_CONSOLE_BYTE_ECHO_EXIT,
    EFI_VTABLE_FIELD_CALL,
    SYSV_VTABLE_FIELD_CALL,
    EFI_TWO_TABLE_FUNCTION_LEAVES,
    EFI_OUT_PARAM_CALL,
    CROSS_CONSOLE_BYTE_TARGETS,
    SCALAR_HOST_CALL_FIELD_STORE,
    RUNTIME_DUTCH_FLAG_PARTITION_EXIT,
];

pub const FAIL_CANARIES: &[&str] = &[
    CONSOLE_BYTE_FIELD_TARGET_REJECTED,
    IMMUTABLE_ARG_FOR_MUT_PARAM_REJECTED,
    FLOAT_WRAPPING_DOMAIN_REJECTED,
];
