//! Bitwise values retain their fixed-width signed or unsigned carrier.

#[test]
fn integer_and_publishes_and_executes_exact_fixed_width_values() {
    for (primitive, c_type, extrema) in [
        ("u8", "uint8_t", "UINT8_MAX, 128, 0x55, 0xaa"),
        ("u16", "uint16_t", "UINT16_MAX, 32768, 0x5555, 0xaaaa"),
        (
            "u32",
            "uint32_t",
            "UINT32_MAX, UINT32_C(2147483648), 0x55555555, 0xaaaaaaaa",
        ),
        (
            "u64",
            "uint64_t",
            "UINT64_MAX, UINT64_C(9223372036854775808), UINT64_C(0x5555555555555555), UINT64_C(0xaaaaaaaaaaaaaaaa)",
        ),
        ("i8", "int8_t", "-1, INT8_MIN, INT8_MAX, -86, 85"),
        ("i16", "int16_t", "-1, INT16_MIN, INT16_MAX, -21846, 21845"),
        (
            "i32",
            "int32_t",
            "-1, INT32_MIN, INT32_MAX, -1431655766, 1431655765",
        ),
        (
            "i64",
            "int64_t",
            "-1, INT64_MIN, INT64_MAX, -INT64_C(6148914691236517206), INT64_C(6148914691236517205)",
        ),
    ] {
        let source = format!(
            "machine intersect(left: {primitive}, right: {primitive}) -> {primitive} {{ left & right }}"
        );
        let artifact = super::produce_candidate(&source, "intersect")
            .unwrap_or_else(|error| panic!("produce {primitive} bitwise AND: {error:#?}"));
        super::publication::assert_four_targets(&artifact, 0);
        let driver = format!(
            "#include <stdint.h>\n\
             #include <stddef.h>\n\
             extern {c_type} omega_entry({c_type} left, {c_type} right);\n\
             int main(void) {{\n\
                 const {c_type} values[] = {{ 0, 1, {extrema} }};\n\
                 const size_t count = sizeof(values) / sizeof(values[0]);\n\
                 for (size_t left = 0; left < count; ++left) {{\n\
                     for (size_t right = 0; right < count; ++right) {{\n\
                         if (omega_entry(values[left], values[right]) !=\n\
                             ({c_type})(values[left] & values[right])) return 1;\n\
                     }}\n\
                 }}\n\
                 return 0;\n\
             }}\n"
        );
        super::publication::assert_host_execution(&artifact, 0, &driver);
    }
}
