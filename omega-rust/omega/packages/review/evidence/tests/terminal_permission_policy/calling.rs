use super::*;

const SOURCE: &str = r#"use calling;
pub data FsCallingPolicy {}
pub FsCallingConformance: FsCallingPolicy satisfies CallingPolicy;
pub machine FsCallingPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
satisfies CallingPolicy::plan
{
    let mut output: BoundaryEntryPlan;
    output.call.convention = CallingConvention::SystemVAMD64;
    output.call.parameter_count = 1;
    output.call.parameters[0].shape.class = AbiValueClass::Integer;
    output.call.parameters[0].shape.byte_size = 8;
    output.call.parameters[0].shape.alignment = 8;
    output.call.parameters[0].location_count = 1;
    output.call.parameters[0].locations[0] = ValueLocation::Register {
        register: MachineRegister::X86Rdi, value_byte_offset: 0, byte_size: 8,
    };
    output.call.has_result = true;
    output.call.result.shape.class = AbiValueClass::Integer;
    output.call.result.shape.byte_size = 8;
    output.call.result.shape.alignment = 8;
    output.call.result.location_count = 1;
    output.call.result.locations[0] = ValueLocation::Register {
        register: MachineRegister::X86Rax, value_byte_offset: 0, byte_size: 8,
    };
    output.call.stack_alignment = 16;
    output.call.ordinary_clobbers.register_count = 1;
    output.call.ordinary_clobbers.registers[0] = MachineRegister::X86R10;
    output.call.entry_control = EntryControl::CallReturn;
    output.state.permitted_transitive_use.general_registers = true;
    BoundaryPlanResult::Accepted { plan: output }
}
pub boundary trait FilesystemBase {
    machine read(descriptor: u64) -> u64;
}
pub boundary trait FilesystemHost: FilesystemBase + Calling<FsCallingPolicy> {}
"#;

#[test]
fn inherited_permission_keeps_calling_meaning_but_not_policy_implementation_identity() {
    let fixture = fixtures::Fixture::filesystem(SOURCE, false, "FilesystemHost", "read");
    let checked = fixture.check(Some(read_permission()));
    assert!(checked.selected_provider_plans().plans().is_empty());
    let policy = project(&checked, fixture.target);
    let [service] = policy.services() else {
        panic!("one inherited permission service")
    };
    let [method] = service.methods() else {
        panic!("one declaring parent method")
    };
    assert_eq!(method.requirement_owner().path(), "FilesystemBase");
    let calling = method
        .calling()
        .expect("requirement-only accepted schema retains calling context");
    assert_eq!(calling.boundary_trait(), service.service());
    assert_eq!(calling.requirement_trait(), method.requirement_owner());
    assert_eq!(service.permissions()[0].requirement(), method.requirement());
    assert_eq!(calling.physical().ordinary_clobbers().len(), 1);

    let renamed_source = SOURCE.replace("FsCallingPolicy", "RenamedFsCallingPolicy");
    let renamed = fixtures::Fixture::filesystem(&renamed_source, false, "FilesystemHost", "read");
    let renamed = project(&renamed.check(Some(read_permission())), renamed.target);
    assert_eq!(policy, renamed);
    assert_eq!(
        policy.canonical_bytes().unwrap(),
        renamed.canonical_bytes().unwrap()
    );

    let changed_source = SOURCE.replace("MachineRegister::X86R10", "MachineRegister::X86R11");
    let changed = fixtures::Fixture::filesystem(&changed_source, false, "FilesystemHost", "read");
    let changed = project(&changed.check(Some(read_permission())), changed.target);
    assert_eq!(service.permissions(), changed.services()[0].permissions());
    assert_ne!(
        calling.physical(),
        changed.services()[0].methods()[0]
            .calling()
            .unwrap()
            .physical()
    );
    assert_ne!(policy, changed);
    assert_ne!(
        policy.canonical_bytes().unwrap(),
        changed.canonical_bytes().unwrap()
    );
}
