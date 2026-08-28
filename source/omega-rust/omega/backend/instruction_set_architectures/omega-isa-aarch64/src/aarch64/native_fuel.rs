//! Compatibility exports for terminal-Psi native-fuel encoding.

pub use omega_terminal_isa_aarch64::{
    AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT, AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT,
    AARCH64_NATIVE_FUEL_FAILURE_BRANCH_OFFSET, encode_native_fuel_charge,
    encode_native_fuel_cold_dispatch, native_fuel_charge_clobbers,
    native_fuel_cold_dispatch_clobbers,
};
