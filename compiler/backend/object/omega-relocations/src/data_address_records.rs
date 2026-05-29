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
            relocation_plan.record_set.records.insert(RelocationRecord {
                function_symbol_handle,
                selected_instruction_index,
                text_offset: operand_text_offset,
                byte_width: 4,
                symbol_handle,
                kind: RelocationKind::Aarch64Page21,
            });
            relocation_plan.record_set.records.insert(RelocationRecord {
                function_symbol_handle,
                selected_instruction_index,
                text_offset: operand_text_offset + 4,
                byte_width: 4,
                symbol_handle,
                kind: RelocationKind::Aarch64PageOffset12,
            });
        }
        Architecture::X86_64 => {
            relocation_plan.record_set.records.insert(RelocationRecord {
                function_symbol_handle,
                selected_instruction_index,
                text_offset: operand_text_offset,
                byte_width: 8,
                symbol_handle,
                kind: RelocationKind::X86_64Absolute64,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::insert_data_address_relocations_for_architecture;
    use omega_core::arena::{Arena, Handle};
    use omega_object_file::{RelocationKind, RelocationPlan};
    use omega_target::{Architecture, NativeTarget};

    #[test]
    fn inserts_target_specific_data_address_relocations() {
        let function_symbol = Handle::from_arena_index(1);
        let data_symbol = Handle::from_arena_index(2);

        let mut aarch64_plan = RelocationPlan {
            target: NativeTarget::linux_arm64(),
            record_set: omega_object_file::RelocationRecordSet {
                records: Arena::new(),
            },
        };
        insert_data_address_relocations_for_architecture(
            Architecture::Aarch64,
            &mut aarch64_plan,
            function_symbol,
            7,
            12,
            data_symbol,
        );
        let aarch64_records: Vec<_> = aarch64_plan
            .record_set
            .records
            .iter()
            .map(|(_, record)| record)
            .collect();

        assert_eq!(aarch64_records.len(), 2);
        assert_eq!(aarch64_records[0].kind, RelocationKind::Aarch64Page21);
        assert_eq!(aarch64_records[0].text_offset, 12);
        assert_eq!(aarch64_records[0].byte_width, 4);
        assert_eq!(aarch64_records[1].kind, RelocationKind::Aarch64PageOffset12);
        assert_eq!(aarch64_records[1].text_offset, 16);
        assert_eq!(aarch64_records[1].byte_width, 4);

        let mut x86_plan = RelocationPlan {
            target: NativeTarget::windows_x64(),
            record_set: omega_object_file::RelocationRecordSet {
                records: Arena::new(),
            },
        };
        insert_data_address_relocations_for_architecture(
            Architecture::X86_64,
            &mut x86_plan,
            function_symbol,
            7,
            12,
            data_symbol,
        );
        let x86_records: Vec<_> = x86_plan
            .record_set
            .records
            .iter()
            .map(|(_, record)| record)
            .collect();

        assert_eq!(x86_records.len(), 1);
        assert_eq!(x86_records[0].kind, RelocationKind::X86_64Absolute64);
        assert_eq!(x86_records[0].text_offset, 12);
        assert_eq!(x86_records[0].byte_width, 8);
    }
}
