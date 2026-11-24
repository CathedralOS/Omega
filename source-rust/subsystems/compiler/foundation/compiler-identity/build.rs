//! Fingerprint active Rust inputs, embedded Omega sources, and Cargo's toolchain
//! and target settings. Package acceptance separately identifies the loaded
//! Omega library, which may come from outside the checkout.

mod cargo_outputs;

use cargo_outputs::CargoOutputs;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use wire::digest_encoder::DomainDigestEncoder;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
        io::Error::other("Cargo did not provide the compiler manifest directory")
    })?);
    let root = manifest
        .ancestors()
        .find(|path| {
            path.join("source-rust").is_dir() && path.join("rust-toolchain.toml").is_file()
        })
        .ok_or_else(|| io::Error::other("compiler is outside the Omega workspace"))?;
    let output = PathBuf::from(
        std::env::var_os("OUT_DIR")
            .ok_or_else(|| io::Error::other("Cargo did not provide OUT_DIR"))?,
    );
    let cargo_outputs = CargoOutputs::new(
        &output,
        std::env::var_os("CARGO_TARGET_DIR").map(|directory| root.join(directory)),
        std::env::var_os("TARGET").as_deref(),
    );
    let mut paths = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
        .map(|file| root.join(file))
        .to_vec();
    collect_inputs(
        &root.join("source-rust"),
        &cargo_outputs,
        InputTree::Rust,
        &mut paths,
    )?;
    // The library loader keys the selected library separately. These bytes
    // also include declarations embedded directly into the Rust executable.
    for directory in [root.join("source/library"), root.join(".cargo")] {
        if directory.is_dir() {
            collect_inputs(&directory, &cargo_outputs, InputTree::Other, &mut paths)?;
        }
    }
    paths.sort_unstable();
    let mut digest = DomainDigestEncoder::new(b"omega-compiler-build-inputs-v3\0");
    for path in paths {
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .ok_or_else(|| io::Error::other("non-UTF-8 compiler source path"))?
            .replace('\\', "/");
        println!("cargo:rerun-if-changed={}", path.display());
        digest.bytes(relative.as_bytes());
        digest.bytes(&fs::read(path)?);
    }
    let rustc =
        std::env::var_os("RUSTC").ok_or_else(|| io::Error::other("Cargo did not provide RUSTC"))?;
    let version = Command::new(rustc).arg("-vV").output()?;
    if !version.status.success() {
        return Err(io::Error::other("cannot identify the Rust compiler toolchain").into());
    }
    digest.bytes(&version.stdout);
    for name in [
        "HOST",
        "TARGET",
        "PROFILE",
        "OPT_LEVEL",
        "DEBUG",
        "CARGO_ENCODED_RUSTFLAGS",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        digest.bytes(name.as_bytes());
        digest.bytes(std::env::var(name)?.as_bytes());
    }
    let mut configuration = std::env::vars()
        .filter(|(name, _)| name.starts_with("CARGO_CFG_") || name.starts_with("CARGO_FEATURE_"))
        .collect::<Vec<_>>();
    configuration.sort();
    for (name, value) in configuration {
        digest.bytes(name.as_bytes());
        digest.bytes(value.as_bytes());
    }
    let path = output.join("compiler-identity.bin");
    let digest = digest.finish();
    if !fs::read(&path).is_ok_and(|previous| previous == digest) {
        fs::write(path, digest)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum InputTree {
    Rust,
    IntegrationTests,
    Other,
}

fn collect_inputs(
    directory: &Path,
    cargo_outputs: &CargoOutputs<'_>,
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
        if entry.file_name() == ".git"
            || (!matches!(tree, InputTree::Other)
                && kind.is_dir()
                && entry.file_name() == "_recycling")
            || cargo_outputs.contains(&path)
        {
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
            collect_inputs(&path, cargo_outputs, child, paths)?;
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
