//! The loaded compiler's build-input identity for retained package acceptance.
//!
//! The build script embeds actual Rust source/configuration/toolchain identity;
//! moving or replacing the executable's pathname cannot change these bytes in
//! the running process. This is not a claim of a verified compiler or host.
//! External toolchain library sources must be joined by the accepting caller.

pub const fn compiler_identity() -> [u8; 32] {
    *include_bytes!(concat!(env!("OUT_DIR"), "/compiler-identity.bin"))
}
