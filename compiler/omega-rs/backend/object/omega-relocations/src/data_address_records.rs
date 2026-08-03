use crate::RelocationPlanningInput;
use omega_object_file::{ObjectSymbolHandle, RelocationKind, RelocationPlan, RelocationRecord};
use omega_target::Architecture;

pub(super) fn insert_data_address_relocations(
    input: RelocationPlanningInput<'_>,
    relocation_plan: &mut RelocationPlan,
    function_symbol_handle: ObjectSymbolHandle,
    selected_instruction_index: u32,
    operand_text_offset: usize,
    symbol_handle: ObjectSymbolHandle,
) {
    insert_data_address_relocations_for_architecture(
        input.target.architecture,
        relocation_plan,
        function_symbol_handle,
        selected_instruction_index,
        operand_text_offset,
        symbol_handle,
    );
}

fn insert_data_address_relocations_for_architecture(
    architecture: Architecture,
    relocation_plan: &mut RelocationPlan,
    function_symbol_handle: ObjectSymbolHandle,
    selected_instruction_index: u32,
    operand_text_offset: usize,
    symbol_handle: ObjectSymbolHandle,
) {
    match architecture {
        Architecture::Aarch64 => {
            relocation_plan.push_record(RelocationRecord {
                origin: omega_object_file::RelocationOrigin::Instruction {
                    function_symbol_handle,
                    selected_instruction_index,
                },
                section: omega_object_file::SectionKind::Text,
                offset: operand_text_offset,
                byte_width: 4,
                symbol_handle,
                addend: 0,
                kind: RelocationKind::Aarch64Page21,
            });
            relocation_plan.push_record(RelocationRecord {
                origin: omega_object_file::RelocationOrigin::Instruction {
                    function_symbol_handle,
                    selected_instruction_index,
                },
                section: omega_object_file::SectionKind::Text,
                offset: operand_text_offset + 4,
                byte_width: 4,
                symbol_handle,
                addend: 0,
                kind: RelocationKind::Aarch64PageOffset12,
            });
        }
        Architecture::X86_64 => {
            relocation_plan.push_record(RelocationRecord {
                origin: omega_object_file::RelocationOrigin::Instruction {
                    function_symbol_handle,
                    selected_instruction_index,
                },
                section: omega_object_file::SectionKind::Text,
                offset: operand_text_offset,
                byte_width: 8,
                symbol_handle,
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::insert_data_address_relocations_for_architecture;
    use omega_object_file::{RelocationKind, RelocationPlan};
    use omega_target::{Architecture, NativeTarget};
    use psi_arena::Handle;

    #[test]
    fn inserts_target_specific_data_address_relocations() {
        let function_symbol = Handle::from_arena_index(1);
        let data_symbol = Handle::from_arena_index(2);

        let mut aarch64_plan = RelocationPlan::with_target(NativeTarget::linux_arm64());
        insert_data_address_relocations_for_architecture(
            Architecture::Aarch64,
            &mut aarch64_plan,
            function_symbol,
            7,
            12,
            data_symbol,
        );
        let aarch64_records: Vec<_> = aarch64_plan.records().map(|(_, record)| record).collect();

        assert_eq!(aarch64_records.len(), 2);
        assert_eq!(aarch64_records[0].kind, RelocationKind::Aarch64Page21);
        assert_eq!(aarch64_records[0].offset, 12);
        assert_eq!(aarch64_records[0].byte_width, 4);
        assert_eq!(aarch64_records[1].kind, RelocationKind::Aarch64PageOffset12);
        assert_eq!(aarch64_records[1].offset, 16);
        assert_eq!(aarch64_records[1].byte_width, 4);

        let mut x86_plan = RelocationPlan::with_target(NativeTarget::windows_x64());
        insert_data_address_relocations_for_architecture(
            Architecture::X86_64,
            &mut x86_plan,
            function_symbol,
            7,
            12,
            data_symbol,
        );
        let x86_records: Vec<_> = x86_plan.records().map(|(_, record)| record).collect();

        assert_eq!(x86_records.len(), 1);
        assert_eq!(x86_records[0].kind, RelocationKind::Absolute64);
        assert_eq!(x86_records[0].offset, 12);
        assert_eq!(x86_records[0].byte_width, 8);
    }
}
