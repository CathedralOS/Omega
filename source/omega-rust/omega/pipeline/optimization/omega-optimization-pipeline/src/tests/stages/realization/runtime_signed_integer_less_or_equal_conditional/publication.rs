//! Exact signed-inclusive machine bytes, object custody, and callable publication.

use crate::tests::*;

use super::fixture::staged_object_artifact;

#[test]
fn runtime_i64_parameter_less_or_equal_reaches_signed_object_and_callable_on_both_isas() {
    use omega_calling_conventions::{CallingPolicy, MachineRegister};

    let i64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
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
        let text = &artifact.source().object().text_section.bytes;
        match target.architecture {
            omega_target::Architecture::X86_64 => {
                assert!(text.windows(3).any(|bytes| bytes == [0x48, 0x39, 0xfe]));
                assert!(text.windows(2).any(|bytes| bytes[0] == 0x7c));
                assert!(!text.windows(2).any(|bytes| bytes[0] == 0x72));
            }
            omega_target::Architecture::Aarch64 => {
                assert!(text
                    .windows(4)
                    .any(|bytes| bytes == [0x3f, 0x00, 0x00, 0xeb]));
                assert!(text.windows(4).any(|bytes| {
                    let word = u32::from_le_bytes(bytes.try_into().unwrap());
                    word & 0xff00_001f == 0x5400_000b
                }));
                assert!(!text.windows(4).any(|bytes| {
                    let word = u32::from_le_bytes(bytes.try_into().unwrap());
                    word & 0xff00_001f == 0x5400_0003
                }));
            }
        }

        let object_identity = artifact.source().object().identity;
        let object_bytes = artifact.source().container().bytes.clone();
        let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
        validate_optimized_ordinary_callable_entry(&callable).unwrap();
        let entry = callable.entry();
        assert_eq!(entry.calling_policy, policy);
        assert_eq!(entry.parameters.len(), 2);
        assert!(entry
            .parameters
            .iter()
            .all(|parameter| parameter.scalar_type == i64_type));
        assert_eq!(entry.parameters[0].abi_register, parameters[0]);
        assert_eq!(entry.parameters[1].abi_register, parameters[1]);
        assert_eq!(entry.result.declaration.scalar_type, u64_type);
        assert_eq!(entry.result.abi_register, result);
        assert_eq!(entry.returns.len(), 2);
        assert_eq!(
            entry
                .returns
                .iter()
                .map(|returned| returned.value)
                .collect::<Vec<_>>(),
            [ValueId::new(19_908).unwrap(), ValueId::new(19_909).unwrap()]
        );
        assert_eq!(
            callable.source().source().object().identity,
            object_identity
        );
        assert_eq!(callable.source().source().container().bytes, object_bytes);
        assert_eq!(
            OptimizedOrdinaryCallableEntryRecord::decode(&entry.encode().unwrap()).unwrap(),
            *entry
        );
    }
}
