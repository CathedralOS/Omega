//! Embed build-input identity, not the pathname of a running executable.
//!
//! Accepted package inputs are compiler-versioned. A release label or Git HEAD
//! misses uncommitted development changes, so both development and release
//! builds identify the actual Rust source/configuration and Rust toolchain.
//! This is an invalidation key, not compiler attestation. The separately loaded
//! Omega library is identified by its loader at acceptance/compilation time.

mod build_profile;

use optimization_core::digest_encoder;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    // This is a build requirement, not a convention callers must remember.
    // Cargo passes explicit rustflags after its profile's codegen options.
    let optimization = build_profile::optimization_level()?;
    println!("cargo:rustc-env=OMEGA_COMPILER_OPT_LEVEL={optimization}");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
        io::Error::other("Cargo did not provide the compiler manifest directory")
    })?);
    let root = manifest
        .ancestors()
        .nth(4)
        .ok_or_else(|| io::Error::other("compiler is outside the Omega workspace"))?;
    let mut paths = Vec::new();
    collect_inputs(root, &root.join("omega-rust"), &mut paths)?;
    // Some intrinsic owners embed bundled Omega declarations in the Rust
    // image. Those bytes are distinct from the library selected at runtime.
    collect_inputs(root, &root.join("source/library"), &mut paths)?;
    for relative in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        paths.push(relative.to_owned());
    }
    let cargo_configuration = root.join(".cargo");
    if cargo_configuration.is_dir() {
        collect_inputs(root, &cargo_configuration, &mut paths)?;
    }
    paths.sort();
    let mut digest =
        digest_encoder::DomainDigestEncoder::new(b"omega-rust-compiler-build-inputs-v1\0");
    for relative in paths {
        let path = root.join(&relative);
        println!("cargo:rerun-if-changed={}", path.display());
        frame(&mut digest, relative.as_bytes());
        frame(&mut digest, &fs::read(path)?);
    }
    let rustc =
        std::env::var_os("RUSTC").ok_or_else(|| io::Error::other("Cargo did not provide RUSTC"))?;
    let version = Command::new(rustc).arg("-vV").output()?;
    if !version.status.success() {
        return Err(io::Error::other("cannot identify the Rust compiler toolchain").into());
    }
    frame(&mut digest, &version.stdout);
    for name in [
        "HOST",
        "TARGET",
        "PROFILE",
        "OPT_LEVEL",
        "DEBUG",
        "CARGO_ENCODED_RUSTFLAGS",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        frame(&mut digest, name.as_bytes());
        frame(&mut digest, std::env::var(name)?.as_bytes());
    }
    let mut configuration = std::env::vars()
        .filter(|(name, _)| name.starts_with("CARGO_CFG_") || name.starts_with("CARGO_FEATURE_"))
        .collect::<Vec<_>>();
    configuration.sort();
    for (name, value) in configuration {
        frame(&mut digest, name.as_bytes());
        frame(&mut digest, value.as_bytes());
    }
    let output = PathBuf::from(
        std::env::var_os("OUT_DIR")
            .ok_or_else(|| io::Error::other("Cargo did not provide OUT_DIR"))?,
    );
    fs::write(output.join("compiler-identity.bin"), digest.finish())?;
    Ok(())
}

fn frame(digest: &mut digest_encoder::DomainDigestEncoder, bytes: &[u8]) {
    digest.bytes(bytes);
}

fn collect_inputs(root: &Path, directory: &Path, paths: &mut Vec<String>) -> io::Result<()> {
    // Watching directories includes new/deleted source files, not only edits to
    // today's roster. Build outputs live outside the compiler source tree.
    println!("cargo:rerun-if-changed={}", directory.display());
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = entry.path();
        if kind.is_symlink() {
            return Err(io::Error::other(format!(
                "compiler build input cannot be a symbolic link: {}",
                path.display()
            )));
        }
        if kind.is_dir() {
            if !matches!(entry.file_name().to_str(), Some("target" | ".git")) {
                collect_inputs(root, &path, paths)?;
            }
        } else if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("rs" | "toml" | "omg")
        ) {
            paths.push(
                path.strip_prefix(root)
                    .map_err(io::Error::other)?
                    .to_str()
                    .ok_or_else(|| io::Error::other("non-UTF-8 compiler source path"))?
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}
