use super::{AsmAuthorityAdmission, validate_asm_discharge};
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
