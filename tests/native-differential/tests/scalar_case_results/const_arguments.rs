//! Call-computed indices cross a real consumer before source-free execution.

use super::{CanonicalTerminalArtifact, NativeTarget, membership, publish};

struct Sources(std::path::PathBuf);

impl Drop for Sources {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn composed_call_indices_reach_native_consumers_after_source_removal() {
    let directory = std::env::temp_dir().join(format!(
        "omega-native-const-arguments-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let sources = Sources(directory);
    let root = sources.0.join("main.omg");
    std::fs::write(
        &root,
        "machine identity<T>(value: T) -> T { value }
         machine seven() -> u64 { 7 }
         machine ready() -> bool { true }
         domain<const N: u64> u64::Index<N>;
         domain<const B: bool> u64::Gate<B>;
         machine consume(value: u64 in Index<9>, allowed: u64 in Gate<true>) -> u64 { value as u64 }
         machine read() -> u64 {
             let value: u64 in Index<(match ready() {
                 true -> identity<u64>(seven()) + 2, false -> 4
             })> = 9 as u64 in Index<9>;
             let allowed: u64 in Gate<(!false && identity<bool>(true))> = 1 as u64 in Gate<true>;
             consume(value, allowed)
         }",
    )
    .unwrap();
    let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(&root, None))
        .expect("composed indices pass source compilation and exact consumer matching");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .expect("composed indices reach Terminal");
    let artifact = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    drop(checked);
    drop(sources);
    assert!(
        !root.exists(),
        "native publication cannot use original source"
    );
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    membership::execute(
        &artifact,
        "#include <stdint.h>\nextern uint64_t omega_entry(void);\nint main(void) { return omega_entry() == UINT64_C(9) ? 0 : 1; }",
    );
}
