//! Ordinary default Build execution through the public compiler SDK.
//! Run: mbx test -p compiler --test default_build
#[path = "support/default_build.rs"]
mod workflow;
fn main() {
    workflow::run();
}
