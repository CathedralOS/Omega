//! Carrier-polymorphic aliases preserve their atoms through native realization.

use super::{CanonicalTerminalArtifact, NativeTarget, membership, publish};

struct Sources(std::path::PathBuf);

impl Drop for Sources {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn generic_alias_constant_reaches_native_execution() {
    let directory = std::env::temp_dir().join(format!(
        "omega-native-generic-aliases-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let sources = Sources(directory);
    let root = sources.0.join("main.omg");
    std::fs::write(
        &root,
        "domain<U> U::Marked requires true;
         domain<V> V::Both = V::Marked;
         domain<T> T::Alias = T::Both & T::Marked;
         const VALUE: u64 in Alias = 7;
         machine read_constant() -> u64 { VALUE as u64 }",
    )
    .unwrap();
    let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(&root, None))
        .expect("generic aliases pass ordinary source compilation");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read_constant")
        .produce_artifact()
        .expect("generic alias constant reaches Terminal");
    let artifact = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    drop(checked);
    drop(sources);

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
        "#include <stdint.h>\nextern uint64_t omega_entry(void);\nint main(void) { return omega_entry() == 7 ? 0 : 1; }",
    );
}
