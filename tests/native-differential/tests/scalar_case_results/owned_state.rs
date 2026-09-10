use super::*;

#[test]
fn owned_case_state_argument_preserves_tag_payload_and_borrowed_storage() {
    let source = concat!(
        include_str!("choose.omg"),
        "\n",
        include_str!("owned_state.omg")
    );
    assert_owned_case_source(
        source,
        &[
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ],
    );
}

#[test]
fn owned_payloadless_case_state_argument_preserves_tag_and_borrowed_storage() {
    assert_owned_case_source(
        include_str!("owned_tag.omg"),
        &[
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ],
    );
}

fn assert_owned_case_source(source: &str, targets: &[NativeTarget]) {
    let artifact = produce_source("collect_owned", source);
    for &target in targets {
        let (image, offset) = publish(&artifact, target);
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
                include_str!("collect.c"),
            );
        }
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        {
            let _ = (image, offset);
            eprintln!(
                "SKIP: owned case runtime requires a matching direct aggregate host; cross-target publication was checked"
            );
        }
    }
}
