fn main() {
    for name in [
        "OMEGA_RELEASE_VERSION",
        "OMEGA_RELEASE_COMMIT",
        "OMEGA_RELEASE_LIBRARY_SHA256",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let version = std::env::var("OMEGA_RELEASE_VERSION")
        .unwrap_or_else(|_| format!("{}-dev", std::env::var("CARGO_PKG_VERSION").unwrap()));
    let commit = std::env::var("OMEGA_RELEASE_COMMIT").unwrap_or_else(|_| "unrecorded".to_owned());
    let host = std::env::var("TARGET").unwrap();
    println!("cargo:rustc-env=OMEGA_VERSION={version}");
    println!("cargo:rustc-env=OMEGA_COMMIT={commit}");
    println!("cargo:rustc-env=OMEGA_HOST={host}");
}
