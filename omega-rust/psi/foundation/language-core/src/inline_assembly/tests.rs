//! Inline assembly tests.

use super::{
    AsmAuthorityRequirement, AsmCacheOperationKind, AsmCatalogEntry, AsmControlRegister,
    AsmFenceKind, AsmFlagsDataFlow, AsmInstructionAvailability, AsmInstructionRefusal,
    AsmInstructionSerializationKind, AsmInstructionShape, AsmInterruptFlagEffect,
    AsmMemoryOrdering, AsmMemoryTransferKind, AsmOperandAccess, AsmSchedulingHintKind,
    AsmTargetApplicability, asm_catalog_entry,
};

#[test]
fn catalog_separates_user_and_deriver_availability() {
    let AsmCatalogEntry::Contract(hlt) = asm_catalog_entry("hlt").expect("hlt contract") else {
        panic!("hlt must be a contracted instruction");
    };
    assert_eq!(hlt.availability, AsmInstructionAvailability::UserChecked);
    assert_eq!(
        hlt.required_authority,
        AsmAuthorityRequirement::MachineOwner
    );

    let AsmCatalogEntry::Contract(iretq) = asm_catalog_entry("iretq").expect("iretq contract")
    else {
        panic!("iretq must be a contracted instruction");
    };
    assert_eq!(iretq.availability, AsmInstructionAvailability::DeriverOnly);

    let AsmCatalogEntry::Contract(lidt) = asm_catalog_entry("lidt").expect("lidt contract") else {
        panic!("lidt must be a contracted instruction");
    };
    assert_eq!(lidt.availability, AsmInstructionAvailability::DeriverOnly);
    assert_eq!(lidt.shape, AsmInstructionShape::DescriptorTableLoad);
    assert_eq!(lidt.required_authority, AsmAuthorityRequirement::IdtControl);
    assert_eq!(lidt.operands.len(), 1);
    assert!(lidt.operands[0].requires_place());
    assert_eq!(lidt.operands[0].target_register, "r10");
    assert_eq!(lidt.clobbers, &["r10"]);
}

#[test]
fn port_contracts_pin_operands_and_realized_clobbers() {
    let AsmCatalogEntry::Contract(out) = asm_catalog_entry("out").expect("out contract") else {
        panic!("out must be a contracted instruction");
    };
    assert_eq!(out.target, AsmTargetApplicability::X86_64);
    assert_eq!(out.required_authority, AsmAuthorityRequirement::PortIo);
    assert_eq!(
        out.operands
            .iter()
            .map(|operand| (
                operand.role,
                operand.target_register,
                operand.access,
                operand.expected_type_name,
            ))
            .collect::<Vec<_>>(),
        vec![
            ("port", "dx", AsmOperandAccess::Read, "u16"),
            ("value", "al", AsmOperandAccess::Read, "u8"),
        ]
    );
    assert_eq!(out.clobbers, &["rax", "rdx", "r10", "r11", "r15"]);

    let AsmCatalogEntry::Contract(input) = asm_catalog_entry("in").expect("in contract") else {
        panic!("in must be a contracted instruction");
    };
    assert_eq!(
        input
            .operands
            .iter()
            .map(|operand| (
                operand.role,
                operand.target_register,
                operand.access,
                operand.expected_type_name,
            ))
            .collect::<Vec<_>>(),
        vec![
            ("destination", "al", AsmOperandAccess::Write, "u8"),
            ("port", "dx", AsmOperandAccess::Read, "u16"),
        ]
    );
    assert_eq!(input.clobbers, &["rax", "rdx", "r10", "r15"]);
}

#[test]
fn flags_contracts_are_explicit_and_stack_balanced_by_lowering() {
    let AsmCatalogEntry::Contract(snapshot) = asm_catalog_entry("pushfq").expect("pushfq contract")
    else {
        panic!("pushfq must be contracted");
    };
    assert_eq!(snapshot.required_authority, AsmAuthorityRequirement::None);
    assert_eq!(
        snapshot.flags_data_flow,
        AsmFlagsDataFlow::SnapshotToOperand
    );
    assert_eq!(snapshot.operands[0].access, AsmOperandAccess::Write);
    assert_eq!(snapshot.operands[0].expected_type_name, "u64");
    assert_eq!(snapshot.clobbers, &["r10", "r15"]);

    let AsmCatalogEntry::Contract(restore) = asm_catalog_entry("popfq").expect("popfq contract")
    else {
        panic!("popfq must be contracted");
    };
    assert_eq!(
        restore.required_authority,
        AsmAuthorityRequirement::MachineOwner
    );
    assert_eq!(
        restore.flags_data_flow,
        AsmFlagsDataFlow::RestoreFromOperand
    );
    assert_eq!(
        restore.interrupt_flag_effect,
        AsmInterruptFlagEffect::RestoreFromOperand
    );
    assert_eq!(restore.operands[0].access, AsmOperandAccess::ReadPlace);
    assert_eq!(restore.clobbers, &["r10", "r15"]);
}

#[test]
fn msr_contracts_pin_structured_value_flow_and_machine_authority() {
    let AsmCatalogEntry::Contract(read) = asm_catalog_entry("rdmsr").expect("rdmsr contract")
    else {
        panic!("rdmsr must be contracted");
    };
    assert_eq!(read.target, AsmTargetApplicability::X86_64);
    assert_eq!(
        read.required_authority,
        AsmAuthorityRequirement::MachineOwner
    );
    assert_eq!(read.operands[0].access, AsmOperandAccess::Write);
    assert_eq!(read.operands[0].target_register, "edx:eax");
    assert_eq!(read.operands[1].expected_type_name, "u32");
    assert_eq!(read.clobbers, &["rax", "rcx", "rdx", "r10", "r11", "r15"]);

    let AsmCatalogEntry::Contract(write) = asm_catalog_entry("wrmsr").expect("wrmsr contract")
    else {
        panic!("wrmsr must be contracted");
    };
    assert_eq!(
        write.required_authority,
        AsmAuthorityRequirement::MachineOwner
    );
    assert_eq!(write.operands[0].target_register, "ecx");
    assert_eq!(write.operands[1].expected_type_name, "u64");
    assert_eq!(write.clobbers, &["rax", "rcx", "rdx", "r10", "r11", "r15"]);
}

#[test]
fn control_register_contracts_pin_exact_u64_flow_and_machine_authority() {
    for register in [
        AsmControlRegister::Cr0,
        AsmControlRegister::Cr2,
        AsmControlRegister::Cr3,
        AsmControlRegister::Cr4,
    ] {
        let AsmCatalogEntry::Contract(read) =
            asm_catalog_entry(register.read_mnemonic()).expect("control-register read contract")
        else {
            panic!("control-register read must be contracted");
        };
        assert_eq!(
            read.shape,
            AsmInstructionShape::ControlRegisterRead(register)
        );
        assert_eq!(read.target, AsmTargetApplicability::X86_64);
        assert_eq!(
            read.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );
        assert_eq!(read.operands[0].access, AsmOperandAccess::Write);
        assert_eq!(read.operands[0].target_register, register.name());
        assert_eq!(read.operands[0].expected_type_name, "u64");
        assert_eq!(read.clobbers, &["r10", "r15"]);
    }

    for register in [
        AsmControlRegister::Cr0,
        AsmControlRegister::Cr3,
        AsmControlRegister::Cr4,
    ] {
        let mnemonic = register
            .write_mnemonic()
            .expect("writable control register");
        let AsmCatalogEntry::Contract(write) =
            asm_catalog_entry(mnemonic).expect("control-register write contract")
        else {
            panic!("control-register write must be contracted");
        };
        assert_eq!(
            write.shape,
            AsmInstructionShape::ControlRegisterWrite(register)
        );
        assert_eq!(
            write.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );
        assert_eq!(write.operands[0].access, AsmOperandAccess::Read);
        assert_eq!(write.operands[0].target_register, register.name());
        assert_eq!(write.operands[0].expected_type_name, "u64");
        assert_eq!(write.clobbers, &["rax", "r10", "r11", "r15"]);
    }

    assert_eq!(asm_catalog_entry("write_cr2"), None);
}

#[test]
fn register_move_contracts_delegate_operand_checking_to_the_assignment() {
    for mnemonic in ["mov", "movq"] {
        let AsmCatalogEntry::Contract(contract) =
            asm_catalog_entry(mnemonic).expect("register-move contract")
        else {
            panic!("{mnemonic} must be contracted");
        };
        assert_eq!(contract.shape, AsmInstructionShape::RegisterMove);
        assert_eq!(
            contract.availability,
            AsmInstructionAvailability::UserChecked
        );
        assert_eq!(contract.target, AsmTargetApplicability::Any);
        assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
        assert!(contract.operands.is_empty());
        assert!(contract.clobbers.is_empty());
    }
}

#[test]
fn catalog_names_semantic_refusal_classes() {
    for mnemonic in [
        "ret", "retq", "call", "br", "blr", "retf", "jmpq", "b", "bl", "bx", "cbz", "tbz", "loop",
        "loopne", "jcxz", "jrcxz", "int", "int3", "je", "jne", "jz", "jae", "jbe", "jng", "jnle",
        "jo", "js", "jpe", "jnp", "jc", "jnc",
    ] {
        assert_eq!(
            asm_catalog_entry(mnemonic),
            Some(AsmCatalogEntry::Refused(
                AsmInstructionRefusal::HiddenControlExit
            )),
            "{mnemonic} stays a hidden-exit refusal"
        );
    }
    for mnemonic in [
        "ldp",
        "stp",
        "push",
        "pop",
        "pushq",
        "enter",
        "leave",
        "ldrb",
        "ldrsw",
        "strh",
        "ldur",
        "sturh",
        "ldtrb",
        "sttr",
        "ldxr",
        "stxrh",
        "ldaxr",
        "stlxrb",
        "ldxp",
        "stlxp",
        "ldar",
        "stlrh",
        "swp",
        "swpal",
        "cas",
        "caspal",
        "ldadd",
        "ldeor",
        "ldsmax",
        "ldumin",
        "xchg",
        "xadd",
        "cmpxchg",
        "cmpxchg8b",
        "xlatb",
        "movsb",
        "lodsq",
        "stosw",
        "scasb",
        "cmpsq",
        "insb",
        "outsw",
    ] {
        assert_eq!(
            asm_catalog_entry(mnemonic),
            Some(AsmCatalogEntry::Refused(
                AsmInstructionRefusal::UnmodeledMemoryAccess
            )),
            "{mnemonic} stays an unmodeled-memory refusal"
        );
    }
    // Supervisor traps are service-admission candidates, not hidden exits;
    // address arithmetic and ordering barriers access no memory. Each keeps
    // the unknown-mnemonic failure rather than borrowing a semantic refusal.
    for mnemonic in ["db", "svc", "hvc", "smc", "brk", "lea", "dmb", "dsb"] {
        assert_eq!(
            asm_catalog_entry(mnemonic),
            None,
            "{mnemonic} stays an unknown mnemonic"
        );
    }
}

#[test]
fn memory_transfer_contracts_pin_place_operands_and_operand_order() {
    // The canonical unordered transfers carry the modeled memory contract:
    // the operand is a typed Omega place, so provenance, permission and
    // exact-type checking are the place's own, and no authority, ordering
    // obligation or realized clobber is invented.
    for (mnemonic, kind) in [
        ("ldr", AsmMemoryTransferKind::Load),
        ("str", AsmMemoryTransferKind::Store),
    ] {
        let AsmCatalogEntry::Contract(contract) =
            asm_catalog_entry(mnemonic).expect("memory-transfer contract")
        else {
            panic!("{mnemonic} must be contracted");
        };
        assert_eq!(contract.shape, AsmInstructionShape::MemoryTransfer(kind));
        assert_eq!(contract.target, AsmTargetApplicability::Aarch64);
        assert_eq!(
            contract.availability,
            AsmInstructionAvailability::UserChecked
        );
        assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
        assert_eq!(contract.memory_ordering, AsmMemoryOrdering::None);
        assert!(contract.operands.is_empty());
        assert!(contract.clobbers.is_empty());
    }
    // Width-suffixed, offset/unscaled, ordered and multi-register spellings
    // are each a different contract and stay refused.
    for mnemonic in [
        "ldrb", "ldrsw", "strh", "ldur", "sturh", "ldxr", "stxrh", "ldar",
    ] {
        assert_eq!(
            asm_catalog_entry(mnemonic),
            Some(AsmCatalogEntry::Refused(
                AsmInstructionRefusal::UnmodeledMemoryAccess
            )),
            "{mnemonic} stays an unmodeled-memory refusal"
        );
    }
}

#[test]
fn fence_contracts_pin_ordering_without_invented_clobbers() {
    for (mnemonic, kind) in [
        ("lfence", AsmFenceKind::Load),
        ("sfence", AsmFenceKind::Store),
        ("mfence", AsmFenceKind::Full),
    ] {
        let AsmCatalogEntry::Contract(contract) =
            asm_catalog_entry(mnemonic).expect("fence contract")
        else {
            panic!("{mnemonic} must be contracted");
        };
        assert_eq!(contract.memory_ordering, AsmMemoryOrdering::Fence(kind));
        assert_eq!(contract.target, AsmTargetApplicability::X86_64);
        assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
        assert!(contract.operands.is_empty());
        assert!(contract.clobbers.is_empty());
    }
}

#[test]
fn pipeline_directive_contracts_pin_no_authority_and_no_clobbers() {
    for (mnemonic, kind, target) in [
        (
            "serialize",
            AsmInstructionSerializationKind::Serialize,
            AsmTargetApplicability::X86_64,
        ),
        (
            "isb",
            AsmInstructionSerializationKind::InstructionSynchronizationBarrier,
            AsmTargetApplicability::Aarch64,
        ),
    ] {
        let AsmCatalogEntry::Contract(contract) =
            asm_catalog_entry(mnemonic).expect("serialization contract")
        else {
            panic!("{mnemonic} must be contracted");
        };
        assert_eq!(
            contract.shape,
            AsmInstructionShape::InstructionSerialization(kind)
        );
        assert_eq!(contract.target, target);
        assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
        assert_eq!(contract.memory_ordering, AsmMemoryOrdering::None);
        assert!(contract.operands.is_empty());
        assert!(contract.clobbers.is_empty());
    }

    for (mnemonic, kind, target) in [
        (
            "pause",
            AsmSchedulingHintKind::SpinPause,
            AsmTargetApplicability::X86_64,
        ),
        (
            "yield",
            AsmSchedulingHintKind::Yield,
            AsmTargetApplicability::Aarch64,
        ),
        (
            "nop",
            AsmSchedulingHintKind::Nop,
            AsmTargetApplicability::Any,
        ),
        (
            "wfe",
            AsmSchedulingHintKind::WaitForEvent,
            AsmTargetApplicability::Aarch64,
        ),
        (
            "wfi",
            AsmSchedulingHintKind::WaitForInterrupt,
            AsmTargetApplicability::Aarch64,
        ),
        (
            "sev",
            AsmSchedulingHintKind::SendEvent,
            AsmTargetApplicability::Aarch64,
        ),
        (
            "sevl",
            AsmSchedulingHintKind::SendEventLocal,
            AsmTargetApplicability::Aarch64,
        ),
    ] {
        let AsmCatalogEntry::Contract(contract) =
            asm_catalog_entry(mnemonic).expect("scheduling-hint contract")
        else {
            panic!("{mnemonic} must be contracted");
        };
        assert_eq!(contract.shape, AsmInstructionShape::SchedulingHint(kind));
        assert_eq!(contract.target, target);
        assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
        assert!(contract.operands.is_empty());
        assert!(contract.clobbers.is_empty());
    }
}

#[test]
fn cache_operation_contracts_pin_machine_owner_and_no_operands() {
    for (mnemonic, kind) in [
        ("wbinvd", AsmCacheOperationKind::WriteBackInvalidate),
        ("invd", AsmCacheOperationKind::Invalidate),
        ("wbnoinvd", AsmCacheOperationKind::WriteBackNoInvalidate),
    ] {
        let AsmCatalogEntry::Contract(contract) =
            asm_catalog_entry(mnemonic).expect("cache-operation contract")
        else {
            panic!("{mnemonic} must be contracted");
        };
        assert_eq!(contract.shape, AsmInstructionShape::CacheOperation(kind));
        assert_eq!(contract.target, AsmTargetApplicability::X86_64);
        assert_eq!(
            contract.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );
        assert_eq!(
            contract.availability,
            AsmInstructionAvailability::UserChecked
        );
        assert!(contract.operands.is_empty());
        assert!(contract.clobbers.is_empty());
    }
}

#[test]
fn interrupt_control_contracts_pin_authority_and_delayed_sti_semantics() {
    for (mnemonic, flag_effect) in [
        ("cli", AsmInterruptFlagEffect::Disable),
        ("sti", AsmInterruptFlagEffect::EnableAfterNextInstruction),
    ] {
        let AsmCatalogEntry::Contract(contract) =
            asm_catalog_entry(mnemonic).expect("interrupt-control contract")
        else {
            panic!("{mnemonic} must be contracted");
        };
        assert_eq!(contract.target, AsmTargetApplicability::X86_64);
        assert_eq!(
            contract.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );
        assert_eq!(contract.interrupt_flag_effect, flag_effect);
        assert!(contract.operands.is_empty());
        assert!(contract.clobbers.is_empty());
    }
}
