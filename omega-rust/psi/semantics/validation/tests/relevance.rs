//! `[erased]` noninterference for statement-call receivers: the receiver of a
//! `receiver.state(...)` call is the callee's `self` operand, so it is a
//! runtime place exactly like a direct read. A runtime call through an erased
//! local, parameter, attached field, or a projection crossing an erased field
//! rejects with the same diagnostic family as a runtime read; calls to proof
//! machines and calls inside proof machines are proof computation and keep
//! their erased receivers.
//!
//! A `builder.roots.bind(slot, implementation)` statement is not a call: its
//! receiver and its delegated `ProductEntryRef` operand are places read at
//! build evaluation, so an erased receiver or operand rejects identically.

fn rejects(source: &str, fragment: &str) {
    let diagnostics = validation::validate_program(&crate::front_end::typed_program(source))
        .expect_err("erased receiver on a runtime call must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(fragment)),
        "expected {fragment:?}: {diagnostics:#?}"
    );
}

fn accepts(source: &str) {
    validation::validate_program(&crate::front_end::typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

#[test]
fn erased_field_receiver_rejects() {
    rejects(
        "data Counter { count: u64; }
         machine Counter::bump(&mut self) { self.count = 1; }
         data Main { c [erased]: Counter; }
         machine Main::inspect(&mut self) { self.c.bump(); }",
        "erased field `c` has no runtime value",
    );
}

#[test]
fn proof_context_receiver_accepts_erased() {
    // A free machine whose signature mentions proof-only data is a proof
    // machine: its whole body is proof context, so the erased receiver inside
    // is not a runtime use.
    accepts(
        "data P { case Z; case S(prev: P); }
         machine P::touch(self) {}
         machine lemma(witness: P) -> u64 {
             let p [erased]: P = P::Z {};
             p.touch();
             0
         }",
    );
}

#[test]
fn erased_local_root_binding_operand_rejects() {
    rejects(
        "data Build {} \
         data ProductEntryRef {} \
         machine build(builder: &mut Build, entry [erased]: ProductEntryRef) { \
             let e [erased]: ProductEntryRef = entry; \
             builder.roots.bind(macos_arm64::ProgramEntry, e); \
         }",
        "erased local `e` has no runtime value",
    );
}
