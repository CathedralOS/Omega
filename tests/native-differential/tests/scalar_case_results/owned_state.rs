use super::*;

#[test]
fn fresh_match_case_flows_through_owned_state_and_borrowed_storage() {
    assert_owned_case_source(
        include_str!("match_tag.omg"),
        &[
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ],
    );
}

#[test]
fn nested_case_selection_survives_later_scalar_selection() {
    let source = include_str!("match_tag.omg")
        .replace("2 -> Tag::Second,\n        _ -> Tag::Third", "_ -> match selector { 2 -> Tag::Second, _ -> Tag::Third }")
        .replace("transition { _ -> inspect(out, observed) }", "let continued: u64 = match selector { 0 -> 0, _ -> selector };\n    transition { _ -> inspect(out, observed, continued) }")
        .replace("state inspect(out: &mut [u8], observed: Tag)", "state inspect(out: &mut [u8], observed: Tag, continued: u64)");
    assert_owned_case_source(
        &source,
        &[
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ],
    );
}

#[test]
fn scalar_prefix_and_call_compose_with_selected_case_and_later_local() {
    let source = include_str!("match_tag.omg")
        .replace("let observed: Tag", "let prefix: u64 = selector & 255;\n    consume_scalar(prefix);\n    let observed: Tag")
        .replace("transition { _ -> inspect(out, observed) }", "let continued: u64 = selector & 255;\n    transition { _ -> inspect(out, observed, continued) }")
        .replace("state inspect(out: &mut [u8], observed: Tag)", "state inspect(out: &mut [u8], observed: Tag, continued: u64)")
        + "\nmachine consume_scalar(value: u64) {}";
    assert!(source.contains("consume_scalar(prefix);"));
    assert_owned_case_source(
        &source,
        &[
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ],
    );
}

#[test]
fn match_result_returns_through_an_ordinary_owned_call() {
    let source = include_str!("match_tag.omg").replace(
        "let observed: Tag = match selector {\n        0 -> Tag::Empty,\n        1 -> Tag::First,\n        2 -> Tag::Second,\n        _ -> Tag::Third\n    };",
        "let observed: Tag = choose_tag(selector);",
    ) + "\nmachine choose_tag(selector: u64) -> Tag { match selector { 0 -> Tag::Empty, 1 -> Tag::First, 2 -> Tag::Second, _ -> Tag::Third } }";
    assert!(source.contains("let observed: Tag = choose_tag(selector);"));
    let local = source
        .replace(
            "-> Tag { match selector",
            "-> Tag { let selected: Tag = match selector",
        )
        .replace(
            "_ -> Tag::Third } }",
            "_ -> Tag::Third }; let continued: u64 = selector & 255; selected }",
        );
    assert!(local.contains("let selected: Tag"));
    for source in [&source, &local] {
        assert_owned_case_source(
            source,
            &[
                NativeTarget::linux_x64(),
                NativeTarget::linux_arm64(),
                NativeTarget::macos_arm64(),
                NativeTarget::windows_x64(),
            ],
        );
    }
}

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
