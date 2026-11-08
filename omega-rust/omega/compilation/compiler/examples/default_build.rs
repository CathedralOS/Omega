//! Ordinary default Build execution through the public compiler SDK.
//! Run: mbx run -p compiler --example default_build
#[path = "support/default_build.rs"]
mod workflow;
fn main() {
    workflow::run();
}
