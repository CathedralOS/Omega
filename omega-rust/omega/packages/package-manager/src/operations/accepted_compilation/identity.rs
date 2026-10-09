//! Join the loaded compiler and the actually selected toolchain library.

use sha2::{Digest, Sha256};


pub(crate) fn compiler_input_identity() -> Result<[u8; 32], String> {
    let library = package_compilation::toolchain_library::identity()?;
    let mut digest = Sha256::new();
    digest.update(b"omega-accepted-compiler-inputs-v1\0");
    digest.update(compiler::compiler_identity());
    digest.update(library.as_bytes());
    Ok(digest.finalize().into())
}

