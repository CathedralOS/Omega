//! Execute the validated, relocated text with the caller's actual Unit ABI.

use super::{Command, NEXT_SCRATCH_DIRECTORY, Ordering, ScratchDirectory, SystemTime};

pub(super) fn execute(image: &image_emission::ExecutableImage, entry_offset: usize, wide: bool) {
    let output = image.output();
    assert_eq!(output.final_image_imports, 0);
    assert!(output.final_data_bytes.is_empty());
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = NEXT_SCRATCH_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "omega-affine-unit-call-{}-{nonce}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    let bytes = output
        .final_text_bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    // Keep the complete text intact: internal PC-relative calls already carry
    // their final relocation. Only the C-visible entry symbol is introduced.
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
    let second_field = if wide { "uint64_t second;" } else { "" };
    let second_value = if wide { ", ~values[index]" } else { "" };
    let driver = format!(
        "#include <stdint.h>\n\
         typedef struct {{ uint64_t first; {second_field} }} Payload;\n\
         extern void entry(Payload);\n\
         int main(void) {{\n\
             const uint64_t values[] = {{ 0, UINT64_MAX, UINT64_C(0x5eedcafedeadbeef) }};\n\
             for (unsigned index = 0; index < 3; ++index) {{\n\
                 Payload value = {{ values[index]{second_value} }};\n\
                 entry(value);\n\
             }}\n\
             return 0;\n\
         }}\n"
    );
    std::fs::write(&assembly_path, assembly).unwrap();
    std::fs::write(&driver_path, driver).unwrap();
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker");
    assert!(
        link.status.success(),
        "{}",
        String::from_utf8_lossy(&link.stderr)
    );
    assert!(Command::new(executable_path).status().unwrap().success());
}
