//! The loaded compiler's build-input identity for retained package acceptance.
//!
//! The build script embeds actual Rust source/configuration/toolchain identity;
//! moving or replacing the executable's pathname cannot change these bytes in
//! the running process. This is not a claim of a verified compiler or host.
//! The complete input identity additionally joins the selected toolchain library.

pub fn compiler_input_identity() -> Result<[u8; 32], String> {
    let library = package_compilation::toolchain_library::identity()?;
    let mut digest = optimization_core::digest_encoder::DomainDigestEncoder::new(
        b"omega-accepted-compiler-inputs-v1\0",
    );
    digest.raw(&compiler_identity());
    digest.raw(library.as_bytes());
    Ok(digest.finish())
}

pub const fn compiler_identity() -> [u8; 32] {
    *include_bytes!(concat!(env!("OUT_DIR"), "/compiler-identity.bin"))
}

/// The effective Rust optimization level embedded by this compiler's build.
/// Runtime environment variables cannot change the loaded producer's profile.
pub const fn compiler_optimization_level() -> &'static str {
    env!("OMEGA_COMPILER_OPT_LEVEL")
}

/// Machine-readable build metadata shared by the CLI and corpus runner.
pub fn compiler_build_profile() -> String {
    format!(
        "{{\"schema\":\"omega-compiler-build-profile-v1\",\"opt_level\":\"{}\"}}",
        compiler_optimization_level()
    )
}
