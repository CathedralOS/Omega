//! Execute actual load/store fragments separately to observe every stored byte.

pub(super) fn execute(load: &[u8], store: &[u8], count: u16) {
    let symbol = if cfg!(target_os = "macos") {
        "_fragment_round_trip"
    } else {
        "fragment_round_trip"
    };
    let mut assembly = format!(".text\n.p2align 4\n.globl {symbol}\n{symbol}:\n");
    if cfg!(target_arch = "aarch64") {
        assembly.push_str("mov x12, x0\nsub sp, sp, #64\n");
        for offset in (0..64).step_by(8) {
            assembly.push_str(&format!(
                "ldr x11, [x12, #{offset}]\nstr x11, [sp, #{offset}]\n"
            ));
        }
    } else {
        assembly.push_str("mov %rdi, %r8\nsub $64, %rsp\n");
        for offset in (0..64).step_by(8) {
            assembly.push_str(&format!(
                "mov {offset}(%r8), %r11\nmov %r11, {offset}(%rsp)\n"
            ));
        }
    }
    append_bytes(&mut assembly, load);
    // The tested source's identity producer transfers the two native argument
    // registers to the two native result registers, without modifying bits.
    if cfg!(target_arch = "x86_64") {
        assembly.push_str("mov %rdi, %rax\nmov %rsi, %rdx\n");
    }
    append_bytes(&mut assembly, store);
    if cfg!(target_arch = "aarch64") {
        for offset in (0..64).step_by(8) {
            assembly.push_str(&format!(
                "ldr x11, [sp, #{offset}]\nstr x11, [x12, #{offset}]\n"
            ));
        }
        assembly.push_str("add sp, sp, #64\nret\n");
    } else {
        for offset in (0..64).step_by(8) {
            assembly.push_str(&format!(
                "mov {offset}(%rsp), %r11\nmov %r11, {offset}(%r8)\n"
            ));
        }
        assembly.push_str("add $64, %rsp\nret\n");
    }
    if !cfg!(target_os = "macos") {
        assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    }
    let driver = format!(
        "#include <stdint.h>\n#include <string.h>\n
         extern void fragment_round_trip(uint8_t *);
         int main(void) {{
             for (unsigned seed = 0; seed < 256; ++seed) {{
                 uint8_t actual[64], expected[64];
                 memset(actual, 0xcc, sizeof(actual));
                 for (unsigned index = 0; index < {count}; ++index)
                     actual[index] = (uint8_t)(seed + 37 * index);
                 memcpy(expected, actual, sizeof(actual));
                 memcpy(expected + 16, actual, {count});
                 fragment_round_trip(actual);
                 if (memcmp(actual, expected, sizeof(actual)) != 0) return 1;
             }}
             return 0;
         }}"
    );
    super::super::affine_call_result_host::compile_and_run(&assembly, &driver);
}

fn append_bytes(assembly: &mut String, bytes: &[u8]) {
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    assembly.push_str(&format!(".byte {bytes}\n"));
}
