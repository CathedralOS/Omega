use crate::tests::*;

use super::fixture::staged_object_artifact;

#[test]
fn u64_parameter_not_equal_zero_reaches_linux_object_and_callable_on_both_isas() {
    use calling_conventions::{CallingPolicy, MachineRegister};

    for (target, policy, parameter, result) in [
        (
            NativeTarget::linux_x64(),
            CallingPolicy::SystemVAMD64,
            MachineRegister::X86Rdi,
            MachineRegister::X86Rax,
        ),
        (
            NativeTarget::linux_arm64(),
            CallingPolicy::Aapcs64,
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(0),
        ),
    ] {
        let artifact = staged_object_artifact(target);
        validate_optimized_object_artifact(&artifact).unwrap();
        assert_eq!(artifact.source().object().relocation_record_count, 0);
        assert_eq!(artifact.source().object().symbols.len(), 1);
        let text = &artifact.source().object().text_section.bytes;
        assert!(!text.is_empty());
        let emission = artifact.source().source().source().source();
        let compare = emission.fragments().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|span| {
                span.alternative.family
                    == selected_instructions::MachineAlternativeFamily::CompareI64Zero
            })
            .unwrap();
        let source = emission.source();
        let operand = &source.program().machine.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.instruction == compare.instruction)
            .unwrap()
            .operands[0];
        let name = &source
            .register_environment()
            .physical()
            .model()
            .views
            .iter()
            .find(|view| view.id == operand.view)
            .unwrap()
            .name;
        match target.architecture {
            target::Architecture::X86_64 => {
                let bytes = &compare.bytes;
                assert_eq!(bytes.len(), 3);
                assert_eq!(bytes[0] & 0xf8, 0x48);
                assert_eq!(bytes[1], 0x85);
                assert_eq!(bytes[2] & 0xc0, 0xc0);
                let left = (bytes[2] & 7) | ((bytes[0] & 1) << 3);
                let right = ((bytes[2] >> 3) & 7) | ((bytes[0] & 4) << 1);
                assert_eq!(left, right, "TEST must read the same allocated value twice");
                let names = [
                    "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10",
                    "r11", "r12", "r13", "r14", "r15",
                ];
                assert_eq!(names[usize::from(left)], name);
                assert!(
                    text.windows(bytes.len())
                        .any(|published| published == bytes)
                );
                assert!(
                    text.windows(2).any(|bytes| bytes[0] == 0x75),
                    "x86 object must branch to the nonzero arm using short JNE"
                );
            }
            target::Architecture::Aarch64 => {
                let word = u32::from_le_bytes(compare.bytes.as_slice().try_into().unwrap());
                assert_eq!(word & 0xffff_fc1f, 0xf100_001f);
                assert_eq!(format!("x{}", (word >> 5) & 31), *name);
                assert_eq!(compare.provenance.fuel.len(), 2);
                assert!(
                    text.windows(4).any(|bytes| {
                        u32::from_le_bytes(bytes.try_into().unwrap()) & 0xff00_001f == 0x5400_0001
                    }),
                    "AArch64 branches on the comparison flags with B.NE"
                );
            }
        }

        let object_identity = artifact.source().object().identity;
        let object_bytes = artifact.source().container().bytes.clone();
        let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
        validate_optimized_ordinary_callable_entry(&callable).unwrap();
        let entry = callable.entry();
        assert_eq!(entry.calling_policy, policy);
        assert_eq!(entry.parameters.len(), 1);
        assert_eq!(entry.parameters[0].abi_register, parameter);
        assert_eq!(
            entry.parameters[0].fixed_view,
            entry.parameters[0].assigned_view
        );
        assert_eq!(entry.result.abi_register, result);
        assert_eq!(entry.returns.len(), 2);
        assert_eq!(
            entry
                .returns
                .iter()
                .map(|returned| returned.value)
                .collect::<Vec<_>>(),
            [ValueId::new(20_108).unwrap(), ValueId::new(20_109).unwrap()]
        );
        assert!(entry.returns.iter().all(|returned| {
            returned.view == entry.result.view
                && returned.storage_units == entry.result.storage_units
        }));
        assert_eq!(
            callable.source().source().object().identity,
            object_identity
        );
        assert_eq!(callable.source().source().container().bytes, object_bytes);
        assert_eq!(
            OptimizedOrdinaryCallableEntryRecord::decode(&entry.encode().unwrap()).unwrap(),
            *entry
        );
        assert_eq!(
            OptimizedOrdinaryCallableEntryManifest::decode(&callable.manifest().record().encode())
                .unwrap(),
            *callable.manifest().record()
        );
    }
}
