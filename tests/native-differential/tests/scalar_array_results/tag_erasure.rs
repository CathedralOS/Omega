//! Explicit qualification erasure preserves the selected call result's payload.
use super::*;

#[test]
fn native_boolean_equality_predicates_preserve_runtime_and_literal_truth_tables() {
    for pattern in ["right", "true", "false"] {
        let source = format!(
            "machine choose(left: bool, right: bool, yes: i64, no: i64) -> i64 {{
                match left {{ {pattern} -> yes, _ -> no }}
            }}"
        );
        let artifact = produce(&source, "choose").expect("produce Boolean equality predicate");
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let restored =
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                    .unwrap();
            let plan = target_artifact(restored, target).unwrap();
            let (image, offset) = publish_target_with_replay_expectation(
                plan,
                "choose",
                target,
                ReplayExpectation::ScalarRecords,
            );
            #[cfg(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            ))]
            if target == NativeTarget::host() {
                let driver = format!(
                    r"
                    #include <stdbool.h>
                    #include <stdint.h>
                    extern int64_t omega_entry(bool, bool, int64_t, int64_t);
                    int main(void) {{
                        for (int left = 0; left < 2; ++left) {{
                            for (int right = 0; right < 2; ++right) {{
                                int64_t expected = (left == {pattern}) ? -17 : 91;
                                if (omega_entry(left, right, -17, 91) != expected) return 1;
                            }}
                        }}
                        return 0;
                    }}
                    "
                );
                native_function::assert_c_text(&image.output().final_text_bytes, offset, &driver);
            }
            #[cfg(not(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            )))]
            let _ = (image, offset);
        }
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: Boolean truth-table execution requires the Linux/macOS C host harness; publication is cross-target only"
    );
}

#[test]
fn explicit_scalar_tag_erasure_preserves_selected_payload_on_native_host() {
    let source =
        include_str!("../../../omega/pass/expressions/explicit_scalar_tag_erasure/main.omg");
    let artifact = produce(source, "choose").expect("produce the unchanged tag-erasure fixture");
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("independently verify canonical explicit erasure");

    let canonical_bytes = artifact.to_bytes();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let restored =
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&canonical_bytes).unwrap();
        let plan = target_artifact(restored, target)
            .expect("explicit erasure retains an ordinary bare entry ABI");
        let (image, offset) = publish_target_with_replay_expectation(
            plan,
            "choose",
            target,
            ReplayExpectation::ScalarRecords,
        );
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        if target == NativeTarget::host() {
            native_function::assert_c_text(
                &image.output().final_text_bytes,
                offset,
                r"
                #include <stdbool.h>
                #include <stdint.h>
                extern int64_t omega_entry(bool, int64_t, int64_t);
                int main(void) {
                    if (omega_entry(true, -17, 91) != -17) return 1;
                    if (omega_entry(false, -17, 91) != 91) return 2;
                    return 0;
                }
                ",
            );
        }
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        let _ = (image, offset);
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: explicit tag-erasure runtime requires Linux x64/ARM64 or macOS ARM64; four-target publication is not host execution"
    );
}
