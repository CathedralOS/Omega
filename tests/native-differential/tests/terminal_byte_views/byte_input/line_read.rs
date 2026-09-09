//! Bounded line completion composes the byte leaf with ordinary scalar-sum returns.
use super::*;

#[test]
fn bounded_line_reader_reaches_terminal_and_native_publication() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_reader(include_str!("../read_line.omg"), "read_line");
        let (image, _) = publish_reader(target, lowered);
        assert!(!image.output().final_text_bytes.is_empty());
    }
}

#[test]
fn bounded_line_reader_preserves_raw_prefix_outcomes_and_unread_suffix() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    {
        let lowered = lower_reader(include_str!("../read_line.omg"), "read_line");
        let (image, entry) = publish_reader(NativeTarget::host(), lowered);
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry,
            include_str!("../read_line.c"),
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!(
        "SKIP: bounded line input requires matching Linux or macOS ARM64 host; Windows native read/aggregate return remain unsupported"
    );
}
