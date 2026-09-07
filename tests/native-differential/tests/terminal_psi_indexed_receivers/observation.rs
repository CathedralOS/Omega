//! Run complete relocated text with a borrowed pointer and inspect its caller.

use super::{Fixture, NativeTarget, host};

pub(super) fn host_matches(target: NativeTarget) -> bool {
    (cfg!(target_os = "linux") || cfg!(target_os = "macos"))
        && ((cfg!(target_arch = "x86_64") && target == NativeTarget::linux_x64())
            || (cfg!(target_arch = "aarch64") && target == NativeTarget::linux_arm64()))
}

pub(super) fn execute(
    image: &image_emission::ExecutableImage,
    entry_offset: usize,
    fixture: &Fixture,
    scalar_prefix: bool,
) {
    let output = image.output();
    assert_eq!(output.final_image_imports, 0);
    assert!(output.final_data_bytes.is_empty());
    assert!(entry_offset < output.final_text_bytes.len());
    // Internal PC-relative calls have already been relocated. Preserve the
    // entire text and its relative positions, including the receiver callee.
    let bytes = output
        .final_text_bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let symbol = if cfg!(target_os = "macos") {
        "_entry"
    } else {
        "entry"
    };
    let mut assembly = format!(
        ".text\n.p2align 4\n.Lomega_text:\n.byte {bytes}\n.globl {symbol}\n.set {symbol}, .Lomega_text + {entry_offset}\n"
    );
    if !cfg!(target_os = "macos") {
        assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    }
    let signature = if scalar_prefix {
        "uint16_t, void *"
    } else {
        "void *"
    };
    let invocation = if scalar_prefix {
        "entry(replacement, root);"
    } else {
        "entry(root);"
    };
    let replacements = if scalar_prefix {
        "0, UINT16_MAX, 0xa517"
    } else {
        "17, 17, 17"
    };
    let driver = format!(
        r#"
        #include <stdint.h>
        #include <stdio.h>
        #include <string.h>
        #include <unistd.h>
        extern void entry({signature});
        int main(void) {{
            alarm(10);
            enum {{ guard_bytes = 16, root_bytes = {root_bytes}, selected_offset = {selected_offset} }};
            const uint16_t replacements[] = {{ {replacements} }};
            for (unsigned trial = 0; trial < 3; ++trial) {{
                uint16_t storage[(root_bytes + 2 * guard_bytes) / 2];
                unsigned char expected[sizeof(storage)];
                unsigned char *bytes = (unsigned char *)storage;
                unsigned char *root = bytes + guard_bytes;
                for (unsigned index = 0; index < sizeof(storage); ++index) {{
                    bytes[index] = (unsigned char)(0x63 + index * 29 + trial * 47);
                }}
                memcpy(expected, bytes, sizeof(storage));
                uint16_t replacement = replacements[trial];
                memcpy(expected + guard_bytes + selected_offset, &replacement, sizeof(replacement));
                {invocation}
                for (unsigned index = 0; index < sizeof(storage); ++index) {{
                    if (bytes[index] != expected[index]) {{
                        fprintf(stderr, "trial %u byte %u: expected %02x, observed %02x\n",
                            trial, index, expected[index], bytes[index]);
                        return 1;
                    }}
                }}
            }}
            return 0;
        }}
    "#,
        root_bytes = fixture.root_bytes,
        selected_offset = fixture.selected_offset
    );
    host::compile_and_run(&assembly, &driver);
    eprintln!(
        "executed relocated {} text on {}-{}; Linux runtime coverage: {}",
        if cfg!(target_arch = "aarch64") {
            "AArch64"
        } else {
            "x86-64"
        },
        std::env::consts::OS,
        std::env::consts::ARCH,
        cfg!(target_os = "linux"),
    );
}
