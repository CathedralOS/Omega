//! Ordinary default Build execution through the public compiler SDK.
//! Run: mbx run -p compiler --example default_build
#[path = "support/default_build.rs"]
mod workflow;
fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--build-profile")) {
        println!("{}", compiler::compiler_build_profile());
        return;
    }
    workflow::run();
}
