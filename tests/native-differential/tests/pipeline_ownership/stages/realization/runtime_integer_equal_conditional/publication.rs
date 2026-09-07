use crate::tests::*;

use super::fixture::staged_object_artifact;

#[test]
fn runtime_u64_parameter_equality_reaches_object_and_callable_on_both_isas() {
    use calling_conventions::{CallingPolicy, MachineRegister};

    for (target, policy, parameters, result) in [
        (
            NativeTarget::linux_x64(),
            CallingPolicy::SystemVAMD64,
            [MachineRegister::X86Rdi, MachineRegister::X86Rsi],
            MachineRegister::X86Rax,
        ),
        (
            NativeTarget::linux_arm64(),
            CallingPolicy::Aapcs64,
            [MachineRegister::Aarch64X(0), MachineRegister::Aarch64X(1)],
            MachineRegister::Aarch64X(0),
        ),
    ] {
        let artifact = staged_object_artifact(target);
        validate_optimized_object_artifact(&artifact).unwrap();
        assert_eq!(artifact.source().object().relocation_record_count, 0);
        assert_eq!(artifact.source().object().symbols.len(), 1);
        let text = &artifact.source().object().text_section.bytes;
        assert!(!text.is_empty());
        match target.architecture {
            target::Architecture::X86_64 => assert!(
                text.windows(3).any(|window| window[0] & 0xf8 == 0x48
                    && window[1] == 0x39
                    && window[2] & 0xc0 == 0xc0),
                "x86 object must contain a register CMP; allocation owns its registers"
            ),
            target::Architecture::Aarch64 => assert!(
                text.windows(4)
                    .any(
                        |window| u32::from_le_bytes(window.try_into().unwrap()) & 0xffe0_fc1f
                            == 0xeb00_001f
                    ),
                "AArch64 object must contain a register CMP; allocation owns its registers"
            ),
        }

        let object_identity = artifact.source().object().identity;
        let object_bytes = artifact.source().container().bytes.clone();
        let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
        validate_optimized_ordinary_callable_entry(&callable).unwrap();
        let entry = callable.entry();
        assert_eq!(entry.calling_policy, policy);
        assert_eq!(entry.parameters.len(), 2);
        assert_eq!(entry.parameters[0].abi_register, parameters[0]);
        assert_eq!(entry.parameters[1].abi_register, parameters[1]);
        assert!(
            entry
                .parameters
                .iter()
                .all(|parameter| parameter.fixed_view == parameter.assigned_view)
        );
        assert_eq!(entry.result.abi_register, result);
        assert_eq!(entry.returns.len(), 2);
        assert_eq!(
            entry
                .returns
                .iter()
                .map(|returned| returned.value)
                .collect::<Vec<_>>(),
            [ValueId::new(19_008).unwrap(), ValueId::new(19_009).unwrap()]
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
