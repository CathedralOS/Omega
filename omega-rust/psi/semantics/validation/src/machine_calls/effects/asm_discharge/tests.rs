use super::{AsmAuthorityAdmission, validate_asm_discharge};
use language_core::inline_assembly::{
    AsmAuthorityRequirement, AsmCatalogEntry, asm_catalog_entry, asm_intrinsic_mnemonic,
};
use symbols::BuiltinFunction;
use typed_trees::TypedTrees;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolution");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typing")
}

fn rejection_messages(source: &str, admission: AsmAuthorityAdmission) -> Vec<String> {
    validate_asm_discharge(&typed(source), admission)
        .map(|_| Vec::new())
        .unwrap_or_else(|diagnostics| {
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect()
        })
}

#[test]
fn hosted_admission_rejects_each_authority_class_with_its_own_label() {
    for (source, fragment) in [
        (
            "machine m() { asm where clobbers none { cli } }",
            "requires machine-owner authority",
        ),
        (
            "machine m() { asm where clobbers none { hlt } }",
            "requires machine-owner authority",
        ),
        (
            "machine m() { asm where clobbers none { invd } }",
            "requires machine-owner authority",
        ),
        (
            "machine m() { asm where clobbers none { wbnoinvd } }",
            "requires machine-owner authority",
        ),
        (
            "data P { port: u16; byte: u8; } machine P::m(&self) { asm where clobbers r10, r11, r15, rax, rdx { out self.port, self.byte } }",
            "requires port-I/O authority",
        ),
        (
            "data P { index: u32; value: u64; } machine P::m(&self) { asm where clobbers r10, r11, r15, rax, rcx, rdx { wrmsr self.index, self.value } }",
            "requires machine-owner authority",
        ),
    ] {
        let messages = rejection_messages(source, AsmAuthorityAdmission::HOSTED);
        assert!(
            messages.iter().any(|message| message.contains(fragment)
                && message.contains("FREESTANDING boundary root")),
            "hosted admission must name the missing class for {source}: {messages:?}"
        );
    }
}

#[test]
fn hosted_admission_passes_authority_free_instructions() {
    for source in [
        "machine m() { asm where clobbers none { lfence; sfence; mfence } }",
        "machine m() { asm where clobbers none { serialize; pause; nop } }",
        "data P { saved: u64; } machine P::m(&mut self) { asm where clobbers r10, r15 { pushfq self.saved } }",
    ] {
        validate_asm_discharge(&typed(source), AsmAuthorityAdmission::HOSTED).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "authority-free instruction rejected under hosted admission: {diagnostics:?}"
                )
            },
        );
    }
}

#[test]
fn machine_owner_admission_covers_every_defined_class() {
    for source in [
        "machine m() { asm where clobbers none { cli; sti } }",
        "machine m() { asm where clobbers none { hlt } }",
        "data P { port: u16; byte: u8; } machine P::m(&mut self) { asm where clobbers r10, r11, r15, rax, rdx { out self.port, self.byte; in self.byte, self.port } }",
        "data P { saved: u64; } machine P::m(&mut self) { asm where clobbers r10, r15 { popfq self.saved } }",
        "data P { index: u32; value: u64; } machine P::m(&mut self) { asm where clobbers r10, r11, r15, rax, rcx, rdx { rdmsr self.value, self.index; wrmsr self.index, self.value } }",
        "machine m() { asm where clobbers none { wbinvd; invd; wbnoinvd } }",
    ] {
        validate_asm_discharge(&typed(source), AsmAuthorityAdmission::MACHINE_OWNER)
            .unwrap_or_else(|diagnostics| {
                panic!("machine-owner admission rejected {source}: {diagnostics:?}")
            });
    }
}

#[test]
fn admission_is_derived_from_the_freestanding_selection() {
    assert_eq!(
        AsmAuthorityAdmission::from_freestanding(true),
        AsmAuthorityAdmission::MACHINE_OWNER
    );
    assert_eq!(
        AsmAuthorityAdmission::from_freestanding(false),
        AsmAuthorityAdmission::HOSTED
    );
}

#[test]
fn port_io_grant_admits_port_io_without_machine_owner_authority() {
    let port_io_only = AsmAuthorityAdmission::HOSTED.with_grants(true, false);
    validate_asm_discharge(
        &typed(
            "data P { port: u16; byte: u8; } machine P::m(&mut self) { asm where clobbers r10, r11, r15, rax, rdx { out self.port, self.byte; in self.byte, self.port } }",
        ),
        port_io_only,
    )
    .unwrap_or_else(|diagnostics| {
        panic!("port-io grant rejected port I/O: {diagnostics:?}")
    });

    for (source, fragment) in [
        (
            "machine m() { asm where clobbers none { cli } }",
            "requires machine-owner authority",
        ),
        (
            "data P { saved: u64; } machine P::m(&mut self) { asm where clobbers r10, r15 { popfq self.saved } }",
            "requires machine-owner authority",
        ),
    ] {
        let messages = rejection_messages(source, port_io_only);
        assert!(
            messages.iter().any(|message| message.contains(fragment)),
            "port-io grant must not admit machine-owner authority for {source}: {messages:?}"
        );
    }
}

#[test]
fn interrupt_table_grant_leaves_port_io_and_machine_owner_closed() {
    let interrupt_table_only = AsmAuthorityAdmission::HOSTED.with_grants(false, true);
    assert!(interrupt_table_only.admits(AsmAuthorityRequirement::IdtControl));
    assert!(!interrupt_table_only.admits(AsmAuthorityRequirement::PortIo));
    assert!(!interrupt_table_only.admits(AsmAuthorityRequirement::MachineOwner));

    let messages = rejection_messages(
        "data P { port: u16; byte: u8; } machine P::m(&self) { asm where clobbers r10, r11, r15, rax, rdx { out self.port, self.byte } }",
        interrupt_table_only,
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("requires port-I/O authority")),
        "interrupt-table grant must not admit port I/O: {messages:?}"
    );
}

#[test]
fn refusals_name_the_exact_granular_grant_that_would_admit() {
    let messages = rejection_messages(
        "data P { port: u16; byte: u8; } machine P::m(&self) { asm where clobbers r10, r11, r15, rax, rdx { out self.port, self.byte } }",
        AsmAuthorityAdmission::HOSTED,
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("b.privileged_services.port_io = true")),
        "port-I/O refusal must name its granular grant: {messages:?}"
    );
    // Machine-owner refusals keep freestanding as the only supply.
    let messages = rejection_messages(
        "machine m() { asm where clobbers none { cli } }",
        AsmAuthorityAdmission::HOSTED,
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("b.freestanding = true")
                && !message.contains("privileged_services")),
        "machine-owner refusal must not offer a granular grant: {messages:?}"
    );
}

#[test]
fn grants_compose_on_top_of_machine_owner_admission() {
    assert_eq!(
        AsmAuthorityAdmission::MACHINE_OWNER.with_grants(false, false),
        AsmAuthorityAdmission::MACHINE_OWNER
    );
    assert_eq!(
        AsmAuthorityAdmission::MACHINE_OWNER.with_grants(true, true),
        AsmAuthorityAdmission::MACHINE_OWNER
    );
    let both = AsmAuthorityAdmission::HOSTED.with_grants(true, true);
    assert!(both.admits(AsmAuthorityRequirement::PortIo));
    assert!(both.admits(AsmAuthorityRequirement::IdtControl));
    assert!(!both.admits(AsmAuthorityRequirement::MachineOwner));
}

/// The instruction labels this module's own table produced before the
/// mnemonic query moved to `language_core::inline_assembly`. Pinned so the
/// move changed no diagnostic label.
const PREVIOUS_STATEMENT_INTRINSIC_LABELS: &[(BuiltinFunction, &str)] = &[
    (BuiltinFunction::AsmHlt, "hlt"),
    (BuiltinFunction::AsmPortOut, "out"),
    (BuiltinFunction::AsmPortIn, "in"),
    (BuiltinFunction::AsmLoadFence, "lfence"),
    (BuiltinFunction::AsmStoreFence, "sfence"),
    (BuiltinFunction::AsmFullFence, "mfence"),
    (BuiltinFunction::AsmDisableInterrupts, "cli"),
    (BuiltinFunction::AsmEnableInterrupts, "sti"),
    (BuiltinFunction::AsmSnapshotFlags, "pushfq"),
    (BuiltinFunction::AsmRestoreFlags, "popfq"),
    (BuiltinFunction::AsmReadMsr, "rdmsr"),
    (BuiltinFunction::AsmWriteMsr, "wrmsr"),
    (BuiltinFunction::AsmReadCr0, "read_cr0"),
    (BuiltinFunction::AsmReadCr2, "read_cr2"),
    (BuiltinFunction::AsmReadCr3, "read_cr3"),
    (BuiltinFunction::AsmReadCr4, "read_cr4"),
    (BuiltinFunction::AsmWriteCr0, "write_cr0"),
    (BuiltinFunction::AsmWriteCr3, "write_cr3"),
    (BuiltinFunction::AsmWriteCr4, "write_cr4"),
    (BuiltinFunction::AsmWriteBackInvalidate, "wbinvd"),
    (BuiltinFunction::AsmInvalidate, "invd"),
    (BuiltinFunction::AsmWriteBackNoInvalidate, "wbnoinvd"),
    (BuiltinFunction::AsmReadSctlrEl1, "read_sctlr_el1"),
    (BuiltinFunction::AsmReadTcrEl1, "read_tcr_el1"),
    (BuiltinFunction::AsmReadTtbr0El1, "read_ttbr0_el1"),
    (BuiltinFunction::AsmReadTtbr1El1, "read_ttbr1_el1"),
    (BuiltinFunction::AsmReadMairEl1, "read_mair_el1"),
    (BuiltinFunction::AsmReadVbarEl1, "read_vbar_el1"),
    (BuiltinFunction::AsmReadTpidrEl1, "read_tpidr_el1"),
    (BuiltinFunction::AsmReadEsrEl1, "read_esr_el1"),
    (BuiltinFunction::AsmReadFarEl1, "read_far_el1"),
    (BuiltinFunction::AsmWriteSctlrEl1, "write_sctlr_el1"),
    (BuiltinFunction::AsmWriteTcrEl1, "write_tcr_el1"),
    (BuiltinFunction::AsmWriteTtbr0El1, "write_ttbr0_el1"),
    (BuiltinFunction::AsmWriteTtbr1El1, "write_ttbr1_el1"),
    (BuiltinFunction::AsmWriteMairEl1, "write_mair_el1"),
    (BuiltinFunction::AsmWriteVbarEl1, "write_vbar_el1"),
    (BuiltinFunction::AsmWriteTpidrEl1, "write_tpidr_el1"),
];

#[test]
fn every_asm_builtin_has_the_mnemonic_the_discharge_table_used_to_spell() {
    for (function, label) in PREVIOUS_STATEMENT_INTRINSIC_LABELS {
        assert_eq!(
            asm_intrinsic_mnemonic(function.name()),
            Some(*label),
            "{function:?} must keep its diagnostic label"
        );
    }
    for function in BuiltinFunction::ALL {
        let mnemonic = asm_intrinsic_mnemonic(function.name());
        assert_eq!(
            mnemonic.is_some(),
            function.is_asm_intrinsic(),
            "{function:?}: exactly the asm intrinsics carry a mnemonic"
        );
        let Some(mnemonic) = mnemonic else { continue };
        let in_previous_table = PREVIOUS_STATEMENT_INTRINSIC_LABELS
            .iter()
            .any(|(previous, _)| previous == &function);
        if in_previous_table {
            continue;
        }
        // The intrinsics the retired table skipped now answer too; each is an
        // authority-free, service-free hint, so the discharge gate and the
        // declaration check treat it exactly as they treated its absence.
        let Some(AsmCatalogEntry::Contract(contract)) = asm_catalog_entry(mnemonic) else {
            panic!("{function:?} must resolve to a catalog contract");
        };
        assert_eq!(
            contract.required_authority,
            AsmAuthorityRequirement::None,
            "{function:?} joined the discharge scan and must require no authority"
        );
        assert_eq!(
            function.asm_intrinsic_service_name(),
            None,
            "{function:?} joined the declaration scan and must reach no service"
        );
    }
    assert_eq!(
        BuiltinFunction::ALL
            .into_iter()
            .filter(|function| function.is_value_position_asm_intrinsic())
            .map(|function| asm_intrinsic_mnemonic(function.name()))
            .collect::<Vec<_>>(),
        [Some("in"), Some("pushfq"), Some("rdmsr")]
    );
}
