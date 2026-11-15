//! Embed build-input identity, not the path of a running executable.
//!
//! Accepted package inputs are compiler-versioned. A release label or Git
//! HEAD misses uncommitted changes, so development and release builds both
//! identify the actual Rust source/configuration and toolchain. This is an
//! invalidation key, not compiler attestation. The separately loaded Omega
//! library is identified by its loader at acceptance/compilation time.

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
    let output = PathBuf::from(
        std::env::var_os("OUT_DIR")
            .ok_or_else(|| io::Error::other("Cargo did not provide OUT_DIR"))?,
    );
    let configured_target = std::env::var_os("CARGO_TARGET_DIR")
        .map(|directory| root.join(directory))
        .filter(|directory| {
            output.starts_with(directory) && !directory.join("Cargo.toml").is_file()
        });
    let target_directory = configured_target.as_deref().or_else(|| {
        output
            .ancestors()
            .find(|directory| directory.join("CACHEDIR.TAG").is_file())
    });
    let mut paths = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
        .map(|file| root.join(file))
        .to_vec();
    collect_inputs(
        &root.join("omega-rust"),
        target_directory,
        InputTree::Rust,
        &mut paths,
    )?;
    // The library loader keys the selected library separately. These bytes
    // also include declarations embedded directly into the Rust executable.
    for directory in [root.join("source/library"), root.join(".cargo")] {
        if directory.is_dir() {
            collect_inputs(&directory, target_directory, InputTree::Other, &mut paths)?;
        }
    }
    paths.sort_unstable();
    let mut digest =
        digest_encoder::DomainDigestEncoder::new(b"omega-rust-compiler-build-inputs-v2\0");
    for path in paths {
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .ok_or_else(|| io::Error::other("non-UTF-8 compiler source path"))?
            .replace('\\', "/");
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
    let path = output.join("compiler-identity.bin");
    let digest = digest.finish();
    if !fs::read(&path).is_ok_and(|previous| previous == digest) {
        fs::write(path, digest)?;
    }
    Ok(())
}

fn frame(digest: &mut digest_encoder::DomainDigestEncoder, bytes: &[u8]) {
    digest.bytes(bytes);
}

#[derive(Clone, Copy)]
enum InputTree {
    Rust,
    IntegrationTests,
    Other,
}

fn collect_inputs(
    directory: &Path,
    target_directory: Option<&Path>,
    tree: InputTree,
    paths: &mut Vec<PathBuf>,
) -> io::Result<()> {
    // Watch the directories even where the identity filters files. Cargo
    // must rerun input collection when the source roster gains or loses a file.
    println!("cargo:rerun-if-changed={}", directory.display());
    let cargo_package = !matches!(tree, InputTree::Other) && directory.join("Cargo.toml").is_file();
    let tree = if cargo_package { InputTree::Rust } else { tree };
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = entry.path();
        if entry.file_name() == ".git" || Some(path.as_path()) == target_directory {
            continue;
        }
        if kind.is_symlink() {
            return Err(io::Error::other(format!(
                "compiler build input cannot be a symbolic link: {}",
                path.display()
            )));
        }
        if kind.is_dir() {
            // Only Cargo's integration-test directory alongside a package
            // manifest is separate. Rust under src, including unit tests,
            // stays in the production input tree.
            let child = if cargo_package && entry.file_name() == "tests" {
                InputTree::IntegrationTests
            } else {
                tree
            };
            collect_inputs(&path, target_directory, child, paths)?;
        } else if kind.is_file()
            && !(matches!(tree, InputTree::IntegrationTests)
                && path.extension().is_some_and(|extension| extension == "rs"))
        {
            // Keep raw assets as well as source and manifests. Some are
            // embedded with include_bytes! or consumed by a build script.
            paths.push(path);
        }
    }
    Ok(())
}
