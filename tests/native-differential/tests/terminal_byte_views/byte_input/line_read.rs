//! Bounded line completion composes the byte leaf with ordinary scalar-sum returns.

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
use super::native_function;
use super::{NativeTarget, lower_reader, publish_reader, try_lower_reader};

/// The hosted byte-input leaf publishes `blocks;` plus an unconditional
/// `crashes Trap` route honestly. A `ConsoleNativeProvider::read_byte`
/// compiler intrinsic satisfying this fixture's pre-envelope nonblocking
/// requirement is therefore no longer the exact toolchain leaf: its caller
/// loses intrinsic source custody, so composed-control construction admits
/// no body for `read_line` at all. The refusal is the honest outcome — the
/// stale spelling must not rejoin the byte-input authority it names.
#[test]
fn a_byte_leaf_without_the_honest_envelope_loses_intrinsic_custody() {
    let source = format!(
        "{}\npub data ConsoleNativeProvider {{}}\n\
        machine ConsoleNativeProvider::read_byte() -> ByteRead\n\
        satisfies Console::read_byte via ForeignBinding::CompilerIntrinsic;",
        include_str!("../read_line.omg")
            .replace("Console::read_byte()", "ConsoleNativeProvider::read_byte()",),
    );
    let error = try_lower_reader(&source, "read_line")
        .expect_err("a nonblocking-requirement byte leaf must not keep intrinsic custody");
    let message = format!("{error:?}");
    assert!(
        message.contains("read_line") && message.contains("no admitted body"),
        "the stale leaf leaves its caller unplanned rather than rejoining host \
         authority: {message}"
    );
}

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
