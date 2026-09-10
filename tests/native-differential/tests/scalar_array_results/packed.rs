//! Odd-sized arrays retain exact bytes through ordinary owned calls and returns.
use super::*;

#[test]
fn packed_array_fragments_preserve_three_byte_tails() {
    check_lengths([3, 11]);
}

#[test]
fn packed_array_fragments_preserve_five_byte_tails() {
    check_lengths([5, 13]);
}

#[test]
fn packed_array_fragments_preserve_six_byte_tails() {
    check_lengths([6, 14]);
}

#[test]
fn packed_array_fragments_preserve_seven_byte_tails() {
    check_lengths([7, 15]);
}

fn check_lengths(lengths: [usize; 2]) {
    for length in lengths {
        let leaves = (0..length)
            .map(|position| {
                if position == 0 {
                    "value".to_owned()
                } else {
                    format!("{}u8", (position * 37 + 129) % 256)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            "machine keep(row: [u8; {length}]) -> [u8; {length}] {{ row }}
             machine forward(row: [u8; {length}]) -> [u8; {length}] {{ keep(row) }}
             machine make(value: u8) -> [u8; {length}] {{ [{leaves}] }}
             machine selected(value: u8) -> [u8; {length}] {{
                 let row: [u8; {length}] = make(value);
                 let copied: [u8; {length}] = keep(row);
                 forward(copied)
             }}"
        );
        // Microsoft uses indirect transport for these sizes; this exercise
        // covers the independently supported direct register ABI on three targets.
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let (image, offset) = publish(&source, "selected", target);
            #[cfg(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            ))]
            if target == NativeTarget::host() {
                let (result_type, fragments) = if length <= 8 {
                    ("uint64_t", "uint64_t fragments[2] = {result, 0};")
                } else {
                    (
                        "Pair",
                        "uint64_t fragments[2] = {result.first, result.second};",
                    )
                };
                let driver = format!(
                    "#include <stdint.h>\n
                     typedef struct {{ uint64_t first; uint64_t second; }} Pair;
                     extern {result_type} omega_entry(uint8_t);
                     int main(void) {{
                         for (unsigned input = 0; input < 256; ++input) {{
                             {result_type} result = omega_entry((uint8_t)input);
                             {fragments}
                             for (unsigned position = 0; position < {length}; ++position) {{
                                 uint8_t expected = position == 0 ? input : (position * 37 + 129) % 256;
                                 uint8_t actual = fragments[position / 8] >> ((position % 8) * 8);
                                 if (actual != expected) return 1;
                             }}
                         }}
                         return 0;
                     }}"
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
            {
                let _ = (image, offset);
                eprintln!("SKIP: packed array runtime requires the Linux/macOS host harness");
            }
        }
    }
}
