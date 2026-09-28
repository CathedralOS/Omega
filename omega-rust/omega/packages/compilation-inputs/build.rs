use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=OMEGA_RELEASE_LIBRARY_SHA256");
    if let Ok(digest) = std::env::var("OMEGA_RELEASE_LIBRARY_SHA256") {
        assert!(
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "OMEGA_RELEASE_LIBRARY_SHA256 must be a SHA-256 hex digest"
        );
        println!("cargo:rustc-env=OMEGA_RELEASE_LIBRARY_SHA256={digest}");
    } else {
        let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let root = manifest
            .ancestors()
            .find(|root| root.join("source/library/core").is_dir())
            .expect("development builds require the Omega source/library tree");
        println!(
            "cargo:rustc-env=OMEGA_DEVELOPMENT_LIBRARY_ROOT={}",
            root.join("source/library").display()
        );
    }
}
