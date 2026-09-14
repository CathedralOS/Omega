use super::{membership, produce_source};

#[test]
fn bounded_byte_field_lengths_observe_exact_direct_and_nested_live_extents() {
    for (expression, expected) in [
        ("self.direct.len", "direct_length"),
        ("self.inner.first.len", "first_length"),
        ("self.inner.second.len", "second_length"),
        ("self.inner.empty.len", "0"),
        (
            "identity(self.inner.first.len) ^ self.inner.second.len",
            "first_length ^ second_length",
        ),
    ] {
        let artifact = produce_source(
            "Outer::length",
            &format!(
                "domain [u8; 3]::Utf8 requires valid_utf8(self);
                 domain [u8; 0]::Utf8 requires valid_utf8(self);
                 data Inner {{
                     first: [u8; 3] in Utf8;
                     second: [u8; 3] in Utf8;
                     empty: [u8; 0] in Utf8;
                 }}
                 data Outer {{ prefix: u64; direct: [u8; 3] in Utf8; inner: Inner; }}
                 machine identity(value: u64) -> u64 {{ value }}
                 machine Outer::length(&self) -> u64 {{
                     transition {{ _ -> ({expression}) }}
                 }}"
            ),
        );
        for target in [
            super::super::NativeTarget::linux_x64(),
            super::super::NativeTarget::linux_arm64(),
            super::super::NativeTarget::macos_arm64(),
            super::super::NativeTarget::windows_x64(),
        ] {
            super::super::publish(&artifact, target);
        }
        // Byte fields retain a leading u64 length followed by inline capacity
        // bytes. Alignment padding belongs to the enclosing record, not capacity.
        membership::execute(
            &artifact,
            &format!(
                "#include <stdint.h>\n#include <stddef.h>\n#include <string.h>\n
                 struct inner {{
                     _Alignas(8) unsigned char first[11];
                     _Alignas(8) unsigned char second[11];
                     _Alignas(8) unsigned char empty[8];
                 }};
                 struct outer {{
                     uint64_t prefix;
                     _Alignas(8) unsigned char direct[11];
                     struct inner inner;
                 }};
                 _Static_assert(sizeof(struct inner) == 40, \"inner extent\");
                 _Static_assert(offsetof(struct inner, second) == 16, \"sibling offset\");
                 _Static_assert(offsetof(struct outer, inner) == 24, \"nested offset\");
                 _Static_assert(sizeof(struct outer) == 64, \"outer extent\");
                 extern uint64_t omega_entry(const struct outer *);
                 int main(void) {{
                     const uint64_t lengths[] = {{ 0, 1, 3 }};
                     for (unsigned case_index = 0; case_index < 3; ++case_index) {{
                         struct outer actual, saved;
                         memset(&actual, 0xa5, sizeof actual);
                         actual.prefix = UINT64_C(0x8123456789abcdef);
                         uint64_t direct_length = lengths[case_index];
                         uint64_t first_length = lengths[(case_index + 1) % 3];
                         uint64_t second_length = lengths[(case_index + 2) % 3];
                         memcpy(actual.direct, &direct_length, 8);
                         memcpy(actual.inner.first, &first_length, 8);
                         memcpy(actual.inner.second, &second_length, 8);
                         memset(actual.inner.empty, 0, 8);
                         memcpy(actual.direct + 8, \"abc\", 3);
                         memcpy(actual.inner.first + 8, \"def\", 3);
                         memcpy(actual.inner.second + 8, \"ghi\", 3);
                         memcpy(&saved, &actual, sizeof actual);
                         if (omega_entry(&actual) != (uint64_t)({expected})) return 1;
                         if (memcmp(&actual, &saved, sizeof actual)) return 2;
                     }}
                     return 0;
                 }}"
            ),
        );
    }
}
