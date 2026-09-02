use crate::tests::*;

use super::fixture::staged_object_artifact;

#[test]
fn runtime_u64_parameter_less_or_equal_reaches_object_and_callable_on_both_isas() {
    use omega_calling_conventions::{CallingPolicy, MachineRegister};

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
        match target.architecture {
            omega_target::Architecture::X86_64 => {
                assert!(
                    text.windows(3).any(|bytes| bytes == [0x48, 0x39, 0xfe]),
                    "x86 object must contain reversed `cmp rsi, rdi`"
                );
                assert!(
                    text.windows(2).any(|bytes| bytes[0] == 0x72),
                    "x86 object must branch to the false arm with JB"
                );
            }
            omega_target::Architecture::Aarch64 => {
                assert!(
                    text.windows(4)
                        .any(|bytes| bytes == [0x3f, 0x00, 0x00, 0xeb]),
                    "AArch64 object must contain reversed `cmp x1, x0`"
                );
                assert!(
                    text.windows(4).any(|bytes| {
                        let word = u32::from_le_bytes(bytes.try_into().unwrap());
                        word & 0xff00_001f == 0x5400_0003
                    }),
                    "AArch64 object must branch to the false arm with B.LO"
                );
            }
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
            [ValueId::new(19_408).unwrap(), ValueId::new(19_409).unwrap()]
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
