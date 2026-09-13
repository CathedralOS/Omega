//! Fresh case operands share the authored argument stream with receiver loans.

use super::{NativeTarget, membership, produce_source, publish};

const COUNT_SOURCE: &str = "data Alignment [copy] { case Byte; case Word; case DoubleWord; case QuadWord; }
         data Region { size: u64; }
         machine Alignment::get_stride(&self) -> u64 [1..=8] {
             transition self {
                 Alignment::Byte -> (1)
                 Alignment::Word -> (2)
                 Alignment::DoubleWord -> (4)
                 Alignment::QuadWord -> (8)
             }
         }
         machine Region::count(&self, unit: u64, alignment: Alignment) -> u64 {
             let stride: u64 [1..=8] = alignment.get_stride();
             let trailing: u64 = ((unit as u64 in Saturating) - (stride as u64 in Saturating)) as u64;
             let region_size: u64 = self.size;
             let effective: u64 = ((region_size as u64 in Saturating) - (trailing as u64 in Saturating)) as u64;
             effective / stride
         }
         machine evaluate(size: u64, unit: u64) -> u64 {
             let region: Region = Region { size: size };
             region.count(unit, Alignment::DoubleWord)
         }
         machine borrow_stride(alignment: Alignment) -> u64 { alignment.get_stride() }
         machine inspect_stride() -> u64 { borrow_stride(Alignment::DoubleWord) }";

#[test]
fn owned_case_argument_lends_its_captured_input_storage_to_the_getter() {
    let artifact = produce_source("inspect_stride", COUNT_SOURCE);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    membership::execute(
        &artifact,
        "#include <stdint.h>\nextern uint64_t omega_entry(void);\n
         int main(void) { return omega_entry()!=4; }",
    );
}

#[test]
fn fresh_case_count_borrows_its_owned_argument_and_uses_the_result_range() {
    let artifact = produce_source("evaluate", COUNT_SOURCE);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    membership::execute(
        &artifact,
        "#include <stdint.h>\n
         extern uint64_t omega_entry(uint64_t size, uint64_t unit);
         int main(void) {
             const uint64_t sizes[] = {0,1,3,4,16,UINT64_MAX};
             const uint64_t units[] = {0,1,4,5,8,UINT64_MAX};
             for (unsigned size_index=0; size_index<6; ++size_index)
                 for (unsigned unit_index=0; unit_index<6; ++unit_index) {
                     uint64_t size=sizes[size_index], unit=units[unit_index];
                     uint64_t trailing=unit>4 ? unit-4 : 0;
                     uint64_t effective=size>trailing ? size-trailing : 0;
                     if (omega_entry(size,unit)!=effective/4) return 1;
                 }
             return 0;
         }",
    );
}

#[test]
fn case_fields_execute_between_scalar_actuals_only_on_the_selected_path() {
    let artifact = produce_source(
        "evaluate",
        "data Payload [copy] { case Empty; case Item(value: u64); }
         machine replace(value: &mut u64, replacement: u64) -> u64 {
             let previous: u64 = value;
             value = replacement;
             previous
         }
         machine consume(first: u64, payload: Payload, last: u64) -> u64 {
             first ^ last
         }
         machine evaluate(selected: bool, value: &mut u64) -> u64 {
             match selected {
                 true -> consume(replace(&mut value, 11),
                     Payload::Item { value: replace(&mut value, 22) },
                     replace(&mut value, 33)),
                 false -> 0
             }
         }",
    );
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    membership::execute(
        &artifact,
        "#include <stdbool.h>\n#include <stdint.h>\n
         extern uint64_t omega_entry(bool selected, uint64_t *value);
         int main(void) {
             uint64_t value = UINT64_C(0x8123456789abcdef);
             if (omega_entry(false, &value) != 0 || value != UINT64_C(0x8123456789abcdef)) return 1;
             return omega_entry(true, &value) != (UINT64_C(0x8123456789abcdef) ^ 22) || value != 33;
         }",
    );
}

#[test]
fn fresh_case_call_preserves_shared_receiver_and_scalar_actual() {
    let artifact = produce_source(
        "evaluate",
        "data Alignment [copy] { case Byte; case Word; }
         data Region { size: u64; }
         machine Region::count(&self, unit: u64, alignment: Alignment) -> u64 {
             transition alignment in Alignment::Word {
                 true -> (self.size ^ unit)
                 false -> (0)
             }
         }
         machine evaluate(size: u64, unit: u64) -> u64 {
             let region: Region = Region { size: size };
             region.count(unit, Alignment::Word)
         }",
    );
    membership::execute(
        &artifact,
        "#include <stdint.h>\n
         extern uint64_t omega_entry(uint64_t size, uint64_t unit);
         int main(void) {
             uint64_t size = UINT64_C(0x8123456789abcdef);
             uint64_t unit = UINT64_C(0xfedcba9876543210);
             return omega_entry(size, unit) != (size ^ unit);
         }",
    );
}
