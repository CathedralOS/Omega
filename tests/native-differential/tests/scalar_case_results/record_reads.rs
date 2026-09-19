//! Ordinary record observations retain the completed owner's field identity.

use super::{membership, produce_source};

const DECLARATIONS: &str = "data Record { payload: u64; }
    data Choice { case Empty; case Some(value: u32); }";

const DRIVER: &str = "#include <stdbool.h>\n#include <stdint.h>\nextern uint64_t omega_entry(bool selected, uint64_t payload);\nint main(void) { return omega_entry(true, UINT64_MAX) == UINT64_MAX && omega_entry(false, UINT64_C(0x123456789abcdef0)) == UINT64_C(0x123456789abcdef0) ? 0 : 1; }";

#[path = "record_reads/parameters.rs"]
mod parameters;

#[path = "record_reads/byte_lengths.rs"]
mod byte_lengths;

#[path = "record_reads/call_requirements.rs"]
mod call_requirements;

#[test]
fn immutable_record_local_field_reaches_native_execution() {
    let artifact = produce_source(
        "observe",
        &format!(
            "{DECLARATIONS}
        machine observe(selected: bool, payload: u64) -> u64 {{
            let retained: Record = Record {{ payload: payload }};
            retained.payload
        }}"
        ),
    );
    membership::execute(&artifact, DRIVER);
}

#[test]
fn record_field_observation_survives_an_owned_selection() {
    let artifact = produce_source(
        "observe",
        &format!(
            "{DECLARATIONS}
        machine observe(selected: bool, payload: u64) -> u64 {{
            let retained: Record = Record {{ payload: payload }};
            let left: Choice = Choice::Some {{ value: 37 }};
            let right: Choice = Choice::Empty;
            let result: Choice = match selected {{ true -> left, false -> right }};
            retained.payload
        }}"
        ),
    );
    membership::execute(&artifact, DRIVER);
}

#[test]
fn shared_record_getter_observes_the_constructed_local() {
    let artifact = produce_source(
        "observe",
        &format!(
            "{DECLARATIONS}
        machine Record::get_payload(&self) -> u64 {{ self.payload }}
        machine observe(selected: bool, payload: u64) -> u64 {{
            let retained: Record = Record {{ payload: payload }};
            retained.get_payload()
        }}"
        ),
    );
    membership::execute(&artifact, DRIVER);
}

#[test]
fn padded_record_reads_preserve_distinct_local_roots_and_ordered_operands() {
    let artifact = produce_source(
        "observe",
        "
        data Record { prefix: u8; payload: u64; }
        machine identity(value: u64) -> u64 { value }
        machine observe(selected: bool, payload: u64) -> u64 {
            let left: Record = Record { payload: payload, prefix: 1 };
            let right: Record = Record { prefix: 2, payload: 81985529216486895 };
            identity(left.payload) ^ right.payload
        }",
    );
    membership::execute(
        &artifact,
        "#include <stdbool.h>\n#include <stdint.h>\nextern uint64_t omega_entry(bool selected, uint64_t payload);\nint main(void) { return omega_entry(true, UINT64_MAX) == (UINT64_MAX ^ UINT64_C(81985529216486895)) && omega_entry(false, 0) == UINT64_C(81985529216486895) ? 0 : 1; }",
    );
}

#[test]
fn boolean_record_reads_compose_with_short_circuit_calls() {
    let artifact = produce_source(
        "observe",
        "
        data Record { prefix: u64; selected: bool; }
        machine identity(value: bool) -> bool { value }
        machine observe(selected: bool, other: bool) -> bool {
            let record: Record = Record { prefix: 18446744073709551615, selected: selected };
            record.selected && identity(other)
        }",
    );
    membership::execute(
        &artifact,
        "#include <stdbool.h>\nextern bool omega_entry(bool selected, bool other);\nint main(void) { return !omega_entry(false, false) && !omega_entry(false, true) && !omega_entry(true, false) && omega_entry(true, true) ? 0 : 1; }",
    );
}

#[test]
fn signed_record_field_reads_preserve_their_carrier() {
    let artifact = produce_source(
        "observe",
        "
        data Record { prefix: u8; payload: i16; }
        machine observe(payload: i16) -> i16 {
            let record: Record = Record { prefix: 17, payload: payload };
            record.payload
        }",
    );
    membership::execute(
        &artifact,
        "#include <stdint.h>\nextern int16_t omega_entry(int16_t payload);\nint main(void) { return omega_entry(INT16_MIN) == INT16_MIN && omega_entry(-12345) == -12345 && omega_entry(INT16_MAX) == INT16_MAX ? 0 : 1; }",
    );
}
#[test]
fn projected_shared_actual_compares_original_nested_records() {
    let artifact = super::produce_source(
        "Outer::equals",
        "
        data Inner { left: u64; right: u64; }
        data Outer { prefix: u64; inner: Inner; sibling: Inner; }
        machine Inner::equals(&self, other: &Inner) -> bool {
            self.left == other.left && self.right == other.right
        }
        machine Outer::equals(&self, other: &Outer) -> bool {
            self.inner.equals(&other.inner)
        }
    ",
    );
    for target in [
        super::NativeTarget::linux_x64(),
        super::NativeTarget::linux_arm64(),
        super::NativeTarget::macos_arm64(),
        super::NativeTarget::windows_x64(),
    ] {
        super::publish(&artifact, target);
    }
    super::membership::execute(
        &artifact,
        "#include <stdbool.h>\n#include <stdint.h>\n
        struct inner { uint64_t left, right; };
        struct outer { uint64_t prefix; struct inner inner, sibling; };
        extern bool omega_entry(const struct outer *, const struct outer *);
        int main(void) {
            struct outer left={3,{UINT64_MAX,UINT64_C(0x8123456789abcdef)},{7,8}};
            struct outer right={5,{UINT64_MAX,UINT64_C(0x8123456789abcdef)},{9,10}};
            if (!omega_entry(&left,&right) || !omega_entry(&left,&left)) return 1;
            right.inner.right=0;
            if (omega_entry(&left,&right)) return 2;
            right.inner.right=left.inner.right;
            right.inner.left=0;
            return omega_entry(&left,&right) || left.prefix!=3 || left.sibling.left!=7;
        }",
    );
}

#[test]
fn projected_mutable_call_wraps_and_read_after_return_observes_original_storage() {
    let artifact = produce_source(
        "Outer::update",
        "
        data Inner { tag: u8; value: i32 in Wrapping; }
        data Outer { prefix: u64; inner: Inner; sibling: Inner; }
        machine Inner::increment(&mut self, delta: i32 in Wrapping) {
            self.value = self.value + delta;
        }
        machine Inner::read(&self) -> i32 in Wrapping {
            transition { _ -> (self.value) }
        }
        machine Outer::update(&mut self, delta: i32 in Wrapping) -> i32 in Wrapping {
            let prior: i32 in Wrapping = self.inner.read();
            self.inner.increment(delta);
            let current: i32 in Wrapping = self.inner.read();
            transition { _ -> (prior + current) }
        }",
    );
    for target in [
        super::NativeTarget::linux_x64(),
        super::NativeTarget::linux_arm64(),
        super::NativeTarget::macos_arm64(),
        super::NativeTarget::windows_x64(),
    ] {
        super::publish(&artifact, target);
    }
    membership::execute(
        &artifact,
        "#include <stdint.h>\n#include <string.h>\n
        struct inner { uint8_t tag; int32_t value; };
        struct outer { uint64_t prefix; struct inner inner, sibling; };
        extern int32_t omega_entry(int32_t delta, struct outer *);
        int main(void) {
            const int32_t initial[] = { INT32_MAX, INT32_MIN, -7, 19 };
            const int32_t deltas[] = { 1, -1, 4, -3 };
            for (unsigned case_index = 0; case_index < 4; ++case_index) {
                struct outer actual, expected;
                memset(&actual, 0xa5, sizeof actual);
                actual.prefix = UINT64_C(0x8123456789abcdef);
                actual.inner.tag = 17;
                actual.inner.value = initial[case_index];
                actual.sibling.tag = 31;
                actual.sibling.value = -12345;
                memcpy(&expected, &actual, sizeof expected);
                uint32_t next_bits = (uint32_t)initial[case_index] + (uint32_t)deltas[case_index];
                memcpy(&expected.inner.value, &next_bits, sizeof next_bits);
                uint32_t result_bits = (uint32_t)initial[case_index] + next_bits;
                if ((uint32_t)omega_entry(deltas[case_index], &actual) != result_bits) return 1;
                if (memcmp(&actual, &expected, sizeof actual)) return 2;
            }
            return 0;
        }",
    );
}

#[test]
fn scalar_updates_preserve_neighboring_bounded_byte_storage() {
    let artifact = produce_source(
        "Outer::update",
        "
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        domain [u8; 9]::Utf8 requires valid_utf8(self);
        domain [u8; 0]::Utf8 requires valid_utf8(self);
        data Inner {
            prefix: [u8; 3] in Utf8;
            value: i32 in Wrapping;
            middle: [u8; 9] in Utf8;
            suffix: [u8; 0] in Utf8;
        }
        data Outer { prefix: u64; inner: Inner; sibling: Inner; }
        machine Inner::increment(&mut self, delta: i32 in Wrapping) {
            self.value = self.value + delta;
        }
        machine Inner::read(&self) -> i32 in Wrapping {
            transition { _ -> (self.value) }
        }
        machine Outer::update(&mut self, delta: i32 in Wrapping) -> i32 in Wrapping {
            let prior: i32 in Wrapping = self.inner.read();
            self.inner.increment(delta);
            let current: i32 in Wrapping = self.inner.read();
            transition { _ -> (prior + current) }
        }",
    );
    for target in [
        super::NativeTarget::linux_x64(),
        super::NativeTarget::linux_arm64(),
        super::NativeTarget::macos_arm64(),
        super::NativeTarget::windows_x64(),
    ] {
        super::publish(&artifact, target);
    }
    // This function-level harness supplies valid empty buffers. It establishes
    // no hosted ProgramEntry receiver eligibility or byte-content operations.
    membership::execute(
        &artifact,
        "#include <stdint.h>\n#include <stddef.h>\n#include <string.h>\n
        struct inner {
            _Alignas(8) unsigned char prefix[11];
            int32_t value;
            _Alignas(8) unsigned char middle[17];
            _Alignas(8) unsigned char suffix[8];
        };
        struct outer { uint64_t prefix; struct inner inner, sibling; };
        _Static_assert(sizeof(struct inner) == 48, \"bounded storage extent\");
        _Static_assert(offsetof(struct inner, value) == 12, \"scalar offset\");
        extern int32_t omega_entry(int32_t delta, struct outer *);
        int main(void) {
            const int32_t initial[] = { INT32_MAX, INT32_MIN, -7, 19 };
            const int32_t deltas[] = { 1, -1, 4, -3 };
            for (unsigned case_index = 0; case_index < 4; ++case_index) {
                struct outer actual, expected;
                memset(&actual, 0, sizeof actual);
                actual.prefix = UINT64_C(0x8123456789abcdef);
                actual.inner.value = initial[case_index];
                actual.sibling.value = -12345;
                memcpy(&expected, &actual, sizeof expected);
                uint32_t next_bits = (uint32_t)initial[case_index] + (uint32_t)deltas[case_index];
                memcpy(&expected.inner.value, &next_bits, sizeof next_bits);
                uint32_t result_bits = (uint32_t)initial[case_index] + next_bits;
                if ((uint32_t)omega_entry(deltas[case_index], &actual) != result_bits) return 1;
                if (memcmp(&actual, &expected, sizeof actual)) return 2;
            }
            return 0;
        }",
    );
}
