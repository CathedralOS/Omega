//! Atomic publication keeps the checked graph instead of substituting ordinary
//! load/store records for the instruction-observed result.

use super::fragment_container::compile_attached_entry;
use image_emission::{emit_direct_executable_image, validate_function_fragment_object_artifact};
use target::TargetProfile;

#[test]
fn atomic_fragment_publication_retains_source_and_rejects_missing_replay() {
    let compiled = compile_attached_entry(
        r#"
        data Main { prefix: u64; counter: AtomicU32; }
        machine Main::main(main: &mut Main) {
            main.prefix = 3;
            main.counter.store(10, NoOrdering);
            let prior: u32 in Wrapping = main.counter.fetch_add(5, NoOrdering);
            main.counter.store(prior as u32, NoOrdering);
        }
        "#,
        "Main::main",
        TargetProfile::MacosArm64,
    );
    validate_function_fragment_object_artifact(&compiled.container, &compiled.artifact)
        .expect("atomic object retains the source/physical replay");

    let mut changed = compiled.artifact.clone();
    changed.text_bytes_mut_for_test()[0] ^= 1;
    assert!(validate_function_fragment_object_artifact(&compiled.container, &changed).is_err());

    let mut missing = compiled.artifact;
    missing.clear_fragment_replay_for_test();
    let error = emit_direct_executable_image(&missing, 3).unwrap_err();
    assert_eq!(
        error.message,
        "graph storage and aggregate results require retained physical replay evidence"
    );
}
